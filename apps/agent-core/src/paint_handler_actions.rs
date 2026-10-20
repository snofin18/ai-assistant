//! UIA, input, and capture primitives for the Paint T3.1 handlers.
//!
//! Responsibilities:
//! - resolve adapter targets inside an already-resolved window scope;
//! - run the platform primitives the seven T3.1 tools need (read / invoke /
//!   select / key / drag / capture);
//! - gate every synthetic input behind the shared target lease.
//!
//! Boundaries:
//! - does not decide policy or approval;
//! - does not parse tool arguments or assemble tool envelopes.
//!
//! Invariants:
//! 1. every platform call is polled through `support::poll_immediate`;
//! 2. platform failures are mapped to `ErrorCode`-bearing `ToolBusError`s;
//! 3. input primitives take an exclusive window lease keyed by app + window.
//!
//! Related documents: ADR-0067 (explicit `CoordinateSpace`), ADR-0085
//! (`element_bounds`), and `apps/agent-core/src/paint_handlers.rs`.
use crate::handler_support as support;
use crate::target_lease::window_lease_key;
use assistant_platform_api::{
    CaptureOptions, CoordinateSpace, ElementBounds, ElementGeometry, Fingerprint, FingerprintScope,
    ImageRef, KeyChord, KeyModifier, KeyTarget, PointerAction, ResolvedElement, ResolvedWindow,
    Selection, UiAutomationProvider, WindowFilter, WindowGeometry, WindowProvider, WindowState,
};
use assistant_storage::BlobId;
use assistant_tool_bus::{ToolBusError, ToolOutput};
use serde_json::{Map, Value, json};
use std::time::Duration;

use super::paint_handler_support::{
    normalized_point, parse_canvas_size, png_dimensions, required_png_path, user_interaction,
    user_interaction_with, verify_failed,
};
use super::{
    DragRequest, PaintHandlerContext, PhysicalPointPx, TARGET_BRUSH_TOGGLE, TARGET_CANVAS,
    TARGET_FILE_OPEN_DIALOG, TARGET_FILE_OPEN_FILENAME, TARGET_FILE_OPEN_OPEN_BUTTON,
    TARGET_LAYER_ITEM, TARGET_MAIN_WINDOW, TARGET_SAVE_AS_DIALOG, TARGET_SAVE_AS_FILENAME,
    TARGET_SAVE_AS_SAVE_BUTTON, TARGET_SHAPE_FILL_TOGGLE, TARGET_SHAPE_OUTLINE_TOGGLE,
    TARGET_SHAPE_SIZE_TOGGLE, TARGET_SHAPES_TOGGLE, TARGET_STATUS_BAR, TARGET_ZOOM_VALUES_COMBO,
    TOOL_FILE_OPEN, TOOL_FILE_SAVE_AS,
};

/// 展开 / 折叠后的**有界重解**次数与间隔（形状库异步物化、图层面板展开，见 `docs/memory/apps/paint.md` §11）。
///
/// 上限 = 20 × 50 ms = 1 s：既给 Paint 留物化时间，也是**硬上限**（ADR-0063 同族：不许无限轮询）。
const GALLERY_ATTEMPTS: usize = 20;
const GALLERY_INTERVAL_MS: u64 = 50;

/// `Ctrl+N` 之后等待 Paint 线程弹层的最长时间：20 × 50 ms = 1 s 硬上限。
///
/// Paint 的未保存三态框是异步 `WinUI` 弹层；`SendInput` 返回不等于弹层已经创建。
/// 这里只轮询 `window_state`（平台侧一次有界 `EnumThreadWindows`），
/// 发现弹层立即 fail-closed。
const POPUP_WAIT_ATTEMPTS: usize = 20;
const POPUP_WAIT_INTERVAL_MS: u64 = 50;
const FILE_DIALOG_ATTEMPTS: usize = 30;
const FILE_DIALOG_INTERVAL_MS: u64 = 100;
const OPEN_CANVAS_ATTEMPTS: usize = 30;
const SAVE_FILE_ATTEMPTS: usize = 30;

struct FileDialogShortcut<'a> {
    key: &'a str,
    modifiers: Vec<KeyModifier>,
    label: &'a str,
}

impl<P> PaintHandlerContext<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    pub(super) fn resolve_window(
        &self,
        target: &str,
        tool: &str,
    ) -> Result<ResolvedWindow, ToolBusError> {
        let descriptor =
            self.targets
                .descriptor(target)
                .map_err(|error| ToolBusError::EnvelopeAssembly {
                    tool: tool.to_owned(),
                    reason: error.to_string(),
                })?;
        let result = support::poll_immediate(
            self.platform.resolve_window(descriptor),
            tool,
            "resolve_window",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn resolve_element(
        &self,
        target: &str,
        scope: &ResolvedWindow,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        let descriptor =
            self.targets
                .descriptor(target)
                .map_err(|error| ToolBusError::EnvelopeAssembly {
                    tool: tool.to_owned(),
                    reason: error.to_string(),
                })?;
        let chain =
            assistant_platform_api::SelectorChain::new(descriptor.element_candidates().to_vec())
                .with_allow_fallback_entry(descriptor.allow_fallback_entry());
        let result = support::poll_immediate(
            self.platform.resolve_element(scope, &chain),
            tool,
            "resolve_element",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    /// 解析图层面板列表（`layersList`）；折叠时用 **`Alt+L`** 展开，再**有界轮询**重解。
    ///
    /// 为什么用 L2 快捷键而不是解析面板的 toggle 按钮：实测（`eval/tasks/paint/t3.1/probe-evidence.json`
    /// 的 `layers_panel`）「`Alt+L` 开合图层面板，**不需要**解析本地化的 toggle 按钮」；而那个父候选
    /// `name_regex "层"` 在真机上有 **4** 个命中（`TargetAmbiguous`），解析它必然失败。
    ///
    /// `Alt+L` 是**开关**，所以只在 `layersList` 缺失时才发键（状态感知）——面板已展开时**一个键都不发**；
    /// 发键后 1 s 内仍解析不到 → fail-closed（不猜、不无限等）。
    pub(super) fn resolve_layer_list(
        &self,
        task_id: &str,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        match self.resolve_element(TARGET_LAYER_ITEM, window, tool) {
            Ok(layer_list) => return Ok(layer_list),
            Err(ToolBusError::UnknownTool { .. }) => {}
            Err(error) => return Err(error),
        }
        self.send_key(task_id, window, "L", vec![KeyModifier::Alt], tool)?;
        self.poll_for_element(TARGET_LAYER_ITEM, window, tool)
    }

    /// 在 `GALLERY_ATTEMPTS × GALLERY_INTERVAL_MS`（= 1 s 硬上限）内反复重解 `target`；
    /// 超时上抛**最后一次**错误，绝不返回"近似"结果。
    ///
    /// 形状库（异步物化）与图层面板（`Alt+L` 展开）共用同一段有界轮询 —— 一处实现，避免两套行为漂移。
    fn poll_for_element(
        &self,
        target: &str,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        let mut last_error = None;
        for _attempt in 0..GALLERY_ATTEMPTS {
            match self.resolve_element(target, window, tool) {
                Ok(element) => return Ok(element),
                Err(error) => last_error = Some(error),
            }
            std::thread::sleep(Duration::from_millis(GALLERY_INTERVAL_MS));
        }
        Err(last_error.unwrap_or_else(|| {
            support::invalid_arguments(
                tool,
                format!("`{target}` did not materialize within the bounded poll window"),
            )
        }))
    }

    /// Waits until `target` is no longer present, bounded by the same gallery
    /// window used for materialization.
    ///
    /// Menu-item handlers use this after `Invoke` so the caller's next snapshot
    /// cannot race the flyout-close animation. A still-present item is a real
    /// failure, not a reason to sleep longer.
    pub(super) fn wait_for_element_absent(
        &self,
        target: &str,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        for _attempt in 0..GALLERY_ATTEMPTS {
            match self.resolve_element(target, window, tool) {
                Err(ToolBusError::UnknownTool { .. }) => return Ok(()),
                Ok(_) => {}
                Err(error) => return Err(error),
            }
            std::thread::sleep(Duration::from_millis(GALLERY_INTERVAL_MS));
        }
        Err(verify_failed(format!(
            "{tool}: `{target}` remained in the UIA tree after Invoke"
        )))
    }

    /// 解析形状库里的目标形状项；折叠时先展开再**有界轮询**重解。
    ///
    /// 实测（2026-10-14 真机探针）：形状库折叠时形状项**还没物化**，直接解析只会
    /// `TargetNotFound`；容器内那个 `AutomationId=CollapsedButton` 的按钮用
    /// `InvokePattern` 能展开（同一元素上的 `ExpandCollapsePattern::Expand()` 在真机上失败），
    /// 展开后 `GridView` 是**异步**填充的，所以要按固定次数 × 间隔重解，而不是解一次就算。
    ///
    /// 错误语义：折叠判定只接受 `ToolBusError::UnknownTool`（= 平台 `TargetNotFound`
    /// 经 `handler_support::map_platform_error` 映射后的形态）；其它错误直接上抛，
    /// 免得把真正的管线故障当成"还没展开"。1 s 内仍没物化 → fail-closed。
    pub(super) fn wait_for_shape_item(
        &self,
        window: &ResolvedWindow,
        target: &str,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        match self.resolve_element(target, window, tool) {
            Ok(item) => return Ok(item),
            Err(ToolBusError::UnknownTool { .. }) => {}
            Err(error) => return Err(error),
        }
        let toggle = self.resolve_element(TARGET_SHAPES_TOGGLE, window, tool)?;
        self.invoke_element(&toggle, "invoke", tool)?;
        self.poll_for_element(target, window, tool)
    }

    /// 解析画笔 flyout 里的目标笔型；折叠时先展开再在有界窗口内重解。
    ///
    /// §13.10 实测 `BrushesSplitButton` 支持 `ExpandCollapsePattern.Expand()`；flyout
    /// 内的 `MenuItem` 在展开前不在树里。找不到只接受 `UnknownTool`（平台 `TargetNotFound`
    /// 的映射），其它管线错误直接上抛。
    pub(super) fn wait_for_brush_item(
        &self,
        window: &ResolvedWindow,
        target: &str,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        self.wait_for_shape_group_flyout_item(window, TARGET_BRUSH_TOGGLE, "expand", target, tool)
    }

    /// 解析「形状」组里第二个 flyout（形状轮廓 / 形状填充）的条目。
    ///
    /// 与画笔不同，这两个 flyout 必须先展开形状库才会出现在树里（§13.9 / §13.11 / §13.12），
    /// 所以是「展开 `CollapsedButton` → 展开目标 flyout → 有界重解」三步。实测这两个
    /// flyout 在未选中形状时也能展开并枚举全部 7 项（2026-10-19 只读复测），且同刻只会
    /// 存在一个 flyout 的条目 —— 打开填充会自动收起轮廓（同次实测），所以按 name 解析无歧义。
    pub(super) fn wait_for_outline_flyout_item(
        &self,
        window: &ResolvedWindow,
        target: &str,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        self.wait_for_shape_group_flyout_item(
            window,
            TARGET_SHAPE_OUTLINE_TOGGLE,
            "invoke",
            target,
            tool,
        )
    }

    /// 形状组第二个 flyout 的填充风格分支；语义与 `wait_for_outline_flyout_item` 相同。
    pub(super) fn wait_for_fill_flyout_item(
        &self,
        window: &ResolvedWindow,
        target: &str,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        self.wait_for_shape_group_flyout_item(
            window,
            TARGET_SHAPE_FILL_TOGGLE,
            "invoke",
            target,
            tool,
        )
    }

    /// 形状组第三类 flyout（`大小` / 形状笔宽）的条目（ADR-0098 D4）。
    ///
    /// 通道与轮廓 / 填充同形（展开形状库 → 展开 `大小` → 有界重解），差别只在于
    /// **目标条目按 `AutomationId` 解析**（`Size1` / `Size3` / `Size5` / `Size8`），
    /// 因为 `大小` 按钮与画笔 `大小` Slider、`Size1` 条目三者同名（§13.16）。
    /// 形状组没有活动形状时该 flyout 展开后为空 → 有界窗口内解不到 → 既有
    /// `TargetNotFound` fail-closed；本方法**不**替调用方去点一个形状（不施加静默副作用）。
    pub(super) fn wait_for_size_flyout_item(
        &self,
        window: &ResolvedWindow,
        target: &str,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        self.wait_for_shape_group_flyout_item(
            window,
            TARGET_SHAPE_SIZE_TOGGLE,
            "invoke",
            target,
            tool,
        )
    }

    fn wait_for_shape_group_flyout_item(
        &self,
        window: &ResolvedWindow,
        toggle_target: &str,
        toggle_action: &str,
        target: &str,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        match self.resolve_element(target, window, tool) {
            Ok(item) => return Ok(item),
            Err(ToolBusError::UnknownTool { .. }) => {}
            Err(error) => return Err(error),
        }
        // 第二级 flyout 的按钮只在形状库里；但形状库可能已经被上一轮展开。
        // 直接对已展开的 CollapsedButton 再 `invoke` 会把它收起来，导致下一次
        // `expand` 撞在折叠状态上（TASK-309 真机路径实测）。因此先解二级按钮：
        // 解得到说明上一轮留下了展开的形状库，直接复用；解不到才展开形状库。
        let toggle = match self.resolve_element(toggle_target, window, tool) {
            Ok(toggle) => toggle,
            Err(ToolBusError::UnknownTool { .. }) => {
                let shapes = self.resolve_element(TARGET_SHAPES_TOGGLE, window, tool)?;
                self.invoke_element(&shapes, "invoke", tool)?;
                self.poll_for_element(toggle_target, window, tool)?
            }
            Err(error) => return Err(error),
        };
        // 形状组的三个 flyout 按钮支持 `Invoke + ExpandCollapse`。直接走 `expand` 时，
        // 平台层在 `Expand()` 返回后立刻读状态，会撞上 WinUI 的异步展开窗口（实测
        // state 仍为 collapsed）；探针脚本之所以通过，是因为它在读取前固定等了
        // 500 ms。这里不放宽平台后置条件，改用按钮同样支持的 `Invoke`，再用目标
        // MenuItem 是否在有界轮询窗口内出现作为真实效果判据。`BrushesSplitButton`
        // 是例外：它必须走 `expand` 才会打开菜单，因此由调用方显式传入动作。
        if let Err(error) = self.invoke_element(&toggle, toggle_action, tool) {
            let expand_state_lag = toggle_action == "expand"
                && matches!(
                    &error,
                    ToolBusError::Mcp { message, .. }
                        if message.contains(
                            "invoke_action(expand) postcondition failed: ExpandCollapseState is 0"
                        )
                );
            if !expand_state_lag {
                return Err(error);
            }
        }
        self.poll_for_element(target, window, tool)
    }

    pub(super) fn element_bounds(
        &self,
        element: &ResolvedElement,
        tool: &str,
    ) -> Result<ElementBounds, ToolBusError> {
        let result = support::poll_immediate(
            self.platform.element_bounds(element),
            tool,
            "element_bounds",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    /// 窗口几何视图（设备像素矩形 + 所属坐标空间），一次查询（ADR-0092 细则 3 / ADR-0093）。
    ///
    /// 用途：合成点击前做「点必须落在窗口矩形内」的判定（ADR-0092 细则 8）。平台没实现本查询
    /// → `CapabilityMissing`，由 `map_platform_error` 映射后上抛，**不得**退回空矩形。
    pub(super) fn window_geometry(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<WindowGeometry, ToolBusError> {
        let result = support::poll_immediate(
            self.platform.window_geometry(window),
            tool,
            "window_geometry",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    /// 窗口状态视图；`occluded` 同时覆盖中心点遮挡与目标线程上的阻塞弹层。
    pub(super) fn window_state(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<WindowState, ToolBusError> {
        let result =
            support::poll_immediate(self.platform.window_state(window), tool, "window_state")?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    /// 在 1 s 硬上限内轮询目标窗口是否出现阻塞弹层。
    ///
    /// 语义：`true` = 已发现弹层，调用方必须立即 fail-closed；`false` = 有界窗口内
    /// 没有发现弹层，允许继续。该函数不发送任何输入。
    pub(super) fn wait_for_blocking_popup(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<bool, ToolBusError> {
        for _attempt in 0..POPUP_WAIT_ATTEMPTS {
            if self.window_state(window, tool)?.occluded() {
                return Ok(true);
            }
            std::thread::sleep(Duration::from_millis(POPUP_WAIT_INTERVAL_MS));
        }
        Ok(false)
    }

    /// Resolves one file-dialog window and verifies that it belongs to a
    /// different process than the Paint window.
    ///
    /// The selector itself is class-based (`#32770` + `Window`), so the
    /// process check is the second half of the identity contract. A timeout is
    /// `Transient`; a missing or same-process dialog is `TargetNotFound`.
    pub(super) fn resolve_file_dialog(
        &self,
        target: &str,
        tool: &str,
    ) -> Result<ResolvedWindow, ToolBusError> {
        let main_window = self.resolve_window(TARGET_MAIN_WINDOW, tool)?;
        let mut last_error = String::from("dialog did not appear");
        for _attempt in 0..FILE_DIALOG_ATTEMPTS {
            let listed = support::poll_immediate(
                self.platform.list_windows(&WindowFilter::any()),
                tool,
                "list_windows",
            )?
            .map_err(|error| support::map_platform_error(tool, &error))?;
            let main_app_id = listed
                .iter()
                .find(|info| info.window().id() == main_window.id())
                .map(|info| info.app_id().to_owned());
            if let Some(main_app_id) = main_app_id {
                for info in &listed {
                    if !info.app_id().eq_ignore_ascii_case(&main_app_id)
                        || !info.window().display_label().contains("[#32770]")
                    {
                        continue;
                    }
                    let state = self.window_state(info.window(), tool)?;
                    if state.foreground() {
                        return Ok(info.window().clone());
                    }
                    last_error = format!(
                        "matching `{target}` dialog exists but is not foreground: {}",
                        info.window().display_label()
                    );
                }
            }
            std::thread::sleep(Duration::from_millis(FILE_DIALOG_INTERVAL_MS));
        }
        Err(ToolBusError::Transport {
            reason: format!("{tool}: {last_error}"),
        })
    }

    /// Waits for a dialog target to become unresolvable after `Escape` / Open /
    /// Save. A still-present target after the bounded wait is `Transient`.
    pub(super) fn wait_for_window_absent(
        &self,
        target: &str,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        for _attempt in 0..FILE_DIALOG_ATTEMPTS {
            match self.resolve_window(target, tool) {
                Err(ToolBusError::UnknownTool { .. }) => return Ok(()),
                Ok(_) => {}
                Err(error) => return Err(error),
            }
            std::thread::sleep(Duration::from_millis(FILE_DIALOG_INTERVAL_MS));
        }
        Err(ToolBusError::Transport {
            reason: format!("{tool}: dialog `{target}` did not close within the bound"),
        })
    }

    fn open_expected_file_dialog(
        &self,
        task_id: &str,
        window: &ResolvedWindow,
        dialog_target: &str,
        shortcut: FileDialogShortcut<'_>,
        tool: &str,
    ) -> Result<ResolvedWindow, ToolBusError> {
        self.send_key(task_id, window, shortcut.key, shortcut.modifiers, tool)?;
        match self.resolve_file_dialog(dialog_target, tool) {
            Ok(dialog) => Ok(dialog),
            Err(error) => {
                if self.window_state(window, tool)?.occluded() {
                    return Err(user_interaction_with(
                        tool,
                        &format!(
                            "{} did not produce the file dialog and Paint remains occluded",
                            shortcut.label
                        ),
                    ));
                }
                Err(error)
            }
        }
    }

    pub(super) fn set_element_value(
        &self,
        element: &ResolvedElement,
        value: &str,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let result =
            support::poll_immediate(self.platform.set_value(element, value), tool, "set_value")?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    fn read_open_canvas(
        &self,
        expected: (u32, u32),
        tool: &str,
    ) -> Result<(u32, u32, f64), ToolBusError> {
        let window = self.resolve_window(TARGET_MAIN_WINDOW, tool)?;
        for _attempt in 0..OPEN_CANVAS_ATTEMPTS {
            let status = self.resolve_element(TARGET_STATUS_BAR, &window, tool)?;
            let text = self.read_element_text(&status, tool)?;
            if let Some((width, height)) = parse_canvas_size(&text)
                && (width, height) == expected
            {
                let canvas = self.resolve_element(TARGET_CANVAS, &window, tool)?;
                let bounds = self.element_bounds(&canvas, tool)?;
                let zoom_ratio = f64::from(bounds.width()) / f64::from(width);
                if zoom_ratio.is_finite() && zoom_ratio > 0.0 {
                    return Ok((width, height, zoom_ratio));
                }
            }
            std::thread::sleep(Duration::from_millis(FILE_DIALOG_INTERVAL_MS));
        }
        Err(verify_failed(format!(
            "{tool}: opened document size did not become {}x{}",
            expected.0, expected.1
        )))
    }

    pub(super) fn open_file_dialog_output(
        &self,
        task_id: &str,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        if arguments
            .get("discard_unsaved_confirmed")
            .and_then(Value::as_bool)
            != Some(false)
        {
            return Err(support::invalid_arguments(
                TOOL_FILE_OPEN,
                "discard_unsaved_confirmed must be false",
            ));
        }
        let path = required_png_path(arguments, "path", TOOL_FILE_OPEN, true)?;
        let expected = png_dimensions(path.as_path(), TOOL_FILE_OPEN)?;
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_FILE_OPEN)?;
        let window_state = self.window_state(&window, TOOL_FILE_OPEN)?;
        if window_state.occluded() {
            return Err(user_interaction_with(
                TOOL_FILE_OPEN,
                &format!(
                    "Paint was already occluded before Ctrl+O \
                     (minimized={}, foreground={}, occluded={})",
                    window_state.minimized(),
                    window_state.foreground(),
                    window_state.occluded()
                ),
            ));
        }
        let before = self.fingerprint_event(&window, TOOL_FILE_OPEN)?;
        let started = std::time::Instant::now();
        let dialog = self.open_expected_file_dialog(
            task_id,
            &window,
            TARGET_FILE_OPEN_DIALOG,
            FileDialogShortcut {
                key: "O",
                modifiers: vec![KeyModifier::Control],
                label: "Ctrl+O",
            },
            TOOL_FILE_OPEN,
        )?;
        let filename = self
            .resolve_element(TARGET_FILE_OPEN_FILENAME, &dialog, TOOL_FILE_OPEN)
            .map_err(|error| ToolBusError::UnknownTool {
                tool: format!(
                    "{TOOL_FILE_OPEN}: filename lookup in `{}` failed: {error}",
                    dialog.display_label()
                ),
            })?;
        self.set_element_value(&filename, path.to_string_lossy().as_ref(), TOOL_FILE_OPEN)?;
        let open_button =
            self.resolve_element(TARGET_FILE_OPEN_OPEN_BUTTON, &dialog, TOOL_FILE_OPEN)?;
        self.invoke_element(&open_button, "invoke", TOOL_FILE_OPEN)?;
        self.wait_for_window_absent(TARGET_FILE_OPEN_DIALOG, TOOL_FILE_OPEN)?;
        let zoom_combo = self.resolve_element(TARGET_ZOOM_VALUES_COMBO, &window, TOOL_FILE_OPEN)?;
        self.set_element_value(&zoom_combo, "50%", TOOL_FILE_OPEN)?;
        let (width, height, zoom_ratio) =
            self.read_open_canvas(expected, TOOL_FILE_OPEN)
                .map_err(|_| {
                    user_interaction_with(
                        TOOL_FILE_OPEN,
                        "opened document size did not become the fixture size",
                    )
                })?;
        {
            let mut document = self
                .document
                .lock()
                .map_err(|_| super::paint_handler_support::unavailable("paint document state"))?;
            document.opened_path = Some(path.to_string_lossy().to_string());
            document.canvas_width_px = Some(width);
            document.canvas_height_px = Some(height);
            document.zoom_ratio = Some(zoom_ratio);
        }
        let after = self.fingerprint_event(&window, TOOL_FILE_OPEN)?;
        Ok(ToolOutput::json(json!({
            "opened_path": path.to_string_lossy(),
            "canvas_width_px": width,
            "canvas_height_px": height,
            "zoom_ratio": zoom_ratio,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": super::paint_handler_support::elapsed_ms(started),
        })))
    }

    pub(super) fn save_as_dialog_output(
        &self,
        task_id: &str,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        if arguments.get("overwrite_existing").and_then(Value::as_bool) != Some(false) {
            return Err(support::invalid_arguments(
                TOOL_FILE_SAVE_AS,
                "overwrite_existing must be false",
            ));
        }
        let target = required_png_path(arguments, "target_path", TOOL_FILE_SAVE_AS, false)?;
        if crate::notepad_files::snapshot_file(&target, TOOL_FILE_SAVE_AS)?.exists {
            return Err(user_interaction(TOOL_FILE_SAVE_AS));
        }
        let expected = self
            .document
            .lock()
            .map_err(|_| super::paint_handler_support::unavailable("paint document state"))
            .map(|document| document.canvas_width_px.zip(document.canvas_height_px))?;
        let window = self.resolve_window(TARGET_MAIN_WINDOW, TOOL_FILE_SAVE_AS)?;
        let before = self.fingerprint_event(&window, TOOL_FILE_SAVE_AS)?;
        let started = std::time::Instant::now();
        let dialog = self.open_expected_file_dialog(
            task_id,
            &window,
            TARGET_SAVE_AS_DIALOG,
            FileDialogShortcut {
                key: "S",
                modifiers: vec![KeyModifier::Control, KeyModifier::Shift],
                label: "Ctrl+Shift+S",
            },
            TOOL_FILE_SAVE_AS,
        )?;
        let filename = self.resolve_element(TARGET_SAVE_AS_FILENAME, &dialog, TOOL_FILE_SAVE_AS)?;
        self.set_element_value(
            &filename,
            target.to_string_lossy().as_ref(),
            TOOL_FILE_SAVE_AS,
        )?;
        if crate::notepad_files::snapshot_file(&target, TOOL_FILE_SAVE_AS)?.exists {
            self.send_key(task_id, &dialog, "Escape", Vec::new(), TOOL_FILE_SAVE_AS)?;
            self.wait_for_window_absent(TARGET_SAVE_AS_DIALOG, TOOL_FILE_SAVE_AS)?;
            return Err(user_interaction(TOOL_FILE_SAVE_AS));
        }
        let save_button =
            self.resolve_element(TARGET_SAVE_AS_SAVE_BUTTON, &dialog, TOOL_FILE_SAVE_AS)?;
        self.invoke_element(&save_button, "invoke", TOOL_FILE_SAVE_AS)?;
        let created = wait_for_file_created(&target, TOOL_FILE_SAVE_AS)?;
        if !created.exists {
            return Err(verify_failed(format!(
                "{TOOL_FILE_SAVE_AS}: target file was not created"
            )));
        }
        let saved = png_dimensions(target.as_path(), TOOL_FILE_SAVE_AS)?;
        if let Some(expected) = expected
            && saved != expected
        {
            return Err(verify_failed(format!(
                "{TOOL_FILE_SAVE_AS}: saved dimensions {saved:?} do not match document \
                 {expected:?}"
            )));
        }
        self.wait_for_window_absent(TARGET_SAVE_AS_DIALOG, TOOL_FILE_SAVE_AS)?;
        {
            let mut document = self
                .document
                .lock()
                .map_err(|_| super::paint_handler_support::unavailable("paint document state"))?;
            document.opened_path = Some(target.to_string_lossy().to_string());
        }
        let after = self.fingerprint_event(&window, TOOL_FILE_SAVE_AS)?;
        Ok(ToolOutput::json(json!({
            "target_path": target.to_string_lossy(),
            "file_created": true,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": super::paint_handler_support::elapsed_ms(started),
        })))
    }

    /// 元素几何视图（设备像素矩形 + 所属坐标空间），一次查询（ADR-0092 细则 3 / ADR-0093）。
    ///
    /// `space` 直接可喂给 `pointer_action`，因此点击工具栏控件**一次查询**就够，不必再问显示器目录
    /// （热路径纪律，ADR-0092 细则 5）。平台没实现 → `CapabilityMissing`，显式失败。
    pub(super) fn element_geometry(
        &self,
        element: &ResolvedElement,
        tool: &str,
    ) -> Result<ElementGeometry, ToolBusError> {
        let result = support::poll_immediate(
            self.platform.element_geometry(element),
            tool,
            "element_geometry",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn read_element_text(
        &self,
        element: &ResolvedElement,
        tool: &str,
    ) -> Result<String, ToolBusError> {
        let result = support::poll_immediate(self.platform.read_text(element), tool, "read_text")?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn invoke_element(
        &self,
        element: &ResolvedElement,
        action: &str,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let result = support::poll_immediate(
            self.platform.invoke_action(element, action),
            tool,
            "invoke_action",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn select_element(
        &self,
        element: &ResolvedElement,
        selection: &Selection,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let result =
            support::poll_immediate(self.platform.select(element, selection), tool, "select")?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn fingerprint_event(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<Fingerprint, ToolBusError> {
        let result = support::poll_immediate(
            self.platform
                .fingerprint(window, &FingerprintScope::WholeWindow),
            tool,
            "fingerprint",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn send_key(
        &self,
        task_id: &str,
        window: &ResolvedWindow,
        key: &str,
        modifiers: Vec<KeyModifier>,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let lease_key = window_lease_key(&self.app_id, window.id(), tool)?;
        self.input_leases.run(task_id, lease_key, tool, || {
            let chord = KeyChord::new(key.to_owned(), modifiers);
            let result = support::poll_immediate(
                self.platform
                    .key_action(&chord, &KeyTarget::Window(window.clone())),
                tool,
                "key_action",
            )?;
            result.map_err(|error| support::map_platform_error(tool, &error))
        })
    }

    pub(super) fn drag(
        &self,
        task_id: &str,
        window: &ResolvedWindow,
        coordinate_space: &CoordinateSpace,
        points: DragRequest,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let start = normalized_point(points.start, coordinate_space, tool)?;
        let drop_at = normalized_point(points.end, coordinate_space, tool)?;
        let action = PointerAction::DragTo {
            drop_at,
            drop_coordinate_space: coordinate_space.clone(),
        };
        let lease_key = window_lease_key(&self.app_id, window.id(), tool)?;
        self.input_leases.run(task_id, lease_key, tool, || {
            let result = support::poll_immediate(
                self.platform
                    .pointer_action(coordinate_space, start, &action),
                tool,
                "pointer_action",
            )?;
            result.map_err(|error| support::map_platform_error(tool, &error))
        })
    }

    /// 在元素中心发一次 L4 合成点击（形状项这类真机无可用 UIA pattern 的元素唯一可用的通道）。
    ///
    /// 为什么走合成点击（`DRIFT-256-5`）：Paint 11.2605.81.0 的形状项 `Button|矩形` 全树唯一，
    /// 但 `TogglePattern.Toggle()` 两向都是**空操作**，且 `Button|矩形` 无 `InvokePattern`、
    /// `ListItem|矩形` 无 `SelectionItemPattern` —— 没有任何可用 pattern，只有真实鼠标点击能选中。
    ///
    /// 步骤（ADR-0092 细则 5 的调用上限 = 1 次元素几何 + 1 次 `pointer_action`）：
    /// 1. 取元素几何（设备像素矩形 + 所属坐标空间）；
    /// 2. 取窗口几何，断言元素中心落在窗口矩形内 —— **半开区间** `[left, right) × [top, bottom)`，
    ///    越界 → `ToolInvalidArgs`，**禁止 clamp / 悄悄挪点**（ADR-0092 细则 8 / 铁律 9）；
    /// 3. 用元素几何自带的 `space` 归一化中心点，在窗口独占租约内发一次 `PointerAction::Click`。
    ///
    /// 中心点用 `left + width / 2` 而不是 `(left + right) / 2`：前者不引入 `i32` 溢出（ADR-0092
    /// 的指针约束正是为极端坐标准备的），且保证落在 `[left, right)` 内。
    pub(super) fn click_element_center(
        &self,
        task_id: &str,
        window: &ResolvedWindow,
        element: &ResolvedElement,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let element_geometry = self.element_geometry(element, tool)?;
        let window_geometry = self.window_geometry(window, tool)?;
        let bounds = element_geometry.bounds();
        let center = PhysicalPointPx {
            horizontal_px: bounds.left() + bounds.width() / 2,
            vertical_px: bounds.top() + bounds.height() / 2,
        };
        let window_bounds = window_geometry.bounds();
        if center.horizontal_px < window_bounds.left()
            || center.horizontal_px >= window_bounds.right()
            || center.vertical_px < window_bounds.top()
            || center.vertical_px >= window_bounds.bottom()
        {
            return Err(support::invalid_arguments(
                tool,
                format!(
                    "element center ({}, {}) falls outside window bounds ({}, {})-({}, {})",
                    center.horizontal_px,
                    center.vertical_px,
                    window_bounds.left(),
                    window_bounds.top(),
                    window_bounds.right(),
                    window_bounds.bottom()
                ),
            ));
        }
        let space = element_geometry.space();
        let point = normalized_point(center, space, tool)?;
        let lease_key = window_lease_key(&self.app_id, window.id(), tool)?;
        self.input_leases.run(task_id, lease_key, tool, || {
            let result = support::poll_immediate(
                self.platform
                    .pointer_action(space, point, &PointerAction::Click),
                tool,
                "pointer_action",
            )?;
            result.map_err(|error| support::map_platform_error(tool, &error))
        })
    }

    pub(super) fn capture_window(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<ImageRef, ToolBusError> {
        let result = support::poll_immediate(
            self.platform.capture(window, &CaptureOptions::new(true)),
            tool,
            "capture",
        )?;
        let image = result.map_err(|error| support::map_platform_error(tool, &error))?;
        BlobId::parse(image.blob_id()).map_err(|error| ToolBusError::EnvelopeAssembly {
            tool: tool.to_owned(),
            reason: format!("capture returned an invalid content address: {error}"),
        })?;
        Ok(image)
    }
}

fn wait_for_file_created(
    target: &std::path::Path,
    tool: &str,
) -> Result<assistant_verify::FileSnapshot, ToolBusError> {
    let mut snapshot = crate::notepad_files::snapshot_file(target, tool)?;
    for _attempt in 0..SAVE_FILE_ATTEMPTS {
        if snapshot.exists {
            return Ok(snapshot);
        }
        std::thread::sleep(Duration::from_millis(FILE_DIALOG_INTERVAL_MS));
        snapshot = crate::notepad_files::snapshot_file(target, tool)?;
    }
    Ok(snapshot)
}

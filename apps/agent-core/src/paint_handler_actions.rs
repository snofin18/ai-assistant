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
    Selection, UiAutomationProvider, WindowGeometry, WindowProvider, WindowState,
};
use assistant_storage::BlobId;
use assistant_tool_bus::ToolBusError;
use std::time::Duration;

use super::paint_handler_support::normalized_point;
use super::{
    DragRequest, PaintHandlerContext, PhysicalPointPx, TARGET_LAYER_ITEM, TARGET_RECTANGLE_TOOL,
    TARGET_SHAPES_TOGGLE,
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
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        match self.resolve_element(TARGET_RECTANGLE_TOOL, window, tool) {
            Ok(item) => return Ok(item),
            Err(ToolBusError::UnknownTool { .. }) => {}
            Err(error) => return Err(error),
        }
        let toggle = self.resolve_element(TARGET_SHAPES_TOGGLE, window, tool)?;
        self.invoke_element(&toggle, "invoke", tool)?;
        self.poll_for_element(TARGET_RECTANGLE_TOOL, window, tool)
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

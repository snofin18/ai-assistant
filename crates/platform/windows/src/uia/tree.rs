//! 树遍历 / 树快照 / 状态指纹（架构 v2 §7.1 / §7.2 / §7.3、§13.2）。
//!
//! 职责：按 `TreeOptions` 裁剪出节点数与规范化文本，并把规范化文本压成 `Fingerprint`。
//! 边界：**不做**树的可视化 / 裁剪策略优化（§7.1 的上下文预算是调用方的事）、
//! **不做**视觉兜底、**不做**快照缓存。
//!
//! ## 为什么用「显式栈」而不是递归
//! UIA 的 control view 树深度不受本层控制（浏览器可到几十层），递归遍历在深树上会**栈溢出**；
//! 而栈溢出是 abort，不是 `Result`，违反铁律 1 的"要么可验证成功，要么带 `ErrorCode` 失败"。
//!
//! ## 不变量
//! 1. **不静默截断**：节点数超过 `MAX_TRAVERSAL_NODES` → `TargetUnresponsive`，
//!    绝不返回"看起来完整"的部分快照（铁律 1）。
//! 2. **属性读取失败即失败**：某个节点的属性读不到 → 整个快照报错，
//!    而不是把"读不到"写成空串（那会让两个不同状态产生同一个指纹 = 静默失败）。
//! 3. **规范化文本非本地化优先**：角色用 `role_name`（control type 的规范英文名）、
//!    类名与 `AutomationId` 都是机器标识；**不含** `Name` / `LocalizedControlType` 等
//!    本地化属性（ADR-0022 D4）—— 因此同一界面换语言后指纹不变。
//! 4. 遍历顺序 = **文档顺序**（先序、子节点从左到右），因此规范化文本可复现、可回放。
//!
//! 相关：架构 v2 §7.3、`docs/spike-reports/SPIKE-A.md`、`docs/memory/apps/notepad.md` §3。

use assistant_platform_api::{
    Fingerprint, FingerprintScope, PlatformResult, ResolvedWindow, TreeOptions, TreeSnapshot,
};
use windows::Win32::UI::Accessibility::{IUIAutomationElement, IUIAutomationTreeWalker};
use windows::core::{BOOL, HRESULT};

use crate::digest::sha256_fingerprint;
use crate::{error, handles, uia};

use super::bstr_to_string;

/// 单次遍历的节点上限（超出 → 明确报错）。
///
/// 取值理由：Spike A 实测记事本 control view 树只有 **32** 个节点（深度 ≤8），
/// 20000 是它的 600 倍，足以覆盖浏览器等大树；同时把最坏耗时限在可接受的量级
/// （每个节点 ~5 次 UIA 属性读，见 README「已知限制」的性能说明）。
const MAX_TRAVERSAL_NODES: u32 = 20_000;

/// 一次遍历的产物。
struct Traversal {
    /// 计入的节点数（按 `TreeOptions` 裁剪后）。
    node_count: u32,
    /// 规范化文本（进 SHA-256）。
    canonical: String,
}

/// 抓取树快照（`TreeOptions` 裁剪 + 指纹）。
///
/// # Errors
/// - 窗口句柄失效 / 窗口已关闭 → `TargetNotFound`
/// - UIA 不可用或 UIPI 拦截 → `PlatformPermission`
/// - 节点数超预算 → `TargetUnresponsive`（不返回部分快照）
pub fn snapshot_tree(root: &ResolvedWindow, options: &TreeOptions) -> PlatformResult<TreeSnapshot> {
    let hwnd = handles::hwnd_from_window_handle(root.id())?;
    let element = uia::element_for_window(hwnd)?;
    let traversal = traverse(&element, options.max_depth(), options.include_offscreen())?;
    let fingerprint = Fingerprint::parse(sha256_fingerprint(&traversal.canonical))?;
    Ok(TreeSnapshot::new(
        root.clone(),
        fingerprint,
        traversal.node_count,
    ))
}

/// 计算状态指纹（§7.3 的一等接口）。
///
/// # Errors
/// - `WholeWindow`：窗口句柄失效 → `TargetNotFound`；UIA 不可用 → `PlatformPermission`
/// - `Element`：该元素句柄不在**本线程**的元素表里 → `TargetNotFound`（线程本地语义）
pub fn fingerprint(
    window: &ResolvedWindow,
    scope: &FingerprintScope,
) -> PlatformResult<Fingerprint> {
    match scope {
        FingerprintScope::WholeWindow => {
            let hwnd = handles::hwnd_from_window_handle(window.id())?;
            let element = uia::element_for_window(hwnd)?;
            fingerprint_of(&element)
        }
        FingerprintScope::Element(resolved) => handles::with_element(resolved.id(), fingerprint_of),
        _ => Err(error::invalid_args(
            "unknown fingerprint scope: refusing to guess what to fingerprint (fail closed)",
        )),
    }
}

/// 对一棵子树算指纹（不限深度、含离屏节点 —— 指纹必须反映**完整**状态）。
fn fingerprint_of(element: &IUIAutomationElement) -> PlatformResult<Fingerprint> {
    let traversal = traverse(element, None, true)?;
    Fingerprint::parse(sha256_fingerprint(&traversal.canonical))
}

/// 按 `max_depth` / `include_offscreen` 遍历 control view 树（显式栈、文档顺序）。
///
/// 语义细节：
/// - 根节点深度 = `0`；`max_depth = Some(0)` 只快照根
/// - `include_offscreen = false` 时**跳过该节点及其整棵子树**（离屏容器的子节点通常也离屏，
///   逐个再查一次属性只是把同样的开销乘上节点数）
fn traverse(
    root: &IUIAutomationElement,
    max_depth: Option<u32>,
    include_offscreen: bool,
) -> PlatformResult<Traversal> {
    crate::com::with_automation(|automation| {
        // SAFETY: 只读地取 control view walker（UIA 的推荐遍历器，跳过纯布局节点）。
        let walker = unsafe { automation.ControlViewWalker() }
            .map_err(|failure| error::error_from_hresult(failure.code().0, "ControlViewWalker"))?;
        let mut traversal = Traversal {
            node_count: 0,
            canonical: String::new(),
        };
        let mut stack: Vec<(IUIAutomationElement, u32)> = vec![(root.clone(), 0)];
        while let Some((element, depth)) = stack.pop() {
            let offscreen = is_offscreen(&element)?;
            if !include_offscreen && offscreen {
                continue;
            }
            traversal.node_count = traversal.node_count.saturating_add(1);
            if traversal.node_count > MAX_TRAVERSAL_NODES {
                return Err(error::target_unresponsive(format!(
                    "tree traversal exceeded the budget of {MAX_TRAVERSAL_NODES} nodes; \
                     narrow TreeOptions.max_depth or set include_offscreen = false"
                )));
            }
            traversal
                .canonical
                .push_str(&canonical_line(&element, depth, offscreen)?);
            if max_depth.is_some_and(|limit| depth >= limit) {
                continue;
            }
            let children = children_of(&walker, &element)?;
            // 逆序压栈 → 弹出时即为**文档顺序**（左到右）。
            for child in children.into_iter().rev() {
                stack.push((child, depth.saturating_add(1)));
            }
        }
        Ok(traversal)
    })
}

/// 取一个节点的全部子节点（control view，文档顺序）。
fn children_of(
    walker: &IUIAutomationTreeWalker,
    element: &IUIAutomationElement,
) -> PlatformResult<Vec<IUIAutomationElement>> {
    let mut children = Vec::new();
    // SAFETY: 只读遍历；`GetFirstChildElement` / `GetNextSiblingElement` 不接管任何所有权。
    let first = unsafe { walker.GetFirstChildElement(element) };
    let mut current = walk_step_for_actions(first, "TreeWalker::GetFirstChildElement")?;
    while let Some(child) = current {
        // SAFETY: 同上的只读遍历。
        let next = unsafe { walker.GetNextSiblingElement(&child) };
        children.push(child);
        current = walk_step_for_actions(next, "TreeWalker::GetNextSiblingElement")?;
    }
    Ok(children)
}

/// 把 UIA 的「没有更多元素」约定（`S_OK` + NULL，被 `windows` 投影成 `Err(HRESULT(0))`）
/// 翻成 `Ok(None)`；真失败原样上抛。`actions` 模块复用同一约定。
pub(super) fn walk_step_for_actions(
    outcome: windows::core::Result<IUIAutomationElement>,
    context: &str,
) -> PlatformResult<Option<IUIAutomationElement>> {
    match outcome {
        Ok(element) => Ok(Some(element)),
        Err(failure) if failure.code() == HRESULT(0) => Ok(None),
        Err(failure) => Err(error::error_from_hresult(failure.code().0, context)),
    }
}

/// 一个节点的规范化行（每行以 `\n` 结尾；字段顺序固定）。
fn canonical_line(
    element: &IUIAutomationElement,
    depth: u32,
    offscreen: bool,
) -> PlatformResult<String> {
    // SAFETY: 以下均为只读属性查询；返回的 BSTR 由本进程持有并在 drop 时释放。
    let control_type = unsafe { element.CurrentControlType() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "CurrentControlType"))?;
    let class_name = unsafe { element.CurrentClassName() }
        .map(|value| bstr_to_string(&value))
        .map_err(|failure| error::error_from_hresult(failure.code().0, "CurrentClassName"))?;
    let automation_id = unsafe { element.CurrentAutomationId() }
        .map(|value| bstr_to_string(&value))
        .map_err(|failure| error::error_from_hresult(failure.code().0, "CurrentAutomationId"))?;
    let enabled = unsafe { element.CurrentIsEnabled() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "CurrentIsEnabled"))?;
    Ok(format!(
        "{depth}|{}|{class_name}|{automation_id}|offscreen={offscreen}|enabled={}\n",
        uia::role_name(control_type),
        enabled.as_bool()
    ))
}

/// 节点是否离屏。
fn is_offscreen(element: &IUIAutomationElement) -> PlatformResult<bool> {
    // SAFETY: 只读属性查询。
    unsafe { element.CurrentIsOffscreen() }
        .map(BOOL::as_bool)
        .map_err(|failure| error::error_from_hresult(failure.code().0, "CurrentIsOffscreen"))
}

#[cfg(test)]
mod tests {
    /// 预算常量的**下界**必须高于 Spike A 实测的 32 个节点，否则记事本自己都会超预算。
    #[test]
    fn test_traversal_budget_covers_spike_a_notepad_tree() {
        // 常量断言用 `const { .. }`：编译期就失败（clippy 的 `assertions_on_constants` 要求）。
        const {
            assert!(
                super::MAX_TRAVERSAL_NODES > 32 * 100,
                "预算必须远高于 Spike A 实测的 32 节点"
            );
        };
    }
}

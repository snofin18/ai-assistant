# ADR-0087　虚拟化列表改用 `IUIAutomation::ContentViewWalker`

状态：Accepted　日期：2026-10-09（2026-10-10 实现纠偏：与已授权路径对齐）　Supersedes：—　Superseded by：—
关联：ADR-0022 D4/D5（可见文本只作本地化兜底）、ADR-0086（ExactName 兜底）、
`tasks/TASK-256-paint-real-contract-calibration.md`（`DRIFT-256-2`）、
`crates/platform/windows/src/uia/actions.rs`

## 背景

Paint 11 的形状库 / 调色板是 UIA 虚拟化列表（VirtualizedList）：UIA 客户端用
`IUIAutomationTreeWalker` 遍历时，`ControlViewWalker` 只返回控件视图里的已渲染子节点；
`ContentViewWalker` 提供内容视图的逻辑子项，是本 ADR 选择的 `Selection::ByIndex` 视图。
UIA 规范不保证所有 provider 都为未渲染项物化节点；若 provider 仍不暴露目标子项，
本路径必须显式失败，不能退回“取第一个”或猜坐标。
`tasks/TASK-256-*.md` 在 `Ctrl+N` + `Alt+L` 之后枚举形状库直接子节点
`child[0]=ScrollViewer`、`child[1..3]=3 个 Invoke 项`、矩形 (item index 3) 没渲染，
`actions::select` 用 `ControlViewWalker` + `child_at(3)` 拿到第 3 个 Invoke 项
（ellipse），后续 `Invoke` 回退到无 InvokePattern 的元素，触发
`CapabilityMissing: element does not support InvokePattern`。
这是 `Selection::ByIndex` 公共 API 在虚拟化列表下的系统性失能。

## 决策（一句话）

`crates/platform/windows/src/uia/actions.rs::child_at` 由 `ControlViewWalker`
换成 **`ContentViewWalker`**；`Selection::ByIndex` 仍表示第 n 个 content-view 子项；
`walk_step_for_actions` 复用同一套
`Err(HRESULT(0)) → Ok(None)` 约定。配合 `ensure_realized` 在子项拿到后
调用 `ScrollItemPattern::ScrollIntoView` 把未渲染项滚进可见区，平台层不变
公共 API，`Selection::ByIndex` 在虚拟化列表下也能取到正确目标。

细则：

1. **范围最小**：只改 `child_at` 的 walker 源与 `ensure_realized` 调用点；`Selection`
   枚举、`walk_step_for_actions` 行为、`resolve` / `decide_selection` 全部不动。
2. **可见性优先**：`ensure_realized` 在子项支持 `ScrollItemPattern` 时调用
   `ScrollIntoView`，把虚拟化项滚进可见区；不支持时静默跳过，不把无害的滚动
   失败升级为 fatal。
3. **不扩大公共 API**：`Selection::ByIndex` 语义不变（"第 n 个逻辑子项"），与
   ADR-0022 D5（locale_dependent 兜底）一致；不引入 `Selection::ByName` 或
   `Selection::ByAutomationId` 等新变体。
4. **回退不变**：`actions::select` 的 `CapabilityMissing → Invoke` 兜底仍
   保留；`ContentViewWalker` 只解决"取到目标"，`Invoke` / `ScrollIntoView`
   仍走原 fail-closed 路径。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 新增 `Selection::ByName(String)`（locale_dependent 兜底）| ❌ 否决 | 为单平台目标扩公共枚举，且 Name 仍是本地化属性；未证明它比修 walker 更小、更稳定 |
| 2 | 新增 `Selection::ByItemContainer` 走 `ItemContainerPattern::FindItemByProperty` | ❌ 否决 | 要再扩 `crates/platform/api` 公共 API、补 replay/unsupported/fake 全链路；当前卡只授权修 walker 路径 |
| 3 | 在 `paint_handlers` 改用 `pointer_action` 估算坐标 | ❌ 否决 | 坐标靠猜测，DPI / 缩放不同需重测；本质是"绕过 UIA"，与铁律 5（API 优先）冲突 |
| 4 | **把 `child_at` 改用 `ContentViewWalker`，`ensure_realized` 调 `ScrollIntoView`** | ✅ 采纳 | 一次改动覆盖形状库 / 调色板两类虚拟化列表；不扩公共 API；不绕 UIA；`Selection::ByIndex` 语义保留（"第 n 个逻辑子项"） |

## 影响

- `crates/platform/windows/src/uia/actions.rs::child_at`：walker 源从
  `automation.ControlViewWalker()` 换成 `automation.ContentViewWalker()`。
- 同文件新增 `ensure_realized(element: &IUIAutomationElement)`：在子项支持
  `ScrollItemPattern` 时调 `ScrollIntoView`，失败静默忽略。
- `actions::select` 在 `child_at` 之后调 `ensure_realized(&target)`，未渲染子项
  被滚进可见区后再走 `SelectionItemPattern` / `Invoke` 回退。
- `crates/platform/api` / `Selection` 枚举 / `crates/platform/windows/src/uia/resolve.rs`
  不动。
- `crates/platform/windows/src/uia/tree.rs`（`fingerprint` 走 `ControlViewWalker`）
  不动 —— fingerprint 故意只看已渲染节点，hash 必须反映**当前可见**状态。
- `crates/platform/windows/src/uia/resolve.rs`（element resolver）不动 ——
  父候选解析在已解析父元素的子树上跑，仍走 `ControlViewWalker`，避免对
  虚拟化父元素产生意外跨子树命中。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| `ContentViewWalker` 在某些 provider 上语义不一致或仍不返回未物化项 | 仍以显式失败收口：walker 调用失败或遍历耗尽时返回 `TargetNotFound`；真机验收必须确认 Paint 11 的矩形 / 红色色块可命中，否则记录 `DRIFT`，不得静默回退到猜 index / 坐标 |
| `ScrollIntoView` 在非虚拟化元素上变 no-op | `ensure_realized` 失败忽略，不影响路径 |
| `child_at(索引)` 越界时 `walk_step_for_actions` 返回 `Ok(None)` → `TargetNotFound`（与现行为一致） | 不变 |

## 验证方式

1. **正向（真机）**：Paint 11.2605.81.0 形状库 / 调色板，`Selection::ByIndex(3)`
   命中矩形 / 红色色块；`Selection::ByIndex(0)` 命中黑色。
2. **负向（真机）**：把索引设到 > 元素数 → `TargetNotFound`（同现行为）。
3. **fake / replay**：`actions::select` 在 fake 平台上的契约不变，`cargo test --workspace` 全绿。
4. **回归**：`cargo run -p assistant-agent-core --test production_paint test_paint_t3_1_runs_to_completed_on_the_fake_platform` 与真机 `test_paint_t3_1_runs_on_real_paint -- --ignored` 都过。

**重新评估的触发条件**：未来如果需要按"稳定 id"而非"逻辑 index"取子项（避免
受重新排序影响），再独立评估 `ItemContainerPattern` + `Selection::ByName`。
本 ADR 不否决那条路，但当前 scope 不引入。

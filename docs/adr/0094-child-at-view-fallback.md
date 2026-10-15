# ADR-0094　勘误 ADR-0087：`child_at` 按「content view → control view」有序回退

状态：Accepted　日期：2026-10-15　Supersedes：**ADR-0087 的「单一 `ContentViewWalker`」口径**（其余条款不变）　Superseded by：—

关联：ADR-0087（本勘误的对象）、ADR-0022 D4/D5（可见文本只作兜底 / 候选链）、ADR-0022 E6、
`tasks/TASK-256-paint-real-contract-calibration.md`、`tasks/TASK-296-*.md`（`DRIFT-296-1`）、
`docs/memory/apps/paint.md` §12.3

## 背景

ADR-0087 决定 `crates/platform/windows/src/uia/actions.rs::child_at` 由 `ControlViewWalker` 改为
**`ContentViewWalker`**，理由是「形状库 / 调色板都是虚拟化列表，content view 提供逻辑子项」，
并把「provider 不暴露目标子项时必须显式失败」写成不变量。

2026-10-15 真机一手证据（只读探针，用与 handler **完全相同**的选择器链解析
`foreground_color_button`，`DRIFT-296-1`）：

| `child_at` 的 walker | `select(ByIndex(0..5))` |
|---|---|
| `ContentViewWalker`（ADR-0087 现行实现） | **全部** `TargetNotFound: child index N is out of range` |
| `ControlViewWalker` | **全部成功** |

被解析到的元素是调色板 `List`（240×72 @ `(888,240)`）。即：**Paint 调色板的色块是 control 元素、
但不是 content 元素** —— 该 `List` 在 content view 下 **0 个子项**。ADR-0087 的「两类列表都走
content view」这条**对调色板不成立**，`Selection::ByIndex` 在调色板上系统性失能
（T3.1 真机链路正卡在这里）。

## 决策（一句话）

`child_at` 改为**按视图有序回退**：先在 **content view** 取第 n 个子项（保持 ADR-0087 的
「逻辑子项」语义）；该视图给不出第 n 项时再在 **control view** 取；两个视图都取不到才显式
`TargetNotFound`。

细则：

1. **顺序固定：content → control**。content view 能给出第 n 项时**绝不**看 control view ——
   形状库这类「逻辑列表」的行为与本勘误前**完全一致**。
2. **不扩公共 API**：`Selection::ByIndex` 语义仍是「第 n 个子项」，不新增 `Selection` 变体；
   ADR-0087 选项 1 / 2 的否决理由不变。
3. **fail-closed 不变**：两个视图都拿不到 → `TargetNotFound`，**绝不**退回「取第一个」、猜坐标或
   猜 index。
4. **`ensure_realized` 不变**：`select` 拿到子项后仍按 ADR-0087 调
   `ScrollItemPattern::ScrollIntoView`。
5. **ADR-0087 的其余条款全部保留**：不扩 API、不绕 UIA、`tree.rs` 指纹仍走 `ControlViewWalker`
   （指纹只反映已渲染状态）、`resolve.rs` 的父候选解析不动、`actions::select` 的
   `CapabilityMissing → Invoke` 回退不变。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 整体换回 `ControlViewWalker` | ❌ 否决 | 会退回 ADR-0087 记录的旧问题：control view 里 `ScrollViewer` 占 child 0，index 偏移会命中错误项 |
| 2 | 仅在 content view **一个子项都没有**时才用 control view | ❌ 否决 | 更保守，但会漏掉「content view 只物化了前几项」的虚拟化列表；且证据只支持「content view 为空」这一形态，不足以证明它在另一形态下更优 |
| 3 | `ItemContainerPattern::FindItemByProperty` + 稳定 id | ❌ 否决 | 要扩 `crates/platform/api` 公共 API 与 fake / replay 全链路；ADR-0087 已否决，本勘误不重启那条路 |
| 4 | **content → control 有序回退** | ✅ 采纳 | 一次改动同时覆盖「逻辑列表（形状库）」与「仅 control 视图（调色板）」，不改 API、不猜 |

## 影响

- `crates/platform/windows/src/uia/actions.rs`：`child_at` 拆出 `nth_child(walker, element, index, context)`；
  `child_at` 依次用 `ContentViewWalker` → `ControlViewWalker` 调用它；两者皆 `None` →
  `TargetNotFound`（错误信息写明两个视图都越界）。
- `crates/platform/api` / `Selection` / `uia/resolve.rs` / `uia/tree.rs` **不动**。
- 新增真机 `#[ignore]` 用例 `crates/platform/windows/tests/palette_view_fallback.rs`
  （正向：调色板 `ByIndex(3)` 命中；负向：`ByIndex(5000)` → `TargetNotFound`）。
- 证据落 `docs/memory/apps/paint.md` §12.3；`DRIFT-296-1` 由本 ADR + 落地卡闭环。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 两个视图的元素集合语义不同，回退可能取到「不是一个意思」的第 n 项 | **只在 content view 给不出第 n 项时**才回退；真机负向用例覆盖「两侧都越界必须显式失败」；形状库这类逻辑列表仍由 content view 独占 |
| control view 含非交互装饰节点（如 `ScrollViewer`）造成偏移 | 形状库路径**已不再**用 `Selection::ByIndex`（改为 selector + 合成点击，TASK-294）；调色板真机实测 `ByIndex(0..5)` 命中预期色块 |
| 将来出现「两个视图都不对」的容器 | 仍 fail-closed；再按 ADR-0087 的重新评估触发条件评估 `ItemContainerPattern` |

## 验证方式

1. **真机正向**：`cargo test -p assistant-platform-windows --test palette_view_fallback -- --ignored`
   —— Paint 11.2605.81.0 调色板 `Selection::ByIndex(3)` 经 control view 命中色块。
2. **真机负向**：同用例 `ByIndex(5000)` → `TargetNotFound`（不猜）。
3. **回归**：`cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` /
   `cargo test --workspace`；fake `production_paint` 契约不变。
4. **端到端**：`cargo test -p assistant-agent-core --test production_paint -- --ignored`（T3.1 真机）复跑。

**重新评估的触发条件**：若某列表出现「content view 与 control view 的第 n 项指向不同语义元素」，
再评估 `ItemContainerPattern` + 稳定 id 寻址（ADR-0087 的触发条件不变）。

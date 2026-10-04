# ADR-0070　`RoleAndParent` 的父候选只作为作用域，不得作为目标返回

状态：**Accepted**（2026-10-04，按用户 2026-10-04 预授权代为裁决）　日期：2026-10-04　Supersedes：—　Superseded by：—
关联：`docs/PARKING_LOT.md` PL-094、ADR-0017、ADR-0043、ADR-0044、`crates/platform/api/src/target.rs`、`crates/platform/windows/src/uia/{resolve,search}.rs`、`tasks/TASK-031-ui-element-picker-selector-candidates.md`

## 背景（为什么现在要决定）

`RoleAndParent` 的 `SelectorValue` 已经在同一链内用 `parent_id` 引用一个父候选，`search.rs` 也会先解析该父候选，再在其子树内按角色寻找子元素。这个结构表达的是“先限定作用域，再找目标”。

但 `resolve_element` 当前把链内所有候选都传给 `find_all` 并逐个尝试，且调用方固定使用 `min_score_to_try = 0.0`。当父候选自身命中时，顶层循环会把父元素当作目标返回，导致：

- 父候选的职责在顶层解析中从“作用域”变成了“目标”，与 `RoleAndParent` 的语义矛盾；
- 子候选不存在时，调用方可能得到父元素而不是显式失败，形成“看起来成功但实际选错目标”的静默失败；
- UI 拾取器目前在无法确认平台语义时 fail-closed 地不生成 `RoleAndParent`，能力因此被人为闲置。

这属于元素解析链契约与公共行为，不能靠局部调用方约定；必须先以 ADR 冻结语义（铁律 10）。

## 决策（一句话）

**采纳选项 (a)：同一链内被任一 `RoleAndParent.parent_id` 引用的候选只作为子树作用域；它绝不进入顶层目标候选集合，父元素本身永不作为解析结果返回。**

## 决策细化

| # | 内容 |
|---|---|
| **D1** | 对一条 `SelectorChain`，收集所有合法 `SelectorValue::RoleAndParent { parent_id, .. }` 的 `parent_id`；链中 `id` 命中该集合的候选定义为 **parent scope candidate**。 |
| **D2** | `resolve_element` 的顶层尝试只遍历非 parent scope candidate。parent scope candidate 仍保留在链中，供 `search::find_all` / `role_under_parent` 递归解析作用域，不删除链数据、不改 wire 形状。 |
| **D3** | 一条只含 parent scope candidate、没有可返回目标候选的链是配置错误：在碰 COM 之前返回 `ToolInvalidArgs`。空链的既有 `ToolInvalidArgs` 语义不变。 |
| **D4** | 父候选的解析结果只决定子搜索范围：唯一命中 → 在父元素子树内搜子角色；无命中 → `RoleAndParent` 候选零匹配并最终按无顶层匹配返回 `TargetNotFound`；歧义 → `TargetAmbiguous`；候选种类不支持 → `CapabilityMissing`；UIA / COM 调用失败 → 原错误显式透传。 |
| **D5** | 子角色无命中、未知角色或子匹配歧义时，沿用现有显式失败语义：分别为 `TargetNotFound`、`CapabilityMissing`、`TargetAmbiguous`。任何情况下都不得回退返回父元素。 |
| **D6** | 本次不改变 `min_score_to_try`、`locale_dependent` 降权、稳定排序或 `OnAmbiguous::ErrorAndAsk` 的行为。顶层 `resolve_element` 仍使用 `min_score_to_try = 0.0`；parent scope candidate 的作用域解析本来就不参与该阈值，保持现状。 |
| **D7** | 不新增 `SelectorKind` / `SelectorValue` 字段、不新增依赖或抽象层。ADR-0043 的窗口 scope 仍是外层边界；本 ADR 只细化窗口内的父候选作用域。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 维持现状，在 UI / Adapter 层继续禁止生成 `RoleAndParent` | ❌ 否决 | 只把缺陷从平台层移到调用方约定；任何直接调用平台链的代码仍可能把父元素返回，且 `RoleAndParent` 的作用域表达能力被闲置。 |
| 2 | 判定 Windows 通道不支持 `RoleAndParent`，移除可用集合并统一报 `CapabilityMissing` | ❌ 否决 | `search.rs` 已有正确的父作用域解析能力；缺的是顶层候选分类，不是平台能力。删除会让未来拾取器失去一种稳定候选表达，并把可修复的语义缺陷变成永久能力缺口。 |
| 3 | **父候选只作为作用域，顶层过滤并保留递归解析（本 ADR）** | ✅ **采纳** | 与现有链模型和 `role_under_parent` 结构一致；父单独命中时自然走 `TargetNotFound`；改动集中在解析决策，不扩公共类型。 |
| 4 | 给候选新增 `is_scope` / helper 标志，由生成方显式标记 | ❌ 否决 | `parent_id` 已经表达引用关系；再手写标志会形成第二个事实源，并允许数据自相矛盾（被引用却是目标、未引用却声明为作用域）。 |

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

- `crates/platform/windows/src/uia/{resolve,search}.rs`：顶层过滤、只含作用域候选的配置错误、父候选不返回、递归搜索与可测试结果形状；单测覆盖父 + 子选子、父命中但子缺失。
- `docs/PARKING_LOT.md` 关闭 PL-094；本 ADR 同步 `docs/adr/README.md` 与 `docs/memory/decisions.md`；不改 `docs/spec/**`。

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. 单元证据：父 + 子在链内时，顶层只选择 `RoleAndParent` 目标候选；父实体不进入候选尝试。
2. 负向证据：父实体命中、子实体不存在时返回 `TargetNotFound`，并明确记录这不是父实体解析成功。
3. `cargo test -p assistant-platform-windows`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --all --check` 全绿；非 Windows 纯 crate 仍按 ADR-0045 跑目标 clippy。
4. **重新评估触发**：出现“同一链内父候选既需作为目标、又需作为其他候选作用域”的合法场景；或链生成方要求显式声明作用域而不使用 `parent_id` 引用关系。

## 相关 ADR

- ADR-0017（未实现项必须显式登记并显式失败）
- ADR-0043（元素解析必须有窗口 scope；本 ADR 细化窗口内的父候选 scope）
- ADR-0044（平台层歧义策略只有 fail-closed 的 `error_and_ask`）

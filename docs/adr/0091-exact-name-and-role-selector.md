# ADR-0091　`SelectorKind::ExactNameAndRole`：精确名 + 角色组合候选

状态：Accepted　日期：2026-10-14　Supersedes：—　Superseded by：—
关联：ADR-0022 D4/D5（可见文本只作本地化兜底）、ADR-0086（`ExactName`）、ADR-0043（scope）、
`tasks/TASK-256-paint-real-contract-calibration.md`（`DRIFT-256-3`）

## 背景

真实 Paint 11.2605.81.0 里**形状库容器**（`NamedContainerAutomationPeer` / `ControlType.Group`）与它
**自己的** `DropDownButton`（`ControlType.Button`）**同名 `形状`** —— 2026-10-14 真机探针（raw 树 154
节点）证实：`Name` 精确等于 `形状` 的**可及**元素有 2 个（容器 + 按钮；另有 1 个 `TextBlock` 不是
可及元素）。`ExactName`（ADR-0086）因此命中 2 个 → `RoleAndParent` 的父候选歧义 → `TargetAmbiguous`，
形状库**定位不了**。

现有候选种类都无法表达「精确名 **且** 角色」：`ClassAndRole` 是 class+role、没有 name（分不开形状 /
颜色两个 `NamedContainerAutomationPeer/Group`）；`ExactName` 只有 name（与容器的 `DropDownButton` 撞名）。

## 决策（一句话）

`SelectorKind` **加法式**新增 **`ExactNameAndRole`**，取值 `SelectorValue::ExactNameAndRole { name, role }`：
Windows 用 UIA `AndCondition(NameProperty == name, ControlTypePropertyId == role)` 实现；
`locale_dependent = true`（名是可见文本）、最低分兜底；0 / 多命中由解析层显式失败。

细则：

1. **加法式**：`SelectorKind` / `SelectorValue` 都是 `#[non_exhaustive]`，新增变体不破坏既有调用方。
2. **语义边界**：与 `ExactName` 同一套「逐字符相等」（不带 `MatchSubstring`）；`role` 走既有的
   **非本地化角色词表**（`uia::control_type_from_role`），未知 role → `Unsupported`（不是静默不匹配）。
3. **空 `name`** → `ToolInvalidArgs`（空取名会匹配一切）。
4. **作用域**：与其它元素候选一样，只能在已解析窗口 scope 内搜索（ADR-0043）。
5. **回放同语义**：`crates/replay` 用录制节点按 `name` + `role` 过滤；Windows **窗口域**返回
   `Unsupported`（顶层窗口角色恒为 Window）。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 给 `SelectorValue::ClassAndRole` 加可选 `name` | ❌ 否决 | 改动**既有**公共形状（序列化兼容面更大） |
| 2 | 只用 `ExactName` + 接受多命中 | ❌ 否决 | 违反铁律 1（不猜）；`RoleAndParent` 的父候选必须唯一 |
| 3 | **新增独立的 `ExactNameAndRole`** | ✅ 采纳 | 加法式；语义单一；与现有 `ExactName` / `ClassAndRole` 并列，不互相污染 |

## 影响

- `crates/platform/api/src/target.rs`：新增 `SelectorKind::ExactNameAndRole` +
  `SelectorValue::ExactNameAndRole { name, role }` + 模块不变量 4 的兜底清单。
- `crates/platform/windows/src/uia/search.rs`：新增 `name_and_role_search`（`AndCondition`）。
- `crates/platform/windows/src/window/candidates.rs`：窗口域 → `Unsupported`。
- `crates/replay/src/provider.rs`：新增 `exact_name_and_role_matches`。
- `adapters/com.microsoft.paint/**`：形状库 / 形状项**将**改用新候选 —— adapter 与 handler 的落地属
  **TASK-256**（随该卡的真机校准一并提交；本 ADR 只冻结候选种类本身）。
- 不改：`protocol/**` schema、`ErrorCode`、既有候选种类。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 新变体被当成"更强的 name 匹配"而滥用为**主选** | 文档 + 不变量 4 明确它属**本地化兜底**（`locale_dependent`）；主选仍走 `AutomationId` / `ClassAndRole` |
| 与 `ExactName` 语义漂移（一个带子串、一个不带） | 两处都走 `CreatePropertyCondition`（**不带** `MatchSubstring`）；回放侧独立实现同样只做相等 |

## 验证方式

1. `cargo test -p assistant-platform-api -p assistant-replay -p assistant-platform-windows`。
2. `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` + `python eval/tasks/paint/t3.1/validate.py`。
3. 真机：TASK-256 的 ignored 验收（形状库锚点不再 `TargetAmbiguous`）。
4. `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace`。

**重新评估的触发条件**：若将来出现「精确名 + class」的需求，再独立评估，不就地扩 `ClassAndRole`。

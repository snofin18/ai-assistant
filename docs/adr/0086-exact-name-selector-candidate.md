# ADR-0086　无 aid 容器的定位锚点：精确名候选（ExactName）

状态：Accepted　日期：2026-10-09　Supersedes：—　Superseded by：—
关联：ADR-0022（D4 禁可见文本作主 selector / D5 本地化只作兜底）、
`tasks/TASK-256-paint-real-contract-calibration.md`（`DRIFT-256-1`）、
`tasks/TASK-257-paint-shape-container-anchor.md`、`docs/memory/apps/paint.md` §11、
`crates/platform/api/src/target.rs`、`crates/platform/windows/src/uia/search.rs`

## 背景（为什么现在要决定）

Paint 11.2605.81.0 真机实测（2026-10-09，见 `eval/tasks/paint/t3.1/probe-evidence.json`
的 `keytip_and_palette_probe`）：形状库与调色板的**容器**是 `Group`，其 `Name` 是本地化文本
（`形状` / `颜色`）；两者内部的画廊是 `GridView` / `List`。这两个容器与画廊
**全部没有 `AutomationId`、也没有可区分的 `ClassName`**。

现有候选种类在真机上逐条失败：

| 候选 | 真机结果 | 原因 |
|---|---|---|
| `automation_id` | 不可用 | 容器与画廊的 `AutomationId` 均为空 |
| `class_and_role class=GridViewItem role=ListItem` | `TargetAmbiguous`（43 命中） | 形状项与颜色项同类同角色 |
| `name_regex "形状"` | 命中 **3** 个（`形状` / `形状轮廓` / `形状填充`） | `NameRegex` 走 UIA **原生子串**匹配（`PropertyConditionFlags_MatchSubstring`，DRIFT-017-4），不是真 regex |

父候选歧义 → `RoleAndParent` 在父阶段就返回 `TargetAmbiguous`，**根本进不到子项解析**。
`TASK-256` 因此停在 `DRIFT-256-1`，真机十次验收无法开跑。

## 决策（一句话）

`SelectorKind` 新增 **`ExactName`**：用 UIA `PropertyCondition(NameProperty, value)`
（**不带** `PropertyConditionFlags_MatchSubstring`）做 **精确相等**匹配；标
`locale_dependent = true`，只作**兜底**，且必须 fail-closed。

细则：

1. **取值**复用既有 `SelectorValue::Text`；`ExactName` 只接受 `Text`，其它取值返回
   `Unsupported`（与 `AutomationId` / `NameRegex` 现有约定一致）。
2. **精确语义**：`Name` 与取值**逐字符相等**才命中；`形状` 命中容器，`形状轮廓` **不**命中。
3. **本地化**：`locale_dependent` 必须为 `true`；分数上限低于所有非本地化候选，
   多语言环境按架构 v2 §6.3 自动降权，**永不**升级为主 selector（ADR-0022 D4 / D5）。
4. **fail-closed**：0 命中 → `TargetNotFound`；>1 命中 → `TargetAmbiguous`（铁律 1，
   不得「取第一个」）。
5. **零依赖**：UIA 原生支持精确匹配，**不引入 regex 引擎**（保持 DRIFT-017-4 的结论）。
6. **不替代 `AutomationId`**：有稳定 aid 的目标继续优先用 aid；`ExactName` 只服务
   「既无 aid、又无区分性 class、且子项同质」的容器型目标。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 引入 regex 引擎，让 `NameRegex` 支持 `^形状$` | ❌ 否决 | DRIFT-017-4 已定「本仓没有 regex 引擎」；新增依赖 = 漂移触发器 ①，且只为一条兜底 |
| 2 | 新增结构化序号候选（class + role + 第 N 个） | ❌ 否决 | ADR-0022 E6：UIA 只承诺「按树中遇到的顺序返回」，**未承诺跨版本稳定**；一旦前面插入新画廊，序号会**静默**指向错元素（铁律 1 的头号敌人） |
| 3 | 把容器名升级成主 selector（直接用本地化名定位） | ❌ 否决 | ADR-0022 D4 + AGENTS.md §7 明令禁用可见文本作主 selector |
| 4 | 用 `role_and_parent` 套 `name_regex` 消歧 | ❌ 否决 | 真机已证父候选阶段即 `TargetAmbiguous`（子串命中 3 个），链路进不去 |
| 5 | **精确名候选（`ExactName`），仅兜底** | ✅ **采纳** | 零依赖、精确消歧、失败模式是显式升级；与 ADR-0022 D5 第 5 级「本地化兜底」的定位一致 |

> 选项 2 不是永久否决：若将来出现「连 Name 都没有」的同质容器，再单独评估
> 「序号 + 期望匹配数」的 fail-closed 变体。本轮不引入。

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

- `crates/platform/api/src/target.rs`：`SelectorKind`（`#[non_exhaustive]`）**加法式**新增
  `ExactName` 变体；`SelectorValue` **不改形状**（复用 `Text`）。
- `crates/platform/windows/src/uia/search.rs`：新增 `ExactName` 分支，复用
  `property_search(scope, UIA_NamePropertyId, value, /* substring */ false)`。
- replay / unsupported / fake provider：同语义（精确相等 + 0/多命中 fail-closed）。
- `adapters/com.microsoft.paint/selectors/targets.json`：形状库与调色板容器改用
  `exact_name` 兜底候选（`locale_dependent=true`，低分）。
- 不改 `ErrorCode`；不新增 crate / 第三方依赖；不改 IPC / DB schema。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 换系统语言后 `ExactName` 全失配 | 标 `locale_dependent=true` + 只作兜底；失配即显式升级（`TargetNotFound`），**不得**静默改选别的容器 |
| 容器名含不可见字符（尾随空格 / 全角差异） | 实现卡必须**真机复验精确字符串**（`CurrentName()` 逐字符），失配即 fail-closed，不得靠 `trim` 猜 |
| 有人把 `ExactName` 当主 selector 用 | 分数上限 + 代码评审；`docs/spec` 与 app 档案写清它只是第 5 级兜底 |
| 与 `NameRegex` 语义混淆 | 文档与注释明写：`NameRegex` = UIA 子串，`ExactName` = 逐字符相等；两者都本地化、都只兜底 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. **正向（真机）**：`exact_name "形状"` + `role Group` 在 Paint 11.2605.81.0 上命中
   **唯一**容器；`exact_name "颜色"` 同理。
2. **负向（真机）**：把取值改成 `形状轮廓` → 容器**不**命中（证明是精确而非子串）；
   把取值改成空 → 当场拒绝（配置错误）。
3. **多语言**：系统语言切 en-US 后该兜底失效并显式升级，**不得**选到别的容器。
4. **回归**：`AutomationId` / `ClassAndRole` / `RoleAndParent` 的既有行为不变；
   `cargo test --workspace` 全绿。

**重新评估的触发条件**：Paint 后续版本给容器补上 `AutomationId`（届时降级 `ExactName` 到
最低分甚至删除）；或出现「连 Name 都没有」的同质容器需求（回到选项 2）。

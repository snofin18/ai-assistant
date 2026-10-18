# ADR-0096　Paint 工具栏寻址：按总分类拆工具 + 组内有界路径

状态：**Accepted**（2026-10-18，人类「按你的建议做」授权；TASK-317）
日期：2026-10-18
Supersedes：—
Superseded by：—
关联：**TASK-309**、**TASK-317**、ADR-0095、`AGENTS.md` §7（禁止通用逃生舱）、
`docs/memory/apps/paint.md` §13.6（同名冲突）/ §13.8（ControlView 实测）/ §13.17（工具 vs 画笔）/ §13.18（树结构）

---

## 背景

Paint 的工具栏是**两级（且更深）的树**（`paint.md` §13.18 实测）：

- **总分类 8 个**：选择 / 图像 / 工具 / 画笔 / 形状 / 颜色 / Copilot / 层 —— 在 ControlView 里就是 `Group`。
- **子项**挂在各组下：工具 → 铅笔 / 填充 / 文本 / 橡皮擦 / 颜色选取器 / 放大镜；形状 → 23 个形状；画笔 → 9 种笔型。
- **还有三级**：图像 → 翻转 → 水平翻转 / 垂直翻转；形状 → 画廊 → 矩形。

而现在的 `paint.tool.select` 用一份**扁平 enum** 表达选择：

```text
["pencil","brush","eraser","fill","rectangle","ellipse","line","color_picker"]
```

三个问题：① 把**不同总分类**的子项混成一份全局清单（`pencil`/`eraser`/`fill`/`color_picker` 属「工具」，`rectangle`/`ellipse`/`line` 属「形状」，`brush` 属「画笔」）；② **漏项**（真实存在的「文本」「放大镜」根本没进 enum）；③ **同名无法区分** —— §13.6 记过 `蜡笔`/`记号笔`/`油画笔`/`普通铅笔`/`水彩画笔` 这 5 个名字在**三个 flyout 里都出现**。

## 决策（一句话）

**对外按总分类拆成多个语义工具；每个工具内部用该总分类自己的词汇表 + 有界路径（1..=3 级）寻址；每组的切换与读回通道各自声明。**

### D1　对外语义面：一个总分类一个工具

| 工具 | 总分类 | 状态 |
|---|---|---|
| `paint.tool.select` | 工具 | 改（enum → 组内词汇表） |
| `paint.shape.select` | 形状 | 新 |
| `paint.brush.select` | 画笔 | 新 |
| `paint.image.transform` | 图像 | 新（含三级） |
| `paint.color.select_foreground` | 颜色 | 已有，归一化到同族 |
| `paint.layers.*` | 层 | 新 |
| 选择 / Copilot | — | 本轮不进 schema |

**明确否决**「单一全局工具 + 全局路径」（如 `paint.ribbon.invoke(path=[...])`）：那是 `AGENTS.md` §7 所禁通用逃生舱（`execute_code` / 通用 `run_shell`）的 UI 版 —— 策略引擎无法对"路径字符串"做有意义的放行判断（铁律 3），能力矩阵会被掏空，静态可审计性消失。

### D2　组内寻址：有界路径，不是自由文本

组内子项用 `path: string[]` 表达：**长度 1..=3**，**逐元素必须命中该总分类声明的词汇表**；未知元素 / 超深 / 空数组 → 既有 `ToolInvalidArgs` fail-closed（不新增 ErrorCode）。示例：

```json
{ "path": ["矩形"] }                 // 形状组，1 级
{ "path": ["翻转", "水平翻转"] }      // 图像组，2 级
```

### D3　词汇表按组声明、按组校验

每个工具自带本组词汇表，**不跨组共享名字**。这样同名子项不再有歧义（§13.6 的 5 个同名项各自落在自己的组里），新增子项也不必改全局 enum。

### D4　通道按组声明（实测事实，不是设计偏好）

| 总分类 | 切换通道（实测） | 备注 |
|---|---|---|
| 工具 | `TogglePattern.Toggle()` **有效** | §13.8 实测 `Off → On` |
| 画笔 | flyout `MenuItem`（`Invoke`/`Toggle`） | flyout 项无稳定 aid |
| 形状 | `Toggle()` **空操作** → 只能 L4 合成点击 | `DRIFT-256-5` |
| 颜色 | 20 色 `ListItem`（`Invoke`） | 仅本地化 name |

### D5　读回必须真实，不可用会粘住的信号

实测：把工具从橡皮擦切回画笔后，`EraserTool` 的 `ToggleState` **仍报 `On`**。因此**不得**用 `ToggleState` 当「当前工具」的读回；读回不可靠时必须显式标注未验证，**不得**用自有记录冒充 UI 状态（铁律 1）。

### D6　迁移

`paint.tool.select` 的扁平 enum 废弃，取值收敛为「工具」组子项；其余总分类各自成工具。适配包 `tools.json` 与 selector pack 同步；`paint_handlers.rs` 里「除 rectangle 全拒」的分支随之替换为按组分发。

## 影响

- 改**公共 Tool schema** → 属铁律 10，先有本 ADR 再落代码。
- 组内路径让三级（图像 → 翻转 → 水平翻转）与同名消歧同时成立，且新增子项不再动公共接口。

## 备选方案（未采纳）

1. **单一工具 + 全局路径**：通用逃生舱，否决（见 D1）。
2. **维持扁平 enum，只补「文本」「放大镜」**：不解决同名歧义与三级寻址，且每加一个形状都要动公共 schema。
3. **纯按名字寻址（不分组）**：§13.6 已证同名跨 flyout 重复，必然 `TargetAmbiguous`。

## 验证方式

- 契约：非法 `path`（未知元素 / 长度 0 / 长度 >3）必须 `ToolInvalidArgs`。
- 同名消歧：同一子项名出现在两个组时，两个工具各自解析到各自组的元素。
- 三级：`paint.image.transform` 的 `path: ["翻转","水平翻转"]` 在真机可达。
- 读回：不得出现"UI 未变但输出成功"（铁律 1 回归用例）。

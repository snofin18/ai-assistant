# spec: 能力矩阵（capability matrix）

> 摘要：Capability matrix 规范（Host / Skill / Platform 三层 × 资源访问 / 副作用 / 风险级）。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（ADR-0042、ADR-0104、ADR-0105、ADR-0107 与 ADR-0108 已批准字段语义）　版本：0.5　日期：2026-11-23
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

定义**能力矩阵**：每个 capability 的资源访问类别、副作用、风险级与审批要求，作为策略引擎（**默认拒绝**）的唯一放行依据。

> **名称澄清（ADR-0042）**：本 spec 的「能力矩阵」= **每条能力的风险·审批声明**（Rust `CapabilityEntry`）， > 它是**策略引擎的输入**（TASK-021）。它与另外两个**同名但不同物**的概念不是一件事： > ① **`CapabilityCatalog`** = `protocol/capability-matrix/capability-1.0.json` 的**稳定能力标识目录** > （`<layer>.<capability>` 2 段式 + `stability` + `version`，**不含**风险/审批列）； > ② **`CapabilityMatrix`** = 架构 v2 §13.1.2 的**运行时探测结果**（Rust `assistant_platform_api::CapabilityMatrix`： > `probed_at` / `platform` / `session` / `channels` / `degradations`）。

## 2. 范围

**管**：capability 的三个维度（Resource / SideEffect / Risk）+ Approval 列、风险单调升级规则、Resource 与 Risk 的枚举白名单。

**不管（不做清单）**：
- 不管**策略规则 DSL** 与判定算法（归 TASK-021 `crates/policy`；本 spec 只给矩阵数据与不变量）
- 不管 Tool 与 capability 的**绑定**（归 `docs/spec/tool-schema.md` 的 `capabilities` 字段）
- 不管审批 UI / 人工确认交互（归产品层）
- 不定义 L5（资金 / 对外发布类）之外的业务语义；L5 **永久禁止自动化**（铁律 6）

## 3. 类型定义

三层（Host / Skill / Platform）× 三维（资源访问 / 副作用 / 风险级）的矩阵。

```
| Capability           | Resource | Side Effect | Risk | Approval |
|---------------------|----------|-------------|------|----------|
| WindowEnumerate     | read     | none        | L1   | auto     |
| WindowRead          | read     | none        | L1   | auto     |
| WindowActivate      | invoke   | UI focus    | L2   | auto     |
| FileRead            | read     | none        | L1   | auto     |
| FileWrite           | write    | disk        | L2   | auto     |
| FileDelete          | destroy  | disk        | L3   | required |
| ProcessSpawn        | invoke   | proc        | L3   | required |
| NetworkEgress       | send     | net         | L4   | required |
| MoneySend           | send     | ext-system  | L5   | forbidden |
| ClipboardRead       | read     | clipboard   | L2   | auto     |
| ClipboardWrite      | write    | clipboard   | L2   | auto     |
```

## 4. 不变量

1. **单调升级**：L1 → L5 风险越升越高；不允许 downgrade（L3 不能声明成 L2）。
2. **Approval 强制**：L3+ 必须 `Approval = required`；L5 必须 `Approval = forbidden`（= 强制拒绝）。
3. **Resource 类型枚举**：read / write / send / invoke / destroy = 5 类，不允许模糊（例 "modify"）。
4. **SideEffect 必填**：必须明确声明 `none` / 列举副作用（= `focus change` / `proc spawn` / `net egress`）。

---

## 5. 与其他 spec 的关系

| 引用方向 | spec | 关系 |
|---|---|---|
| 被依赖 | `docs/spec/tool-schema.md` | Tool 声明的 `capabilities` 必须是本矩阵中**已存在**的项 |
| 被依赖 | `docs/spec/error-codes.md` | 矩阵的 Risk / Approval 列对应 `PolicyDenied` / `ApprovalRequired` |
| 被依赖 | `docs/spec/ipc-protocol.md` | 握手阶段交换 capability 列表用于权限协商 |
| 相关 | `docs/spec/audit-event.md` | 策略判定结果（deny / approval）必须落 audit event |

## 6. Adapter capability 引用

Adapter tool envelope 的 `capabilities` 是 capability catalog 的**稳定标识引用集合**，不是
运行时探测快照：

- 每个 Adapter tool 至少声明一个 capability，数组内不得重复。
- 每个标识必须命中 `protocol/capability-matrix/capability-1.0.json` 的 `capabilities[].id`；
  未知标识、空数组、跨 bundle 悬空引用均 fail-closed。
- Adapter 不能借此声明新的 capability；新增 capability 仍先走 catalog ADR/schema 变更。
- `CapabilityMatrix` 的运行时探测结果继续由平台层产生，Adapter envelope 不得伪造
  `probed_at/platform/session/channels/degradations`。

字段与 envelope 的 owner 见 **ADR-0104** D4；机器校验由后续通用 Adapter validator 实现。

## 7. Validator catalog consumption

`docs/spec/adapter-bundle.md` §6 冻结 validator 的 capability 引用规则：

- envelope `capabilities` 必须非空、去重，并按 UTF-8 byte order 升序；
- 每个 id 必须逐字命中本文件对应的
  `protocol/capability-matrix/capability-1.0.json#/capabilities/*/id`；
- validator MUST NOT 从 runtime `CapabilityMatrix` 的 `probed_at/platform/session/channels/
  degradations` 推断静态授权；
- validator MUST NOT 维护 Adapter 专用 capability alias 或第二份 catalog。

后续 catalog generation 必须以 protocol catalog 为输入；若该文件变化，validator 侧只能通过
生成/引用更新，不能手改一份平行枚举。catalog 生成与 `codegen --check` 接入属于 validator
implementation 卡，不在 TASK-383 冒充完成。

## 8. Excel L1 COM capability family

ADR-0107 冻结七个 `experimental` / `0.1.0` 的 Excel L1 COM capability：

| Capability | Resource | Side effect | Risk floor | Approval floor | 允许语义 |
|---|---|---|---|---|---|
| `com.workbook_open` | invoke | process spawn / file open | L3 | required | 专用 owned 实例打开明确工作簿并记录身份与回收责任 |
| `com.workbook_read` | read | none | L1 | auto | 读取 workbook/worksheet identity、命名区域、尺寸和计算元数据 |
| `com.range_read` | read | none | L1 | auto | 显式 `value2` / `formula` / `text` 有界读取 |
| `com.range_write` | write | workbook content | L2 | required | 有界 scalar literal 写入；固定 `value2`，公式形态字符串失败；L1 snapshot + shadow copy |
| `com.workbook_save` | write | disk | L2 | required | 原位保存；冲突检测、影子副本和身份复验 |
| `com.chart_create` | write | workbook content | L2 | required | 从已确认 Range 创建封闭类型 chart object |
| `com.chart_export` | write | disk | L2 | required | create-new PNG；不覆盖既有目标 |

不变量：

1. `com.workbook_close` 不是独立 capability；close / Quit / COM release / PID reap 是
   `com.workbook_open` 的生命周期责任。
2. 所有读取必须保持 workbook 内容不变；UIA、keyboard、clipboard 或 `element.*` 不得冒充
   COM range 读写。
3. 所有写能力的审批下限为 `required`；Stage 2 `allow_unattended=false` 不被本 spec 放宽。
4. `com.range_write` 当前只允许 bounded scalar literal 且固定 `value2`；以 `=`、`+`、`-`
   或 `@` 开头的文字字符串必须失败，公式写入、结构修改与任意 COM method/property dispatch
   必须新 ADR。
5. `com.chart_export` 只允许 create-new；目标已存在、身份漂移或半文件残留必须失败。
6. Capability catalog 出现 ID 只表示词汇已批准，不表示 runner 或 Tool 已实现。

消费矩阵：

- `com.workbook_open` Tool 必须**只含该一个 capability**，并位于 write/high|critical/
  approval/non-idempotent/`l3_irreversible` Tool；不得与其他 capability 混用，也不得藏在
  read 或 L1/L2 Tool 中。
- T2.1 read Tool 只可用 workbook/range read，并作用于已 open 且身份可复验的 owned workbook；
- T2.2 增加 bounded scalar-literal range write + in-place save；
- T2.3 增加 chart create + create-new PNG export；
- Tool envelope 仍必须按 ADR-0104/0105 声明非空、去重、排序的精确子集。

永久拒绝 `com.execute`、`com.eval`、`com.invoke`、`com.macro`、`com.raw_dispatch`、
`com.script`、`office.*` 与作为 alias 的 `spreadsheet.*`。

## 9. Excel T2.1 concrete consumer

ADR-0108 把前述 `com.*` 映射到四个尚未实现的目标 Tool：

| Tool | Capability | Effect / Reversibility | 约束 |
|---|---|---|---|
| `excel.workbook.open` | 仅 `com.workbook_open` | write / `l3_irreversible` | high / approval / non-idempotent；`.xlsx` local absolute path；无既有 Excel |
| `excel.workbook.read` | `com.workbook_read` | read / `none_readonly` | task-scoped session；不读 cell values |
| `excel.range.read` | `com.range_read` | read / `none_readonly` | 显式 `value2` / `formula` / `text`；最多 4096 cells / 256 KiB |
| `excel.range.summarize` | `com.range_read` | read / `none_readonly` | 固定 `value2`；确定性 count/sum/min/max |

实现未合入、fake tests 未通过或真机未验收前，`tools.json` 必须保持空数组，ToolBus 不得注册
这些名称。完整输入输出、session、cleanup 与错误矩阵归
`docs/spec/excel-t21-readonly-workflow.md`。

---

## 附录：演进记录

| 版本 | 日期 | 变更 |
|---|---|---|
| 0.1 | 2026-09-20 | 初稿（项目进度督察后批量补齐 stage-0 DoD #5）|
| 0.2 | 2026-11-17 | 新增 §6 Adapter capability 引用；依据 **ADR-0104** / TASK-376 |
| 0.3 | 2026-11-18 | 新增 §7 validator catalog consumption、排序与禁止 runtime-matrix 推断；依据 ADR-0105 / TASK-383 |
| 0.4 | 2026-11-23 | 新增 §8 Excel L1 COM capability family、风险/审批下限与 broad escape 拒绝；依据 ADR-0107 / TASK-393 |
| 0.5 | 2026-11-23 | 新增 §9 Excel T2.1 四 Tool consumer 与未实现前空 surface 闸门；依据 ADR-0108 / TASK-394 |

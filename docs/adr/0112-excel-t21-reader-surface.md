# ADR-0112: Excel T2.1 read-only runner surface

状态：**Accepted**（2026-11-25，TASK-401 后续批准范围）
日期：2026-11-25
澄清：ADR-0108、ADR-0110、ADR-0111
相关：TASK-401、TASK-402
落点：`docs/spec/excel-t21-readonly-workflow.md`

## 背景

TASK-401 已证明 platform-windows 的窄公共 `ExcelComRunner` 可以安全完成真实 COM
单工作簿 smoke，但公共面只覆盖 active-sheet A1 text read。agent-core 的
`ExcelT21Runtime` 需要 workbook metadata、named ranges、calculation metadata、
range 三模式读取、fingerprint 与 ownership 验证。

直接在 agent-core 中持有 `IDispatch` 会违反铁律 7/8；在 `assistant-platform-api`
新增 Excel 专用 trait 又会违反 ADR-0111。因此需要扩展 platform-windows handle，
仍保持中立结果类型。

## 决策

**在 `assistant-platform-windows::excel_com_runner` 内完成完整只读表面；agent-core
只做中立平台类型到现有 ExcelT21 领域类型的私有映射。**

公共面固定为：

- owned process identity：PID、creation FILETIME、executable path；
- workbook snapshot：canonical path、read-only、worksheets、named ranges、calculation；
- worksheet：exact name、one-based ordinal、optional used-range address；
- named range：exact name、workbook/worksheet scope、refers-to；
- range selector：named range 或 worksheet name/ordinal + A1 cell bounds；
- range read mode：`value2` / `formula` / `text`；
- cell value type：blank / number / text / boolean / error；
- deterministic workbook fingerprint；
- ownership verification。

约束：

1. COM 对象仍只在 worker thread；公共类型只含自包含数据。
2. 平台层拒绝大于 4096 cells 的 range；不得截断。
3. workbook snapshot 的 worksheets/named ranges 必须有硬上限，超过失败。
4. A1 parser 与 agent-core 一致，最大 `XFD1048576`，拒绝前导零和反向范围。
5. worksheet-scoped name 由 Excel `Name.Name` 的 `Sheet!Name` 形状解析；若实机
   不满足，必须显式失败并记 DRIFT，不得猜作用域。
6. fingerprint 是 workbook metadata 的确定性 SHA-256；不代表文件内容 hash。
7. 生产 `tools.json`、rollback、ToolBus 与 assembly 继续为空。

## 后果

- agent-core 可以下一步实现真实 `ExcelT21Runtime` 适配，而不跨线程携带 COM。
- 平台 API 变宽，但仍不暴露模型可见 Tool。
- 真实 COM 验收需覆盖 metadata、named range、三种 range mode、fingerprint 与进程收敛。

## 拒绝方案

- 把 `IDispatch` 或 windows crate 类型返回给 agent-core。
- 在 `platform/api` 新建 Excel trait。
- 在 agent-core 重新实现 COM 生命周期。
- 对超大 range 截断或把 workbook metadata 超限降级成成功。

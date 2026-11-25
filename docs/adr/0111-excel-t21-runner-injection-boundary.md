# ADR-0111: Excel T2.1 runner injection boundary

状态：**Accepted**（2026-11-25，人类批准 TASK-401）
日期：2026-11-25
澄清：ADR-0110
相关：TASK-399、TASK-401
落点：`docs/spec/excel-t21-readonly-workflow.md`

## 背景

ADR-0110 要求真实 COM 与人类在场单次 `.xlsx` 验收先于生产 Tool 注册，同时禁止把
没有生产 caller 的私有 runner 用 `#[allow(dead_code)]` 留在生产代码里。

TASK-399 因此把 runner 放在 `#[cfg(test)]` 中，只验证线程、队列、关闭和 Windows
IDispatch 编译边界。下一步若要让 agent-core 的真实 COM 适配器和人工验收使用该 runner，
必须先确定跨 crate 的安全注入边界与 assembly 责任。

## 决策

**由 `assistant-platform-windows` 持有 Excel-specific COM worker 实现，并通过一个窄公共
handle 暴露显式操作；`agent-core` 继续持有 Excel T2.1 领域类型和运行时适配，不向
`assistant-platform-api` 增加 Excel 专用 trait。**

具体约束：

1. `crates/platform/windows/src/excel_com_runner.rs` 在 Windows 生产构建中为
   `pub`，但只暴露可安全跨线程请求的 handle、可序列化/自包含结果类型和显式操作。
2. COM object、STA guard、IDispatch、owned PID 和 worker 状态只能存在于 dedicated
   worker thread；公共 handle 不得携带 COM 指针。
3. TASK-401 的公共面只覆盖真实 COM 验收所需的 `create application`、owned
   `process id`、本地 `.xlsx` read-only open、active sheet cell text read、close 和
   bounded shutdown。完整四 Tool 域映射由后续卡实现，不得用 smoke 结果冒充完成。
4. `agent-core` 的 `ExcelT21Runtime` 仍是私有领域边界；后续适配器必须把平台 handle
   映射为现有 `OpenedWorkbook` / `WorkbookSnapshot` / `RangeSnapshot`，不得把领域类型
   下沉为 Excel 专用 `platform/api` trait。
5. 真实 COM 验收测试必须显式 ignored/人工触发，读取环境变量指定的单个 `.xlsx`；
   运行前要求没有既有 `EXCEL.EXE`，运行后要求 owned PID 终止、文件 digest 不变且本轮
   不残留 Excel 进程。
6. 本 ADR 不放宽 ADR-0108/0110：生产 `tools.json`、rollback recipe、production
   assembly 的 Tool 注册与生产 ToolBus 继续为空，直到单次真机和重复收敛通过。

## 影响

- TASK-399 的 fake tests 保留；Windows 生产构建新增一个可由 agent-core 验收测试调用的
  公共 handle。
- 平台层承担 Excel COM 线程与对象生命周期；agent-core 只负责领域校验与映射。
- 真实 COM 验收可以先于模型可见 Tool 完成，不需要伪造生产 caller。

## 拒绝的方案

- 在 `assistant-platform-api` 新增 Excel 专用 trait：把应用知识放进通用平台接口，
  违反 ADR-0110 的边界。
- 让 COM handle 或 IDispatch 跨进程/跨线程返回：违反铁律 8 和 COM apartment 约束。
- 用 `#[allow(dead_code)]` 保留无 caller 的私有 runner：ADR-0110 已明确禁止。
- 在单次真实 COM 验收前填写 `tools.json` 或注册 ToolBus：会让模型看到未验证能力。

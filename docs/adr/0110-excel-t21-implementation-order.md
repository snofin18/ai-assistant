# ADR-0110: Excel T2.1 implementation order and runner boundary

状态：**Accepted**（2026-11-24，人类确认“ADR-0108 D10 优先”）
日期：2026-11-24
Clarifies：ADR-0108
Supersedes：无
Superseded by：无
关联：TASK-394、TASK-395、TASK-398、TASK-399
落点：`docs/spec/excel-t21-readonly-workflow.md`

## 背景

ADR-0108 D10 明确要求：真实 COM、本地 `.xlsx` 单次验收和重复运行收敛
全部通过前，不得把四个 Excel T2.1 Tool 写入生产 `adapters/com.microsoft.excel/tools/tools.json`。

Excel T2.1 spec §11 曾把 “Adapter tool envelopes/rollback” 排在 Windows COM runner
之前，同时 spec §1 又声明其不定义 ToolSchema 实例或 `tools.json`。这两个表述会诱导
实现者在 runner 尚不存在时填充生产 Tool surface，直接违反 ADR-0108。

## 决策

一句话：**ADR-0108 D10 优先；先实现并验证私有 COM runner，真实 COM 与重复收敛完成
后才写生产 Tool envelope 和注册 ToolBus。**

固定顺序为：

1. fake runtime、session registry、输入 parser 与负向测试；
2. 私有 `ToolHandler` 包装与真实 in-process ToolBus envelope 测试，仍不进入生产装配；
3. Windows COM runner 私有原型与 fake tests，仍不启动或暴露生产 Tool；
4. 人类在场本地 `.xlsx` 单次验收；
5. 重复运行、进程/session/handle 收敛与失败证据；
6. 只有以上全部通过，才写生产 `tools.json`、rollback recipe、production assembly，
   并注册四个 Tool。

第 3 步允许实现真实 COM 类型、dedicated STA worker 和 fake backend 测试，但不得：

- 创建或启动 Excel；
- 读取真实工作簿；
- 进入生产 assembly；
- 暴露公共 `platform/api` Excel 专用 trait；
- 写 `tools.json`、rollback recipe 或 ToolBus registry。

为了提高实现速度，只计划用于第 3 步验证、且尚未有生产 caller 的私有 runner 可以是
test-only 模块。它必须在 `#[cfg(test)]` 下由 fake tests 实际执行，不得用
`#[allow(dead_code)]` 掩盖没有 caller 的生产代码。

## 后果

- spec 不再把生产 Tool envelope 描述成 COM 前的步骤；
- TASK-399 可以先验证线程、有界队列、顺序化和真实 IDispatch 编译边界；
- 生产 Tool surface 继续为空，避免把未实现能力暴露给模型；
- 下一张卡若要接生产 caller，必须先提出公共注入边界或 assembly 变更并获人类裁决。

## 拒绝条件

- 用 test-only prototype 冒充生产 COM runner 已完成；
- 在真实 COM 与重复收敛前填写 `tools.json` 或注册 ToolBus；
- 用 `#[allow(dead_code)]` 保留无 caller 的私有生产 runner；
- 把 COM handle、PID 或 HWND 当作跨线程/跨进程 token；
- 为绕过边界新增 Excel 专用 `platform/api` trait 或依赖。

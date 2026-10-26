# 真实应用人工验收边界

日期：2026-10-26　关联：TASK-328、TASK-056、TASK-057、TASK-058、TASK-305

## 1. 决策

真实 Edge、GUI 登录、表单提交、商业应用和凭据相关验证只允许在**人类在场的会话**中进行。
automation 不得自行点击真实应用、登录账号、处理验证码、复制用户 profile、提交外部表单或
以推测结果代替人工验收。

这条边界不是放宽 Blocked 任务，而是明确哪些剩余工作不能再由无人值守 automation 补齐。

## 2. 每次人工验收必须具备

1. 明确的应用、目标 URL / 文档、操作步骤和停止条件。
2. 使用专用 profile 或临时文档；不得读取、复制或附加用户日常 profile / Cookie。
3. 提交、保存、覆盖、下载等不可逆动作必须停在 point-of-no-return 之前，由人类确认。
4. 运行前后证据必须落盘：命令、退出码、截图或 DOM / UIA 摘要、输出文件、失败原因。
5. 人工操作必须由人类完成；agent 可以解释步骤、检查结果和记录证据，但不能把 GUI 自动点击
   冒充人类在场。
6. 人工会话结束前必须写 `机制=human` 轮次文件，正式重置 `N_auto`。

## 3. 当前任务分类

| 任务 | 当前状态 | 人工在场是否足够 | 说明 |
|---|---|---|---|
| TASK-056 | Blocked | 否 | 已有 T5.1 contract、离线 fixture 与 browser runtime 自检，但产品命令尚未装配完整 browser task-package 执行与 CSV 产出；人工在场只能在该实现完成后做最终真机验收 |
| TASK-057 | Blocked | 否 | 卡面仍是占位稿，`form_fill` provider、diff / 来源归因和提交前人工确认尚未实现 |
| TASK-058 | Blocked | 否 | 安全 regression workflow 与阶段总验收判据尚未展开；必须先有可执行的安全断言 |
| TASK-305 | Done（superseded） | 不适用 | 后续 TASK-306 修复弹层 fail-closed，TASK-307/309 已用当前断言完成真机路径；原 A/B/C 改动不再执行 |

## 4. 对 automation 的硬限制

- automation 只能做离线实现、离线测试和文档审计。
- automation 遇到真实 GUI、真实登录、验证码、商业应用或凭据时，必须记录 Blocked / DRIFT 并停止。
- automation 不得把“人类可以帮忙点击”解释为“automation 可以自动点击”。
- 真实应用验收结果只能作为人工证据追加，不能由 automation 代签。

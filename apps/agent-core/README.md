# assistant-agent-core

TASK-029 的 binary-layer Host 装配库。

## 职责

- 打开唯一写连接并合并 storage / audit 迁移。
- 显式注入 session、memory、model、policy、tool-bus、audit 与 platform 组件。
- 保持 `crates/core` 只作为可装配组件库，不承担装配。
- 暴露 Host 装配正向测试与缺组件负向测试。
- 提供 `RuntimeExecutor`，按 ADR-0056 单步驱动 Policy → Tool → Verify → Commit，
  并附带真实 `ToolBusInvoker` 与 `EnvelopeObservationCollector` 边界适配器。
- 提供 ADR-0058 的 `--production` 装配根：加载 Notepad Adapter 声明、注册 5 个
  Host handler、注入确定性 TaskPackage Provider，并把 RuntimeExecutor 快照接到
  `SnapshotEventSource`。`--serve-ui` 会在任务结束后进入 UI 监听循环。
- 提供 ADR-0084 / ADR-0085 的 Paint T3.1 装配分支：按显式 `AdapterKind::Paint`
  加载 Paint target 目录，注册 7 个 Paint handler，并用 `element_bounds` 读取 canvas
  物理矩形；Notepad 路径保持默认行为不变。
- 提供 UI 命令/事件适配层（`ui_ipc` + `ui_control`）：版本化命令信封、未知字段/
  未知 kind 的 fail-closed 解析、以及把 approve/deny/pause/cancel/takeover 应用到
  真实 task-engine 的处理器。

## 边界

- 不实现编排算法、策略判定、工具执行或平台 FFI。
- 不提供第二装配点，不在 `core` 内创建基础设施实现。
- 不默认补 Provider、SessionStore、MemoryRetriever 或 AppMapReader。
- 不把 UI 业务逻辑放进本 crate。

## 不变量

1. 每个必需组件都必须由调用方显式提供；缺失时返回 `host_component_missing`。
2. 数据库句柄只由本装配层持有，并通过互斥锁共享给短生命周期 adapter。
3. 装配失败必须返回带 `ErrorCode` 的结构化错误，不返回半装配 Host。
4. 工具通道必须在 tokio 运行时内启动，并在 `shutdown` 时显式关闭。
5. 成功提交必须消费 verify 的不透明 receipt；Policy deny、审批缺失、
   verification failure 与未知工具结果都不能当成成功。
6. `RuntimeExecutor::advance` 通过 `ToolInvoker` 异步调用工具；工具返回
   `ok = false` 时按信封里的 `ErrorCode` 失败，不得进入 verify。
7. `EnvelopeObservationCollector` 要求信封 `data` 带 `fingerprint`；缺失即显式
   observation 失败并升级人工，不伪造默认指纹。
8. UI 命令在解析通过前不得触达处理器；拒绝必须带稳定 `ErrorCode`
   （非法输入 → `ToolInvalidArgs`，未知任务 → `TargetNotFound`，未知审批 →
   `UserInteraction`）。
9. UI 事件只从 task snapshot 投影；committed 步骤携带真实 post fingerprint，
   未知状态不映射为成功。

## 已知限制

- 生产 `SessionStore` 仍等待 storage conversation/session 记录 API（PL-092）；
  当前装配接受注入的 store，测试使用 `MemorySessionStore`。
- 1a 的 Plan 来源是确定性 TaskPackage Provider；真实 LLM Provider 仍归 1c 前。
- `notepad.tab.new` 在平台 API 暴露稳定 tab count 之前显式 `CapabilityMissing`，
  不猜测成功；对应靶机/Adapter 缺口由 PL-097 跟踪。
- `notepad.file.save_as` 已接 Adapter selector 和文件 before/after observation，
  但 `notepad-like` 靶机没有跨进程对话框，真机路径仍待后续靶机扩展。
- Host 以 `WindowProvider + UiAutomationProvider` 约束的泛型平台注入具体实现；
  `WindowsPlatform` 当前未实现 `PlatformService` trait，因此不把未实现的 trait
  当作已满足的能力。装配入口在非 Windows 目标上显式失败，避免 self-check 假阳性。

## 相关文档

- `docs/spec/core-orchestration.md`
- `docs/spec/runtime-execution.md`
- `docs/adr/0056-runtime-execution-contract.md`
- `docs/adr/0053-core-orchestration-layer-interface.md`
- `tasks/TASK-029-binary-skeleton-agent-core-desktop-ui.md`
- `tasks/TASK-103-runtime-executor-host-dispatch-verifyoutcome.md`
- `tasks/TASK-104-ui-core-ipc-approval-wiring.md`

# assistant-agent-core

TASK-029 的 binary-layer Host 装配库。

## 职责

- 打开唯一写连接并合并 storage / audit 迁移。
- 显式注入 session、memory、model、policy、tool-bus、audit 与 platform 组件。
- 保持 `crates/core` 只作为可装配组件库，不承担装配。
- 暴露 Host 装配正向测试与缺组件负向测试。

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

## 已知限制

- 生产 `SessionStore` 仍等待 storage conversation/session 记录 API（PL-092）；
  当前装配接受注入的 store，测试使用 `MemorySessionStore`。
- 具体模型 Provider 仍归后续 provider integration 卡；本卡只装配 trait 注入点。
- Host 以 `WindowProvider + UiAutomationProvider` 约束的泛型平台注入具体实现；
  `WindowsPlatform` 当前未实现 `PlatformService` trait，因此不把未实现的 trait
  当作已满足的能力。装配入口在非 Windows 目标上显式失败，避免 self-check 假阳性。

## 相关文档

- `docs/spec/core-orchestration.md`
- `docs/adr/0053-core-orchestration-layer-interface.md`
- `tasks/TASK-029-binary-skeleton-agent-core-desktop-ui.md`

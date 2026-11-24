# ADR-0109：ToolBus handler failure ErrorCode bridge

状态：**Accepted**（2026-11-24，人类批准方案 A）
日期：2026-11-24
Clarifies：ADR-0108
Supersedes：无
Superseded by：无
关联：TASK-020、TASK-394、TASK-395、TASK-397
落点：`crates/tool-bus/src/error.rs`、`crates/tool-bus/README.md`

## 背景

ADR-0108 要求 T2.1 handler 失败保留既有 `ErrorCode` 分类。当前
`assistant_tool_bus::ToolBusError` 只能通过固定内部变体和 `Mcp` 数字码映射有限的
`ErrorCode`；`TargetAmbiguous`、`PlatformPermission`、`VerifyFailed`、`Fatal` 等类别
无法由 handler 无损返回。

把 handler 错误改由独立桥类型再转换成 `ToolBusError` 会形成第二套错误事实源，并让
重试分类、message 和 envelope 语义存在漂移风险。ToolBus 已经依赖
`assistant_protocol::ErrorCode`，且 `ToolHandler` 的唯一错误出口就是 `ToolBusError`。

## 决策

### D1：唯一加法式变体

在既有 `ToolBusError` 中增加唯一新变体：

```rust
#[error("{message}")]
HandlerFailure {
    code: ErrorCode,
    message: String,
}
```

- `code` 必须是既有 typed `ErrorCode`，禁止裸 `u16`、字符串或 `Mcp` 数字码；
- `message` 应非空，并原样进入 `ToolEnvelope.error.message`；若为空或纯空白，server
  在信封边界改用该 `ErrorCode` 的规范 `message_for_model`，不得生成空错误消息；
- `Display` 只返回 handler 提供的 message，不把 code 拼成不可解析文本；
- `error_code()` 原样返回 `code`；
- server 的既有错误信封路径继续调用 `error_code()` 和 `Display`，不新增第二套桥。

### D2：边界与不变量

- `HandlerFailure` 只用于进程内受信任 handler 的显式失败；
- 参数 schema 校验仍在 handler 之前；不得把模型输入错误绕过为 handler failure；
- 不新增 ErrorCode、不修改 ErrorCode 枚举、不修改重试矩阵；
- 不修改 `ToolHandler`、`ToolOutput`、ToolSchema、IPC、DB schema 或 protocol schema；
- 内部注册/schema/transport 等既有错误变体的映射保持不变。

### D3：验证

- 单元合同测试遍历 `ErrorCode::ALL`，验证 13 类 code 全部经 `HandlerFailure` 往返且
  message 保真；
- ToolBus round-trip 测试让 handler 返回 `HandlerFailure`，验证失败信封的
  `ok=false`、`error.code` 和目标 message；
- 13 类 `ErrorCode` 必须逐一穿过 server error envelope；空/纯空白 message 必须有
  canonical fallback 测试；
- 不通过放宽测试断言、增加 `Mcp` 特例或新增 ErrorCode 来通过。

## 后果

- TASK-395 的 `DRIFT-395-1` 获得公共错误桥，可继续实现 handler 包装与 ToolBus 注册；
- 仍不代表 Excel COM runner、真实 `.xlsx`、watchdog 或人类在场验收完成；
- 新增公共 API 变体必须保持加法式；后续若需新的语义，必须另立 ADR。

## 拒绝条件

- 新增独立错误桥、第二套 handler 错误枚举或字符串错误码；
- 把 `TargetAmbiguous`、`PlatformPermission`、`VerifyFailed`、`Fatal` 冒充为
  `Transient` 或 `Fatal`；
- 修改 `ToolHandler` 签名、ErrorCode 集合、ToolSchema 或现有 handler 信封形状；
- 用 `HandlerFailure` 绕过参数校验、Policy、HITL 或 cleanup postcondition。

## 验证方式

1. `cargo test -p assistant-tool-bus` 覆盖 13 类 ErrorCode 与 server 失败信封；
2. `cargo fmt --all --check`、tool-bus clippy、workspace tests；
3. schema/codegen、文档门禁、ADR 索引、LEDGER 与 diff-check 全绿；
4. 检查 `adapters/com.microsoft.excel/tools/tools.json` 仍为空，未启动 Excel/GUI。

## 何时重新讨论

需要错误码子集、错误证据引用、跨进程外部 MCP server 的 handler 错误，或需要改变
`ToolHandler` 返回类型时，必须新 ADR。

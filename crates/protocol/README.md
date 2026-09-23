# assistant-protocol crate (TASK-011)

> 阶段 1A1 基础设施：协议单一事实源 + Rust/TS 类型生成。

## 职责

定义跨 crate / 跨语言边界的协议类型。所有类型由 `protocol/**/*.json` 生成（`xtask codegen`）。

## 边界

- 不实现业务逻辑：仅类型 + 校验入口
- 不调用平台 API（arch test 拦截）
- 不产生运行时代理（schema 直接走 serde）

## 不变量

1. **生成物不得手工编辑**（第一条不变量）—— `crates/protocol/src/*.rs` 当前是手写初版，
   后续会被 `xtask codegen` 自动生成并覆盖。手写修改都会被 `codegen --check` 检测为 drift
2. 所有公开类型 `#[non_exhaustive]`（便于向后兼容扩展）
3. 所有公开类型 `Serialize + Deserialize<'de>`
4. `ErrorCategory` 枚举 = 13 类（与 v2 §8.7 对齐）；新增 = ADR
5. 生成文件首行 = `// GENERATED — DO NOT EDIT`（`codegen` 会校验）

## 模块

- `error_code` —— 13 类 ErrorCategory + ErrorDefinition
- `envelope` —— ToolEnvelope（v2 §5.3）+ EnvelopeError
- `audit_event` —— AuditEvent + 12 类事件类型 + 4 类 actor
- `capability` —— Capability + 3 类 stability

## 依赖

- `serde`（核心）—— 数据结构序列化
- `serde_json`（Value）—— EnvelopeData / target / metadata 等 open 类型字段

## 验收

- `cargo test -p assistant-protocol`（4 个测试，详 `src/lib.rs::tests`）
- `cargo run -p xtask -- verify-schemas`（5 份 schema 验证 PASSED）
- `cargo run -p xtask -- codegen --check`（drift 检测 PASSED）

## 下一步

- TASK-012（存储层）= 依赖本 crate
- TASK-016（platform/api trait）= 依赖本 crate

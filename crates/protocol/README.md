# assistant-protocol crate (TASK-011)

> 阶段 1A1 基础设施：协议**单一事实源**（`protocol/**/*.json`）+ 由 `xtask codegen` 生成的 Rust 类型。

## 职责

定义跨 crate / 跨语言边界的协议类型：统一返回信封（v2 §5.3）、错误分类学（v2 §8.7）、
工具元 schema、能力标识目录、审计事件（v2 附录 D）。所有类型由 schema **生成**，
Rust 侧不写第二份事实源。

## 边界（不做什么）

- 不实现业务逻辑：只有类型 + 极少量构造 / 取值辅助
- 不调用平台 API（`arch` 护栏会拦）
- 不做 JSON Schema 的运行时校验：构建期归 `xtask verify-schemas`，运行期归后续校验层（TASK-015）
- 除 `serde` / `serde_json` 外不引依赖（登记见 `docs/DEPENDENCIES.md`）

## 不变量

1. **生成物不得手工编辑**（第一条不变量）：`src/generated/*.rs` 由
   `cargo run -p xtask -- codegen` 产出；手改会被 `cargo run -p xtask -- codegen --check`
   判为 drift（退出码 1）
2. 生成物首行恒为 `// GENERATED — DO NOT EDIT.`
3. 每个公开类型都带 `#[non_exhaustive]`（类型别名除外）
4. 每个公开类型都是 `Serialize + Deserialize<'de>`
5. `ErrorCategory` 恰好 13 类，与 v2 §8.7 的 13 行一一对应（斜杠行合并为一行，理由写在
   schema 的 `description` 里）；新增 / 改名 = 契约变更 = 需 ADR
6. `ErrorCode` 是 `ErrorCategory` 的类型别名（不另设一套取值）
7. 审计事件的 `prev_hash` / `self_hash` 是 **SHA-256 小写 hex（64 字符）**；首事件 `prev_hash` 为空串

## 生成链路（改 schema 时的标准动作）

```powershell
cargo run -p xtask -- verify-schemas      # 先过守门人
cargo run -p xtask -- codegen             # 写盘
cargo fmt --all --check                   # 生成物必须天生 rustfmt 稳定（渲染器不跑 rustfmt）
cargo run -p xtask -- codegen --check     # 闭环：必须 0 drift
```

## 已知限制（详见 `tasks/TASK-011-protocol-schema-codegen.md` §7）

- 开放字段（`data` / `details` / `target` / `args`）是 `serde_json::Value`，**无大小上限** → 交 TASK-012
- `protocol/tool-schema` 与 `protocol/audit-event` 里的 `$ref` 目前没有任何校验器解析（TASK-015）
- CI 里 `verify-schemas` / `codegen --check` 仍是 `continue-on-error: true`（转硬需改
  `.github/workflows/ci.yml`，不在 TASK-011 的 write scope 内）

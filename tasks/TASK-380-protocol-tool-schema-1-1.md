# TASK-380 protocol ToolSchema 1.1 与 Adapter tool envelope 生成类型

- 状态：Review
- 阶段：2　子阶段：2b Adapter contract implementation　批次：治理池　依赖：ADR-0104、TASK-376　预估：S　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031「一卡一文件」）。

---

## write scope

- `tasks/TASK-380-protocol-tool-schema-1-1.md`
- `protocol/tool-schema/tool-schema-1.1.json`
- `xtask/src/render.rs`
- `xtask/src/verify_schemas.rs`
- `crates/protocol/src/generated/tool_schema.rs`
- `crates/protocol/README.md`
- `crates/protocol/src/lib.rs`
- `docs/spec/tool-schema.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

落实 ADR-0104 的第 2 步中与 protocol 类型直接相关的部分：新增 ToolSchema 1.1 和
`adapter_tool_envelope.v1` meta-schema，生成 `none_readonly` 与 envelope Rust 类型；
保留 ToolSchema 1.0 兼容窗口。通用只读 Adapter bundle validator 留给后续独立卡。

## In scope

- 新增 `tool-schema-1.1.json`，保留现有 `tool-schema-1.0.json`。
- 1.1 在 `reversibility` 中加入 `none_readonly`，并声明只允许 `effect=read`。
- 在同一 schema 的 `definitions` 中声明 `AdapterToolEnvelopeV1`：ToolSchema、非空 capability、非空 postcondition。
- 扩展 `xtask verify-schemas`：同时校验 1.0 与 1.1，并锁住 1.1 新增字段/envelope。
- 让 `xtask codegen` 从 1.1 生成 `ToolReversibility::NoneReadonly` 与 `AdapterToolEnvelopeV1`。
- 更新 spec/计划/状态与 LEDGER；补充正向和负向门禁证据。

## Out of scope

- 不实现通用 Adapter bundle validator、投影生成器或四套现有 pack 迁移。
- 不修改 `adapters/**`、`crates/tool-bus`、运行时 registry、loader、Policy、产品代码或依赖。
- 不改 ToolSchema 1.0；不改 ADR-0104 或其他 Accepted 契约。
- 不新增 crate、模块、公共 trait、Tool、ErrorCode、IPC schema 或第三方依赖。

## 必须遵守

1. protocol JSON 是唯一事实源；生成物禁止手写业务逻辑。
2. 1.0 继续可读；1.1 是加法演进，不得让旧字段静默改义。
3. unknown version / unknown field / write + `none_readonly` / 空 envelope 必须 fail-closed。
4. 生成器必须稳定、确定，`codegen --check` 后零 drift。
5. 不把“meta-schema 已生成”冒充“运行时已强制”或“通用 validator 已完成”。

## 验收命令

```powershell
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen
cargo fmt --all --check
cargo run -p xtask -- codegen --check
cargo test -p xtask
cargo clippy -p xtask --all-targets -- -D warnings
cargo test -p assistant-protocol
cargo clippy -p assistant-protocol --all-targets -- -D warnings
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- check-ledger
git diff --check
```

预期：6 份协议 schema 通过；1.0 保留；1.1 生成类型包含 `NoneReadonly` 与 envelope；
负向测试证明错误 schema 会报错；全部门禁、PR checks、merge 与 post-merge main CI 回填。

## DoD

- [ ] `tool-schema-1.1.json` 与 1.0 同时存在且均通过结构校验。
- [ ] `none_readonly` 只允许 read；1.1 meta-schema 明确该约束。
- [ ] `AdapterToolEnvelopeV1` 生成类型包含 ToolSchema、非空 capabilities/postconditions 语义说明。
- [ ] `codegen --check` 零 drift，1.0 生成兼容未被改坏。
- [ ] 负向测试覆盖缺 envelope、write+none_readonly、未知字段/版本。
- [ ] 文档明确通用 validator 与运行时强制仍未实现。
- [ ] 全部门禁、PR checks、merge 与 post-merge main CI 回填。

<!-- ══ 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发 ⑤；
     状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-380 protocol ToolSchema 1.1 与 Adapter tool envelope 生成类型
【目标】落实 ADR-0104 的 ToolSchema 1.1 / AdapterToolEnvelopeV1 生成类型，保留 1.0
【write scope】仅：本卡、tool-schema 1.1、xtask render/verify、protocol 生成物/README/lib、tool-schema spec、计划/状态/LEDGER
【铁律】schema 单一事实源；未知字段/版本 fail-closed；生成确定性；不把 meta-schema 冒充运行时强制
【禁止】不实现通用 bundle validator；不改 adapters/runtime/loader/Policy；不新增依赖/crate/公共 trait
【验收】6 schemas；1.0+1.1；NoneReadonly+envelope；codegen 0 drift；xtask/protocol tests/clippy；文档门禁
【依赖】ADR-0104 / TASK-376 Done；TASK-379 Done
【疑问】无
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-380-protocol-tool-schema-1-1.md`
- `protocol/tool-schema/tool-schema-1.1.json`
- `xtask/src/render.rs`
- `xtask/src/verify_schemas.rs`
- `crates/protocol/src/generated/tool_schema.rs`
- `crates/protocol/README.md`
- `crates/protocol/src/lib.rs`
- `docs/spec/tool-schema.md`
- 计划/PLAN/README/LEDGER 的允许区域

### 3. 验收输出摘要（命令 → 结果）

- `cargo run -p xtask -- verify-schemas` → 6 schemas / 0 error / PASSED。
- `cargo run -p xtask -- codegen` → ToolSchema 生成物更新；随后 `codegen --check` → 0 drift。
- `cargo fmt --all --check` → PASSED。
- `cargo test -p xtask` → 520 passed / 0 failed。
- `cargo clippy -p xtask --all-targets -- -D warnings` → exit 0，仅既有 unknown lint 提示。
- `cargo test -p assistant-protocol` → 9 passed / 0 failed。
- `cargo clippy -p assistant-protocol --all-targets -- -D warnings` → exit 0，仅既有 unknown lint 提示。
- 负向测试覆盖：缺 `none_readonly` read-only 规则、缺 `AdapterToolEnvelopeV1`、envelope 未知字段。
- PR / CI / merge 与 post-merge main CI 待提交后回填。

### 4. DoD 逐条核对

- [x] `tool-schema-1.1.json` 与 1.0 同时存在且均通过结构校验。
- [x] `none_readonly` 只允许 read；1.1 meta-schema 明确该约束。
- [x] `AdapterToolEnvelopeV1` 生成类型包含 ToolSchema、非空 capabilities/postconditions 语义说明。
- [x] `codegen --check` 零 drift，1.0 文件保留且未改坏。
- [x] 负向测试覆盖缺 read-only 规则、缺 envelope、unknown field。
- [x] 文档明确通用 Adapter validator 与运行时强制仍未实现。
- [ ] 全部门禁、PR checks、merge 与 post-merge main CI 回填。

### 5. 偏差

- 无。

### 6. 实施中发现的更合理做法（非漂移，已直接落地 + 理由）

- 将 ToolSchema 与 envelope 放在同一个 1.1 schema 的 `definitions` 下，保持一个 canonical 文件入口，同时生成单个 `tool_schema.rs`，避免新增生成模块清单。
- envelope `postconditions` 在 protocol 边界保留为 JSON 值；可执行 assertion 解析继续归 verify/validator owner，避免 protocol crate 反向依赖 verify。

### 7. 遗留问题（进 `docs/PARKING_LOT.md` 的编号）

- 通用只读 Adapter bundle validator、projection 重新生成、现有四套 pack 收敛仍需后续卡。

### 8. 新增长期记忆（FACT / PITFALL / REJECTED 条目原文，若无写“无”）

- 无；实现事实由 schema、生成物、测试和 spec 保存。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

- 1.1 是加法演进，1.0 文件必须继续保留；当前生成类型指向 1.1。
- `none_readonly` 的跨字段约束目前由 meta-schema 的 `allOf` 表达，不是运行时自动校验器。
- envelope postconditions 是跨边界 JSON，完整可执行性仍由后续 validator 负责。

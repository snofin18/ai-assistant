# TASK-281　tool-bus ToolDefinition 声明契约覆盖率补齐（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/tool-bus/tests/definition_contract.rs`、`tasks/TASK-281-tool-bus-definition-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-13-round-7.md`、`docs/automations/2026-10-13-report.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`docs/spec/tool-schema.md` §4、`crates/tool-bus/README.md` 不变量 7

**目标**

仅为 `assistant-tool-bus` 增加 `ToolDefinition` 注册期声明契约的公开测试，把
`crates/tool-bus/src/registry.rs` 的行覆盖率从 79.52% 提升到 ≥90%（crate 独立行覆盖率从 87.93%
提升到 ≥89%）。重点覆盖名字规则（三段式 + 两个元工具例外）、根 schema 非对象 / 不可强制关键字的
fail-closed 拒绝，以及 `with_output_schema` / `with_idempotent` / `with_requires_approval` /
`with_tags` / `with_annotations` 的成功与失败分支。不修改生产逻辑，不新增依赖。

**背景**

2026-10-13 round 7 的 `registry.rs` 行覆盖率为 79.52%。既有 `toolset_governance.rs` 覆盖了显式注解
冲突与 `requires_approval` 收紧失败，但名字非法、根 schema 非对象、不可强制关键字、
`with_output_schema` / `with_idempotent` / `with_tags` 与 `critical` / `l3` 的默认确认这些分支
没有回归证据。候选来自 `docs/automations/2026-10-12-report.md` §③（纯逻辑、无 GUI / keychain）。

**步骤**

1. 新建 `crates/tool-bus/tests/definition_contract.rs`。
2. 覆盖名字规则：非法名字 → `InvalidToolName`；`toolset.list` / `toolset.search` 例外放行。
3. 覆盖根 schema 非对象与不可强制关键字的 `UnsupportedSchemaKeyword`。
4. 覆盖 `with_output_schema`（对象 / `null` 放行、标量拒绝）、`with_idempotent`、`with_tags`、
   默认确认与 `with_requires_approval(true)`、注解一致时的放行。
5. 复测 `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 89` 并跑全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅新增测试文件）
- [ ] 新增测试只调用公开 API，覆盖声明契约
- [ ] `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 89` 通过，且 `registry.rs` 行覆盖率 ≥90%
- [ ] `cargo test -p assistant-tool-bus` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 89
cargo test -p assistant-tool-bus
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- memory-counts; cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan; cargo run -p xtask -- docscan; cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger; cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas; cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-281 tool-bus ToolDefinition 声明契约覆盖率补齐
【目标】仅新增公开契约测试，把 `registry.rs` 行覆盖率从 79.52% 提升到 ≥90%
【write scope】仅：新建 `crates/tool-bus/tests/definition_contract.rs` 与本卡列出的状态同步文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、CI/lint、生产行为；启动真实 GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 模块覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3；运行日 2026-10-13 round 7（**机制 = human**）
【疑问】无

### 2. 实际改动文件

- `crates/tool-bus/tests/definition_contract.rs`（新增 5 个公开契约测试）
- `tasks/TASK-281-tool-bus-definition-contract-coverage.md`
- `LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/automations/2026-10-13-round-7.md`、`docs/automations/2026-10-13-report.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-tool-bus` 行覆盖率 87.93%，`registry.rs` 79.52%（region 84.67%）。
- `cargo test -p assistant-tool-bus --test definition_contract`：5 passed。
- `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 89`：EXIT 0；
  crate 行覆盖率 **89.91%**；`registry.rs` 行覆盖率 **94.38%**（region 96.00%）。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0。
- xtask：`hygiene` scanned=409 / 0E / 128W / PASSED，`docscan` 0E/264W，
  `memory-counts`、`adr-index`、`check-ledger`、`card-check`、`refscan`(0E/0W)、
  `check-comments`、`verify-schemas`、`check-migrations` 全部 PASSED。
- PR #360；pull_request CI = **11/11 SUCCESS**；合并前 base=`main`、mergeable=`MERGEABLE`、
  mergeStateStatus=`CLEAN`；merge `1c19646`。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅新增 `crates/tool-bus/tests/definition_contract.rs`。
- 新增测试只调用公开 API 覆盖声明契约：满足；5 个测试覆盖名字规则、根 schema 拒绝、
  output / idempotent / tags / approval builder 与注解一致放行。
- `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 89` 且模块 ≥90%：
  满足，crate 89.91%、模块 94.38%。
- `cargo test -p assistant-tool-bus` 全绿：满足。
- `cargo fmt --all --check` / `clippy --all-targets -- -D warnings` / `test --workspace` 全绿：
  满足（首次 clippy 报 `redundant clone`，移除后通过）。
- 全部现行 xtask 硬门禁 PASSED：满足。
- 热点文件 guard：满足；`LEDGER.md` / `docs/memory/facts.md` / `MEMORY.md`（收口）与
  `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`（回填）先 acquire 后 release。

### 5. 偏差

- 首次 commit 又误落在上一张卡的合并回填分支（忘了先开分支）；已在 `origin/main` 上重建
  `codex/task-281-tool-bus-definition-contract` 并 cherry-pick，旧分支本地指针复位；未 push 错误分支。
- 首次 `clippy -D warnings` 报 `redundant clone`（`critical.clone()` 后未再用），移除后通过。

### 6. 更合理做法

- 声明契约单独放 `definition_contract.rs`，与 `toolset_governance.rs` 的挂载 / 指纹用例分开，
  便于独立回归注册期 fail-closed 判据。

### 7. 遗留问题

- `registry.rs` 剩余未覆盖行是不可达的协议未知枚举分支（`_ => "unrecognized-*"` 与 `to_mcp_tool` 的
  对象校验），公开 API 不可达。
- `handler.rs`（75.38%）的未覆盖率多为平凡访问器；`bus.rs`（75.82%）需真实传输，均不再硬凑。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `registry.rs` 行覆盖率 79.52% → 94.38%、`assistant-tool-bus` crate
  87.93% → 89.91%，并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API，未改变生产控制流。
2. 名字规则、根 schema 拒绝、各 builder 分支是否都断言了精确错误变体。
3. 覆盖率数字是否由同一提交上的独立 `cargo llvm-cov -p assistant-tool-bus` 实跑得到。

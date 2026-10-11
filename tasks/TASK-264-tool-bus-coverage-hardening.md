# TASK-264　assistant-tool-bus 契约测试覆盖率加固（章程 §2 P2）

- 状态：**Done**（2026-10-11）
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/tool-bus/tests/coverage_contract.rs`、`tasks/TASK-264-tool-bus-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-11-round-2.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/tool-bus/README.md`

**目标**

仅为 `assistant-tool-bus` 增加白盒契约测试，把该 crate 的独立行覆盖率提升到 ≥85%，
重点覆盖实例参数校验的全部已支持关键字、信封组装 / 截断失败关闭路径与稳定错误分类；
不修改生产逻辑，不读取真实桌面，不启动任何 GUI。

**背景**

2026-10-11 round 2 的独立实测基线为 `assistant-tool-bus` 行覆盖率 73.12%
（1864 行中覆盖 1363 行）；`schema/instance.rs` 仅 47.09%，`error.rs`、`mount.rs`、
`envelope.rs` 仍有多条公开契约分支未覆盖。当前 Ready 队列均为真实 GUI 或人工安全设计工作，
按 §2 P2 做可机器验证的测试强化。

**步骤**

1. 新建 `crates/tool-bus/tests/coverage_contract.rs`。
2. 通过公开 `validate_arguments` 覆盖 draft-07 子集关键字、JSON pointer、Unicode 长度与深度上限。
3. 通过公开 `assemble_envelope` / `error_envelope` 覆盖来源、证据、截断与失败关闭契约。
4. 覆盖 `ToolBusError::error_code` 全部变体与稳定 Display，以及配置 / 挂载选择的公开构造面。
5. 复测 `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 85`。
6. 跑 `cargo test -p assistant-tool-bus` 与全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅修改测试文件）
- [ ] 新增测试覆盖实例校验、信封组装 / 截断、错误码映射与挂载选择边界
- [ ] `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 85` 通过
- [ ] `cargo test -p assistant-tool-bus` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 85
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

【任务】TASK-264 assistant-tool-bus 契约测试覆盖率加固
【目标】仅新增测试，把 `crates/tool-bus` 独立行覆盖率提升到 ≥85%
【write scope】仅：本卡正文所列文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改生产行为、契约、依赖、CI/lint；读取真实桌面 / GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `crates/tool-bus/tests/coverage_contract.rs`
- `tasks/TASK-264-tool-bus-coverage-hardening.md`
- `LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/memory/facts.md`、`MEMORY.md`（规模表同步）
- `docs/automations/2026-10-11-round-2.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-tool-bus` 行覆盖率 73.12%（1864 行中覆盖 1363 行）。
- `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 85`：EXIT 0；
  1864 行中覆盖 1596 行，行覆盖率 **85.62%**；`schema/instance.rs` 93.88%、
  `envelope.rs` 88.17%、`mount.rs` 92.97%、`error.rs` 100%。
- `cargo test -p assistant-tool-bus`：9 个新增 coverage 契约测试、9 个 schema 关键字测试、
  5 个 tool-bus 测试、7 个工具集治理测试与 1 个 doctest 全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0，workspace 469 passed / 0 failed。
- `xtask` 现行硬门禁全部 PASSED；`cargo deny check` 为 advisories / bans / licenses / sources 全 ok；
  `cargo test -p assistant-core arch::` 10 passed；`replay --suite core` 2 steps / 0 mismatch。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；只新增 `crates/tool-bus/tests/coverage_contract.rs`；PR #306 / merge `f9058dc`。
- 实例校验、信封组装 / 截断、错误码映射与挂载选择边界：满足；新增 9 个公共 API 契约测试。
- crate 独立行覆盖率 ≥85%：满足，实测 85.62%。
- `cargo test -p assistant-tool-bus` 与全 workspace 门禁：满足。
- 全部现行 xtask 硬门禁：满足，见本卡验收输出与轮次报告。
- 热点文件 guard：满足，`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` /
  `docs/memory/facts.md` / `MEMORY.md` 均先 acquire 后编辑并 release。

### 5. 偏差

- 初次实现为 758 行，超过章程 §6 的单卡 400 行审阅预算；已在本轮内压缩并重排为 380 行，
  保持同一覆盖面与 85.62% 行覆盖率，未留下预算偏差。
- `hygiene` 维持基线 0 Error / 123 Warning / PASSED，无新增软阈值告警。

### 6. 更合理做法

- 对 draft-07 子集使用表驱动用例聚合，复用 `property_schema` 与 `violations` helper；
  对错误码映射用两段式表避免单个测试函数超过 80 行。

### 7. 遗留问题

- 真实 NamedPipe / GUI 验收仍由有人值守靶机流程承接；本卡只覆盖无 IO 的公共契约。
- 若后续继续扩展 tool-bus 契约测试，应优先拆出 `coverage_contract_envelope.rs` 或等价文件，
  避免继续推高单文件行数。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-tool-bus` 独立行覆盖率从 73.12% 提升到 85.62%，
  并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否全部只调用公开 API，生产源码零改动。
2. schema 与信封失败路径是否断言了具体错误/JSON pointer/错误码，而不是只断言“没有 panic”。
3. 覆盖率数字是否由同一提交上的独立 `cargo llvm-cov -p assistant-tool-bus` 实跑得到。

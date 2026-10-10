# TASK-263　assistant-replay 契约测试覆盖率加固（章程 §2 P2）

- 状态：**Done**（2026-10-11）
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/replay/tests/replay.rs`、`tasks/TASK-263-replay-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-10-round-6.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/replay/README.md`

**目标**

仅为 `assistant-replay` 增加白盒契约测试，把该 crate 的独立行覆盖率提升到 ≥75%，
重点覆盖 recording 反序列化错误、失败关闭的 provider 分支与未录制写动作；
不修改生产逻辑，不读取真实桌面，不启动任何 GUI。

**背景**

自动化运行日 2026-10-10 round 6 的独立实测基线为 `assistant-replay`
行覆盖率 63.61%（841 行中覆盖 535 行）；`error.rs` 完全未覆盖，
`provider.rs` 的未录制写动作、窗口过滤与失败分支仍有缺口。
当前 Ready 队列均为真实 GUI 或人工安全设计工作，按 §2 P2
做可机器验证的测试强化。

**步骤**

1. 在既有 `crates/replay/tests/replay.rs` 中补齐 `ReplayError` 的稳定 Display 契约。
2. 覆盖 recording 字段类型、空树、非法 bounds / fingerprint / optional 字段等失败路径。
3. 覆盖 provider 的未录制写动作、窗口过滤、错误窗口句柄与 bounds 边界。
4. 复测 `cargo llvm-cov -p assistant-replay --tests --fail-under-lines 75`。
5. 跑 `cargo test -p assistant-replay` 与全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅修改测试文件）
- [ ] 新增测试覆盖 recording 校验失败路径与 provider 失败关闭分支
- [ ] `cargo llvm-cov -p assistant-replay --tests --fail-under-lines 75` 通过
- [ ] `cargo test -p assistant-replay` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-replay --tests --fail-under-lines 75
cargo test -p assistant-replay
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

【任务】TASK-263 assistant-replay 契约测试覆盖率加固
【目标】仅新增测试，把 `crates/replay` 独立行覆盖率提升到 ≥75%
【write scope】仅：本卡正文所列文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改生产行为、契约、依赖、CI/lint；读取真实桌面 / GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `crates/replay/tests/replay.rs`
- `tasks/TASK-263-replay-coverage-hardening.md`
- `LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/memory/facts.md`、`MEMORY.md`（规模表同步）
- `docs/automations/2026-10-10-round-6.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-replay` 行覆盖率 63.61%（841 行中覆盖 535 行）。
- `cargo llvm-cov -p assistant-replay --tests --fail-under-lines 75`：EXIT 0；
  841 行中覆盖 755 行，行覆盖率 **89.77%**；`provider.rs` 89.61%、
  `model.rs` 89.47%、`error.rs` 100%。
- `cargo test -p assistant-replay`：27 个 replay 契约测试全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0，workspace 469 passed / 0 failed。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；只改 `crates/replay/tests/replay.rs`。
- recording 校验失败路径与 provider 失败关闭分支：满足；覆盖全部 `ReplayError`
  Display、JSON / 顶层形状 / window / tree / read_text 字段错误、未录制写动作、
  错误窗口句柄、无效 bounds、窗口过滤、非整窗指纹与父候选歧义。
- crate 独立行覆盖率 ≥75%：满足，实测 89.77%。
- `cargo test -p assistant-replay` 与全 workspace 门禁：满足。
- 全部现行 xtask 硬门禁：见本卡验收输出与轮次报告。
- 热点文件 guard：满足，六个热点文件先 acquire 后编辑并 release。

### 5. 偏差

- `crates/replay/tests/replay.rs` 由 390 行增至 781 行，新增 391 行，
  仍在单卡 400 行审阅预算内；该测试文件跨过 600 行软阈值，预计新增一条
  `hygiene/file-too-long` Warning，0 Error、verdict 仍为 PASSED。
- 触发时刻 06:00 超过章程 §6 的 05:00 收尾建议；本轮按用户创建的 06:00
  automation prompt 明确执行，并在轮次报告中记录。

### 6. 更合理做法

- 复用既有 `fixture_json` / `mutated_fixture` / `poll_once` 测试夹具，避免新造
  平行 fixture；失败用例用小型 mutation table 聚合，保持覆盖收益同时控制 diff。

### 7. 遗留问题

- 无产品遗留；真实 GUI 相关卡仍由有人值守流程承接。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-replay` 独立行覆盖率从 63.61%
  提升到 89.77%，并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否全部只调用既有公共 API，生产源码零改动。
2. 失败路径断言是否检查稳定 `ErrorCode` / `ReplayError` 变体，而不是只检查
   “没有 panic”。
3. 覆盖率数字是否由同一提交上的独立 `cargo llvm-cov -p assistant-replay` 实跑得到。

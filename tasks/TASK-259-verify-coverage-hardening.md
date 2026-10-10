# TASK-259　verify 契约测试覆盖率加固（章程 §5.3）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/verify/tests/**`、`tasks/TASK-259-verify-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-10-round-3.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/verify/README.md`

**目标**

仅为 `crates/verify` 增加白盒契约测试，把该 crate 的行覆盖率提升到 ≥85%，覆盖
postcondition fail-closed 解析、三值求值的不可评估分支、文件与视觉断言边界、
视觉错误码与图像/哈希输入边界；不修改生产逻辑。

**背景**

自动化运行日 2026-10-10 round 3 的实测基线为 `assistant-verify` 行覆盖率
80.43%（1645 行中覆盖 1323 行），低于章程 §5.3 对 `verify` crate 的 85% 门槛。
当前 Ready 队列均需真实 GUI 或人工安全设计判断，因此按 §2 P2 执行可机器验证的测试强化。

**步骤**

1. 新增 `crates/verify/tests/coverage_contract.rs`，只调用现有公共 API。
2. 覆盖所有不支持/畸形 postcondition 的稳定拒绝原因，以及 `visual_assert` 的字段与操作符组合校验。
3. 覆盖 `state_changed` / `state_unchanged` / element / value / file / app_reported 的
   `NotEvaluable`、`Satisfied`、`Falsified` 三类路径。
4. 覆盖 `GrayImage`、`VisualObservation`、pHash/dHash、Hamming distance 和视觉 verdict 的边界。
5. 复测 `cargo llvm-cov -p assistant-verify --tests`，确认 crate 行覆盖率 ≥85%，再跑全局门禁。

**DoD**

- [ ] 生产代码 `crates/verify/src/**` 零改动
- [ ] 新增测试覆盖上述五类契约，正常 / 边界 / 错误路径均存在
- [ ] `cargo llvm-cov -p assistant-verify --tests --fail-under-lines 85` 通过
- [ ] `cargo test -p assistant-verify` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-verify --tests --fail-under-lines 85
cargo test -p assistant-verify
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

【任务】TASK-259 verify 覆盖率加固
【目标】仅新增契约测试，将 `crates/verify` 行覆盖率提升到 ≥85%
【write scope】仅：本卡正文所列文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改 `src/**`、契约、依赖、CI/lint；操作真实 GUI/网络
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `crates/verify/tests/coverage_contract.rs`（新增，仅测试）
- `tasks/TASK-259-verify-coverage-hardening.md`
- `LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/memory/facts.md`、`MEMORY.md`（规模表同步）
- `docs/automations/2026-10-10-round-3.md`

### 3. 验收输出摘要

- 改动前 `cargo llvm-cov -p assistant-verify --tests --summary-only`：总行 1645 / 覆盖 1323 /
  **80.43%**。
- `cargo test -p assistant-verify --test coverage_contract`：10 passed / 0 failed。
- `cargo llvm-cov -p assistant-verify --tests --fail-under-lines 85`：EXIT 0；
  总行 1645 / 覆盖 1456 / **88.51%**。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0；workspace 测试 469 passed。
- `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger /
  check-comments / verify-schemas / check-migrations / codegen --check`：全部 exit 0；
  `hygiene` scanned=394 0E/121W，`card-check` scanned=154 0E/34W，`check-comments` 0E/73W。
- PR / CI / merge hash：待本卡 PR 收口后回填。

### 4. DoD 逐条核对

- 生产源码零改动：满足，`git diff --stat` 仅新增测试与治理/留痕文件。
- 正常 / 边界 / 错误路径覆盖：满足，10 个契约测试覆盖解析拒绝、三值求值、文件转换与视觉输入边界。
- crate 覆盖率 ≥85%：满足，实测 88.51%。
- `cargo test -p assistant-verify` 与全 workspace 门禁：满足。
- 热点文件 guard：满足，六个热点文件先 acquire 后编辑。
- 最终状态：**Review**；PR / CI / merge hash 待回填。

### 5. 偏差

- 本轮多次只读/查询命令误用 `shell=bash` 并立即 exit 127；随后均改用 `shell=powershell` 重跑，
  失败发生在写入前，未污染工作区，也未把未运行命令记作已通过。
- 单卡 diff 预计超过章程 §6 的 400 行审阅预算：测试文件 522 行，另有卡片 / 状态 / 轮次留痕。
  未拆轮的原因是这些测试共同覆盖同一组 fail-closed 覆盖率缺口，拆开会使 `verify` 停留在 85% 以下。

### 6. 更合理做法

- 新增测试按“解析拒绝 / 三值求值 / 文件转换 / 视觉边界”分组，比继续堆入既有测试文件更易审阅，
  也避免触碰生产源码。

### 7. 遗留问题

- TASK-256 / TASK-257 / TASK-044 的真实 Paint GUI 验收仍由有人在场时执行。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-verify` 契约覆盖率从 80.43% 提升到 88.51%；
  同步更新 `MEMORY.md` 规模表并让 `memory-counts` 恢复 0E/0W。

### 9. 给审阅者的关注点

1. 新增文件是否仅调用现有公共 API，且没有改生产源码或放宽断言。
2. `visual_assert` 的解析错误与低置信 / 不可比较输入是否都保持 fail-closed。
3. 覆盖率数字是否由 `cargo llvm-cov -p assistant-verify --tests` 在同一提交上实跑得到。

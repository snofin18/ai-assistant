# TASK-272　assistant-platform-api 契约测试覆盖率加固（章程 §2 P2）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：M　**难度**：M
- **write scope**：`crates/platform/api/tests/coverage_contract.rs`、`tasks/TASK-272-platform-api-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-11-round-10.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/platform/api/README.md`

**目标**

仅为 `assistant-platform-api` 增加公开 API 契约测试，把该 crate 的独立行覆盖率从
62.96% 提升到 ≥85%；覆盖会话、窗口、元素、目标描述、能力矩阵、几何与句柄公开面。
不修改生产逻辑，不新增依赖，不读取真实网络、凭据或 GUI。

**背景**

2026-10-12 round 10 的独立 crate 基线为行覆盖率 62.96%（899 行中覆盖 566 行）。
缺口集中在尚未被 crate 级测试直接调用的公开类型、访问器和纯函数分支；同批
workspace 聚合口径为 83.54%，但不能替代 crate 独立门禁。
当前 Ready 队列仍为真实 GUI 或人工安全设计工作，按 §2 P2 做可机器验证的测试强化。

**步骤**

1. 新建 `crates/platform/api/tests/coverage_contract.rs`，只调用 crate 的公开 API。
2. 覆盖会话、窗口、元素、选择器、目标描述与句柄的构造器、访问器和负向分支。
3. 覆盖能力矩阵的枚举解析、风险升级、审批映射、错误显示与矩阵校验。
4. 覆盖坐标换算、指纹、边界矩形和超时的公开契约。
5. 复测 `cargo llvm-cov -p assistant-platform-api --tests --fail-under-lines 85`。
6. 跑 `cargo test -p assistant-platform-api` 与全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅新增测试文件）
- [ ] 新增测试覆盖公开类型、访问器、纯函数边界和负向路径
- [ ] `cargo llvm-cov -p assistant-platform-api --tests --fail-under-lines 85` 通过
- [ ] `cargo test -p assistant-platform-api` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-platform-api --tests --fail-under-lines 85
cargo test -p assistant-platform-api
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

```text
【任务】TASK-272 assistant-platform-api 契约测试覆盖率加固
【目标】仅新增测试，把 assistant-platform-api 独立行覆盖率从 62.96% 提升到 >=85%
【write scope】仅：crates/platform/api/tests/coverage_contract.rs 与本卡列出的状态同步文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改生产行为、契约、依赖、CI/lint；读取真实桌面 / GUI / 网络 / 凭据
【验收】见「验收命令」-> crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无
```

### 2. 实际改动文件

- `crates/platform/api/tests/coverage_contract.rs`（新增 14 个公共契约测试）
- `tasks/TASK-272-platform-api-coverage-hardening.md`
- `docs/automations/2026-10-11-round-10.md`
- `docs/memory/facts.md`
- `MEMORY.md`
- `LEDGER.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-platform-api` 行覆盖率 62.96%（899 行中覆盖 566 行）。
- `cargo llvm-cov -p assistant-platform-api --tests --fail-under-lines 85`：EXIT 0；
  899 行中覆盖 893 行，行覆盖率 **99.33%**；10 个源文件均 ≥97.69%。
- `cargo test -p assistant-platform-api`：5 个单元测试 + 43 个集成测试 + 1 个 doctest 全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0。
- xtask：`hygiene` scanned=404 / 0E / 128W / PASSED，`memory-counts`、`adr-index`、
  `refscan`、`docscan`、`card-check`、`check-ledger`、`check-comments`、
  `verify-schemas`、`codegen --check`、`check-migrations` 全部 PASSED。
- `cargo deny check`、`cargo test -p assistant-core arch::`、
  `cargo run -p xtask -- replay --suite core` 全绿；workspace 行覆盖率 79.49% / PASSED。
- 非宿主平台 clippy：`x86_64-unknown-linux-gnu` 与 `aarch64-apple-darwin`
  的 `assistant-platform-api --all-targets -- -D warnings` 均 EXIT 0。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅新增测试文件。
- 公开类型、访问器、纯函数边界和负向路径：满足；14 个测试覆盖会话、窗口、元素、
  目标描述、能力矩阵、几何、指纹与句柄公开面。
- crate 独立行覆盖率 ≥85%：满足，实测 99.33%。
- `cargo test -p assistant-platform-api` 与全 workspace 门禁：满足。
- 全部现行 xtask 硬门禁：满足。
- 热点文件 guard：满足；状态同步前 acquire，编辑后 release。
- PR / CI / merge hash：待本轮 PR 收口后回填。

### 5. 偏差

- PowerShell 在本机再次零输出卡住；已按既有逃生路径改用 `cmd.exe`
  执行 git/cargo/gh、WSL 执行文本读取，并记录到本轮报告。
- 两次命令误用 `shell: bash`，均只得到 `cargo: command not found`，未计入验收；
  已用 `cmd.exe` 重跑成功。
- 并行运行多个 xtask / cargo 命令时出现 package cache 等待，但均最终 EXIT 0；
  不影响验收结果。
- 本轮总 diff 预计超过章程 §6 的单卡 400 行审阅预算；新增内容集中在单个测试文件
  与治理记录，属于 §2 P2 覆盖加固回退路径。

### 6. 更合理做法

- 先按 crate 独立 `llvm-cov` 口径选目标，再补公开契约测试；workspace 聚合覆盖率会
  掩盖 crate 自身缺口，不应作为 P2 选卡依据。

### 7. 遗留问题

- PR / CI / merge hash 待本轮 PR 收口后回填。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-platform-api` 独立行覆盖率从 62.96%
  提升到 99.33%，生产源码零改动。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API，且没有借测试改写生产契约。
2. trait 参数类型、能力矩阵、选择器与坐标边界的断言是否完整。
3. 99.33% 覆盖率的实测命令与输出是否可复现。

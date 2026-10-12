# TASK-273　assistant-ipc 契约测试覆盖率加固（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：M　**难度**：M
- **write scope**：`crates/ipc/tests/coverage_contract.rs`、`tasks/TASK-273-ipc-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-12-round-1.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`docs/spec/ipc-protocol.md`、`crates/ipc/README.md`

**目标**

仅为 `assistant-ipc` 增加公开 API 契约测试，把该 crate 的独立行覆盖率从
81.49% 提升到 ≥85%；覆盖帧边界、认证握手、wire message 访问器与进程身份失败路径。
不修改生产逻辑，不新增依赖，不读取真实网络、凭据、桌面或 GUI。

**背景**

2026-10-12 round 1 的独立 crate 基线为行覆盖率 81.49%（1102 行中覆盖 898 行）。
缺口集中在尚未由 crate 级测试直接调用的公开构造器、Debug 脱敏、握手负向路径和
进程身份失败分支；同批 workspace 聚合口径会掩盖 crate 自身缺口。
当前 Ready 队列仍为真实 GUI 或人工安全设计工作，按 §2 P2 做可机器验证的测试强化。

**步骤**

1. 在既有 `crates/ipc/tests/coverage_contract.rs` 中增加公共 API 契约测试。
2. 覆盖帧编码超限、截断前缀、wire message 构造器与 Debug 脱敏。
3. 覆盖握手 token/version/session-id 的负向分支，并保持真实 NamedPipe 测试不变。
4. 覆盖进程身份失败路径与未连接传输的显式错误。
5. 复测 `cargo llvm-cov -p assistant-ipc --tests --fail-under-lines 85`。
6. 跑 crate 测试与全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅修改测试文件）
- [ ] 新增测试覆盖公开类型、纯函数边界和负向路径
- [ ] `cargo llvm-cov -p assistant-ipc --tests --fail-under-lines 85` 通过
- [ ] `cargo test -p assistant-ipc` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-ipc --tests --fail-under-lines 85
cargo test -p assistant-ipc
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
【任务】TASK-273 assistant-ipc 契约测试覆盖率加固
【目标】仅新增测试，把 assistant-ipc 独立行覆盖率从 81.49% 提升到 >=85%
【write scope】仅：crates/ipc/tests/coverage_contract.rs 与本卡列出的状态同步文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改生产行为、契约、依赖、CI/lint；读取真实桌面 / GUI / 网络 / 凭据
【验收】见「验收命令」-> crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无
```

### 2. 实际改动文件

- `crates/ipc/tests/coverage_contract.rs`（新增 10 个公共契约测试）
- `tasks/TASK-273-ipc-coverage-hardening.md`
- `docs/automations/2026-10-12-round-1.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-ipc` 行覆盖率 81.49%（1102 行中覆盖 898 行）。
- `cargo llvm-cov -p assistant-ipc --tests --fail-under-lines 85`：EXIT 0；
  1102 行中覆盖 943 行，行覆盖率 **85.57%**；`handshake.rs` 89.75%、
  `windows_io.rs` 71.84%、`windows_peer.rs` 69.44%、`windows_transport.rs` 89.53%。
- `cargo test -p assistant-ipc`：25 个单元测试 + 19 个 coverage_contract 测试 +
  2 个 handle_discipline 测试 + 1 个 doctest 全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0。
- xtask：`hygiene` scanned=404 / 0E / 128W / PASSED，`memory-counts`、`adr-index`、
  `refscan`、`docscan`、`card-check`、`check-ledger`、`check-comments`、
  `verify-schemas`、`codegen --check`、`check-migrations` 全部 PASSED。
- `cargo deny check`、`cargo test -p assistant-core arch::`、
  `cargo run -p xtask -- replay --suite core` 全绿；workspace 行覆盖率
  79.55% / PASSED。
- 非宿主平台 clippy：`x86_64-unknown-linux-gnu` 与 `aarch64-apple-darwin`
  的 `assistant-ipc --all-targets -- -D warnings` 均 EXIT 0。
- PR #326 的 pull_request CI run `38203102939` = **11/11 SUCCESS**；
  合并前 base=`main`、mergeable=`MERGEABLE`、mergeStateStatus=`CLEAN`；
  merge `b4528fd`。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅修改测试文件。
- 公开类型、纯函数边界和负向路径：满足；新增 10 个测试覆盖帧边界、
  wire message 构造器、Debug 脱敏、握手失败关闭、进程身份失败和未连接传输。
- crate 独立行覆盖率 ≥85%：满足，实测 85.57%。
- `cargo test -p assistant-ipc` 与全 workspace 门禁：满足。
- 全部现行 xtask 硬门禁：满足。
- 热点文件 guard：满足；状态同步前 acquire，编辑后 release。
- PR / CI / merge hash：满足；PR #326 / CI `38203102939` / merge `b4528fd`；
  post-merge main push CI run `38204275524` 已在回填前启动。

### 5. 偏差

- 第一版新增测试把 11 字节全零帧预期为长度错误；真实协议先命中
  `MagicMismatch`。已按协议拆成 0/1/7 字节长度错误与 11 字节 magic 错误，
  未改生产代码或放宽旧断言。
- PowerShell 未卡；但本轮有 13 次命令误用 WSL bash，均为
  `Get-Content` / `Get-ChildItem` / `rg: command not found`，未产生文件、
  锁或验收结果污染；随后全部用 PowerShell 重跑。
- 初始提交总 diff 456 行，超章程 §6 的 400 行审阅预算；其中测试 186 行，
  其余为卡 / 轮次 / LEDGER 证据，按 §2 P2 回退路径保留。

### 6. 更合理做法

- 先以 crate 独立 `llvm-cov` 口径选最小缺口，再用公开 API 补边界；
  对协议测试先读实现分支顺序，再写断言，避免对失败优先级做错误假设。

### 7. 遗留问题

- 无；PR #326 / CI `38203102939` / merge `b4528fd` 已回填。

### 8. 新增长期记忆

- `docs/memory/facts.md`：`assistant-ipc` 独立行覆盖率从 81.49% 提升到
  85.57%，生产源码零改动；PR #326 / merge `b4528fd`。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API，且没有借测试改写生产契约。
2. token 脱敏、握手失败关闭、帧错误分类断言是否完整。
3. 85.57% 覆盖率的实测命令与输出是否可复现。

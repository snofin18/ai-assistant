# TASK-267　assistant-ipc 契约测试覆盖率加固（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/ipc/tests/coverage_contract.rs`、`tasks/TASK-267-ipc-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-11-round-5.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/ipc/README.md`

**目标**

仅为 `assistant-ipc` 增加公共契约测试，把该 crate 的独立行覆盖率从 41.47% 提升到 ≥70%；
重点覆盖真实 Windows NamedPipe 的 client/server 连接、认证握手、消息收发、断连、
peer 身份查询、随机 UUID 生成与 IPC 错误的稳定分类；不修改生产逻辑，不读取真实桌面，
不启动任何 GUI。

**背景**

2026-10-11 round 5 的独立实测基线为 `assistant-ipc` 行覆盖率 41.47%
（1102 行中覆盖 457 行）；`handshake.rs` 54.77%、`windows_io.rs` 5.71%、
`windows_transport.rs` 8.38%、`windows_peer.rs` 0.00%。当前 Ready 队列均为真实
GUI 或人工安全设计工作，按 §2 P2 做可机器验证的测试强化。

**步骤**

1. 新建 `crates/ipc/tests/coverage_contract.rs`。
2. 使用匿名唯一 pipe 名覆盖真实 `NamedPipeTransport` server/client 连接、send/recv/close。
3. 覆盖 `client_handshake` / `server_handshake` 的成功路径与协议失败关闭。
4. 覆盖缺失服务端、角色误用、未连接 peer 身份查询、客户端断开等错误路径。
5. 覆盖 `generate_session_id` 的 canonical UUID 形状和当前进程镜像路径查询。
6. 覆盖 `IpcError` 的全部稳定 error category 与 Display 契约。
7. 复测 `cargo llvm-cov -p assistant-ipc --tests --fail-under-lines 70`。
8. 跑 `cargo test -p assistant-ipc` 与全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅新增测试文件）
- [ ] 新增测试覆盖握手、真实 NamedPipe 传输、断连、peer 身份与错误分类契约
- [ ] `cargo llvm-cov -p assistant-ipc --tests --fail-under-lines 70` 通过
- [ ] `cargo test -p assistant-ipc` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-ipc --tests --fail-under-lines 70
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

【任务】TASK-267 assistant-ipc 契约测试覆盖率加固
【目标】仅新增测试，把 `assistant-ipc` 独立行覆盖率从 41.47% 提升到 ≥70%
【write scope】仅：本卡正文所列文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改生产行为、契约、依赖、CI/lint；读取真实桌面 / GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `crates/ipc/tests/coverage_contract.rs`
- `tasks/TASK-267-ipc-coverage-hardening.md`
- `LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/memory/facts.md`、`MEMORY.md`
- `docs/automations/2026-10-11-round-5.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-ipc` 行覆盖率 41.47%（1102 行中覆盖 457 行）。
- `cargo llvm-cov -p assistant-ipc --tests --fail-under-lines 70`：EXIT 0；
  1102 行中覆盖 898 行，行覆盖率 **81.49%**；`handshake.rs` 80.57%、
  `windows_io.rs` 67.35%、`windows_transport.rs` 89.01%、`windows_peer.rs` 61.11%。
- `cargo test -p assistant-ipc`：25 个单元测试、9 个新增真实 NamedPipe 契约测试、
  2 个 handle discipline 测试与 1 个 doctest 全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0，workspace 469 passed / 0 failed。
- `hygiene` scanned=400 / 0E / 124W / PASSED；`memory-counts`、`adr-index`、`refscan`、
  `docscan`、`card-check`、`check-ledger`、`check-comments`、`verify-schemas`、
  `codegen --check`、`check-migrations` 全部 PASSED。
- `cargo test -p assistant-core arch::`、`cargo deny check`、`replay --suite core` 全绿。
- PR #312：pull_request CI run `38132895715` = **11/11 SUCCESS**；合并前
  base=`main`、mergeable=`MERGEABLE`、mergeStateStatus=`CLEAN`；merge `5a5aa7f`。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅新增 IPC 集成测试文件。
- 握手、真实 NamedPipe、断连、peer 身份与错误分类：满足；9 个测试覆盖成功、
  错误消息、错误版本、错误 token、连接超时、角色误用、客户端断开和随机 UUID。
- crate 独立行覆盖率 ≥70%：满足，实测 81.49%。
- `cargo test -p assistant-ipc` 与全 workspace 门禁：满足。
- 全部现行 xtask 硬门禁：满足，见本卡验收输出与轮次报告。
- 热点文件 guard：满足，六个公共热点文件均先 acquire 后编辑。
- PR / CI / merge hash：满足；PR #312 / CI `38132895715` / merge `5a5aa7f`。

### 5. 偏差

- PowerShell 在本机再次零输出卡住；已按既有逃生路径改用 `cmd.exe` 执行
  git/cargo/gh、WSL 执行文本读取，并记录到本轮报告。
- 首版测试尝试把已创建的 `NamedPipeTransport` 移入工作线程，编译发现其
  Win32 `HANDLE` 使类型不满足 `Send`；改为在线程内创建 server、主线程创建 client，
  既保留真实端到端覆盖，也不修改生产类型。
- `cargo llvm-cov` 有一处误用 WSL shell 导致 `cargo: command not found`；
  随后按环境规则改回 `cmd.exe` 重跑成功。

### 6. 更合理做法

- 对非 `Send` 的平台句柄对象，测试并发的自然形态是每个线程各自创建端点，
  而不是跨线程搬运句柄；本轮据此固定了 IPC 契约测试写法。

### 7. 遗留问题

- PR #312 / CI run `38132895715` / merge `5a5aa7f` 已回填。
- 真实 Paint 验收仍由 TASK-256 / TASK-044 的有人值守流程承接。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-ipc` 独立行覆盖率从 41.47% 提升到
  81.49%，并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API，生产源码零改动。
2. 真实 NamedPipe 测试是否在超时后可靠回收线程与句柄。
3. 握手、断连和 peer 身份失败是否断言了稳定错误分类。

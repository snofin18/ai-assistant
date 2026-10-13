# TASK-280　agent-core ui-server 公开契约覆盖率补齐（章程 §2 P2）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`apps/agent-core/src/ui_server_tests.rs`、`tasks/TASK-280-agent-core-ui-server-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-13-round-6.md`、`docs/automations/2026-10-13-report.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、ADR-0057（Core↔UI 传输契约）、`docs/spec/ipc-protocol.md`

**目标**

仅为 `assistant-agent-core` 的 Core 侧 UI 命令监听端（ADR-0057）增加白盒契约测试，把
`apps/agent-core/src/ui_server.rs` 的行覆盖率从 67.20% 提升到 ≥75%（crate 独立行覆盖率保持 ≥75%）。
重点覆盖 `serve_session_with_events` 的**心跳分支**（有效心跳被观察并回发、会话不符 / `alive=false` 失败）、
**接收超时后的续命**，以及 `serve` 在**缺失 token** 时于开管道前失败关闭。不修改生产逻辑，不新增依赖。

**背景**

2026-10-13 round 6 的 `ui_server.rs` 行覆盖率为 67.20%。既有 `ui_server_tests.rs` 用脚本化 transport
覆盖了请求派发、工具形状拒绝、事件先于响应、allow-list 与处理器错误映射，但没有覆盖**心跳往返 /
异常心跳**、**接收超时续命**与**缺失 token** 三条分支。候选来自
`docs/automations/2026-10-12-report.md` §③（`apps/agent-core` 按模块拆卡）、按 §2 P2 派单。

**步骤**

1. 在既有 `apps/agent-core/src/ui_server_tests.rs` 追加白盒用例（复用其脚本化 transport）。
2. 覆盖有效心跳（观察 + 回发）、会话不符与 `alive=false` 的显式失败。
3. 覆盖接收超时后重新检查看门狗并回发心跳。
4. 覆盖 `serve` 在 token 环境变量缺失时、开管道前返回 `InvalidAuthenticationToken`。
5. 复测 `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 并跑全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅改测试模块）
- [ ] 新增测试覆盖心跳 / 超时 / 缺失 token 三条契约
- [ ] `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 通过，且 `ui_server.rs` 行覆盖率 ≥75%
- [ ] `cargo test -p assistant-agent-core` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75
cargo test -p assistant-agent-core
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

【任务】TASK-280 agent-core ui-server 公开契约覆盖率补齐
【目标】追加白盒契约测试，把 `ui_server.rs` 行覆盖率从 67.20% 提升到 ≥75%
【write scope】仅：`apps/agent-core/src/ui_server_tests.rs` 与本卡列出的状态同步文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、CI/lint、生产行为；真实命名管道 / GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 模块覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3；运行日 2026-10-13 round 6（**机制 = human**）
【疑问】无

### 2. 实际改动文件

- `apps/agent-core/src/ui_server_tests.rs`（追加 3 个白盒契约测试）
- `tasks/TASK-280-agent-core-ui-server-contract-coverage.md`
- `LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/automations/2026-10-13-round-6.md`、`docs/automations/2026-10-13-report.md`

### 3. 验收输出摘要

- 改动前独立实测：`apps/agent-core` 行覆盖率 76.28%，`ui_server.rs` 67.20%（region 59.84%）。
- `cargo test -p assistant-agent-core --lib ui_server`：14 passed（11 既有 + 3 新增）。
- `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75`：EXIT 0；
  crate 行覆盖率 **76.56%**；`ui_server.rs` 行覆盖率 **80.11%**（region 70.68%）。
- （完整门禁与 PR / CI / merge 证据在收口与回填后补记。）

### 4. DoD 逐条核对

- （收口后逐条填写。）

### 5. 偏差

- 无。

### 6. 更合理做法

- 用既有 `ScriptedTransport` / `TimeoutThenDisconnect` 驱动 `serve_session(_with_events)`，
  在**不启动真实命名管道**的前提下覆盖心跳与超时分支；`serve` / `serve_with_events` 的真实管道路径
  已由 `assistant-ipc` 自己的覆盖率卡承接。

### 7. 遗留问题

- `ui_server.rs` 剩余未覆盖行是 `serve_with_events` 的真实命名管道 / 握手 / peer 身份路径与不可达的
  序列化 / 时钟错误分支；前者需要真实管道，已由 ipc crate 的卡覆盖。
- 至此 §③ 里公开可及的低覆盖模块基本做尽，后续建议先重测现场覆盖率再派卡。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `ui_server.rs` 行覆盖率 67.20% → 80.11%、crate 76.28% → 76.56%，
  并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 / 白盒 transport 接缝，未改变生产控制流。
2. 心跳有效 / 会话不符 / `alive=false` / 超时续命 / 缺失 token 是否都断言了精确错误变体。
3. 覆盖率数字是否由同一提交上的独立 `cargo llvm-cov -p assistant-agent-core` 实跑得到。

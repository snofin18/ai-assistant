# TASK-254　PL-027 末行换行清扫与 Error 化

- 状态：**Done**
- 阶段：1　子阶段：治理池　批次：治理池　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `PLAN.md` 与 `plans/stage-1-pilots.md`。

---

- **write scope**：`tasks/TASK-254-pl027-final-newline-cleanup.md`、`xtask/src/repowalk.rs`、`xtask/src/repowalk_top_level_tests.rs`、`xtask/src/hygiene.rs`、`xtask/src/hygiene_tests.rs`、`crates/audit/Cargo.toml`、`crates/protocol/Cargo.toml`、`protocol/audit-event/audit-event-1.0.json`、`protocol/capability-matrix/capability-1.0.json`、`protocol/envelope/envelope-1.0.json`、`protocol/error-codes/error-codes-1.0.json`、`protocol/tool-schema/tool-schema-1.0.json`、`spikes/spike-a-notepad/probe-09-key-control-locate.ps1`、`xtask/README.md`、`docs/governance-ai-agent-execution.md`
- **关联**：`docs/PARKING_LOT.md` PL-027、`docs/adr/0025-hygiene-rule-count-unification.md` D1、`docs/governance-ai-agent-execution.md` §5.4

**目标**

闭合 PL-027：清扫当前 `hygiene/missing-final-newline` 的全部可提交存量，明确跳过被 `.gitignore` 忽略的 Tauri 生成目录，并按 ADR-0025 D1 将规则从 Warning 升为 Error。

**步骤**

1. 将 hygiene 文本遍历排除 `apps/desktop-ui/src-tauri/gen/**`，使本机扫描集合不被 Tauri 忽略生成物污染。
2. 给 8 个受版本控制的存量文件补齐单个末尾 LF。
3. 将 `hygiene/missing-final-newline` 的三类违规（空文件、缺 LF、末尾空行）升为 Error，并同步测试、README、gov 生效状态。
4. 跑完整门禁；追加 LEDGER、关闭 PL-027，并写本轮 automation 报告。

**DoD**

- [ ] `cargo run -p xtask -- hygiene` 为 **0 Error**，输出中不再出现 `hygiene/missing-final-newline`。
- [ ] `cargo test -p xtask repowalk:: hygiene::` 全绿，且测试证明生成目录被跳过、三类换行违规均为 Error。
- [ ] `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace` 全绿。
- [ ] xtask 十一项门禁全 PASSED。
- [ ] LEDGER 追加一行；PL-027 追加闭环行；状态文件同步。

**验收命令**

```powershell
cargo test -p xtask repowalk::
cargo test -p xtask hygiene::
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-254 PL-027 末行换行清扫与 Error 化
【目标】清零 missing-final-newline 存量，排除 Tauri 忽略生成目录，并按 ADR-0025 D1 升为 Error
【write scope】见卡片正文；热点文件另按 guard 协议逐个 acquire / release
【铁律】无静默失败；契约先行；不静默扩大范围；不放宽判据；生成物边界显式
【禁止】不操作真实 GUI；不改 CI/lint 配置/公共 API；不新增依赖/crate；不合并 PR #275
【验收】专项测试 + fmt/clippy/workspace tests + xtask 十一项门禁
【依赖】ADR-0025 D1 已 Accepted；PL-027 实时基线为 10 个 Warning
【疑问】无；生成目录例外仅限 apps/desktop-ui/src-tauri/gen/**
```

### 2. 实际改动文件

- `xtask/src/repowalk.rs`、`xtask/src/hygiene.rs`
- `xtask/src/hygiene_tests.rs`
- `xtask/README.md`
- `crates/audit/Cargo.toml`、`crates/protocol/Cargo.toml`
- `protocol/audit-event/audit-event-1.0.json`
- `protocol/capability-matrix/capability-1.0.json`
- `protocol/envelope/envelope-1.0.json`
- `protocol/error-codes/error-codes-1.0.json`
- `protocol/tool-schema/tool-schema-1.0.json`
- `spikes/spike-a-notepad/probe-09-key-control-locate.ps1`
- `docs/governance-ai-agent-execution.md`
- `docs/PARKING_LOT.md`、`docs/memory/facts.md`、`docs/memory/pitfalls.md`
- `docs/automations/2026-10-07-round-6.md`
- `LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p xtask repowalk::` -> 20 passed / 0 failed。
- `cargo test -p xtask hygiene::` -> 38 passed / 0 failed。
- `cargo fmt --all --check` -> EXIT 0。
- `cargo clippy --all-targets -- -D warnings` -> EXIT 0；仅既有 `unknown lint: clippy::assert_is_empty` 提示。
- `cargo test --workspace` -> EXIT 0（xtask 469 passed；ignored 真机项按既有标记跳过）。
- `cargo test -p assistant-core arch::` -> 10 passed / 0 failed。
- `cargo deny check` -> advisories / bans / licenses / sources 全 ok。
- `cargo run -p xtask -- hygiene` -> scanned=387，**0E/100W**，无 `hygiene/missing-final-newline`。
- `memory-counts` / `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` / `check-comments` / `verify-schemas` / `codegen --check` / `check-migrations` -> 全部 PASSED。
- PR #278：pull_request run `37669241763` 全 11/11 SUCCESS；合并前
  `baseRefName=main`、`mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`；
  merge hash `1aa082b`。

### 4. DoD 逐条核对

- [x] `hygiene` 0 Error 且无 `missing-final-newline`。
- [x] 专项测试全绿：生成目录跳过正反样本 + 三类换行违规均为 Error。
- [x] `fmt` / `clippy` / `workspace tests` 全绿。
- [x] xtask 十一项门禁全 PASSED。
- [x] LEDGER 追加行；PL-027 闭环行与状态文件同步；PR #278 已合并。

### 5. 偏差

无。生成目录排除只限定 `apps/desktop-ui/src-tauri/gen/**`，没有扩大到全局 `gen/`；规则是升严而不是放宽。

### 6. 更合理做法

用精确相对路径排除被忽略的 Tauri 生成子树，而不是把 `gen` 目录名全局加入跳过表；同时用正反测试锁住该边界。

### 7. 遗留问题

PR #275（TASK-044）仍因真实 Paint GUI 验收缺口保持 open，且当前已变
`CONFLICTING/DIRTY`；本轮未触碰、未合并。除此之外无偏差。

### 8. 新增长期记忆

- `docs/memory/facts.md`：PL-027 清零与规则 Error 化事实。
- `docs/memory/pitfalls.md`：工作区遍历不得把 gitignored 生成物当源码。

### 9. 给审阅者的关注点

1. `is_skipped_relative_directory` 是否只命中 Tauri 生成子树，普通源码目录不受影响。
2. `missing-final-newline` 三类违规是否全部为 Error，且现有 8 个受控文件不再触发。
3. 本机 `hygiene` 报告的 100W 均为既有结构告警，不与本轮换行规则混淆。

# TASK-284　StorageBlobSink 公共契约覆盖率补齐（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：TASK-041 / TASK-106 Done　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`apps/agent-core/tests/blob_sink_contract.rs`、`tasks/TASK-284-storage-blob-sink-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-13-round-10.md`、`docs/automations/2026-10-13-report.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5、`docs/adr/0076-windows-gdi-capture-channel-and-redaction.md` D6/D8、`apps/agent-core/src/blob_sink.rs`、`apps/agent-core/README.md`

**目标**

仅为 `StorageBlobSink` 增加跨平台公共契约测试，覆盖一次性 `attach`、未注入 / 弱引用失效 /
尺寸错误 / 内容地址不匹配的显式 `Fatal` 失败，以及成功写入 `BlobKind::Screenshot` 后读回同字节；
把 `blob_sink.rs` 行覆盖率提升到 ≥90%，不修改生产逻辑、不新增依赖。

**背景**

ADR-0076 D6/D8 要求装配层注入 blob writer。实测 `blob_sink.rs` 仅 31/57 行（54.39%），
失败语义与成功落盘缺少跨平台回归。

**步骤**

1. 新建 `apps/agent-core/tests/blob_sink_contract.rs`，只调用公开 API。
2. 覆盖一次注入、重复注入、未注入 / 弱引用失效 / 尺寸错误 / 地址不匹配。
3. 覆盖成功写入、`BlobKind::Screenshot` 元数据与读回字节。
4. 复测 crate / workspace 覆盖率，并跑全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动；新增测试只调用公开 API、不新增依赖
- [ ] `blob_sink.rs` ≥90%，`assistant-agent-core` crate ≥75%
- [ ] `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 与
      `cargo test -p assistant-agent-core` 全绿
- [ ] `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check` 全绿
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75
cargo test -p assistant-agent-core
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- check-comments; cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index; cargo run -p xtask -- refscan; cargo run -p xtask -- docscan
cargo run -p xtask -- card-check; cargo run -p xtask -- check-ledger; cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations; cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录

### 1. 约束回执

【任务】TASK-284 StorageBlobSink 公共契约覆盖率补齐　【目标】仅追加测试，把 `blob_sink.rs`
行覆盖率提升到 ≥90%　【write scope】仅本卡列出的测试与状态 / 轮次文件
【铁律】无静默失败；测试不改生产行为；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、lint/CI、schema/DB/IPC；真实 GUI / 网络 / 凭据
【验收】见上；【依赖】TASK-041 / TASK-106 Done；ADR-0076 D6/D8；【疑问】无

### 2. 实际改动文件

- `apps/agent-core/tests/blob_sink_contract.rs`（新增 7 个跨平台公共契约测试）
- `tasks/TASK-284-...md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/automations/2026-10-13-round-10.md`、`docs/automations/2026-10-13-report.md`

### 3. 验收输出摘要

- 改动前：`blob_sink.rs` 31/57 行（54.39%），crate 76.56%。
- `cargo test -p assistant-agent-core --test blob_sink_contract`：7 passed。
- `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75`：EXIT 0；
  `blob_sink.rs` **57/57 = 100.00%**，crate **76.86%**；workspace **80.26%**。
- `fmt` / `clippy -D warnings` / `test --workspace` / `cargo deny check` / UI / arch / replay
  全部 EXIT 0；clippy 仅保留既有 `clippy::assert_is_empty` unknown-lint 噪音。
- xtask：`hygiene` 0E/117W、`check-comments` 0E/67W、`docscan` 0E/264W，其余硬门禁 PASSED。
- PR #367；pull_request CI = 11/11 SUCCESS；merge `bff4c12`。

### 4. DoD 逐条核对

- 生产逻辑零改动、测试只用公开 API、无新依赖：满足。
- `blob_sink.rs` ≥90%、crate ≥75%、覆盖率命令与全门禁：满足（100.00% / 76.86%）。
- `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`：同批同步完成。

### 5. 偏差

- P3 剩余为设计性复杂度 / 文件长度或公共 API 改名；P6 的 `MEMORY.md` §5 与 ADR-0021 冲突，
  未自行改 ADR/spec，按章程回退 P2。
- 首次 clippy 命中 `missing_const_for_fn` 与 `significant_drop_tightening`；已按 lint 修复。
- 新建卡片首次多一个尾部空行触发 `hygiene/missing-final-newline`；删除后复测 0E。

### 6. 更合理做法

- 用单个跨平台 integration test 直接驱动 public `StorageBlobSink`，临时 SQLite 自清理。

### 7. 遗留问题

- P6 旧路径需治理卡裁决；真实截图落盘仍由 `capture_blob_acceptance` ignored case 承接。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API，并覆盖失败语义而非只堆成功路径。
2. 100% 行覆盖率是否来自可解释的 7 条契约用例，而非统计 artifact。

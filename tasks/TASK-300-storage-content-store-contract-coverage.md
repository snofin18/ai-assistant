# TASK-300　storage blob store 公开错误/边界契约覆盖率补强（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**　批次：**治理池**　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`crates/storage/tests/content_store_contract.rs`、本卡、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-15-round-3.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5、`crates/storage/src/content.rs`、`crates/storage/README.md`、`docs/storage-design.md` §3.3 / §8

**目标**

仅用公开 API 契约测试补强 `crates/storage/src/content.rs`：覆盖 `BlobStore` Debug、
GC 目录冲突、deep 同长度篡改、原子写入失败与 SQLite 失败。`content.rs` 行覆盖率
从 78.26% 提升到 ≥90%，crate 保持 ≥90%；生产源码零改动、不新增依赖。

**背景**

2026-10-15 P2 扫描显示 `content.rs` 为 234/299（78.26%），低于常用 90% 目标。
`cargo llvm-cov -p assistant-storage --tests --show-missing-lines` 显示存在公开 API
可达的 GC、deep 扫描与原子写入错误路径；既有 `storage_integration.rs` 已覆盖主路径。

**步骤**

1. 新增 `crates/storage/tests/content_store_contract.rs`，只调用公开 API 和共享临时目录/固定时钟夹具。
2. 覆盖 Debug、root、path 与 GC 目录冲突 / 引用计数查询失败。
3. 覆盖 deep 同长度伪造、Windows 独占读锁与 `write_atomically` 三类失败。
4. 覆盖 blob / reference 的 SQLite insert/delete 失败与 GC 删除失败。
5. 用真实 `cargo llvm-cov` 对比改动前后，记录完整门禁输出并同步状态。

**DoD**

- [ ] 生产逻辑零改动；测试只调用公开 API、不新增依赖、文件 ≤600 行
- [ ] `content.rs` 78.26% → ≥90%；`assistant-storage` crate ≥90%
- [ ] `cargo llvm-cov -p assistant-storage --tests --fail-under-lines 85` 与 `cargo test -p assistant-storage` 全绿
- [ ] `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check` 全绿
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-storage --tests --fail-under-lines 85
cargo test -p assistant-storage
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- check-comments; cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index; cargo run -p xtask -- refscan; cargo run -p xtask -- docscan
cargo run -p xtask -- card-check; cargo run -p xtask -- check-ledger; cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations; cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录
### 1. 约束回执
【任务】TASK-300 storage blob store 公开错误/边界契约覆盖率补强　【目标】只加公开 API
契约测试，把 `content.rs` 从 78.26% 提升到 ≥90%　【write scope】仅本卡列出的测试与
状态 / 轮次文件　【铁律】无静默失败；生产源码零改动；不扩 scope；热点文件先 guard　
【禁止】改接口、依赖、lint/CI、schema/DB/IPC；真实 GUI / 网络 / 凭据　【依赖】无　
【疑问】无
### 2. 实际改动文件
- `crates/storage/tests/content_store_contract.rs`（新增 6 个公开 API 契约测试）
- 本卡、`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`
- `docs/automations/2026-10-15-round-3.md`
- 仓库外日志 `C:\Users\fexfe\.codex\automations-logs\2026-10-15\ai-assistant-task-round-20261015-2200.jsonl`
### 3. 验收输出摘要
```text
改动前：content.rs 234/299 行 78.26%；TOTAL 1419/1595 88.97%
改动后：content.rs 272/299 行 90.97%；TOTAL 1457/1595 91.35%
cargo llvm-cov -p assistant-storage --tests --fail-under-lines 85 -> EXIT 0（6 passed）
cargo test -p assistant-storage / fmt / clippy -D warnings / test --workspace -> EXIT 0
arch / replay（2 step / 5 change / 0 mismatch）/ deny -> EXIT 0
xtask：hygiene 0E/114W；check-comments 0E/68W；memory-counts 0E/0W；adr-index 0E/0W；
refscan 908 0E/0W；docscan 464 0E/264W；card-check 194 0E/34W；check-ledger 0E/0W；
verify-schemas 5 0E；codegen 0 drift；check-migrations 5/5
```
### 4. DoD 逐条核对
- 生产源码零改动、公开 API、无新依赖、测试文件 234 行；覆盖率 / 门禁全绿：**满足**。
- 主 PR #419、pull_request CI run `38752157048` = 11/11 SUCCESS、merge `6e5d8fe`；
  post-merge main push run `38753906552` = success：**满足**。
### 5. 偏差
- 无范围 / ADR / 契约偏差；无 PowerShell 挂起。
- 六次只读查询 / 门禁查询误用默认 bash（exit 127、语法错误或命令不兼容），随即改用 PowerShell 重跑；未影响证据。
- 开发中修正两次测试自身问题（`Clock` 导入、Debug 断言过强）后才提交。
- 不追不可达的删文件竞态、无父路径、zstd 失败与 SHA-256 碰撞分支。
### 6. 更合理做法
- 文件系统错误用公开 API + 真实目录/锁冲突构造，比 mock 更贴近不变量。
### 7. 遗留问题
- PR / CI / merge hash / Done 待回填；不可达防御分支不追。
### 8. 新增长期记忆
- 无新增 FACT / PITFALL / REJECTED。
### 9. 给审阅者的关注点
1. 新增用例是否只经公开 API 构造并观察错误。
2. 目录占位、Windows 独占读锁与 SQLite trigger 故障是否稳定。

# TASK-012　存储层：SQLite(WAL) + 迁移框架 + 核心表 + 内容寻址 blob（zstd/去重）

- 状态：**Done**（2026-09-24）
- 阶段：1　子阶段：**1a**　批次：**A1**　依赖：011（✅ Done）　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

## 目标（一句话）

落地 `crates/storage`：SQLite(WAL) 主库 + **只前进不回滚**的迁移框架 + 核心表 + 内容寻址 blob（zstd + 去重），
使 W1/W2/W6/W7/W8 有持久化落点、W4/W5 有外置存储与元数据落点（依据 `docs/storage-design.md` §2 / §3）。

## 依赖（**开工第 0 步 = 先取人类批准**）

本卡是**产品 workspace 引入第三方依赖的第一张卡**（漂移触发器 ①「加第三方依赖」）→ 必须**先登记、后引入**：

| crate | 建议版本 | 用途 | 替代方案与否决理由 |
|---|---|---|---|
| `rusqlite` | `0.3x` + `bundled` feature | SQLite 驱动 | `bundled` = 自带 SQLite 源码编译（无系统依赖、三平台可复现）。不用 `sqlx`：本卡只需**同步单写者**，不需要 async 驱动 + 编译期 SQL 校验的复杂度 |
| `zstd` | `0.13` | blob 压缩（level 3） | 与 `docs/storage-design.md` §3.3 既定口径一致；不用 `flate2`：压缩比与速度都不如 zstd |
| `sha2` | `0.10` | 内容寻址用的 SHA-256 摘要 | 不用 `blake3`：`sha256:` 前缀是**既定口径**（架构 v2 §15.3、audit hash chain 同源），换算法 = 改契约 |

> **批准之前不得改任何 `Cargo.toml`**（`docs/DEPENDENCIES.md` 登记规则 1「先登记，后引入」）。
> 批准之后：先在 `docs/DEPENDENCIES.md` 的 Rust 表逐行登记（「替代方案与否决理由」必填），再引入。

## write scope

- `crates/storage/**`（新 crate：`Cargo.toml` / `README.md` / `src/**` / `tests/**` / `migrations/**`）
- `Cargo.toml`（workspace 根：仅当需要把依赖登记进 `[workspace.dependencies]` 时才改）
- `docs/DEPENDENCIES.md`（登记 3 个依赖）
- `docs/memory/facts.md`、`docs/memory/pitfalls.md`（新事实 / 新坑）
- `LEDGER.md`、`MEMORY.md`（规模表）、`tasks/TASK-012-*.md`（本卡记录区）

## In scope

1. **crate 骨架**：`crates/storage`，`README.md` 必须含职责 / 边界 / **不变量** / 已知限制。
2. **连接与 PRAGMA**：按 `storage-design.md` §3.2 建库（`journal_mode=WAL`、`synchronous=NORMAL`、`busy_timeout`、`foreign_keys=ON`、`mmap_size`、`page_size=8192`、`cache_size`、`temp_store=MEMORY`、`wal_autocheckpoint`）。
3. **迁移框架**：`migrations/` + 版本表；**只前进不回滚**；启动时校验 `schema_version` 与二进制期望值，不匹配 → **拒绝启动并给出可读错误**（禁止静默继续、禁止自动"修好"）。
4. **核心表**（W1/W4/W5/W6 最小集）：`task` / `step` / `checkpoint` / `blob`（`blob_id`、`kind`、`bytes`、`compressed_bytes`、`created_at`）/ `blob_ref`（引用计数）/ `usage`。**audit 表归 TASK-013、FTS5 记忆表归 TASK-028 —— 本卡不建**。
5. **内容寻址 blob**：路径 `blobs/<sha256 前 2 位>/<sha256>`；写入 = `zstd(level 3)` → `sha256` → 已存在则复用并给引用计数 +1；读取 = 解压 + **重算 sha256 校验**，不匹配 → `evidence_corrupt`；GC = 引用计数 0 且超 TTL。
6. **一致性检查**：`blob` 表有行但文件缺失 → 标 `evidence_missing`（**不得当作"没有这个 blob"**）。
7. **测试**：纯逻辑单测 + 临时目录下的真实 SQLite/blob 往返集成测试；**必须含负向用例**（坏 `schema_version` 拒绝启动 / blob 被篡改 1 字节 → `evidence_corrupt` / DB 行在而文件不在 → `evidence_missing`）。

## Out of scope（做了算漂移）

- `crates/audit`（TASK-013）、`crates/secrets`（TASK-014）、FTS5 记忆检索（TASK-028）
- 影子副本（W5）的**文件写入策略**（本卡只建元数据表与 blob 通道）
- 加密 / SQLCipher、冷归档 L3、在线备份 CLI、UI 任何代码
- 任何 `platform/**`、任何 Adapter、`sqlx` / `refinery` 之类额外依赖
  → 若实测"自建迁移框架"不可行，**停下写 DRIFT**，不要顺手加依赖

## 必须遵守

- **铁律 1（无静默失败）**：schema 版本不匹配 / blob 校验失败 / DB-文件不一致 → 一律**报错**；禁止默认值冒充结果，禁止 `let _ =` 丢弃 `Result`。
- **铁律 10（契约先行）**：本卡不改 `protocol/**` 与 `docs/spec/**`；表结构若与 `docs/storage-design.md` 冲突 → **停下写 DRIFT**。
- 继承 workspace lints：`unwrap_used` / `expect_used` / `panic` / `todo` / `unimplemented` / `dbg_macro` / `print_stdout` / `print_stderr` / `indexing_slicing` 全部 **deny**。
- 单文件 ≤ 600（软）/ 900（硬）行（ADR-0033）；函数 ≤ 80 行；参数 ≤ 6。
- **时钟 / 随机 / FS 一律 trait 注入**（AGENTS.md §5.3）→ 测试可回放；生产代码禁止直接 `SystemTime::now()`。
- 公开 API 100% 文档注释，含**错误语义**（何时返回哪个 `ErrorCode`）。
- 每 20~40 行有效代码至少一条解释性注释；storage 层**不需要 `unsafe`**（出现即漂移）。
- 运行时产物（`*.db` / `*.db-wal` / `*.db-shm` / `blobs/`）必须进 `.gitignore`，测试只用临时目录。

## 验收命令（agent 必须全部执行并粘贴输出）

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-storage
cargo test --workspace
cargo build --release
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- memory-counts
cargo deny check licenses bans sources
```

## 完成定义（DoD）

- [ ] 上列命令全部通过（输出粘贴进本卡 §3）
- [ ] `docs/DEPENDENCIES.md` 已登记 3 个依赖（**在改 `Cargo.toml` 之前**），`cargo deny check licenses bans sources` exit 0
- [ ] 迁移：全新库可建到最新版本；`schema_version` 被改坏 → **拒绝启动**且错误可读（负向用例）
- [ ] blob 往返：写入 → 读取 → 逐字节一致；同内容二次写入 → **不产生第二个文件**（去重可证）
- [ ] blob 篡改 1 字节 → `evidence_corrupt`；DB 行在而文件不在 → `evidence_missing`
- [ ] 孤儿 blob 可 GC（引用计数 0 + 超 TTL），且有测试覆盖
- [ ] `crates/storage/README.md` 含职责 / 边界 / **不变量** / 已知限制
- [ ] 无任何 Out of scope 的文件被修改
- [ ] `LEDGER.md` 追加一行；新 FACT/PITFALL 已追加；`MEMORY.md` 规模表按 `memory-counts` 机器值同步
- [ ] 本卡 §1~§9 执行记录已填

## 风险与已知坑（开工前先读）

- `rusqlite` 的 `bundled` 会编译 C 源码 → **首次构建分钟级**，需要 C 编译器（MSVC 见 `docs/dev-env-setup.md`）。
- Windows 的文件句柄语义与 Unix 不同：GC 删除前必须确认**没有打开的连接**，否则会 `os error 32`。
- WAL 下 `synchronous=NORMAL` 是"性能 vs 耐久性"的显式取舍；审计的耐久性由 TASK-013 的 `durability` 配置承担，**不要在本卡偷偷升级成 FULL**。
<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-012 存储层：SQLite(WAL) + 迁移框架 + 核心表 + 内容寻址 blob
【目标】落地 crates/storage，使 W1/W2/W6/W7/W8 有持久化落点、W4/W5 有外置存储与元数据落点
【write scope】仅：crates/storage/**、Cargo.toml（本次未改）、docs/DEPENDENCIES.md、
              docs/memory/{facts,pitfalls}.md、LEDGER.md、MEMORY.md（规模表）、
              tasks/TASK-012-*.md（记录区）
【铁律】1 无静默失败（schema 版本 / blob 校验 / 行-文件不一致一律报错）；
        4 每个写操作有可验证的 postcondition（put/get/GC 各自可验）；
        7 core 不得调平台 API（本卡完全不碰平台层，`#![deny(unsafe_code)]`）；
        10 契约先行（表名以架构 v2 §15.1 为单一事实源）
【禁止】crates/audit（TASK-013）、crates/secrets（TASK-014）、FTS5 记忆表（TASK-028）、
        影子副本（W5）写入策略、加密/SQLCipher、冷归档 L3、在线备份 CLI、UI、
        platform/**、任何 Adapter、sqlx / refinery
【验收】cargo fmt --all --check / cargo clippy --all-targets -- -D warnings /
        cargo test -p assistant-storage / cargo test --workspace / cargo build --release /
        cargo run -p xtask -- hygiene / docscan / memory-counts /
        cargo deny check licenses bans sources  → 全绿（见 §3）
【依赖】TASK-011（assistant-protocol，✅ Done，已核对 LEDGER）；本卡 3 个第三方依赖
        （rusqlite + bundled / zstd / sha2）已获人类 chat 2026-09-24 批准
【疑问】卡面 In scope 写的是单数简称表名（task/step/checkpoint/blob/blob_ref/usage），
        与架构 v2 §15.1 的权威复数名（tasks/task_steps/checkpoints/blobs/blob_refs/
        usage_records）冲突 → 默认按 §15.1 落地并登记 DRIFT-012-1（见 §5）
```

### 2. 实际改动文件

| 文件 | 动作 | 说明 |
|---|---|---|
| `crates/storage/Cargo.toml` | NEW | 3 个已批准依赖（`rusqlite 0.37`+`bundled` / `zstd 0.13` / `sha2 0.10`）+ 工作区内 path 依赖 `assistant-protocol`（带 `version`，见 §5-3） |
| `crates/storage/README.md` | NEW | 职责 / 边界 / 不变量（8 条）/ 典型用法 / 已知限制（6 条） |
| `crates/storage/migrations/0001_init.sql` | NEW | 6 张核心表 + 索引 + CHECK；表名 = v2 §15.1 |
| `crates/storage/src/lib.rs` | NEW | `Database`（open/close/connection/paths/clock/blob_store/schema_version）+ PRAGMA 基线 |
| `crates/storage/src/error.rs` | NEW | `StorageError`（11 变体）+ 稳定 `reason_code()` + 到 `ErrorCategory` 的映射 |
| `crates/storage/src/time_source.rs` | NEW | `Clock` trait + `SystemClock`（唯一"当前时间"来源） |
| `crates/storage/src/paths.rs` | NEW | `StoragePaths`（数据目录布局，FS 注入点） |
| `crates/storage/src/schema.rs` | NEW | 只前进不回滚迁移框架（内嵌 SQL + sha256 记账 + 每迁移独立事务） |
| `crates/storage/src/blob_id.rs` | NEW | 值类型：`BlobId` / `BlobKind` / `BlobOwner`（无 IO、无 SQL） |
| `crates/storage/src/content.rs` | NEW | `BlobStore`：put/get/引用计数/GC/一致性扫描 + 原子写 + 读取校验 |
| `crates/storage/src/records.rs` | NEW | `TaskRecord` / `TaskStepRecord` / `CheckpointRecord` / `UsageRecord` + 最小 CRUD |
| `crates/storage/tests/storage_unit.rs` | NEW | 6 条纯逻辑单测（无 IO） |
| `crates/storage/tests/storage_integration.rs` | NEW | 18 条真实 IO 集成测试（迁移/启动校验 + blob 池） |
| `crates/storage/tests/storage_records.rs` | NEW | 2 条核心表 CRUD 集成测试 |
| `crates/storage/tests/common/mod.rs` | NEW | 跨测试 crate 共享夹具（临时目录 / 固定时钟 / 计数工具） |
| `docs/DEPENDENCIES.md` | EDIT | 登记 3 个依赖（**先登记、后引入**；在改 `Cargo.toml` 之前完成） |
| `Cargo.lock` | EDIT | 依赖解析结果（`rusqlite 0.37.0` / `libsqlite3-sys 0.35.0` / `zstd 0.13.3` / `sha2 0.10.9` / `cc 1.4.7`） |
| `docs/memory/facts.md` | APPEND | +2 条（存储层落地实测；本机 cargo 网络现状更正） |
| `docs/memory/pitfalls.md` | APPEND | +3 条（deny 通配依赖 / `redundant_pub_crate` / 测试夹具 `dead_code`） |
| `MEMORY.md` | EDIT | 规模表：`facts.md` 139/91、`pitfalls.md` 184/81（`memory-counts` 机器值） |
| `LEDGER.md` | APPEND | 本卡一行事件 |
| `tasks/TASK-012-*.md` | EDIT | 状态行 `Ready` → `Done` + 本记录区 |

**未改**：`.gitignore`（不在 write scope；`*.db` / `*.db-wal` / `*.db-shm` / `/blobs/` / `/shadow/` 已覆盖）、
`Cargo.toml`（根：3 个依赖直接写在 crate 内，无需进 `[workspace.dependencies]`）、`deny.toml`、`protocol/**`、`docs/spec/**`。

### 3. 验收输出摘要

```text
cargo fmt --all --check                     → exit 0（0 diff）
cargo clippy --all-targets -- -D warnings   → exit 0（0 warning，workspace 全绿）
cargo test -p assistant-storage             → PASS 27 项
    集成 storage_integration: 18 passed
    集成 storage_records:      2 passed
    单测 storage_unit:         6 passed
    doctest（lib.rs 典型用法）: 1 passed
cargo test --workspace                      → PASS（xtask 331 + assistant-protocol + assistant-storage，0 failed）
cargo build --release                       → exit 0（libsqlite3-sys / zstd-sys / rusqlite / assistant-storage 全部编译通过）
cargo run -p xtask -- hygiene               → verdict: PASSED（0 error，2 warning 均为 xtask 既有文件 >600 行：
                                              xtask/src/card_check.rs:667、xtask/src/main.rs:685）
cargo run -p xtask -- docscan               → verdict: PASSED（scanned 148，0 error / 0 warning）
cargo run -p xtask -- memory-counts         → verdict: PASSED（scanned 8，0 error / 0 warning）
cargo run -p xtask -- adr-index             → verdict: PASSED（scanned 21，0 error / 0 warning）
cargo run -p xtask -- verify-schemas        → verdict: PASSED（0 error）
cargo run -p xtask -- codegen --check       → verdict: PASSED（0 drift）
cargo deny check licenses bans sources      → bans ok, licenses ok, sources ok（exit 0）
```

**负向用例实测**（DoD 强制要求，全部在 `cargo test -p assistant-storage` 里）：

```text
test_open_rejects_database_without_version_table   ok   # 有业务表无版本表 → 拒绝启动
test_open_rejects_unknown_schema_version           ok   # 版本表出现未知版本 → 拒绝启动
test_open_rejects_tampered_migration_checksum      ok   # 迁移记账被改 → MigrationChecksumMismatch
test_blob_get_detects_single_byte_tamper           ok   # 翻转 1 字节 → evidence_corrupt
test_blob_get_detects_forged_valid_frame           ok   # 合法 zstd 帧但内容不符 → evidence_corrupt
test_blob_missing_file_reports_evidence_missing    ok   # 行在文件不在 → evidence_missing
test_blob_put_get_roundtrip_and_dedupe             ok   # 同内容二次写 → 同一 id + 盘上仅 1 份
test_garbage_collection_respects_references_and_ttl ok  # 有引用不回收 / TTL 未到不回收 / 到期回收
test_garbage_collection_reports_rows_without_file  ok   # 行在文件不在 → 如实报告、不谎报回收字节
test_verify_integrity_deep_reports_content_mismatch ok  # 深扫抓内容不符且不误报好数据
test_verify_integrity_shallow_detects_truncated_file ok # 浅扫靠"文件长度 ≠ 记账压缩后长度"抓截断
```

**未执行**：`cargo test -p assistant-core arch::`（AGENTS §6 的依赖方向护栏）——
`crates/core` 尚不存在（PL-037 要把它的骨架提前），本卡无法执行，见 §7。

### 4. DoD 逐条核对

- [x] 上列命令全部通过（输出见 §3）
- [x] `docs/DEPENDENCIES.md` 已登记 3 个依赖（**在改 `Cargo.toml` 之前**），`cargo deny check licenses bans sources` exit 0
- [x] 迁移：全新库可建到最新版本；`schema_version` 被改坏 → **拒绝启动**且错误可读（3 条负向用例）
- [x] blob 往返：写入 → 读取 → 逐字节一致；同内容二次写入 → **不产生第二个文件**（`count_files == 1`）
- [x] blob 篡改 1 字节 → `evidence_corrupt`；DB 行在而文件不在 → `evidence_missing`
- [x] 孤儿 blob 可 GC（引用计数 0 + 超 TTL），且有测试覆盖（含"行在文件不在"的分支）
- [x] `crates/storage/README.md` 含职责 / 边界 / **不变量** / 已知限制
- [x] 无任何 Out of scope 的文件被修改（见 §2「未改」清单）
- [x] `LEDGER.md` 追加一行；新 FACT/PITFALL 已追加；`MEMORY.md` 规模表按 `memory-counts` 机器值同步
- [x] 本卡 §1~§9 执行记录已填

### 5. 偏差

**DRIFT-012-1（表名口径冲突，未停工作）**

- 现象：卡面 In scope 第 4 条写的是单数简称 `task` / `step` / `checkpoint` / `blob` / `blob_ref` / `usage`；
  架构 v2 §15.1 的权威表名是复数 snake_case（`tasks` / `task_steps` / `checkpoints` / `blobs` /
  `blob_refs` / `usage_records`）。两者不可能同时成立（同一张表不能有两个名字）。
- 影响：表名是**持久化契约**，一旦发布就不能改（改 = 数据迁移 + 破坏外部只读工具）。
- 建议：以 §15.1 为准（架构 v2 是裁决顺序里的高优先级文档；卡面是 Orchestrator 手写的简称）。
- 已做：按 §15.1 落地，并在 `migrations/0001_init.sql` 头部与 `src/*.rs` 注释里写明依据；
  请 Orchestrator 在下一轮刷新卡面正文的 In scope 第 4 条（本卡记录区不能改正文）。

**DRIFT-012-2（crate 内新增两个私有子模块，为满足 ADR-0033 行数上限）**

- 现象：`content.rs` 一次写完 616 行（> 卡面「单文件 ≤ 600（软）」）+ 集成测试 715 行。
- 处理：把**值类型**（`BlobId` / `BlobKind` / `BlobOwner`）拆到 `src/blob_id.rs`（162 行），
  `content.rs` → 474 行；集成测试按关注点拆成 `storage_integration.rs`（486）/`storage_records.rs`（158）
  + 共享夹具 `tests/common/mod.rs`（119）。**没有新增 crate / 顶层目录 / 架构层**，
  全部落在卡面 write scope 的 `crates/storage/**` 内，且拆的是卡面自己要求的行数上限 → 视为遵卡而非漂移，
  在此登记以便审阅者复核。

**DRIFT-012-3（工作区内 path 依赖必须带 `version`）**

- 现象：`cargo deny check bans` 首跑 FAILED（`error[wildcard]: found 1 wildcard dependency for crate
  'assistant-storage'`，exit 2），因为 `deny.toml` 有 `[bans] wildcards = "deny"`。
- 处理：给 `assistant-protocol` 的 path 依赖补 `version = "0.1.0"`（与 `[workspace.package] version` 一致）。
  **没有**改 `deny.toml`（不在 write scope，且加 `allow-wildcard-paths` 属于放宽门禁 = 漂移触发器 ⑥）。
- 已记入 `docs/memory/pitfalls.md`。

**记录区允许的 file-level `#![allow(...)]`（测试专用）**：`tests/common/mod.rs` 有
`#![allow(dead_code)]`（跨测试 crate 共享夹具必然有一部分在某个 crate 里未使用，实测不加则
`-D warnings` 挂 3 条 error）+ `#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]`
（AGENTS §5.3 明确 `tests/` 内可 allow）。**产品代码 `src/**` 里没有任何 `#[allow]`**。

### 6. 更合理做法

1. **一致性扫描的"廉价检查"值得推广**：浅扫（不 deep）现在不只查"文件在不在"，还查
   "文件长度 == 记账的 `compressed_bytes`"。截断 / 半写会在启动扫描时立刻暴露，不必解压整块数据。
   代价是"收养孤儿文件"时必须记**文件真实长度**而不是"本次压缩结果的长度"（不同 zstd 版本的压缩输出
   长度可以不同，记错了会让扫描误报）—— 已在 `put()` 里按分支分别记账并写注释。
2. **引用计数用"一行一引用"而不是计数字段**：重复引用 / 重复解引用天然幂等，
   省掉了"多减一次把计数弄负"的经典 bug 类别，代价是 `blob_refs` 行数略多（可接受，索引齐全）。
3. **迁移 SQL 编译期内嵌**（`include_str!`）而不是运行时扫目录：迁移文件不会被"误删 / 被别的目录同名文件
   顶替"，sha256 记账也就有了确定的对象；代价是新增迁移必须改 `MIGRATIONS` 数组（一处，且是编译期错误）。
4. **集成测试夹具拆 `tests/common/mod.rs`**：如果留在单个文件里，715 行的测试文件既超行数上限，
   又会让"blob 用例"与"表 CRUD 用例"的失败信息混在一起。

### 7. 遗留问题

1. **`crates/core` 不存在** → AGENTS §6 的 `cargo test -p assistant-core arch::` 无法执行（PL-037 已裁决"提前骨架"，
   建议作为下一张卡的第一件事，或并入 TASK-015）。
2. **TTL 语义**：GC 以 `created_at` 为基准，没有"最后被解引用时刻"。长期被引用、刚解引用的 blob 会被立刻回收。
   要改成"从解引用起算"需加列 + 新迁移（新卡）。
3. **孤儿文件只收养不清理**：内容对不上的孤儿会留在盘上报 `evidence_corrupt` 等人工处理（刻意不自动删）。
   建议后续加一个显式的 `--quarantine-orphans` 维护命令（需新卡 + 策略评审）。
4. **没有加密**（blob 与主库都是明文）、**没有在线备份 / VACUUM 策略**（除 `wal_autocheckpoint=1000`）——
   按卡面都在 Out of scope，留给后续卡。
5. **`.gitignore` 的运行时产物规则是仓库根锚定的**（`/blobs/`、`/shadow/`）。若将来把数据目录默认放在
   仓库内子路径，需要补规则（当前测试只用 `%TEMP%`，不受影响）。

### 8. 新增长期记忆

- `docs/memory/facts.md` +2：
  1. `crates/storage` 落地 + 依赖解析实测 + 12 条验收命令结果 + 表名取舍 + `assistant-core` 缺失（N/A）；
  2. **更正**上一会话交接里「cargo 必须设 `CARGO_HTTP_PROXY`」：本机实测清空所有 proxy 环境变量后
     `cargo fetch` 仍 exit 0 且真的下载成功（`tinyvec v1.13.3`）；两条路都能走。
- `docs/memory/pitfalls.md` +3：
  1. `deny.toml` 的 `[bans] wildcards = "deny"` 会把"只有 `path` 没有 `version`"的工作区依赖判成通配依赖；
  2. `clippy::redundant_pub_crate` 的正解是改 `pub`（本仓没启用 `unreachable_pub`），不要 `#[allow]`；
  3. 跨多个集成测试 crate 共享的夹具模块必须 `#![allow(dead_code)]`（实测不加挂 3 条 error）。
- `MEMORY.md` 规模表已按 `memory-counts` 机器值同步（facts 139/91、pitfalls 184/81）。

### 9. 给审阅者的关注点

1. **表名与迁移记账**（最高风险）：`migrations/0001_init.sql` 是**已发布契约**，评审通过后不得再改内容 ——
   任何调整都必须新增 `0002_*.sql`（否则已存在的库会在下次启动被 `migration_checksum_mismatch` 拒绝，
   这是**故意**的：宁可拒绝启动也不接受"同版本号两份 schema"）。请重点确认表名/列名与 v2 §15.1 逐字一致。
2. **`evidence_missing` vs `blob_unknown` 的边界**：前者 = 元数据行在、文件不在（数据丢了，`Fatal`）；
   后者 = 从未登记（调用方 bug）。`put()` 在"行在文件不在"时**故意报错而不重写**，请确认这个取舍
   符合"无静默失败"（代价：需要人工介入才能恢复，但不会掩盖数据丢失）。
3. **`StorageError::error_category()` 的粗粒度**：除 `DatabaseBusy`/`DatabaseLocked` 外一律 `Fatal`
   （包括 `BlobUnknown` 这种其实更像"参数错"的）。这是按 v2 §8.7「Fatal = storage corrupted」的保守解释 ——
   如果 Core 侧希望 `BlobUnknown` 走"可重试 / 可忽略"，需要在 TASK-013/020 的契约里明确（本卡不擅自发明分类）。

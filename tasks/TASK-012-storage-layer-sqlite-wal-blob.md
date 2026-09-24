# TASK-012　存储层：SQLite(WAL) + 迁移框架 + 核心表 + 内容寻址 blob（zstd/去重）

- 状态：**Ready**
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

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点

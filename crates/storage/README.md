# assistant-storage crate (TASK-012)

> 阶段 1A1 基础设施：**持久化落点**。四层存储里的 L1（SQLite/WAL 主库）与 L2（内容寻址 blob 池）。

## 职责

- **L1 主库**：连接 + PRAGMA 基线 + **只前进不回滚**的迁移框架 + 核心表
  （`tasks` / `task_steps` / `checkpoints` / `blobs` / `blob_refs` / `usage_records`；`audit_logs` 由迁移 `0002` 建，见下）
- **L2 blob 池**：zstd（level 3）压缩 + `sha256` 内容寻址 + 去重 + 引用计数 + GC + 一致性扫描
- 提供**注入点**：`Clock`（时间）与 `StoragePaths`（数据目录根）

## 边界（不做什么）

- 不含业务规则：状态机、重试、预算、撤销锚点管理、审计 hash chain 都**不**在这里
- **`audit_logs` 的 DDL 走本 crate 的迁移链**（`migrations/0002_audit_logs.sql`，TASK-013），但该表的**语义与读写归 `crates/audit`**（本 crate 不碰 hash chain）；不建 FTS5 记忆表（TASK-028）、不实现影子副本（W5）的写入策略
- 不调用任何平台 API（`arch` 护栏会拦）；不提供多写者 / 只读连接池
- 不做加密 / SQLCipher、冷归档 L3、在线备份 CLI
- 除 `rusqlite`(bundled) / `zstd` / `sha2` 外不引第三方依赖（登记见 `docs/DEPENDENCIES.md`）

## 不变量

1. **无静默失败**（铁律 1）：schema 版本不符 / blob 校验失败 / 行与文件不一致 —— 一律报错，
   绝不"用默认值继续跑"，也绝不把"行在文件不在"当成"没有这个 blob"
2. **只有 Core 持写连接**（`docs/storage-design.md` §7）；本 crate 不提供多写者并发入口
3. **只前进不回滚**：任何结构调整都新增 `migrations/000N_*.sql`，**不改已发布文件**；
   已应用的迁移按 sha256 记账，事后改动 → 启动直接拒绝
4. `blob_id` **就是**内容 sha256 的小写 hex（64 字符）；同内容 ⇒ 同 id ⇒ 只存一份；
   **每次读取都重算 sha256**
5. 引用计数 = `blob_refs` 的行数（一行一个引用 ⇒ 重复引用 / 重复解引用天然幂等）
6. GC 只删"引用数 0 **且** 创建时间早于 TTL"的 blob；删除顺序是"先文件后行"，
   文件删失败则该行**不**删（保持"文件与行同进同退"）
7. 时钟 / 数据目录根**注入**（`Clock` / `StoragePaths`）：生产用 `SystemClock`，
   测试用固定时钟 + 临时目录即可回放
8. 运行时产物（`*.db` / `-wal` / `-shm` / `blobs/` / `shadow/`）**绝不**落在仓库内
   （仓库根 `.gitignore` 已覆盖；测试只用 `%TEMP%`）

## 典型用法

```rust
use std::sync::Arc;
use assistant_storage::{BlobKind, BlobOwner, Database, StoragePaths, SystemClock};

let paths = StoragePaths::new("D:/data/assistant");
let database = Database::open(&paths, Arc::new(SystemClock))?;
let blobs = database.blob_store();

let id = blobs.put(database.connection(), BlobKind::TreeSnapshot, b"{}")?;
blobs.add_reference(database.connection(), &id, &BlobOwner::new("step", "s_1")?)?;
assert_eq!(blobs.get(database.connection(), &id)?, b"{}");
# Ok::<(), assistant_storage::StorageError>(())
```

目录布局：

```text
<root>/assistant.db            L1 SQLite 主库（WAL 下还有 -wal / -shm）
<root>/blobs/<前 2 位>/<sha256>  L2 内容寻址 blob 池（256 个子目录）
<root>/shadow/<task_id>/       W5 影子副本（本卡只建目录，写入策略归后续卡）
```

## 已知限制

- **TTL 以 `created_at` 为基准**：目前没有"最后被解引用时刻"这一列，所以一个长期被引用、
  刚刚解引用的 blob 只要创建得够早就会被立刻回收。要改成"从解引用起算"需加列 + 迁移（新卡）
- **孤儿文件只"收养"不清理**：写盘成功但记账失败会留下孤儿文件；下次 `put` 同内容会校验后
  收养它。**内容对不上的孤儿会报 `evidence_corrupt` 并留在盘上**等人工处理（刻意不自动删）
- **`evidence_missing` / `evidence_corrupt` 是 `reason_code`，分类仍是 `Fatal`**：
  细粒度在 `reason_code()`，粗粒度分类不发明新枚举（与 `docs/storage-design.md` §8 一致）
- **没有加密**：blob 与主库都是明文；密钥/加密归后续卡
- **没有在线备份 / VACUUM 策略**：`wal_autocheckpoint=1000` 之外不做维护
- **单写连接**：本 crate 只给一个写连接；读连接池归后续的 Core 装配

## 相关文档

- `docs/storage-design.md`（§3.2 PRAGMA、§3.3 blob、§4 表映射、§7 并发、§8 一致性）
- `cross-platform-ai-assistant-architecture-v2.md` §15
- `docs/spec/error-codes.md`（错误分类）
- `tasks/TASK-012-storage-layer-sqlite-wal-blob.md`（本卡正文 + 执行记录）

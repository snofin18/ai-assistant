# 数据存储方案推演

> 版本：1.0　日期：2026-09-16　上位：架构 v2.2 §15（本文为其推演依据）
> 决策状态：**待 Spike H（TASK-009）实测确认**；本文给出方案、理由、预算与验证方法。
> 目标：① 选对存储方式 ② **加速运行时读写**（用户明确要求）③ 保证一致性与可恢复性 ④ 可人工检查、可备份、可脱敏导出

---

## 1. 需求推演：先分清 workload，再选存储

把"存储"当单一问题会导致错误选型。本项目的数据实际上是**五种完全不同的 workload**：

| # | 数据类型 | 写模式 | 读模式 | 单条大小 | 延迟要求 | 量级 |
|---|---|---|---|---|---|---|
| W1 | 任务/步骤状态、检查点 | 每步 1 次批量写 | 恢复时批量读、UI 分页读 | 1~20 KB | 写 < 10 ms | 万级 |
| W2 | 审计事件 | **高频追加**（每步多条） | 时间线范围扫描、按 task_id 查 | 0.5~3 KB | 写摊销 < 1 ms | 十万~百万级 |
| W3 | 策略/授权/Adapter/App Map/Capability | 极低频写（人工编辑） | **每次工具调用都读** | 1~200 KB | **读 < 50 µs** | 十~百级 |
| W4 | UI 树快照、截图 | 低频大写（每步或失败时） | 回放、时间线展示、审计取证 | 100 KB~5 MB | 写 < 30 ms | 万级，**占磁盘大头** |
| W5 | 影子副本（文件备份）、导出物 | 中频，可能很大 | 回滚时读一次 | KB~数百 MB | 不敏感 | 百级 |
| W6 | 用量/成本记录 | 高频小写 | 聚合查询（日/月/按模型） | < 0.5 KB | 可批量 | 百万级 |
| W7 | 记忆（任务历史检索） | 低频 | **全文检索** | KB | 检索 < 50 ms | 万级 |
| W8 | selector 统计（自愈学习） | 中频小写（每次定位） | 解析时读 | < 0.5 KB | 读 < 1 ms | 万级 |

**关键推论**：

1. **W3 绝不能走磁盘 IO**：它在每次工具调用的热路径上 → 必须**进程内内存化**，磁盘只是它的持久化形态（且应是**人类可编辑的文件**，因为 Adapter/App Map 需要人工维护）。
2. **W4 绝不能塞进关系库主表**：单条最大 5 MB、总量占磁盘 90%+，且高度重复 → 必须**外置 + 内容寻址去重 + 压缩**。
3. **W2/W6 是追加型高频小写** → 需要**批量 flush**，否则 fsync 会成为瓶颈。
4. **W1 需要事务与恢复语义** → 关系库最合适。
5. **W7 需要全文检索** → FTS5，不必上向量库。
6. **W5 本来就是文件** → 直接文件系统，DB 只存元数据与哈希。

---

## 2. 选型评估

| 方案 | 优点 | 缺点 | 结论 |
|---|---|---|---|
| **SQLite（WAL）** | 零运维、单文件、完整事务、FTS5、跨平台一致、Rust 生态成熟（`rusqlite`/`sqlx`）、可 SQLCipher 加密、在线备份（`.backup`/`VACUUM INTO`）、**可人工检查**（DB Browser/CLI） | 单写者（写串行）；大 blob 不擅长 | ✅ **W1/W2/W6/W7/W8 主存储** |
| redb / sled（纯 Rust KV） | 写性能好、无 SQL 开销 | 无 SQL/JOIN/FTS、生态小、sled 长期 beta、**人工检查困难**、迁移工具需自建 | ❌ 不选（AI 协作项目里"人能直接读懂存储"是重要价值） |
| LMDB | mmap 读极快 | 单写者、无 SQL、需自建索引与迁移 | ❌ 收益不明显（W3 已内存化，读快慢无关） |
| SurrealDB / 嵌入式文档库 | 灵活查询 | 体积与复杂度、事务与生态成熟度不如 SQLite | ❌ 过重 |
| PostgreSQL 等外部服务 | 能力强 | **桌面应用不应要求用户装数据库** | ❌ 排除 |
| 纯文件（JSON/TOML） | 人类可编辑、可 diff、可 git 跟踪 | 无事务、无索引、并发写危险 | ✅ **仅用于 W3（Adapter/App Map/策略/配置）** |
| 文件系统 + 内容寻址 | 去重、不可变、易校验 | 无查询能力 | ✅ **用于 W4/W5，DB 存元数据** |

**SQLite 写串行的影响评估**：本项目**只有 Core 一个写者**（见 §7），写操作是"每步一次检查点 + 批量审计 flush"，实测预算（§6）远低于 SQLite WAL 的能力（WAL 下小事务可达数千 TPS）→ **不构成瓶颈**。

---

## 3. 四层存储架构

```text
┌─ L0 进程内热缓存（内存）──────────────────────────────────────────────┐
│ PolicySet（预编译规则匹配树） │ Permissions │ Adapters + AppMaps      │
│ CapabilityMatrix │ 活跃 Task/Step 状态 │ TargetDescriptor 解析缓存      │
│ · 启动加载 → 运行期零 IO     · 文件 watcher 触发失效与重载            │
│ · 写透（write-through）到 L1/L2 文件                                  │
└──────────────────────────────────────────────────────────────────────┘
┌─ L1 SQLite（WAL, synchronous=NORMAL）────────────────────────────────┐
│ tasks / task_steps / permissions / policy_rules / audit_logs /       │
│ usage_records / selector_stats / undo_anchors / bound_targets /      │
│ conversations / model_configs / capability_snapshots / evidence(元数据)│
│ + FTS5: memory_fts（任务历史与偏好检索）                              │
│ · 单写连接（串行化，避免 SQLITE_BUSY）+ N 只读连接（UI/查询）          │
│ · 审计与用量：ring buffer → 定时(200ms)/定量(100条) flush            │
└──────────────────────────────────────────────────────────────────────┘
┌─ L2 内容寻址 blob 存储（文件系统）───────────────────────────────────┐
│ blobs/<前2位>/<sha256>            （zstd 压缩，去重）                 │
│ shadow/<task_id>/<原文件名>.<ts>  （影子副本，W5）                    │
│ · SQLite 只存 blob_id + 元数据（kind/bytes/compressed_bytes/created） │
│ · 树快照优先存「相对上一步的 diff」，全树仅在需要时存                  │
└──────────────────────────────────────────────────────────────────────┘
┌─ L3 冷归档（可选）───────────────────────────────────────────────────┐
│ archive/YYYY-MM.zst.tar  超期审计与证据打包归档；导出供合规审查        │
└──────────────────────────────────────────────────────────────────────┘
```

### 3.1 L0：热缓存的设计要点（**这是"加速运行时读写"的最大来源**）

| 数据 | 加载 | 失效 | 备注 |
|---|---|---|---|
| 策略规则 | 启动时解析 TOML/DSL → **编译成决策树/索引**（按 tool 名、risk、effect 建哈希桶） | 文件变更 / UI 修改 | 判定路径无字符串比较、无正则回溯 → 目标 < 50 µs |
| 授权（permissions） | 启动加载未过期项 → 内存 map（按 subject+tool+scope） | TTL 到期由定时任务清理 | 判定 O(1) |
| Adapter + App Map | 启动加载全部已启用 Adapter 的 TOML/JSON → 反序列化为结构体 | 文件 watcher | **按 app_id 索引**；App Map 片段按"当前目标应用"选择性注入模型上下文（不是全量） |
| CapabilityMatrix | 探测后缓存 + TTL（默认 5 min）+ 关键事件触发重探（会话解锁、portal 授权变化、应用启动） | TTL / 事件 | 重探是异步的，不阻塞执行 |
| 活跃 Task/Step | 内存为权威副本，DB 是检查点 | 任务结束 | **热点状态不逐次落库**，仅每步一次检查点 |
| TargetDescriptor 解析结果 | Host 内缓存 element + runtime_id + 指纹 | 指纹不符 / 调用失败 | v2 §6.1「handle 是缓存不是身份」 |

**写透规则**：内存改动必须同步（或在一个事务内）落盘，禁止"只改内存"——否则崩溃后策略/授权状态与审计不一致。

### 3.2 L1：SQLite 配置

```sql
PRAGMA journal_mode = WAL;           -- 读写不互斥
PRAGMA synchronous = NORMAL;         -- WAL 下的安全/性能平衡点
PRAGMA busy_timeout = 5000;          -- 兜底，正常不应触发（单写者）
PRAGMA foreign_keys = ON;
PRAGMA mmap_size = 268435456;        -- 256 MB，加速大范围扫描
PRAGMA page_size = 8192;             -- 建库时设置；利于大行与扫描
PRAGMA cache_size = -65536;          -- 64 MB 页缓存
PRAGMA temp_store = MEMORY;
PRAGMA wal_autocheckpoint = 1000;    -- 页；避免 WAL 无限增长
```

- **审计表可单独放一个 DB 文件**并设 `synchronous = FULL`（若合规要求每条审计都不可丢），主库保持 `NORMAL`。这是"性能 vs 耐久性"的显式取舍，配置项：`audit.durability = batched | immediate | separate_db_full`。
- 迁移：**自建**（不引 `sqlx` / `refinery`；口径见 **ADR-0038**）—— 只前进不回滚、**编译期内嵌**（`include_str!`）、sha256 记账；**每个拥有表的 crate 声明自己的迁移**（版本号登记表见 §3.4）；启动校验「库已应用的版本集合 ⊆ 装配后的 `MigrationSet`」且不高于 `expected_version()`，不符则拒绝启动并提示（避免静默数据损坏）。

### 3.3 L2：blob 存储

```text
写入：content → zstd(level 3) → sha256 → blobs/ab/cdef...  → 返回 blob_id
      （若 blob_id 已存在 → 直接复用，写引用计数 +1）
读取：blob_id → 读文件 → 解压 → 校验 sha256（不匹配 = 损坏，报 evidence_corrupt）
GC  ：引用计数为 0 且超过 TTL → 删除；后台低优先级任务，可暂停
```

- **去重收益推演**：UI 树快照在连续步骤间高度相似，且同一应用同一状态的快照会重复出现 → 内容寻址天然去重；配合"存 diff 而非全树"，预计磁盘占用降低 **一个数量级以上**（Spike H 实测确认，见 `MEMORY.md` §6 ASSUMPTION）。
- **压缩级别选择**：zstd level 3 是速度/压缩比的最佳折中；快照类可试 level 6~9（写入不频繁）。
- **目录分片**：sha256 前 2 位分片（256 个子目录），避免单目录百万文件。
- **影子副本（W5）不进 blob 池**：因为需要按原路径/原文件名快速恢复，且可能很大 → 单独 `shadow/<task_id>/` 目录 + DB 元数据 + TTL。

### 3.4 迁移登记表（版本号分配的 SSOT）

**口径**（**ADR-0038**）：`crates/storage` 只提供**迁移机制**（`Migration` / `MigrationSet` /
`Database::open` 的**必填**迁移集参数），**不拥有**表清单；每张表的 DDL 与它**拥有者 crate** 同处
（`crates/<owner>/migrations/NNNN_<slug>.sql`），由拥有者公开 `pub const MIGRATIONS: &[Migration]`。应用侧在**唯一装配点**
合并后开库。**加一张表 = 只改自己那个 crate。**

版本号**全局唯一、从 1 连续**（`MigrationSet::register` 拦重号、`validate()` 拦缺号）。本表是
**版本号分配的单一事实源** —— 新增迁移**先在本表占号**，再写 SQL。

| 版本 | 拥有者 crate | 迁移文件 | 建出的表 / 对象 |
|---|---|---|---|
| 0001 | `crates/storage` | `crates/storage/migrations/0001_init.sql` | `tasks` / `task_steps` / `checkpoints` / `blobs` / `blob_refs` / `usage_records` |
| 0002 | `crates/audit` | `crates/audit/migrations/0002_audit_logs.sql` | `audit_logs` + `idx_audit_ts` + 两个 append-only 触发器 |

> **为什么 0002 在 `crates/audit` 而不是 `crates/storage`**：TASK-013 曾把它放在
> `crates/storage/migrations/`（当时迁移链没有外部入口）→ 违反「DDL 与拥有者同处」。
> TASK-202 按 ADR-0038 把文件**移动**过去（内容一字不改 → sha256 checksum 不变 → 已有库仍可打开）。

> **已知遗留（PL-047）**：本表仍是**手工回填**的 —— ADR-0030 的教训是「靠记得回填的护栏会失效」。
> 机器化（xtask 扫描 `crates/*/migrations/*.sql`，校验号段唯一 + 与本表一致）归 **TASK-015**
> （它才拥有 gov §5.4 规则计数与 ADR-0025 / ADR-0030 的口径）。

---

## 4. 数据到存储的映射（对应 v2 §15.1 的表）

| 表 | 层 | 说明 |
|---|---|---|
| `conversations` / `tasks` / `task_steps` | L1 | 状态与检查点；`plan_json` 若 > 64 KB 则外置到 blob |
| `audit_logs` | L1（可独立库） | 追加不可改 + `prev_hash`/`hash`；`detail_json` 大字段外置 blob |
| `usage_records` | L1 | 批量写；按月分区视图或定期归档到 L3 |
| `permissions` / `policy_rules` | L1 + **L0 缓存** | 热路径 |
| `registered_apps` / `adapters` / `bound_targets` | L1 + L0 | Adapter 本体是**文件**（`adapters/*/adapter.toml`），DB 只存索引与健康度 |
| `selector_stats` | L1 | 自愈学习数据；批量写（每步一次合并更新，不每次定位都写） |
| `undo_anchors` / `shadow_copies` | L1 元数据 + **L2 内容** | `content_ref` 指向 blob；`shadow_path` 指向影子目录 |
| `evidence` / `tree_snapshots` | L1 元数据 + **L2 内容** | 快照存 diff；截图存原图（已脱敏） |
| `model_configs` | L1 + L0 | **密钥只存 `key_ref`**，实体在 OS keychain（v2 §12.5） |
| `capability_snapshots` | L1 + L0 | 带 TTL |
| `memory_fts`（FTS5 虚表） | L1 | 任务历史、偏好、领域笔记的全文检索 |
| App Map / Adapter / 策略 / 出域配置 | **文件（TOML/JSON）** | 人类可编辑、可 diff、可进 git（`adapters-private/` 除外） |

---

## 5. 运行时读写加速措施汇总（对应用户诉求）

| # | 措施 | 针对 | 预期收益 | 风险与对策 |
|---|---|---|---|---|
| 1 | L0 全量内存化 + 规则预编译 | W3 | 策略判定 < 50 µs（相对每次查库 ~0.5 ms，**提升约 10~100 倍**） | 内存占用上升（Adapter 多时）→ 按启用状态懒加载 |
| 2 | WAL + `synchronous=NORMAL` | W1/W2/W6 | 写吞吐提升约一个数量级 | 断电可能丢最后若干事务 → 审计可单独 `FULL`；检查点每步一次保证可恢复 |
| 3 | 审计/用量 ring buffer 批量 flush（200 ms 或 100 条） | W2/W6 | 消除每步多次 fsync | 崩溃丢最近 200 ms → 高风险动作用 `immediate` 模式（Tool 声明 `audit.durability`） |
| 4 | 热点状态不逐次落库，仅每步检查点 | W1 | 单步写放大从 N 次降到 1 次 | 崩溃可能丢当前步中间态 → 由幂等判定 + `NeedsHuman` 兜底（v2 §8.5） |
| 5 | blob 内容寻址 + 去重 + zstd | W4 | 磁盘与 IO 双降（预计 ≥ 5x） | 写前需算 sha256（1 MB 约 2~3 ms）→ 可接受 |
| 6 | 树快照存 diff 而非全树 | W4 | 体积再降一个量级 | 回放需重建 → 定期存"全树基帧"（如每 20 步） |
| 7 | 写连接单一 + 只读连接池 | W1/W2 vs UI 查询 | UI 滚动时间线不阻塞执行 | 只读连接需处理快照隔离（WAL 下天然支持） |
| 8 | `mmap_size` + `page_size=8192` + `cache_size` | 范围扫描 | 时间线/审计查询加速 | 内存占用上升 → 设上限 |
| 9 | 索引按查询模式建，且**不过度建索引** | W2/W6 | 查询快且写不被拖累 | 每次加索引需评估写放大 |
| 10 | FTS5 而非自研/向量库 | W7 | 免维护、检索 < 50 ms | 语义检索能力弱 → 阶段 4 后再评估本地嵌入 |
| 11 | Adapter/App Map 走文件 + watcher | W3 | 人类可编辑、git 可跟踪、改动即时生效 | 文件损坏需 schema 校验拦截（`verify-schemas`） |
| 12 | 模型侧 prompt cache 提示（稳定前缀） | 非存储，但同源 | token 成本降一个量级 | 前缀变动会失效 → 系统提示与工具集尽量稳定 |
| 13 | 只读工具结果短 TTL 缓存 | 模型调用 | 减少重复读取与 token | 必须声明 `cacheable` 与 TTL，写操作禁止缓存 |

---

## 6. 性能预算与基准方法（Spike H 实测）

### 6.1 预算（超标即缺陷）

| 操作 | 预算 | 测量方法 |
|---|---|---|
| 策略判定（L0） | < 50 µs | 1 万次随机请求取 p99 |
| Adapter/App Map 查询（L0） | < 10 µs | 同上 |
| 单条审计写（batched 摊销） | < 1 ms | 写 1 万条测总时长 |
| 单条审计写（immediate） | < 8 ms | 写 200 条测 p99 |
| 检查点写（Task+Step+指纹） | < 10 ms | 1000 次取 p99 |
| blob 写（1 MB 树快照，含 zstd） | < 30 ms | 100 次取 p99 |
| blob 读（1 MB，含解压+校验） | < 20 ms | 同上 |
| 时间线分页查询（1000 步 + 证据元数据） | < 100 ms | 造 10 万步数据后测 |
| FTS 记忆检索 | < 50 ms | 造 1 万条记录后测 |
| selector 统计批量更新 | < 5 ms | 每步一次 |
| **冷启动**（加载全部配置 + 恢复活跃任务） | < 500 ms | 5 个 Adapter + 1 个进行中任务 |
| 并发（1 写 + 4 读）下的写延迟劣化 | < 2x | 持续 60 s |

### 6.2 数据造景（基准必须用真实形态的数据）

- 树快照样本：从 Spike A/A2/G 采集真实记事本、画图、Edge 页面的 UI 树/DOM 快照（**脱敏后**入库作为 fixture）
- 审计量：造 10 万条事件（模拟 3 个月使用）
- 任务量：造 1 万个任务、10 万个步骤
- blob 量：造 5000 个快照（含高重复率，验证去重收益）

### 6.3 对比实验（可选，时间允许时做）

同 workload 下 SQLite(WAL) vs redb：量化差距。若 redb 在 W2 上有 > 3x 优势，可考虑审计独立库用 KV；否则维持 SQLite（**可人工检查的价值优先**）。

---

## 7. 多进程与并发访问策略

**规则：只有 Core 持有写连接。** UI 与 Host **不得**打开 SQLite 写连接。

| 进程 | 访问方式 |
|---|---|
| `assistant-core` | 1 个写连接（串行化所有写）+ 内部只读连接池 |
| `assistant-ui` | 优先经 IPC 向 Core 请求；**大量只读查询（时间线滚动）可用只读连接**（`SQLITE_OPEN_READONLY` + WAL），避免 IPC 传输大结果集 |
| `automation-host` | **不访问 DB**；一切经 IPC（保持 host 无状态、易重启） |
| `mcp-server-*` | **不访问 DB** |
| `evaluator` | 只读连接（离线分析） |

**推演两个真实并发场景**：
1. UI 正在滚动时间线（大量只读分页查询）同时 Core 在写审计与检查点 → WAL 下读不阻塞写、写不阻塞读；只读连接看到的是快照，可能出现"刚写的步骤还没显示" → UI 需要主动刷新（Core 推送事件），**不要靠轮询**。
2. 两个 agent 任务并行（阶段 2 之后可能）→ 仍是单写连接串行化，租约在内存 + DB 双记录；**DB 是唯一仲裁者**（内存租约崩溃后失效）。

---

## 8. 一致性、崩溃与恢复

| 场景 | 期望行为 | 实现 |
|---|---|---|
| 写 DB 中途崩溃 | 事务原子性保证不留半成品 | SQLite 事务 + WAL |
| blob 写成功但 DB 未记录 | **孤儿 blob** | GC 按引用计数清理；启动时做一次一致性扫描（低优先级） |
| DB 记录了但 blob 缺失/损坏 | 必须检测并标记 | 读取时校验 sha256；缺失 → `evidence_missing`，损坏 → `evidence_corrupt`；**不得静默忽略**（铁律 1） |
| 影子副本被用户手工删除 | 回滚不可用 | 回滚前校验副本存在与哈希；不可用则按 incident 处理（v2 §9.8） |
| 审计 hash chain 断裂 | 可能被篡改 | 启动时校验最近 N 条；断裂则告警并锁定为只读，要求人工介入 |
| schema 与二进制不匹配 | 拒绝启动 | 启动校验 `schema_version`，提示升级/降级路径 |
| WAL 文件异常增长 | 磁盘耗尽 | `wal_autocheckpoint` + 定期 `PRAGMA wal_checkpoint(TRUNCATE)`（维护窗口） |

---

## 9. 加密、隐私与容量治理

| 项 | 方案 |
|---|---|
| DB 加密 | 默认**不加密**（性能与可检查性优先）；敏感环境可启用 SQLCipher（预计 5~15% 性能损失，需重新测预算） |
| blob 加密 | **截图与树快照才是敏感大头** → 提供"blob 单独加密"选项（对 blob 内容加密后再算 sha256 会导致去重失效 → 采用"加密前哈希做 id、加密后存储"，去重仍有效，但需密钥一致） |
| 隐私模式 | 全局开关：不保存截图、不保存树快照、审计只存摘要（参数哈希） |
| 保留策略 | 截图/快照默认 7 天或 500 MB 上限（先到为准）；影子副本任务确认后 7 天；审计长期保留 + 容量轮转 + L3 归档 |
| 清理任务 | 后台低优先级、可暂停、**用户操作时自动让路**（避免清理拖慢执行） |
| 导出 | 一键导出任务报告（含证据）供审计/协作；导出前必须走脱敏（为将来开源准备，v2 §18.1） |
| 数据目录 | 用 `dirs` crate 定位平台标准数据目录；**仓库内不得出现运行时数据**（`.gitignore` 已覆盖） |

---

## 10. 决策摘要（供 ADR 引用）

1. 主存储 **SQLite（WAL）**，不引入外部数据库服务，不使用纯 KV 库。
2. **四层**：内存热缓存（W3 与热点状态）/ SQLite（结构化 + FTS5）/ 内容寻址 blob（快照与截图，zstd + 去重）/ 冷归档。
3. **配置类数据用文件**（Adapter、App Map、策略、出域配置），人类可编辑、可 diff、可进 git。
4. **只有 Core 写库**；UI 可用只读连接；Host 与 MCP server 不碰 DB。
5. 审计与用量**批量 flush**，高风险动作可切 `immediate`。
6. 树快照**存 diff + 定期基帧**，blob **内容寻址 + 压缩 + 去重**。
7. 一致性异常（孤儿 blob、证据缺失/损坏、hash chain 断裂）**必须显式上报，禁止静默忽略**。
8. 全部性能预算由 **Spike H（TASK-009）实测确认**；超标即调整方案并走 ADR。

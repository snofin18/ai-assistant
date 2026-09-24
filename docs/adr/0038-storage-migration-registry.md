# ADR-0038　存储迁移注册表：各 crate 声明自己的迁移，storage 只提供机制

状态：**Accepted**（2026-09-24，人类 chat 裁决「现在就上正式机制」）　日期：2026-09-24　Supersedes：—　Superseded by：—

来源：**PL-046**（TASK-013 附带发现）；关联：`docs/storage-design.md` §3.2 / §3.4、架构 v2 §15.1 / §15.2、
`crates/storage/src/schema.rs`、`tasks/TASK-013-audit-append-hash-chain-flush.md` §5 DRIFT-013-1、
`tasks/TASK-202-storage-migration-registry.md`、ADR-0037 D1（治理池 200~299）。

---

## 背景（为什么现在要决定）

TASK-013 要给 `audit_logs` 建表时撞上一个**结构性**缺陷：`crates/storage/src/schema.rs` 把**全部**迁移
写成一个编译期内嵌常量（`MIGRATIONS` + `include_str!` + sha256 记账），**没有**任何「别的 crate 往
迁移链里加自己的表」的入口。四个后果：

| # | 后果 | 证据 |
|---|---|---|
| 1 | 加一张表必须改 storage 的**源码** | `MIGRATIONS` 常量与 `SCHEMA_VERSION` 都在 storage 内 |
| 2 | 加一张表必须改 storage 的**测试** | `crates/storage/tests/storage_integration.rs` 有 3 处写死「全库只有 1 个迁移」；TASK-013 实测 `15 passed / 3 failed` |
| 3 | 表的**所有权**与 DDL 的**位置**分离 | `audit_logs` 的语义 / 读写 / hash chain 全归 `crates/audit`，DDL 却必须躺在 `crates/storage/migrations/` |
| 4 | 抬 `SCHEMA_VERSION` 是**全局副作用** | 一个 crate 加表 → 所有打开主库的代码与测试都被牵动 |

TASK-013 当时按人类授权临时扩 write scope，把 `0002_audit_logs.sql` 塞进 storage 的常量（记
DRIFT-013-1），并把根因记成 **PL-046**。人类 2026-09-24 裁决：**现在就上正式机制**。

## 决策（一句话）

**storage 只提供机制、不再拥有表清单**：每个拥有表的 crate 用 `include_str!` 内嵌**自己**的迁移 SQL
并公开 `pub const MIGRATIONS: &[Migration]`；应用侧在**唯一装配点**把它们合并成一个 `MigrationSet`
再交给 `Database::open`。
**加表 = 只改自己那个 crate。**

拆成五条：

**D1　storage 的机制面（公共 API）**

| 项 | 语义 |
|---|---|
| `assistant_storage::Migration` | 一条迁移：`version` / `name` / `sql`；`Migration::new()` 是 `const fn`，SQL 由拥有者 `include_str!` 内嵌 |
| `assistant_storage::MigrationSet` | 有序集合；`register()` 拒绝**重复版本号**；`validate()` 要求 `1..=max` **连续**；`expected_version()` = 最高版本（空集 = 0） |
| `assistant_storage::MigrationSetError` | `DuplicateVersion` / `NonContiguousVersions`，各有稳定 `reason_code()` |
| `Database::open(paths, clock, &MigrationSet)` | **迁移集是必填参数**：不存在「默认只开 storage 自己那部分」的静默路径 |

**D2　拥有者 crate 的义务**

- 目录：`crates/<owner>/migrations/NNNN_<slug>.sql`（DDL 与拥有者同处）
- API：`pub const MIGRATIONS: &[Migration]`（`&'static`；装配点再做注册与校验）
- 测试：**自己**验证「用装配后的集合开一个空库 → 我的表 / 索引 / 触发器都在」+「我的迁移能在上一版本之上升级」
- 一句话：TASK-013 那种「改别人的测试」从此不再发生；义务落在**拥有者**身上（storage 自己的测试只装配自己的 0001）

**D3　唯一装配点**

- 装配 = 把各 crate 的 `MIGRATIONS` 依序 `register_all` 进同一个 `MigrationSet`（随后 `validate()`），再 `Database::open`
- **本轮落点**：`crates/audit/tests/common/mod.rs`（storage + audit）—— 它是当下唯一的跨 crate 消费者，
  同时也是机制的自证
- **正式落点**：Host 装配（TASK-019~028）。**不**放进 `crates/core`：TASK-201 的卡明确「骨架期不发明公共接口」，
  且 core 尚无 `storage` / `audit` 依赖边
- 漏注册**不是静默**：`Database::open` 对「库里存在本集合不认识的版本」仍然拒绝启动（`schema_version_mismatch`）

**D4　版本号分配 = `docs/storage-design.md` §3.4 登记表**

- 全局唯一、从 1 连续；登记表是 SSOT（`版本 | 拥有者 crate | 迁移文件 | 建出的表`）
- 新增迁移 = 先在登记表占号，再写 SQL；装配时 `register()` / `validate()` 会硬拦重号与缺号
- **本轮不做** xtask 的文件名扫描规则（那要动 gov §5.4 规则计数 + ADR-0025 / ADR-0030 的口径，归 TASK-015）
  → 记 **PL-047**

**D5　删除 `SCHEMA_VERSION` 常量**

- 原 `pub const SCHEMA_VERSION: i64` 是「storage 认为的全局最高版本」，在注册表下**必然**说谎
- 替代：`MigrationSet::expected_version()`（= 装配后的最高版本）
- 这是**删公共 API**（漂移 ⑫）：本 ADR 即授权；调用方改 `migrations.expected_version()`

## 考虑过的选项

| # | 方案 | 结论 | 理由 |
|---|---|---|---|
| 1 | 保持现状（storage 持有全表清单） | ❌ | 就是 PL-046：跨 crate 加表要改 storage 源码 + 测试；所有权与 DDL 位置分离 |
| 2 | **迁移注册表 + 唯一装配点**（本 ADR 采纳） | ✅ | 加表 = 只改自己 crate；机制与清单分离；无新依赖 |
| 3 | `linkme` / `inventory` 分布式切片（编译期自动注册） | ❌ | 引新第三方依赖（漂移 ①）；注册顺序隐式不可读；与「审计可回放」的确定性目标冲突 |
| 4 | 运行时扫描 `migrations/` 目录 | ❌ | 非确定性（源码树 vs 构建产物）、启动期 IO、回放不可复现（TASK-012 已否决同类做法） |
| 5 | 每个 crate 建自己的 DB 文件 | ❌ | 跨表 JOIN / 单事务 / 单写者模型全失效（`docs/storage-design.md` §7） |
| 6 | storage 用 feature 反向依赖各 crate | ❌ | 循环依赖（`audit → storage`），Cargo 直接拒绝 |

## 影响范围（本 ADR 实施时）

| 类别 | 文件 | 改动 |
|---|---|---|
| 代码 | `crates/storage/src/migrations.rs` | **新增**：`Migration` / `MigrationSet` / `MigrationSetError` |
| 代码 | `crates/storage/src/schema.rs` | 私有 `MIGRATIONS` 常量 → `pub const MIGRATIONS`（**只含自己的 0001**）；删 `SCHEMA_VERSION`；`apply_pending` 接 `&MigrationSet` |
| 代码 | `crates/storage/src/lib.rs` | `Database::open` 加第 3 个参数；导出新类型；模块文档 |
| 代码 | `crates/storage/migrations/0002_audit_logs.sql` | **移动**到 `crates/audit/migrations/0002_audit_logs.sql`（内容一字不改 → checksum 不变 → 已有库仍可打开） |
| 代码 | `crates/audit/src/lib.rs` | **新增** `pub const MIGRATIONS`（0002；SQL 从 `crates/audit/migrations/` 内嵌） |
| 文档 | `docs/storage-design.md` | §3.2 line 107 迁移口径改述 + 新增 §3.4 迁移登记表 |
| 文档 | `docs/adr/README.md` | §1 加 0038 行 +「下一个可用编号」→ 0039 |
| 文档 | `docs/memory/decisions.md` | 新增 `[DECISION][src:ADR-0038]` 入口 |
| 文档 | `docs/PARKING_LOT.md` | PL-046 关闭 + PL-047 新提（xtask 迁移文件名扫描，归 TASK-015） |
| 任务卡 | `tasks/TASK-202-storage-migration-registry.md` | 本 ADR 的落地卡 |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 漏注册某个 crate 的迁移 → 空库少建表 | ① `Database::open` 对「库里有不认识的版本」拒绝启动（不是静默）；② 每个拥有者 crate 自己测「装配后开空库，我的表在」；③ 装配点**唯一**（D3） |
| 两个 crate 抢同一个版本号 | ① `MigrationSet::register` 硬拦重复号；② `validate()` 硬拦缺号；③ §3.4 登记表先占号 |
| 「登记表靠记得回填」重演 ADR-0030 的教训 | 承认本轮只做到「装配时硬拦 + 文档 SSOT」；**机器扫描文件名**归 PL-047（要动 gov 规则计数，属 TASK-015 的地盘） |
| 移动 `0002_audit_logs.sql` 破坏已有库 | checksum 记的是**文件内容**、不是路径 → 内容一字不改即向后兼容；测试用固定 sha256 断言 |
| `Database::open` 签名变化波及面 | 调用点只有 2 个测试夹具 + 1 处 doctest（均在本卡 write scope 内）；删 `SCHEMA_VERSION` 同批完成 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. **加表不再需要改 storage**：本 ADR 之后 `crates/storage/src/schema.rs` 只应在 **storage 自己加表**时
   被改动；别的 crate 加表若改了它，就是本 ADR 被违反。
2. **storage 的测试不再随别人的表变化**：`crates/storage/tests/storage_integration.rs` 里
   `count_rows(schema_migrations)` 的断言对 storage 自己的集合（= 1）成立，与 audit / secrets / … 无关。
3. **机制自证**：`cargo test -p assistant-audit` 里存在「装配 storage + audit → 空库 → `audit_logs` +
   `idx_audit_ts` + 两个 append-only 触发器都在」的用例。
4. **重号 / 缺号是红灯**：`MigrationSet` 的单测覆盖 `DuplicateVersion` 与 `NonContiguousVersions`。
5. 重新评估触发条件：① 出现第二个跨 crate 消费者（secrets 等）→ 装配点必须**立刻**收敛到 Host，
   否则「唯一装配点」名不副实；② PL-047 落地（xtask 扫描文件名）后，D4 的「登记表 + 装配硬拦」
   应升级为 CI 红灯 —— 那时 D4 需要一次修订。

## 相关 ADR

- ADR-0030（机器校验优于手工回填 —— D4 的 PL-047 遗留正是这条教训的延续）
- ADR-0031（一卡一文件；本 ADR 的落地卡 = TASK-202）
- ADR-0033（单文件行数口径：写作规范 vs CI 门禁）
- ADR-0037 D1（治理池 200~299）

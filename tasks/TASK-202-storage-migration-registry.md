# TASK-202　存储迁移注册表（PL-046 处置：加表不再需要改 storage 的源码与测试）

- 状态：**Done**（2026-09-24）
- 阶段：跨阶段（**治理池 200~299**）　子阶段：—　批次：—（**不在** stage-1 批次表内）　依赖：013（✅ Done）　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md` §「跨阶段治理卡」；号段依据 **ADR-0037 D1**（200~299 = 治理池）。
- 来源：`docs/PARKING_LOT.md` **PL-046**（TASK-013 附带发现，根因对应 DRIFT-013-1）；人类 chat 2026-09-24 裁决 = **「现在就上正式机制」**。
- 契约依据：**ADR-0038**（本卡 = 它的落地卡）；关联 `docs/storage-design.md` §3.2 / **§3.4（迁移登记表）**、**PL-047**（机器化护栏，归 TASK-015）

---

## 目标（一句话）

把「迁移链」从 `crates/storage` 的**私有常量**改成**注册表 + 唯一装配点**：每个拥有表的 crate 声明并测试**自己**的迁移，
`crates/storage` 只提供机制（`Migration` / `MigrationSet` / `Database::open` 的**必填**迁移集参数）—— 于是
**加一张表 = 只改自己那个 crate**，而「漏注册某个 crate」是**拒绝启动**（`schema_version_mismatch`）而不是静默少建表。

## 背景（为什么现在做，而不是等第二张跨 crate 卡）

TASK-013 给 `audit_logs` 建表时撞上的结构性缺陷（PL-046 原文 + DRIFT-013-1 现场）：

| # | 后果 | 现场证据 |
|---|---|---|
| 1 | 加一张表必须改 storage 的**源码** | 迁移清单是 `crates/storage/src/schema.rs` 里的 `MIGRATIONS` 常量 + `SCHEMA_VERSION` |
| 2 | 加一张表必须改 storage 的**测试** | `crates/storage/tests/storage_integration.rs` 有 3 处写死「全库只有 1 个迁移」→ TASK-013 实测 `15 passed / 3 failed`（DRIFT-013-3） |
| 3 | 表的**所有权**与 DDL 的**位置**分离 | `audit_logs` 的语义 / 读写 / hash chain 全归 `crates/audit`，DDL 却必须躺在 `crates/storage/migrations/` |
| 4 | 抬版本号是**全局副作用** | 一个 crate 加表 → 所有打开主库的代码与测试都被牵动 |

「加表要顺手改 storage 的测试」这件事**本身就是缺陷** —— 它不是纪律问题，是**缺少机制**。人类裁决**现在**就上正式机制，
故本卡不做临时绕行、不攒到第二张跨 crate 卡。

## write scope

- `docs/adr/0038-storage-migration-registry.md`（**NEW**，契约先行）
- `docs/adr/README.md`（§1 加 0038 行 +「下一个可用编号」→ 0039）
- `docs/storage-design.md`（§3.2 迁移口径改述 + **新增 §3.4 迁移登记表**）
- `docs/memory/decisions.md`（`[DECISION][src:ADR-0038]`）※ 追加
- `crates/storage/src/migrations.rs`（**NEW**：`Migration` / `MigrationSet` / `MigrationSetError`）
- `crates/storage/src/{schema,lib,error}.rs`、`crates/storage/tests/**`、`crates/storage/README.md`
- `crates/audit/src/lib.rs`、`crates/audit/tests/**`、`crates/audit/README.md`
- `crates/storage/migrations/0002_audit_logs.sql` → **移动**到 `crates/audit/migrations/0002_audit_logs.sql`（内容一字不改）
- `plans/stage-1-pilots.md`（跨阶段治理卡表 + 号段表）※ Orchestrator 代行，见 §5
- `docs/PARKING_LOT.md`（PL-046 关闭行 + PL-047 新提）※ 追加
- `docs/memory/facts.md` / `docs/memory/pitfalls.md` ※ 追加
- `MEMORY.md` §1 规模表 ※ Orchestrator 代行，见 §5
- `LEDGER.md`、`tasks/TASK-013-audit-append-hash-chain-flush.md`（§5 DRIFT-013-3 裁决回填）、本卡

## In scope

1. **契约先行**：`docs/adr/0038-*.md`（D1~D5 + 6 个考虑过的选项 + 影响范围 + 风险 + 验证方式）；登记表加 0038 行、下一个可用号 → 0039。
2. `docs/storage-design.md`：§3.2 的「迁移：`sqlx migrate` 或 `refinery`」改为**自建 + 每 crate 自持迁移**（引 ADR-0038）；**新增 §3.4** 版本号登记表（`版本 | 拥有者 crate | 迁移文件 | 建出的表`）+ 两条说明（为什么 0002 在 audit、PL-047 遗留）。
3. `crates/storage/src/migrations.rs`：`Migration`（`const fn new` + `version()` / `name()` / `pub(crate) sql()` + 手写 `Debug` 只打 `sql_bytes`）、`MigrationSet`（`register` 按序插入并拦重号 / `register_all` / `validate` 要求 `1..=max` 连续 / `expected_version`）、`MigrationSetError`（3 变体 + 稳定 `reason_code()`）。
4. `crates/storage/src/schema.rs`：`pub const MIGRATIONS` **只含自己的 0001**；**删 `pub const SCHEMA_VERSION`**；`apply_pending(conn, now_ms, &MigrationSet)`；`verify_applied` 对「库里有本集合不认识的版本」报 `schema_version_mismatch`（detail 提示是否漏了某个 crate 的 `MIGRATIONS`）。
5. `crates/storage/src/error.rs`：`impl From<MigrationSetError> for StorageError` → 复用既有 `InvalidArgument { field: "migrations" }`（**不新增 reason_code**，不动 `docs/spec/error-codes.md` 契约）。
6. `crates/storage/src/lib.rs`：`Database::open(paths, clock, &MigrationSet)`；导出 `Migration` / `MigrationSet` / `MigrationSetError` / `MIGRATIONS`；模块文档与 doctest 改为装配点写法。
7. **移动** `crates/storage/migrations/0002_audit_logs.sql` → `crates/audit/migrations/0002_audit_logs.sql`（`git mv`，**内容一字不改**）；`crates/audit/src/lib.rs` 新增 `pub const MIGRATIONS`（0002）。
8. 测试：`crates/storage/tests/common/mod.rs` 新增 `migrations()` / `open_database_with`；storage 集成测试改用 `migrations().expected_version()` 并**新增 2 条负向用例**（重号 / 缺号）；`crates/audit/tests/common/mod.rs` = **唯一装配点**（storage + audit + `validate()`）+ `storage_only_migrations()`；audit 集成测试**新增 1 条负向用例**（漏注册 → 拒绝启动，绝不静默降级），并把「v1 库升级」用例改为**真实** v1 库（先用 storage-only 集合建库，不再手工 `DROP TABLE` 造假状态）。
9. 文档同步：`crates/storage/README.md`（边界 / 不变量 / 典型用法）、`crates/audit/README.md`（职责 / 典型用法）、`docs/PARKING_LOT.md`（PL-046 关闭 + PL-047 新提）、`docs/memory/{facts,pitfalls}.md`、`MEMORY.md` §1 规模表、`LEDGER.md`、`tasks/TASK-013-*.md` §5。

## Out of scope（做了算漂移）

- **不引第三方依赖**（漂移 ①）：不引 `sqlx` / `refinery` / `linkme` / `inventory` / `uuid`
- **不新增 / 不改 `docs/spec/error-codes.md` 的 reason_code**（复用既有 `invalid_argument`）
- **不改已发布的 `crates/storage/migrations/0001_init.sql`**、不改迁移执行语义（sha256 记账 / 独立事务 / 只前进不回滚）
- **不把装配点放进 `crates/core`**：TASK-201 卡明确「骨架期不发明公共接口」，且 core 尚无 storage/audit 依赖边 → 正式装配点归 Host（TASK-019~028）
- **不实现 xtask 扫描 `crates/*/migrations/*.sql`**（要动 gov §5.4 规则计数 + ADR-0025 / ADR-0030 口径）→ 记 **PL-047**，归 TASK-015
- 不动 `docs/adr/0038-*.md` 之外的任何 ADR 正文（ADR「只增不改」）；不动 `AGENTS.md`、`PLAN.md`、`docs/governance-*.md`
- 不做 `separate_db_full`（PL-042）、不引入审计代理键（PL-045）、不做外部锚点（PL-044）

## 必须遵守

- **铁律 1**（无静默失败）：漏注册 / 重号 / 缺号 / 降级启动一律**显式报错**；没有「默认只开 storage 那部分」的静默路径。
- **铁律 10**（契约先行）：先落 ADR-0038 + `docs/storage-design.md` §3.4，再改代码。
- **漂移触发器 ⑫**（删公共 API）：删 `pub const SCHEMA_VERSION` 由 **ADR-0038 D5** 显式授权。
- **漂移触发器 ①**：零新依赖；`docs/DEPENDENCIES.md` **不需要**改动（`assistant_storage` 仍是 audit 的既有依赖）。
- **漂移触发器 ⑦**（改测试断言）：storage 集成测试的 3 处断言在 TASK-013 已登记为 DRIFT-013-3；本卡把它改成「对**自己**的迁移集成立」，并**新增**负向用例 —— 只允许变强，不允许放宽。
- 继承 workspace `[lints]`：`unwrap_used` / `expect_used` / `panic` / `todo` / `dbg_macro` / `print_stdout` / `print_stderr` / `indexing_slicing` 全 **deny**（`tests/` 内可用 `expect`）。
- 单文件 ≤ 600（软）/ 900（硬）行（ADR-0033）；公共 API 100% 文档注释；`unsafe` 一处都不许加。
- 新增/改动的 `.md` 一律 **UTF-8 + LF 结尾**（`xtask docscan` 会红）。

## 验收命令（agent 必须全部执行并粘贴输出）

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-storage
cargo test -p assistant-audit
cargo test -p assistant-core arch::
cargo build --release
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- card-check
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo deny check
```

## 完成定义（DoD）

- [ ] `cargo test --workspace` 全绿，且 `cargo test -p assistant-storage` **不再**随 `crates/audit` 的迁移变化（`count_rows(schema_migrations)` 对 storage 自己的集合成立）
- [ ] `MigrationSet::register` 拦重号、`validate()` 拦缺号，两条负向用例在 storage 集成测试里
- [ ] `Database::open` 对「库里有本集合不认识的版本」拒绝启动；audit 有对应的「漏注册 → 拒绝」负向用例
- [ ] `pub const SCHEMA_VERSION` 已删除；仓库内 `grep SCHEMA_VERSION` = 0 命中（`docs/` 的历史描述除外，需逐条确认是历史记录）
- [ ] `crates/audit/migrations/0002_audit_logs.sql` 就位；与移动前的 git blob **逐字节相同**（sha256 checksum 不变 → 已有库仍可打开）
- [ ] `docs/adr/0038-*.md` + 登记表 + `docs/storage-design.md` §3.4 就位；`xtask adr-index` 绿
- [ ] 上列命令全部通过（输出粘贴进本卡 §3）
- [ ] `LEDGER.md` 追加；新 FACT/PITFALL 已追加；`MEMORY.md` 规模表按 `memory-counts` 机器值同步
- [ ] `tasks/TASK-013-*.md` §5 已回填 DRIFT-013-3 的人类裁决
- [ ] 无任何 Out of scope 的文件被修改
- [ ] 本卡 §1~§9 执行记录已填

## 风险与已知坑（开工前先读）

- **移动 SQL 文件必须逐字节相同**：checksum 记的是**内容**、不是路径，所以「移动」是向后兼容的；一旦顺手改一个字，已有库启动即 `migration_checksum_mismatch`。
- **`Database::open` 签名变化是编译期破坏**：仓库内调用点只有 2 个测试夹具 + 2 处 doctest（均在本卡 write scope）；下游 crate 尚未存在。
- **「唯一装配点」是当下的**：`crates/audit/tests/common/mod.rs` 只是阶段 1 的临时装配点；出现**第二个**跨 crate 消费者时必须立刻收敛到 Host（ADR-0038 验证方式 5）。
- **登记表仍是手工回填**：ADR-0030 的教训是「靠记得回填的护栏会失效」—— 机器化归 **PL-047 / TASK-015**，本卡不假装它已生效。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-202 存储迁移注册表（PL-046 处置：任何 crate 加表不再需要改 storage 的源码与测试）
【目标】storage 只提供机制（Migration/MigrationSet），每个拥有表的 crate 声明并测试自己的迁移；加表 = 只改自己那个 crate；漏注册 = 拒绝启动
【write scope】卡面「write scope」13 项（ADR-0038 + 登记表 + storage-design §3.4 + storage 机制与测试 + audit 的 MIGRATIONS 与装配点 + 0002 SQL 移动 + 4 个公共热点文件的追加 + 本卡 + TASK-013 §5 回填）
【铁律】1（无静默失败：漏注册/重号/缺号/降级一律报错）、10（契约先行：先 ADR-0038 再改代码）、9（不静默扩大范围）
【禁止】引第三方依赖；新增/改 reason_code；改 0001_init.sql 与迁移执行语义；把装配点放进 crates/core；实现 xtask 迁移文件名扫描（归 PL-047/TASK-015）
【验收】fmt / clippy / test --workspace / test -p assistant-{storage,audit,core} / build --release / 6 个 xtask 子命令 / cargo deny
【依赖】013（Done；本卡处理的 PL-046 正是它附带发现的缺陷）
【疑问】无。PL-046 的处置方向已由人类 chat 2026-09-24 裁决（「现在就上正式机制」）；号段（治理池 200~299）沿用 TASK-200/201 先例
```

### 2. 实际改动文件

| # | 文件 | 动作 | 说明 |
|---|---|---|---|
| 1 | `docs/adr/0038-storage-migration-registry.md` | NEW | 契约：D1~D5 + 6 个考虑过的选项 + 影响范围 + 风险 + 5 条验证方式 |
| 2 | `docs/adr/README.md` | EDIT | §1 加 `0038` 行；「下一个可用编号」0038 → **0039** |
| 3 | `docs/storage-design.md` | EDIT | §3.2 迁移口径改述；**新增 §3.4 迁移登记表**（版本号分配的 SSOT） |
| 4 | `docs/memory/decisions.md` | APPEND | `[2026-09-24][DECISION][src:ADR-0038]` |
| 5 | `crates/storage/src/migrations.rs` | NEW | `Migration` / `MigrationSet` / `MigrationSetError`（机制，**不含表清单**） |
| 6 | `crates/storage/src/schema.rs` | EDIT | `pub const MIGRATIONS`（只含 0001）；**删 `SCHEMA_VERSION`**；`apply_pending` 接 `&MigrationSet`；`verify_applied` 拦「不认识的版本」 |
| 7 | `crates/storage/src/error.rs` | EDIT | `From<MigrationSetError>` → `InvalidArgument { field: "migrations" }`（复用既有 reason_code） |
| 8 | `crates/storage/src/lib.rs` | EDIT | `Database::open` 加第 3 参；导出新类型；模块文档 + doctest |
| 9 | `crates/storage/migrations/0002_audit_logs.sql` | MOVE | → `crates/audit/migrations/0002_audit_logs.sql`（`git mv`，**内容一字不改**） |
| 10 | `crates/audit/src/lib.rs` | EDIT | 新增 `pub const MIGRATIONS`（0002）+ 模块文档/doctest 装配点写法 |
| 11 | `crates/audit/src/log.rs` | EDIT | 边界注释里的 DDL 路径改到 `crates/audit/migrations/` |
| 12 | `crates/storage/tests/common/mod.rs` | EDIT | `migrations()` / `open_database_with` |
| 13 | `crates/storage/tests/storage_integration.rs` | EDIT | 改用 `expected_version()`；2 条负向用例一度加在这里（涨到 606 行触发 hygiene 软上限）→ 已拆到新文件 |
| 14 | `crates/audit/tests/common/mod.rs` | EDIT | **唯一装配点** `migrations()` + `storage_only_migrations()` + `open_database_with` |
| 15 | `crates/audit/tests/audit_integration.rs` | EDIT | v1 库升级用例改为**真实** v1 库；**新增**「漏注册 → 拒绝启动」负向用例 |
| 16 | `crates/storage/README.md`、`crates/audit/README.md` | EDIT | 边界 / 不变量 / 典型用法与注册表对齐 |
| 17 | `plans/stage-1-pilots.md` | EDIT | 跨阶段治理卡表加 TASK-202；号段表 200~299 行 |
| 18 | `docs/PARKING_LOT.md` | APPEND | **PL-046 关闭** + **PL-047 新提** |
| 19 | `docs/memory/facts.md` / `docs/memory/pitfalls.md` | APPEND | +1 FACT、+3 PITFALL |
| 20 | `MEMORY.md` | EDIT | §1 规模表（按 `memory-counts` 机器值） |
| 21 | `LEDGER.md` | APPEND | 本卡 Done 行（只追加） |
| 22 | `tasks/TASK-013-audit-append-hash-chain-flush.md` | EDIT | §5 DRIFT-013-3 回填「人类 2026-09-24 裁决 = 接受」 |
| 23 | `tasks/TASK-202-storage-migration-registry.md` | NEW | 本卡（正文 + 记录区 §1~§9） |
| 24 | `crates/storage/tests/storage_migrations.rs` | NEW | 迁移集**装配期**负向用例（重号 / 缺号）—— 拆出后 `storage_integration.rs` 回到 556 行，hygiene 维持 2 warning baseline |

**未改**（Out of scope 硬线，逐项核对）：`crates/storage/migrations/0001_init.sql`、`crates/protocol/**`、`protocol/**`、任何 `docs/spec/*`、`docs/adr/0038-*.md` 之外的 ADR、`AGENTS.md`、`PLAN.md`、`docs/governance-*.md`、`deny.toml`、`.github/**`、`docs/DEPENDENCIES.md`（零新依赖）。

### 3. 验收输出摘要

全部命令在**本机 Windows 11 25H2**（`D:\csart\ai-assistant`，PowerShell）执行：

| # | 命令 | 结果 |
|---|---|---|
| 1 | `cargo fmt --all --check` | exit 0（无 diff） |
| 2 | `cargo clippy --all-targets -- -D warnings` | exit 0（`Finished dev profile`） |
| 3 | `cargo test --workspace --no-fail-fast` | exit 0 —— **16 个 test target 全 ok / 0 failed**：audit 集成 12 + tamper 6 + 单测 8 + doctest 1；storage 集成 18 + **迁移负向 2** + records 2 + 单测 6 + doctest 2；protocol 7；xtask 331；core 0 |
| 4 | `cargo build --release` | exit 0 |
| 5 | `cargo test -p assistant-core arch::` | exit 0（`running 0 tests` —— **软门禁 #5 的语义仍是「0 测试通过」**，真断言归 TASK-015） |
| 6 | `cargo run -p xtask -- hygiene` | PASSED（scanned=65 / **0 error** / 2 warning = xtask 既有长文件 baseline） |
| 7 | `cargo run -p xtask -- docscan` | PASSED（scanned=153 / 0e / 0w） |
| 8 | `cargo run -p xtask -- memory-counts` | PASSED（scanned=8 / 0e / 0w；facts 142/94、pitfalls 190/87、decisions 96/52） |
| 9 | `cargo run -p xtask -- adr-index` | PASSED（scanned=22 / 0e / 0w） |
| 10 | `cargo run -p xtask -- card-check` | PASSED（0e / 49w = baseline） |
| 11 | `cargo run -p xtask -- verify-schemas` | PASSED（0 error） |
| 12 | `cargo run -p xtask -- codegen --check` | PASSED（0 drift / 0 error） |
| 13 | `cargo deny check` | exit 0 —— `advisories ok, bans ok, licenses ok, sources ok`（另有 1 条既有的 `unmatched license allowance` 提示，非 error） |

**关键实证（机制是否真的成立）**：

| 判据 | 实测 |
|---|---|
| 迁移文件移动是向后兼容的 | 移动前后 git blob **逐字节相同**：`4d9ffbb25f47de30c2ed577edeeb478d0cfc9b98`（`HEAD:crates/storage/migrations/0002_audit_logs.sql` == `git hash-object crates/audit/migrations/0002_audit_logs.sql`）⇒ sha256 checksum 不变 |
| `SCHEMA_VERSION` 已无活引用 | `git grep -n SCHEMA_VERSION` 的命中**全部**是历史记录（LEDGER / PARKING_LOT / pitfalls / TASK-013 卡 / 本卡），`crates/**` **0 命中** |
| 「加表要改 storage 的测试」已消失 | `crates/storage/tests/common/mod.rs` 的 `migrations()` **只装配 storage 自己的 `MIGRATIONS`** ⇒ `count_rows(schema_migrations) == migrations().expected_version()` 恒成立（与 audit 无关） |
| 漏注册**不是静默** | `crates/audit/tests/audit_integration.rs::test_open_with_partial_migration_set_is_refused_not_silently_downgraded`：用只含 storage 的集合去开一个已应用 0002 的库 → `schema_version_mismatch` |
| 重号 / 缺号当场红灯 | `crates/storage/tests/storage_migrations.rs` 的 2 条负向用例（`migration_duplicate_version` / `migration_non_contiguous_versions` + `open()` 报 `invalid_argument`） |
| 「真 v1 库」升级路径 | `test_migration_0002_upgrades_v1_library_and_is_idempotent` 改为**先用 storage-only 集合建库**（真实 v1）再换完整集合打开 |

### 4. DoD 逐条核对

- [x] `cargo test --workspace` 全绿（395 个测试 / 0 failed），且 storage 的断言不再随 `crates/audit` 变化 —— `migrations()` 只装配自己的 `MIGRATIONS`
- [x] `register()` 拦重号、`validate()` 拦缺号：`crates/storage/tests/storage_migrations.rs` 2 条负向用例（原放在 `storage_integration.rs`，因该文件涨到 606 行触发 hygiene 软上限 → 拆成独立文件，见 §5）
- [x] `Database::open` 对「库里存在本集合不认识的版本」拒绝启动（`verify_applied` + `schema_version_mismatch`）；audit 有对应负向用例
- [x] `pub const SCHEMA_VERSION` 已删除；`crates/**` 0 引用（`git grep` 的其余命中全是历史记录）
- [x] `crates/audit/migrations/0002_audit_logs.sql` 就位，且与移动前 **git blob 相同**（`4d9ffbb2…`）
- [x] `docs/adr/0038-*.md` + 登记表（§1 加 0038 / 下一个可用号 0039）+ `docs/storage-design.md` §3.4 就位；`adr-index` PASSED
- [x] 上列 13 条命令全部通过（见 §3）
- [x] `LEDGER.md` 追加；新 FACT（+1）/ PITFALL（+3）已追加；`MEMORY.md` 规模表按 `memory-counts` 机器值同步（facts 142/94、pitfalls 190/87、decisions 96/52）
- [x] `tasks/TASK-013-*.md` §5 已回填 DRIFT-013-3 的人类裁决（跨卡回填，见 §5 DRIFT-202-1）
- [x] 无 Out of scope 文件被修改（逐项见 §2 的「未改」行）
- [x] 本卡 §1~§9 执行记录已填

### 5. 偏差

**DRIFT-202-1　跨卡回填 `tasks/TASK-013-*.md` §5（触发器 ⑤：超出 write scope）**

- **现象**：AGENTS.md §8 规定 Implementer 只对**自己那张卡**的记录区有写权限；而「DRIFT-013-3 的人类裁决 = 接受」的落点只能是 TASK-013 卡的 §5（那条 DRIFT 登记在那里）。
- **授权来源**：人类 chat 2026-09-24 原话「**接受你做的，我复核下来没有什么问题**」（= 明确要求把裁决记到那条 DRIFT 上）。
- **落地**：只在 TASK-013 卡 §5 的 DRIFT-013-3 块**追加 3 行**（裁决 = 接受 / 根因 PL-046 已由 ADR-0038 + TASK-202 闭环 / 本行为跨卡回填）；**未动**该卡正文区，未动该卡其他任何一节。
- **未停止工作**：授权范围内的动作已全部完成，无未决项。

**DRIFT-202-2　`PLAN.md` 的「当前状态」块与事实不符（触发器 ⑧：文档与代码矛盾）**

- **现象**：`PLAN.md` 的「当前状态」仍写「当前任务卡：**TASK-012 存储层 = 下一张**」「阻塞项 ① TASK-012 需人类先批准 3 个依赖」「下一步动作 ① 批准 TASK-012 依赖 → ② TASK-012 → …」—— 而 012 / 013 / 200 / 201 / 202 均已完成。
- **为什么不改**：`PLAN.md` 自述「agent **不得修改本文件**，只能提案」，AGENTS.md §11.1 也把它列为 **Orchestrator-only**；本卡 write scope 未含它。
- **建议的替换文本**（供人类 / Orchestrator 直接采用）：
  - `当前任务卡：1a 批次 A1（地基层）：TASK-011 ✅ / 012 ✅ / 013 ✅ Done → **TASK-014（secrets / OS keychain）= 下一张**；跨阶段治理卡 TASK-200 / 201 / 202 已 Done。`
  - `阻塞项：① PL-037 已闭环（TASK-201），TASK-015 的 arch 宿主已就位；② TASK-002 仍 Blocked（上游 create_thread 未关）。`
  - `下一步动作：① TASK-014（crates/secrets）→ ② TASK-015 护栏清扫（含 PL-047）→ ③ stage-1 后续批次。`
- **未停止工作**：本卡不越界改 PLAN.md；已在 §9 与完成报告里提请裁决。

**顺带修复（在 write scope 内，不计 DRIFT）**

1. **`docs/adr/README.md` 的裸 NUL 字节**：该文件（`main` 上就有）含 1 个 `0x00`，落在 0035 行的文件名单元格里 → git 把这份 ADR 登记表当**二进制**（`git diff` 恒显示 `Bin … -> … bytes` = 不可复核），而 `docscan` / `hygiene` / `adr-index` 全都抓不到。本卡在编辑该文件时按字节定位修掉（同时补回该行缺失的反引号与一个 `0`），教训写成 PITFALL。⚠ 由于**基线**是二进制，本 PR 里这个文件的 diff 仍是二进制；合并后恢复文本 diff。
2. **`crates/storage/tests/storage_integration.rs` 拆出 `storage_migrations.rs`**：新增 2 条负向用例后该文件涨到 **606 行**，触发 `hygiene/file-too-long`（软上限 600）。为不把基线从 2 warning 抬到 3，把这两条「装配期」用例移到新文件 `crates/storage/tests/storage_migrations.rs`（同属 write scope `crates/storage/tests/**`）。**净效果**：原文件 556 行、hygiene 回到 **2 warning baseline**。
3. **两处文档口径笔误**（同批修）：`docs/storage-design.md` §3.4 的「由拥有者公开 `migrations()`」→「`pub const MIGRATIONS: &[Migration]`」；`docs/adr/0038-*.md` 影响范围表的 `crates/storage/src/migration.rs` → `crates/storage/src/migrations.rs`（ADR 尚未提交，属首次落盘前的更正，不违「ADR 只增不改」）。

### 6. 更合理做法

1. **把「加表要改 storage 的测试」判定为机制缺陷而不是纪律缺陷**：如果只写一句「以后记得同步改 storage 的测试」，那 TASK-013 的 3 处断言会再次爆炸。**正解是让 storage 的测试只对自己那个集合成立** —— 于是「别人的表」与它无关。
2. **迁移集做成 `Database::open` 的必填参数**，而不是「默认 = storage 自己的集合」：默认值会让「忘了注册 audit」变成一个**能跑起来的库**（静默少建表 = 铁律 1 的反面）。
3. **移动 SQL 文件而不改内容**：checksum 记内容不记路径，故「移动」向后兼容；这是「所有权与 DDL 同处」唯一能做到零迁移代价的走法。
4. **`MigrationSetError` 独立于 `StorageError`**：装配期错误（编程错误）不该混进运行期存储故障，也不该为它新增 `reason_code`（那要改 `docs/spec/error-codes.md` 契约）。

### 7. 遗留问题

- **PL-047**（本卡新提）：xtask 扫描 `crates/*/migrations/*.sql`（号段唯一 + 与 §3.4 登记表一致）→ 归 **TASK-015**（它拥有 gov §5.4 规则计数与 ADR-0025 / ADR-0030 口径）。
- **装配点仍是临时的**：`crates/audit/tests/common/mod.rs`。出现第二个跨 crate 消费者（secrets 等）时必须立刻收敛到 Host（TASK-019~028）。
- **PLAN.md 的「当前状态」块未同步**（Orchestrator-only 文件，本卡不越界）→ 见 §5 DRIFT-202-2。
- **`docs/adr/README.md` 的既有 NUL 字节**已在本卡顺带修掉（见 §5 的「顺带修复」1）；`docs/adr/README.md` 在本 PR 里的 diff 仍是二进制（基线是二进制），合并后恢复文本 diff。

### 8. 新增长期记忆

- `docs/memory/facts.md`：+1 FACT（注册表落地 + `SCHEMA_VERSION` 删除 + 0002 移动但 checksum 不变）。
- `docs/memory/pitfalls.md`：+3 PITFALL（① 文本文件里的裸 NUL 会让 git 把它当二进制；② `Arc::clone(clock)` 内联进 `Database::open` 因 unsized coercion 编译失败；③ 手工 `DROP TABLE` 造出的「旧版本库」是假状态）。

### 9. 给审阅者的关注点

1. **`verify_applied` 的判据（风险最高）**：它要求「库里已应用的每个版本都必须出现在装配后的集合里」，并对「`applied_max` 以下的缺号」报错 —— 请确认它拦得住「漏注册 audit」而**不**误伤「合法的 v1 库 + 完整集合」（升级路径）。
2. **`docs/adr/README.md` 的 diff 会显示成二进制**：因为**基线**里那个文件含一个裸 NUL 字节（main 上就存在）。本卡把它修掉，于是这个文件在本 PR 里 diff 不可读（`Bin … -> … bytes`）。**PR 合并后**该文件恢复为文本 diff。复核请用 `git diff --text HEAD~1 -- docs/adr/README.md` 或直接看文件内容（0038 行 + 下一个可用编号 0039）。
3. **`SCHEMA_VERSION` 删除是否可接受**：这是删公共 API（漂移 ⑫），由 ADR-0038 D5 授权；替代 = `MigrationSet::expected_version()`。若审阅者认为应保留一个「兼容常量」，那会让「全局最高版本」这个必然说谎的概念复活 → 请明确驳回并说明。

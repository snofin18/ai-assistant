# TASK-203　`audit_logs` 列语义去重 + 显式链序（PL-045 + PL-043 闭环）

- 状态：**Done**（2026-09-24）
- 阶段：跨阶段（**治理池 200~299**）　子阶段：—　批次：—（**不在** stage-1 批次表内）　依赖：013（✅ Done）、202（✅ Done）　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md` §「跨阶段治理卡」；号段依据 **ADR-0037 D1**（200~299 = 治理池）。
- 来源：`docs/PARKING_LOT.md` **PL-045**（`id` 与 `hash` 语义重复）+ **PL-043**（链尾查询依赖 `rowid`）；人类 chat 2026-09-24 裁决 = **「留下有用且语义明显的唯一部分」**（PL-045）+ **「没有问题就落地」**（PL-043）。
- 契约依据：**ADR-0040**（本卡 = 它的落地卡）；关联 架构 v2 §15.1、`docs/storage-design.md` §3.4 / §4、**ADR-0038**（迁移注册表）

---

## 目标（一句话）

把 `audit_logs` 的两处**语义缺陷**一次性收掉：删掉与 `id` 完全同义的 `hash` 列（PL-045），
并引入显式链序 `sequence INTEGER PRIMARY KEY AUTOINCREMENT` 取代对 `rowid` 的隐式依赖（PL-043）——
**只新增迁移 0003，`0002` 一字不改**（它的 sha256 已记在已有库的 `schema_migrations.checksum`）。

## 背景（为什么现在做）

| # | 缺陷 | 现状证据 | 为什么不能拖 |
|---|---|---|---|
| 1 | `id` 与 `hash` 存**同一个值** | `0002_audit_logs.sql` 两列的注释都写「= 本条 self_hash」 | 每行多 64 字节 + 索引；读者必须问「它俩差在哪」（PL-045） |
| 2 | 「两列必须一致」是**同义反复** | `verify.rs` 的 `row.id != row.hash` 分支 | 真正的判据只有「重算 == 存下来的值」；多一列反而制造歧义 |
| 3 | 链尾查询用 `ORDER BY rowid DESC` | `log.rs` `SELECT_CHAIN_TAIL_SQL` | 本表无 `INTEGER PRIMARY KEY`，SQLite 文档明说 `VACUUM` 可重排 rowid（PL-043） |

人类裁决：PL-045 走「**只留语义明显的那一份**」；PL-043 走「检查下来没问题就落地」。

## write scope

- `docs/adr/0040-audit-log-column-semantics.md`（**NEW**，契约先行）
- `docs/adr/README.md`（§1 加 0039 / 0040 行 +「下一个可用编号」→ 0041）
- `docs/memory/decisions.md`（`[DECISION][src:ADR-0040]`）※ 追加
- `cross-platform-ai-assistant-architecture-v2.md`（**仅** §15.1 的 `audit_logs(...)` 那一行 —— ADR-0040 D6 授权）
- `docs/storage-design.md`（§3.4 登记表加 0003 + §4 表映射行）
- `crates/audit/migrations/0003_audit_logs_semantics.sql`（**NEW**：重建表）
- `crates/audit/src/{lib,log,verify}.rs`
- `crates/audit/tests/**`
- `crates/audit/README.md`
- `docs/PARKING_LOT.md`（PL-043 / PL-045 关闭；PL-042 / PL-044 复核结论）※ 追加
- `tasks/TASK-203-audit-log-column-semantics.md`（本文件）

## Out of scope（写了就停）

- **不改** `crates/audit/migrations/0002_audit_logs.sql` 的**一个字节**（checksum 记账）
- **不实现** PL-042（`separate_db_full`：第二个 DB 文件 + 独立连接 + `synchronous=FULL`）与
  PL-044（外部锚点 / 防整表清空）—— 二者都要独立 ADR + 独立工作量，本卡只做**复核结论**
- **不引任何新依赖**（尤其 `uuid`：真正的代理键方案已被 ADR-0040 选项 4 否决）
- 不动 `crates/storage`、不动 `xtask`、不动 `crates/protocol`

## 必须遵守

- **契约先行**：ADR-0040 先落，再改代码（本卡已遵守）
- **只前进不回滚**：结构调整一律新增 `0003_*.sql`；`0002` 只读
- **无静默失败**：删掉的那条「两列一致」校验必须**显式说明**为什么它消失（不是"悄悄少检查"）
- **改测试断言 = 漂移触发器 ⑦**：本卡确实改了 `test_audit_logs_columns_match_architecture_section_15_1`
  的期望列清单（契约变了）与两处期望版本号 2 → 3 —— 已在 §5 显式登记

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-audit --no-fail-fast
cargo test --workspace --no-fail-fast
cargo run -p xtask -- hygiene / docscan / memory-counts / adr-index / card-check
cargo run -p xtask -- verify-schemas / codegen --check
cargo deny check
```

## DoD

- [ ] `pragma_table_info('audit_logs')` = `sequence, id, prev_hash, ts, actor, task_id, step_id, event_type, detail_json`
- [ ] 已有 **v2 库（有数据）** 能升到 v3：行数不变、链自洽、`sequence` 从 1 连续、索引与两个触发器都在、升级后 append 接得上旧链尾
- [ ] `VACUUM` 后链尾不变（PL-043 的原始触发场景有等价断言）
- [ ] `crates/audit/src/` 的运行期 SQL 里 `rowid` 归零；`hash` 只作为 `prev_hash` / `self_hash` 的一部分出现
- [ ] `0002_audit_logs.sql` 的 git blob **逐字节未变**
- [ ] 全门禁绿 + LEDGER 追加 + `docs/memory/*` 同步

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-203 audit_logs 列语义去重 + 显式链序     【目标】删与 id 同义的 hash、加显式链序 sequence
【write scope】仅：docs/adr/0040-*.md(NEW)、docs/adr/README.md、docs/memory/decisions.md、
              architecture v2 §15.1 一行、docs/storage-design.md、crates/audit/migrations/0003_*.sql(NEW)、
              crates/audit/src/{lib,log,verify}.rs、crates/audit/tests/**、crates/audit/README.md、
              docs/PARKING_LOT.md、tasks/TASK-203-*.md
【铁律】1 无静默失败 / 4 每个写操作有 postcondition / 10 契约先行（ADR 先于代码）/ 9 不静默扩大范围
【禁止】改 0002_audit_logs.sql 一个字节；实现 PL-042 / PL-044；引新依赖（尤其 uuid）
【验收】cargo fmt --check / clippy -D warnings / cargo test -p assistant-audit / cargo test --workspace
        / xtask hygiene|docscan|memory-counts|adr-index|card-check / cargo deny check → 全绿
【依赖】TASK-013（✅ Done，建表）、TASK-202（✅ Done，迁移注册表）—— 已核对 LEDGER
【疑问】无
```

### 2. 实际改动文件

**新增**

| 文件 | 内容 |
|---|---|
| `docs/adr/0040-audit-log-column-semantics.md` | 本卡契约（D1~D6 + 7 个考虑过的选项 + 风险 + 6 条验证方式） |
| `crates/audit/migrations/0003_audit_logs_semantics.sql` | 重建 `audit_logs`：删 `hash`、加 `sequence`、重建 `idx_audit_ts` + 两个 append-only 触发器 |
| `tasks/TASK-203-audit-log-column-semantics.md` | 本文件 |

**修改（代码）**：`crates/audit/src/lib.rs`（`MIGRATIONS` 加 0003 + 不变量 5/6）、
`crates/audit/src/log.rs`（`INSERT` 8 列 / 3 条 `SELECT` 改 `sequence` / `RawAuditRow` 删 `hash`）、
`crates/audit/src/verify.rs`（去掉 `id`/`hash` 两列一致性分支，权威判据只剩「重算 == `id`」）、
`crates/audit/tests/{audit_integration,audit_tamper}.rs` + `tests/common/mod.rs`

**修改（文档）**：`cross-platform-ai-assistant-architecture-v2.md`（**仅** §15.1 的 `audit_logs(...)` 那一行）、
`docs/storage-design.md`（§3.4 加 0003 + §4 表映射行）、`docs/adr/README.md`（0039/0040 + 下一个可用号 0041）、
`crates/audit/README.md`、`docs/PARKING_LOT.md`（PL-043/045 关闭 + PL-042/044 复核）、
`docs/memory/{decisions,facts,pitfalls}.md`、`MEMORY.md`（规模表）

**未改（关键证据）**：`crates/audit/migrations/0002_audit_logs.sql` **不在** `git status` 的改动清单里 ——
一个字节都没动（checksum 记账不破）。

### 3. 验收输出摘要

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | 0 diff |
| `cargo clippy --all-targets -- -D warnings` | exit 0（修掉 2 处：`doc_markdown` 的 `self_hash` 缺反引号；测试里 `index as i64` 触发 `cast_possible_wrap`） |
| `cargo test -p assistant-audit --no-fail-fast` | **29 passed / 0 failed**（integration 13 + tamper 7 + unit 8 + doctest 1） |
| `cargo test --workspace --no-fail-fast` | 全绿（xtask 331 + protocol 7 + storage + audit 29 + core 0） |
| `cargo build --release` | exit 0 |
| `xtask hygiene` | PASSED（0 error / **2 warning = 既有 baseline**） |
| `xtask docscan` | PASSED（0 error / 0 warning） |
| `xtask memory-counts` | PASSED（0 error / 0 warning；facts 144/95、pitfalls 193/89、decisions 101/54） |
| `xtask adr-index` | PASSED（0 error / 0 warning；scanned 24 份 ADR） |
| `xtask card-check` | PASSED（0 error / 49 warning = baseline） |
| `xtask verify-schemas` | PASSED（0 error） |
| `xtask codegen --check` | PASSED（0 drift） |
| `cargo deny check` | advisories ok, bans ok, licenses ok, sources ok |
| `xtask refscan` | 151 error = **既有 baseline**（全部落在 `docs/PARKING_LOT.md` 主表 37/41 行的裸待建号引用，与本卡无关；refscan **不在** CI #12b 的 job 里） |

**关键实测（新增的两条断言）**

1. `test_migration_0003_upgrades_v2_library_preserving_chain`：真 v2 库（storage 0001 + audit 0002，3 行数据）
   → 升到 v3 后 `schema_version = 3`、行数 3、`sequence = [1,2,3]`、链尾不变、读出的 id 顺序逐行一致、
   `verify_chain` 自洽、升级后 append 的 `prev_hash` == 升级前的链尾；`idx_audit_ts` 与两个触发器都在。
2. `test_vacuum_does_not_change_chain_tail`：`VACUUM` 前后 `persisted_hash()` 一致，VACUUM 后新行接得上旧链尾。

### 4. DoD 逐条核对

- [x] `pragma_table_info('audit_logs')` = `sequence, id, prev_hash, ts, actor, task_id, step_id, event_type, detail_json`
      —— `test_audit_logs_columns_match_architecture_section_15_1` 逐字断言
- [x] 已有 v2 库（有数据）能升到 v3：行数 / 链自洽 / `sequence` 连续 / 索引与触发器 / 升级后 append 接旧链尾
- [x] `VACUUM` 后链尾不变（`test_vacuum_does_not_change_chain_tail`）
- [x] `crates/audit/src/` 的运行期 SQL 里 `rowid` 归零（唯一出现 = `log.rs:356` 的解释性注释）
- [x] `0002_audit_logs.sql` 未出现在 `git status` 改动清单里
- [x] 全门禁绿 + LEDGER 追加 + `docs/memory/*` 同步

### 5. 偏差

**DRIFT-203-1　改测试断言（触发器 ⑦，已登记）**

- **现象**：`test_audit_logs_columns_match_architecture_section_15_1` 的期望列清单从 9 列（含 `hash`）
  改成 9 列（含 `sequence`、无 `hash`）；两处期望版本号 2 → 3。
- **为什么是"改断言"而不是"改实现"**：**契约本身变了**（ADR-0040 D1/D6 授权改架构 v2 §15.1 的列清单），
  这条断言的语义是「列形状 == 架构文档」—— 架构文档改了，断言必须跟着改。**这不是**「为了让测试变绿而
  改断言」（那种情形是断言在正确地抓住实现缺陷）。
- **反向证明**：本次同时**新增**了两条断言（升级保链、VACUUM 保链尾）与一条列形状断言 —— 净增断言数，
  不是减少。
- **未停止工作**：契约先行（ADR-0040）已落，改断言是其必然结果。

**DRIFT-203-2　更新架构 v2 §15.1（触发器 ③ + ④，由 ADR-0040 D6 授权）**

- **现象**：`cross-platform-ai-assistant-architecture-v2.md` 的 `audit_logs(...)` 列清单被改（改 DB schema = ③；
  改架构已决事项 = ④）。
- **为什么不单独开 ADR**：ADR-0040 的 D6 **就是**这条授权，且明确「除这一行外不动该文件任何内容」。
- **实际改动**：`git diff` 该文件 = **1 行改 + 1 行新增**（新行列 `sequence` / `id` 的语义）。

### 6. 更合理做法

1. **删列只能重建表，而重建表会带走索引与触发器** —— 所以「重建表的迁移」必须把索引 / 触发器
   **逐条显式重建**，并让测试断言它们升级后仍在。这次如果只写 `CREATE/INSERT/DROP/RENAME` 而忘了重建触发器，
   append-only 的第二道锁会被**静默**摘掉（那才是真正危险的形态）。
2. **不要为了让「升级路径」用例好写而保留 `AuditLog` 对旧 schema 的兼容**：代码只认最新 schema 是**正确**的
   （`no such column: sequence` 是好事）。旧形状的库只能由「当时那份 DDL + 公开链算法」造出来 —— 见 PITFALL。
3. **`AUTOINCREMENT` 不是"更重的写法"，是语义选择**：普通 `INTEGER PRIMARY KEY` 在删尾行后会复用号，
   而 PL-043 的触发场景（归档 / VACUUM 维护）恰好可能删行 → 复用号会让「链序」讲假话。
4. **`sequence` 不写进 `INSERT`**：让 SQLite 分配，避免应用层与数据库层对「下一个号」有两份认知。

### 7. 遗留问题

- **PL-042（`separate_db_full`）未实现** —— 复核结论已记 `docs/PARKING_LOT.md`：需独立 ADR + 独立卡
  （第二个 DB 文件 + 独立连接 + 与主库的迁移/校验协同）；当前「显式失败」行为合规。
- **PL-044（整表清空不可检出）未实现** —— 复核结论：接受为残余风险（需外部锚点，当前架构无落点），
  已在 `crates/audit/README.md` 的「已知限制」写明；触发条件 = 合规 / 无人值守场景。
- **`sequence` 尚未对外暴露**：`AuditRecord` 没有 `sequence` 字段（读出的顺序已经是链序）。
  若将来有「按链序分页 / 断点续读」的需求，需要加字段 —— 那是**加公共 API**，届时应走 ADR。
- **PL-047 仍未机器化**：`docs/storage-design.md` §3.4 的 0003 行仍是手工回填（归 TASK-015）。

### 8. 新增长期记忆

- `docs/memory/facts.md`：+1 FACT（`audit_logs` 列语义去重 + 显式链序落地；v2→v3 升级实测；`0002` 未改）
- `docs/memory/pitfalls.md`：+2 PITFALL（① `ALTER TABLE DROP COLUMN` 的限制 + 重建表会带走索引与触发器；
  ② 代码跟着新迁移走之后，不能再用一个写入器去造"旧形状的库"）
- `docs/memory/decisions.md`：+2 DECISION（ADR-0039 / ADR-0040）

### 9. 给审阅者的关注点

1. **迁移 0003 的重建顺序（风险最高）**：`CREATE audit_logs_new` → `INSERT ... SELECT ... ORDER BY rowid`
   → `DROP TABLE audit_logs` → `ALTER ... RENAME` → 重建 `idx_audit_ts` + 两个触发器。
   请确认：① `ORDER BY rowid` 读的是**旧表**（此时它还是 v2 形状，rowid 就是链序）；
   ② `DROP` 之后触发器被显式重建（否则 append-only 的第二道锁静默消失）；
   ③ `AUTOINCREMENT` 的「不复用」语义是刻意的（PL-043 的归档场景）。
2. **删掉「`id`/`hash` 两列一致」这条校验是否可接受**：ADR-0040 D3 的论据是「两列都只是 `self_hash` 的副本，
   该检查是同义反复；权威判据是「重算 == `id`」，它**保留**」。若审阅者认为该分支提供过独立的检测价值，
   请明确说明在什么攻击场景下（当前分析：改 `hash` 列本身不会改变 `detail_json` 的重算结果，
   故它检出的只是"两列被人为改得不一致"，而那只可能来自对已被篡改的库的二次编辑）。
3. **架构 v2 §15.1 被改了一行**：这是漂移触发器 ③/④，由 ADR-0040 D6 授权。请确认改动范围
   只有 `audit_logs(...)` 那一行 + 一行语义注释，且与 `crates/audit/migrations/0003_*.sql` 逐字一致。

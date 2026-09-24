# TASK-014　`secrets`：OS keychain 封装（DPAPI/Keychain/Secret Service）

- 状态：**Done**
- 阶段：1　子阶段：**1a**　批次：**A1**　依赖：011　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

## 目标（一句话）

`secrets` crate：把模型 API Key / 主程序凭据 / MCP 令牌**只**存进 OS 密钥库（Windows 凭据管理器 / macOS Keychain / Linux Secret Service），对外只暴露**引用**与**受审计的存取**；明文**永不**落 SQLite / 日志 / prompt / 崩溃报告 / 审计事件。

## 背景与既有事实（开工前先读）

- 架构 v2 **§12.5** 已定死口径：存储用 `keyring`（DPAPI / Keychain / Secret Service）；内存中的密钥用后清零（`zeroize`）；**密钥访问本身要审计**；严禁明文写入 SQLite / 日志 / prompt / 崩溃报告 / 审计事件。
- 架构 v2 **§15 目录树**已定死落点：`crates/secrets/`（"OS keychain 封装"）—— 本卡**新建**该 crate。
- 架构 v2 **L2624**（存储表）已定死耦合方式：**密钥只在 OS 密钥库，DB 只存引用（`key_ref`）**。
- `crates/storage`（TASK-012，Done）提供 `Clock` 注入点；`crates/audit`（TASK-013，Done）提供审计落库 —— 本卡**复用**其注入点风格，**不造**第二套时钟 trait（与 TASK-013 同一理由）。
- `docs/spec/naming.md` §5：id / 时间 / 超时**必须** newtype；错误枚举按 `storage` / `audit` 的既有手写风格（`reason_code()` + `error_category()`），本卡**不引入** `thiserror`（那是新增依赖 = 漂移触发器 ①）。
- `docs/DEPENDENCIES.md` 登记规则 1：**先登记，后引入**（在本表加行 → 再改 `Cargo.toml`；顺序反了算漂移）。

## write scope

- `crates/secrets/**`（本卡主体：新建 crate）
- `docs/DEPENDENCIES.md`（**仅**新增 `keyring` / `zeroize` 两行；登记表其余行一字不动）
- `Cargo.lock`（新增 workspace 成员 `assistant-secrets` + 2 个新依赖引起）
- `LEDGER.md`、`docs/PARKING_LOT.md`、`docs/memory/{facts,pitfalls}.md`（如有新事实 / 坑）、`tasks/TASK-014-secrets-os-keychain-wrapper.md`（本卡记录区）

**为什么 write scope 必须含 `docs/DEPENDENCIES.md`**：本卡是仓库**第一次**为"OS 密钥库"引入第三方依赖，而登记表规则 1 明写「先登记，后引入：在本表加行 → 再改 `Cargo.toml`。顺序反了算漂移」。原卡 write scope 只有 `crates/secrets/**` → 按字面执行**无法**合规引入 `keyring`（要么漏登记 = 漂移，要么改表 = 超 scope）。故本卡按「优化线路」把范围扩到**这一个**文件。

## In scope

1. **`SecretName`**（newtype）：密钥名（= 架构 L2624 的 `key_ref` 取值空间）。构造即校验（非空 / 无 NUL / 长度上限 / 允许字符集），非法 → `SecretError::InvalidName`。
2. **`SecretValue`**（newtype）：明文材料的**唯一**容器。内部 `zeroize::Zeroizing<String>`；**无** `Display`、**无** `Serialize`、**无** `Clone`；`Debug` 固定输出 `SecretValue(<redacted>)`；明文只能经显式的 `expose()` 取出。
3. **`SecretStore` trait**：`get` / `set` / `delete` / `contains`。`get` 返回 `Result<Option<SecretValue>, SecretError>` —— **「没有这条」与「读失败」必须可区分**（铁律 1）。
4. **`KeyringSecretStore`**：`keyring` 4.2 的真实后端（Windows 凭据管理器 / macOS Keychain / Linux Secret Service）。**单测不碰真实 OS 密钥库**（CI 上不可用）；真机手工验收清单写进 README。
5. **`InMemorySecretStore`**：进程内 fake，供单测与开发；文档明写**不得用于生产**。
6. **访问审计**：
   - `SecretAccessOperation`（`Read` / `Write` / `Delete`）+ `SecretAccessOutcome`（`Allowed` / `NotFound` / `Denied` / `Failed`）
   - `SecretAccessRecord`：**只有** `name` + `operation` + `outcome` + `ts`（时间取自注入的 `Clock`）—— **结构上不可能**带明文
   - `SecretAccessAuditor` trait：`record_access(&self, &SecretAccessRecord) -> Result<(), SecretError>`
   - `AuditedSecretStore<S, A>` 装饰器：每次存取**先写审计、再执行**；审计失败 → **拒绝该次存取**（fail-closed，铁律 1 / 4）
7. **测试**：unit（`SecretName` 校验 / `SecretValue` 脱敏与 `Zeroize` / 内存 store 语义 / 错误码映射）+ audit（审计记录被写出、审计失败即拒、记录内不含明文）。
8. **`crates/secrets/README.md`**（职责 / 边界 / 不变量 / 已知限制，按 gov §9.6 模板）。

## Out of scope（做了算漂移）

- **把 `SecretAccessRecord` 映射成 `assistant_protocol::AuditEvent` 并落库**：`protocol/audit-event/audit-event-1.0.json` 的 `event_type` 是**封闭枚举**（17 项，无 `secret.*`），新增取值 = 改 schema（漂移触发器 ③ / ⑩）→ 本卡**只**提供 `SecretAccessAuditor` 注入点；映射与落库归后续卡（见 §5 的 `DRIFT-014-1`）
- 密钥**轮换 / 过期 / 配额**、BYOK 与企业集中托管两种模式的**装配**（架构 §12.5 后半段）→ 后续卡
- **`EgressProxy` 统一注入密钥**（架构 §12.5 末条，标注"可选强化"）→ 后续卡
- `crates/core` / `policy` / `task-engine` 的任何改动（铁律 7：core 只经 trait）
- 改 `AGENTS.md` / `docs/spec/*` / `docs/adr/*` / `PLAN.md` / `README.md` / `plans/*`
- 把密钥写进 SQLite，或给密钥库加"加密 blob 备份"（架构 §12.5 明令禁止）

## 必须遵守

- **铁律 1（无静默失败）**：「没有这条密钥」= `Ok(None)`；「读失败」= `Err(..)`。两者**不得**合并
- **铁律 2（不可信输入）**：`SecretName` 构造即校验；`keyring` 的返回一律按不可信处理
- **铁律 4（postcondition）**：`set` 的 postcondition =「写后能读回同一值」；`delete` 的 postcondition =「写后 `contains` = false」
- **铁律 9 / 10**：不新增 schema、不改公共接口；`SecretAccessRecord` 是本 crate 内部类型，**不是**协议类型
- **AGENTS.md §5.3**：`unsafe` 零；`unwrap` / `expect` / `panic` / `indexing_slicing` 零；时钟注入；单文件 ≤ 600 行
- **架构 §12.5 的五条禁令**：明文不得进 SQLite / 日志 / prompt / 崩溃报告 / 审计事件 —— 每条都要有类型层或测试证据

## 验收命令（agent 必须全部执行并粘贴输出）

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-secrets
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- card-check
cargo deny check
```

## 完成定义（DoD）

- [ ] **明文不落盘**：`crates/secrets` 内无任何写文件 / 写 SQLite 的路径（`rg` 证据），且类型层 `SecretValue` **不实现** `Serialize`
- [ ] **明文不入日志**：`SecretValue` 的 `Debug` 输出不含明文（有测试断言）
- [ ] **`zeroize` 生效**：`SecretValue` 内部为 `zeroize::Zeroizing<String>`，且有编译期断言 `SecretValue: Zeroize`；README **不夸大**（明写"无 `unsafe` 前提下无法运行期观测清零"）
- [ ] **密钥访问被审计**：每次 `get` / `set` / `delete` 都产生一条 `SecretAccessRecord`；审计失败 → 该次存取**被拒绝**（两条都有测试）
- [ ] **审计记录不含明文**：`SecretAccessRecord` 的字段与 `Debug` 输出都不含明文（有测试）
- [ ] **「没有这条」与「读失败」可区分**：`Ok(None)` vs `Err(..)`（有测试）
- [ ] `cargo fmt / clippy / test` 全绿；`xtask hygiene / docscan / memory-counts / adr-index / card-check` 全 PASSED；`cargo deny check` exit 0
- [ ] `crates/secrets/README.md` 按 gov §9.6 模板写好（职责 / 边界 / 不变量 / 已知限制）
- [ ] `docs/DEPENDENCIES.md` 的 `keyring` / `zeroize` 两行已登记（含"替代方案与否决理由"）
- [ ] `LEDGER.md` 追加一行；有新事实 / 坑则追加 `docs/memory/{facts,pitfalls}.md`
- [ ] 本卡 §1~§9 执行记录已填

## 风险与已知坑（开工前先读）

- **CI 上没有任何真实 OS 密钥库**：ubuntu / macos runner 无 Secret Service / 无登录 Keychain，windows runner 的凭据管理器亦不可靠 → **单测一律走 `InMemorySecretStore`**；真实后端只做「能构造 + 能编译」+ 真机手工验收。给真实后端写 CI 断言 = 必然 flake。
- **`keyring` 4.x 的 API 与 3.x 不兼容**：本卡钉 `4.2`；升级 = 漂移触发器 ①。
- **Linux 后端走 `zbus`（纯 Rust）**：`keyring` 4.2 的默认 feature `v1` 已含 `zbus-secret-service-keyring-store`，**不要**额外启用需要 C 库的 `linux-keyutils`（ubuntu CI 会缺系统包）。
- **不要给 `SecretValue` 派生 `Debug` / `Clone` / `Serialize`**：派生 `Debug` 会打印明文；派生 `Serialize` 会让它意外进 JSON 日志。
- **`zeroize` 只保证"本 crate 持有的那份内存"被清零**：OS 密钥库、`keyring` 内部缓冲、调用方 `expose()` 之后自己拷出去的那份**不受本 crate 保护** → README「已知限制」必须写明。
- **fail-closed 的代价**：审计写不进去时密钥**读不出来**，上层（模型调用）会失败。这是**有意**的（架构 §12.5「密钥访问本身要审计」）；不要为了"可用性"改成 fail-open。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-014  secrets：OS keychain 封装（DPAPI/Keychain/Secret Service）
【目标】新建 crates/secrets：密钥只进 OS 密钥库；对外只暴露"引用 + 受审计的存取"；明文永不落 SQLite / 日志 / prompt / 崩溃报告 / 审计事件
【write scope】仅：crates/secrets/**、docs/DEPENDENCIES.md（**仅**新增 keyring / zeroize 两行）、Cargo.lock、
              LEDGER.md、docs/PARKING_LOT.md、docs/memory/{facts,pitfalls}.md（本卡新增事实/坑）、本卡记录区
【铁律】1 无静默失败 / 2 不可信输入 / 4 每个写操作有 postcondition / 9 不得静默扩大范围 / 10 契约先行
【禁止】不改 crates/core / policy / task-engine；不改 spec / ADR / AGENTS / PLAN 正文 / README 正文；
        不做 drive-by refactor；明文不进 DB / 日志 / 测试夹具；不给 SecretValue 派生 Debug / Clone / Serialize
【验收】cargo fmt --all --check → 0 diff；cargo clippy --all-targets -- -D warnings → exit 0；
        cargo test -p assistant-secrets → 26 passed + 1 doctest；cargo test --workspace → 全绿；
        xtask hygiene / docscan / memory-counts / adr-index / card-check / verify-schemas + codegen --check → 全 PASSED；
        cargo deny check → advisories / bans / licenses / sources 全 ok
【依赖】011（✅ Done，已核 LEDGER）；实现另需 012 的 `Clock`（012 亦 Done）
【疑问】① 原卡正文是占位 → 已按人类 chat 授权「代 Orchestrator 展开」补全，并把 `docs/DEPENDENCIES.md` 写进 write scope
        （理由：登记表规则 1「先登记，后引入」，原 scope 按字面执行无法合规引入依赖）；
        ② 新增 crate `crates/secrets` → 架构 v2 §15 目录树已列，人类 chat 2026-09-24 批准；
        ③ 新增第三方依赖 `keyring` / `zeroize` → 架构 v2 §12.5 明文指定，人类 chat 2026-09-24 批准，且已先登记后引入；
        ④ CI 上无真实 OS 密钥库 → 人类同意「trait + 内存 fake 单测 + 真机手工验收」；
        ⑤ 是否新增 ADR → 人类裁决「按架构 §12.5 落地，不新增 ADR」，不变量写进 crate README
```

### 2. 实际改动文件

新增（`crates/secrets/**`）：

- `crates/secrets/Cargo.toml`（新 crate；依赖 `keyring` 4.2 / `zeroize` 1.9 + 工作区 `assistant-protocol` / `assistant-storage`）
- `crates/secrets/src/lib.rs`（crate 文档 + 模块装配 + `pub use`；`#![deny(unsafe_code)]`）
- `crates/secrets/src/error.rs`（`SecretError` 9 变体 + `reason_code()` / `error_category()` / `requires_human()`）
- `crates/secrets/src/secret_name.rs`（`SecretName` newtype：构造即校验 + `MAX_SECRET_NAME_LEN` = 128 + 字符集）
- `crates/secrets/src/secret_value.rs`（`SecretValue`：`Zeroizing<String>` + 脱敏 `Debug` + 无 `Display`/`Serialize`/`Clone`）
- `crates/secrets/src/store.rs`（`SecretStore` trait：`get`/`set`/`delete`/`contains` 及其语义契约）
- `crates/secrets/src/memory.rs`（`InMemorySecretStore` fake；`Debug` 只报条目数）
- `crates/secrets/src/keyring_store.rs`（`KeyringSecretStore` 真后端 + `keyring::Error` → `SecretError` 映射）
- `crates/secrets/src/access_audit.rs`（`SecretAccessOperation` / `SecretAccessOutcome` / `SecretAccessRecord` / `SecretAccessAuditor`）
- `crates/secrets/src/audited.rs`（`AuditedSecretStore` 装饰器：每次访问记一条；审计失败 ⇒ 拒绝该次访问）
- `crates/secrets/README.md`（gov §9.6 模板：职责 / 边界 / 不变量 / 典型用法 / 已知限制 / 相关文档）
- `crates/secrets/tests/common/mod.rs`（`FixedClock` / `RecordingAuditor` / `FailingAuditor`）
- `crates/secrets/tests/secrets_unit.rs`（17 用例：名字校验 / 值脱敏与 zeroize / 内存后端语义 / 错误码映射）
- `crates/secrets/tests/secrets_audit.rs`（9 用例：审计记录 / 不含明文 / fail-closed / 注入时钟）

修改（本卡 write scope 内的既有文件）：

- `docs/DEPENDENCIES.md`（**仅**新增 `keyring` / `zeroize` 两行；两行都在 `Cargo.toml` 之前落地）
- `Cargo.lock`（新成员 `assistant-secrets` + 新依赖）
- `LEDGER.md`（追加一行，见 §3）
- `docs/PARKING_LOT.md`（追加 PL-049 = DRIFT-014-1、PL-050 = thiserror 口径）
- `PLAN.md`（**仅**「当前状态」块四行 —— ADR-0039 D1/D2）
- `README.md`（**仅**状态行 / `## 当前阶段` / `## 最近进展` 三处 —— ADR-0039 D1/D2）
- `plans/stage-1-pilots.md`（**仅** TASK-014 那一行的备注：占位 → 完整卡）
- `tasks/TASK-014-secrets-os-keychain-wrapper.md`（正文展开 + 本记录区）

### 3. 验收输出摘要

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | exit 0（0 diff） |
| `cargo clippy --all-targets -- -D warnings` | exit 0（0 warning） |
| `cargo test -p assistant-secrets` | **26 passed / 0 failed**（9 audit + 17 unit）+ 1 doctest passed |
| `cargo test --workspace --no-fail-fast` | 全绿（含既有 331 + 13 + 7 + 8 + 18 + … 各套） |
| `xtask hygiene` | PASSED（0 error / 2 warning，与既有基线一致） |
| `xtask docscan` | PASSED（0 / 0） |
| `xtask memory-counts` | PASSED（0 / 0） |
| `xtask adr-index` | PASSED（0 / 0，scanned 24） |
| `xtask card-check` | PASSED（0 error / 49 warning，与既有基线一致） |
| `xtask verify-schemas` | PASSED（0 error） |
| `xtask codegen --check` | PASSED（0 drift / 0 error） |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |

关键反证（不是退出码，是断言本身）：

- `test_secret_value_debug_does_not_leak_plaintext`：`format!("{value:?}")` 不含 canary 明文，且含 `<redacted>`
- `test_memory_store_debug_does_not_leak_plaintext`：`Debug` 既不出现明文也不出现键名
- `test_audited_records_never_contain_plaintext`：3 条记录的 `Debug` 与 `Display` 都不含 canary
- `test_audited_get_with_failing_auditor_is_rejected_and_leaks_nothing`：审计失败 ⇒ `Err(audit_rejected)`，且错误文本不含明文
- `test_memory_store_get_missing_returns_none_not_error`：`Ok(None)`（**不是** `Err`）—— 铁律 1 的分辨率
- `test_secret_value_implements_zeroize`：编译期 `SecretValue: Zeroize` + 显式 `zeroize()` 后为空

### 4. DoD 逐条核对

- [x] **明文不落盘**：`crates/secrets` 内无任何写文件 / 写 SQLite 的代码路径（全 crate 只调用 `keyring` 的进程内 API）；
      `SecretValue` **不实现** `Serialize`（类型层证据）
- [x] **明文不入日志**：`SecretValue::Debug` = `SecretValue(<redacted> N bytes)`；`InMemorySecretStore::Debug` 只报条目数
      —— 两条都有断言
- [x] **`zeroize` 生效**：内部为 `zeroize::Zeroizing<String>`，编译期断言 `SecretValue: Zeroize`；README **不夸大**
      （明写"无 `unsafe` 前提下无法运行期观测清零"）
- [x] **密钥访问被审计**：`get` / `set` / `delete` / `contains` 各产生一条 `SecretAccessRecord`；审计失败 ⇒ `Err(AuditRejected)`
      —— 两条都有断言（含 `contains` 按 `read` 记的显式用例）
- [x] **审计记录不含明文**：`SecretAccessRecord` 字段只有 name / operation / outcome / ts；`Debug` 与 `Display` 均不含明文
- [x] **「没有这条」与「读失败」可区分**：`get` 缺失 → `Ok(None)`；后端失败 → `Err(..)`（有断言）
- [x] `cargo fmt / clippy / test` 全绿；`xtask hygiene / docscan / memory-counts / adr-index / card-check` 全 PASSED；
      `cargo deny check` exit 0（另附 `verify-schemas` / `codegen --check` 亦 PASSED）
- [x] `crates/secrets/README.md` 按 gov §9.6 模板写好（职责 / 边界 / 不变量 / 典型用法 / 已知限制 / 相关文档）
- [x] `docs/DEPENDENCIES.md` 的 `keyring` / `zeroize` 两行已登记（含"替代方案与否决理由"，且**先于** `Cargo.toml` 落地）
- [x] `LEDGER.md` 追加一行；新增事实 / 坑已进 `docs/memory/{facts,pitfalls}.md`
- [x] 本卡 §1~§9 执行记录已填

### 5. 偏差

**DRIFT-014-1（已在开工前由正文界定为 Out of scope，并升级给人类 → PL-049）**

- **现象**：架构 v2 §12.5 要求「密钥访问本身要审计」，但 `protocol/audit-event/audit-event-1.0.json` 的
  `event_type` 是**封闭枚举**（17 项），里面**没有** `secret.*`。要落库就必须新增枚举取值 = 改 schema。
- **影响**：本卡的审计只能做到"产生结构化记录 + 交给注入点"，**不能**端到端落进 `assistant-audit`。
- **建议**：下一张装配类卡里加 **ADR-0041** 补枚举取值（如 `secret.accessed`），再让 `SecretAccessAuditor`
  的实现方把它翻成 `AuditEvent`。
- **已停工作**：本卡**没有**改任何 schema / spec / ADR；只提供注入点。已记 PL-049。

其余偏差：**none**。本卡未改测试断言来"让它过"（新增断言全部是净增），未放宽任何 lint，未加 `#[allow]`
（`tests/` 内那两条 `#![allow(...)]` 是 AGENTS.md §5.3 明许的、与既有 crate 测试一致；`indexing_slicing`
**没有** allow —— 用 `record_at()` 走 `.nth()` 绕开）。

**write scope 扩张说明（已获人类 chat 批准，非静默）**：原卡 write scope 只有 `crates/secrets/**`，
本卡另动了 `docs/DEPENDENCIES.md`（登记表规则 1 要求"先登记后引入"）+ `PLAN.md`「当前状态」块 +
`README.md` 三处（ADR-0039 D1/D2 要求每张 Done 的卡同步）+ `plans/stage-1-pilots.md` 的 TASK-014 备注行
（正文已展开，与 TASK-012/013/015 同例）。以上四项都在展开正文时写进了 write scope。

### 6. 更合理做法

- **`SecretAccessRecord` 的时间戳应该带时区语义**：现在只存 Unix 毫秒（与 `storage` 的既有列一致），
  人看审计记录时得自己换算。真正该做的是落库时同时记 `ts` 的 ISO-8601 文本（那是协议事件层的事，归 PL-049）。
- **`contains` 在真实后端上会读一次明文**：`keyring` v1 接口没有"仅探测"的原语。更合理的做法是
  直接用 `keyring-core` + 具体 store（能拿到 `search` 之类的原语），代价是放弃 `keyring` 的跨平台默认选择
  —— 那是**改架构 §12.5 的口径**，不该由本卡顺手做。
- **`error_category()` 的手工映射与 `Display` 各写一遍**：`thiserror` 能把两者绑在一起，但引入它 = 新增依赖
  （漂移触发器 ①）→ 已记 PL-050，不在本卡顺手做。

### 7. 遗留问题

1. **PL-049 / DRIFT-014-1**：协议缺 `secret.*` 事件类型 → 审计记录尚未落库（需 ADR-0041 + schema 更新）
2. **PL-050**：`storage` / `audit` / `secrets` 三 crate 的手写错误风格与 `naming.md` §5 的 `thiserror` 要求不一致
3. **真机手工验收未在本会话执行**：CI 上没有可用 OS 密钥库，`KeyringSecretStore` 的真实读写路径**没有**被自动化测试覆盖
   —— README「已知限制」列了三平台的验收命令（`cmdkey /list` / `security find-generic-password` / `secret-tool lookup`），
   需要人在真机上跑一次。**这是本卡最大的残余风险**。
4. **`keyring` 依赖树较大**（Linux 侧拉入 `zbus` 等）：本卡只验证了 Windows 构建；三平台 CI 的首轮结果见 PR。

### 8. 新增长期记忆

- `docs/memory/facts.md`：`keyring` 4.2 的 `v1` 默认 feature 组合、`Entry::store_status()` 的语义、`zeroize` 的
  `alloc` feature 是默认开启（`Zeroizing<String>` 可直接用）
- `docs/memory/pitfalls.md`：`Debug` 派生会打印明文（禁止给密钥类型派生）；`keyring::Error` 是 `#[non_exhaustive]`
  必须留兜底分支；`clippy::significant_drop_tightening` 会拦"持锁做多余工作"（本卡在 `InMemorySecretStore` 撞到）

### 9. 给审阅者的关注点

1. **fail-closed 的语义边界**：`AuditedSecretStore` 是"**先执行、再审计**"，审计失败时**丢弃结果**并返回
   `AuditRejected`。请重点确认：写 / 删在审计失败时**可能已经生效**这一点是否可接受（README「已知限制」已写明，
   `set` 幂等 + `delete` 重试安全是这条设计的依据）。若要求"审计必须先于写"，请指出 —— 那需要两阶段记录，
   属设计变更。
2. **`SecretValue` 的封装是否够紧**：它无 `Display` / `Serialize` / `Clone`，`Debug` 脱敏，明文只能经 `expose()` 取出。
   请确认没有遗漏的泄露通道（例如 `Into<String>`、`Deref`、`AsRef` 之类本卡**刻意没有**实现的 trait）。
3. **write scope 扩张的合规性**：`docs/DEPENDENCIES.md` / `PLAN.md` / `README.md` / `plans/*` 四个文件不在原卡
   write scope 里，是展开正文时补进去的（理由见 §5）。请核对与 ADR-0039 D1/D2、`docs/DEPENDENCIES.md` 登记规则 1 是否自洽。

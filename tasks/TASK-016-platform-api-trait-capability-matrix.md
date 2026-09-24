# TASK-016　`platform/api`：统一 trait + `CapabilityMatrix` + `TargetDescriptor` + `NormalizedPoint` + `Fingerprint` 类型

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：011　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：011（`protocol` crate 的 schema + `ErrorCode` 枚举已落地；本卡**复用**它，不新增错误码）
- **预估**：M　**难度**：M
- **write scope**：`crates/platform/api/**`、`protocol/capability-matrix*`
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）、架构 v2 §3.1（分层）/ §6.2（`TargetDescriptor`）/ §6.9（坐标归一化）/ §7.3（指纹）/ **§13.1.1（抽象接口）/ §13.1.2（能力矩阵）**、`docs/spec/capability-matrix.md`、`docs/spec/naming.md` §7、**铁律 7 / 铁律 8**、ADR-0019 N1（负向验证）

**目标**

新建 `crates/platform/api`（crate 名 `assistant-platform-api`）—— **铁律 7 指定的唯一平台入口**：
纯类型（`TargetDescriptor` / `NormalizedPoint` / `Fingerprint` / `CapabilityMatrix`）+ 三个 trait 的**形状**。
本卡**不实现任何平台**（那是 TASK-017 / 018），也**不实现策略判定**（TASK-021）。

**为什么现在建这个 crate（显式声明，不静默扩范围）**

新建 `crates/platform/` 顶层目录命中**漂移触发器 ②**（加 crate / 顶层目录）。但它是**计划内**的：
① `plans/stage-1-pilots.md` 批次表已把 `crates/platform/api/**` 分给 TASK-016、`crates/platform/windows/**` 分给 TASK-017 / 018 / 040；
② `crates/core/tests/arch_layering.rs`（TASK-015 已 Done）已把 `assistant-platform-api` 写成「铁律 7 明确允许的**唯一**入口」；
③ `docs/DEPENDENCIES.md` 已把 `crates/platform/windows` 登记为 `windows` crate 的阶段 1 使用方。
→ 因此**不是**触发器 ② 所指的「未经计划的扩范围」；按铁律 9 在此**显式声明**，并在执行记录 §5 复述。

**In scope（本卡交付物）**

| # | 交付物 | 依据 |
|---|---|---|
| 1 | `crates/platform/api/Cargo.toml`（`assistant-platform-api`，**零第三方依赖**，只依赖 `crates/protocol`） | 铁律 7 + `docs/DEPENDENCIES.md`「先登记后引入」 |
| 2 | `TargetDescriptor` + 有序候选选择器链（`kind` / `value` / `score` / `locale_dependent` / `ttl_ms`）+ `ResolutionPolicy` | 架构 v2 §6.2 |
| 3 | `NormalizedPoint` + `CoordinateSpace`（内部统一 = **全局逻辑坐标、原点主显示器左上**） | 架构 v2 §6.9 规则 1 / 2 |
| 4 | `Fingerprint`（不可与 `TargetDescriptor` 混用的独立类型） | 架构 v2 §7.3 |
| 5 | `CapabilityMatrix`（运行时探测结果）+ `validate()` 把 `docs/spec/capability-matrix.md` §4 的 4 条不变量变成**会失败的返回值** | 架构 v2 §13.1.2 + spec §4 |
| 6 | 三个 trait 的**形状**：`PlatformService` / `WindowProvider` / `UiAutomationProvider`（`Send + Sync`，全部返回 `Result`，方法可取消 + 带超时参数） | 架构 v2 §13.1.1 |
| 7 | `ResolvedWindow` / `ResolvedElement` = **不透明句柄**（不含平台对象、不可序列化、不跨进程） | **铁律 8** + 架构 v2 §3.2 |
| 8 | `crates/platform/api/README.md`（职责 / 边界 / **不变量** / 已知限制） | gov §5.4 + 阶段 1 DoD |
| 9 | 单测：正向 + **负向**（ADR-0019 N1） | gov §5.5 + ADR-0019 |
| 10 | `protocol/capability-matrix*` 的**口径修正**（见「待裁决」Q2 / Q3） | 本卡 write scope |

**Out of scope（做了算漂移）**

- 任何**平台实现**（`windows` crate、UIA、SendInput、AT-SPI、CDP…）→ TASK-017 / 018 / 040 / 048
- 策略引擎的判定算法与规则 DSL → TASK-021；本卡只提供**矩阵数据 + 不变量**
- Tool 与 capability 的绑定 → `docs/spec/tool-schema.md`
- 真实的 `ResolvedElement` 缓存 / 租约实现 → TASK-025
- 截图 / 脱敏 / 视觉验证 → TASK-041 / 042
- 任何 `unsafe`、任何 `#[allow]` 放宽（漂移触发器 ⑥）
- 新增**第三方**依赖（含 `async-trait`、`tokio`、`serde` 之外的任何 crate）→ 漂移触发器 ①，见「待裁决」Q1
- 改 `crates/core/**` 与 `crates/core/tests/arch*`（不在本卡 write scope；现有断言已把 `assistant-platform-api` 列为**允许**项，无需改动）

**必须遵守**

1. **零平台实现依赖**：`crates/platform/api/Cargo.toml` 的 `[dependencies]` 只许有 `assistant-protocol`（若有）与已登记依赖；`windows` / `windows-sys` / `objc2` / `gtk` / `atspi` 一律禁止（arch test 会拦 `core` 侧，本 crate 侧由评审 + 卡面约束）。
2. **`ResolvedElement` / `ResolvedWindow` 不得实现 `Serialize` / `Deserialize`**（铁律 8：element/句柄不得跨进程）。做法：不派生，并在 README 的不变量节写明**为什么**（不是「暂时没写」）。
3. **`CapabilityMatrix::validate()` 必须能失败**：至少覆盖 ① 风险单调升级（不得 downgrade）② `L3+ ⇒ Approval=required`、`L5 ⇒ Approval=forbidden` ③ `Resource` 只许 5 类（`read/write/send/invoke/destroy`）④ `SideEffect` 必填。每条都要有**负向用例**（喂已知坏矩阵 → 断言真的报错）。
4. **错误必须带 `ErrorCode`**（铁律 1 + `docs/spec/error-codes.md`）：本卡**不新增**错误码；缺什么就复用 `crates/protocol` 现有枚举，并在执行记录里写明复用了哪个。
5. **命名按 `docs/spec/naming.md`**：受控词汇 `Target` / `Lease` / `Fingerprint` / `Anchor`；禁止 `Element`/`Object`/`Item` 混用（**例外**：架构 v2 §13.1.1 的 `ResolvedElement` 是既有术语，本卡保留）。
6. **纯函数优先**：`validate()` / 坐标换算 / 指纹比较等判定逻辑必须是**纯函数**（`xtask/src/arch.rs` 的 flaky 教训见 `docs/memory/pitfalls.md` 2026-09-24 条）。
7. **不得为让门禁变绿而放宽判据或改断言**（AGENTS.md §4 ⑦）。
8. 单文件 ≤ 400 行（软）/ 600（硬）；函数 ≤ 80 行、参数 ≤ 6 个。

**步骤**

1. **环境记录**（OS / `rustc` 版本 / 输入 fixture 路径）—— 本卡无真机 fixture（纯类型 + trait），记录清楚即可。
2. **先读再写**：架构 v2 §3.1 / §6.2 / §6.9 / §7.3 / §13.1.1 / §13.1.2 + `docs/spec/capability-matrix.md` 全文 + `docs/spec/naming.md` §7。**契约先行**：若发现 spec 与架构 v2 冲突 → 记 DRIFT（触发器 ⑧），不要自己挑一个。
3. **建 crate 骨架**：`crates/platform/api/{Cargo.toml,README.md,src/lib.rs}`；`Cargo.toml` 继承 workspace（`edition.workspace = true`、`lints.workspace = true`）。根 `Cargo.toml` 的 `members` 已含 `crates/*`，**无需改根清单**（若实际不生效 → DRIFT）。
4. **按上表 2~7 落地类型与 trait**，每个类型都写文档注释（公共 API 100% 有注释，gov §5.3）。
5. **单测**（`crates/platform/api/tests/` 或 `src/**` 的 `#[cfg(test)]`）：正向 + 负向；**至少**覆盖 `validate()` 的 4 条不变量各一个负向用例、坐标换算、指纹相等/不等语义。
6. **`protocol/capability-matrix*` 修正**：按「待裁决」Q2 / Q3 的人类裁决执行；若裁决未到 → 本卡**只做 Q2 的悬空引用修复**（`description` 里的 `TASK-103` 指向不存在的卡），Q3 留 DRIFT。
7. **跑门禁**（见下）；不合格 → DRIFT（`DRIFT-016-x`）。
8. **填执行记录**（9 节）+ 更新 `LEDGER.md` + 记忆（新事实 / 坑）。

**待裁决（动手前必须有答案；未裁决就只做步骤 6 里 Q2 的最小修复）**

- **Q1（漂移触发器 ①）**：架构 v2 §13.1.1 写的是 `#[async_trait]` —— 引入 `async-trait` crate = **新增第三方依赖**。
  **建议默认**：本卡用 **Rust 原生 `async fn` in trait**（Rust 1.88 / edition 2024 已支持），**不引入任何依赖**；
  `dyn` 兼容性等到 TASK-017 有真实消费方时再按需要裁决（届时若必须 `dyn`，用 `#[trait_variant]` 或手写 `Pin<Box<dyn Future>>`，仍不引依赖）。
  **代价**：与架构 v2 §13.1.1 的字面写法（`#[async_trait]`）不一致 → 需人类确认「文档字面 vs 零依赖」取哪个。
- **Q2（悬空引用）**：`protocol/capability-matrix/capability-1.0.json` 的 `description` 写着「**TASK-103** (revised)」，但仓库 `tasks/` 下**没有** TASK-103 卡（只有 100 / 101）。按 §5.3「无卡号即 CI 失败」的精神，这是悬空引用。
  **建议默认**：改为指向真实来源（架构 v2 §13.1.2），并把 `TASK-103` 字样删掉。属**纯文案修正**，不改任何字段 / 类型。
- **Q3（命名冲突，漂移触发器 ⑧）**：同一个名字 `CapabilityMatrix` 现在指两件不同的事 ——
  ① `protocol/capability-matrix/capability-1.0.json` = **稳定能力标识目录**（`<layer>.<capability>` 2 段式，如 `target.list`）；
  ② 架构 v2 §13.1.2 = **运行时探测结果**（`probed_at` / `platform` / `channels` / `degradations`）。
  **建议默认**：本卡的 Rust 类型用 ②（`CapabilityMatrix` = 探测结果），把 ① 在 spec / schema 层面明确命名为 **`CapabilityCatalog`**（改 `title` 字段 = 改契约 → 需 ADR）。
  **在裁决前**：本卡**只**实现 ②，`protocol/capability-matrix*` 只做 Q2 的文案修复，**不动** ① 的字段与 `title`。

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `crates/platform/api` 落地且 `[dependencies]` **无第三方 crate**（含无 `async-trait`；按 Q1 裁决）
- [ ] `TargetDescriptor` / `NormalizedPoint` / `Fingerprint` / `CapabilityMatrix` 四类型齐备，公共 API **100%** 有文档注释
- [ ] `ResolvedWindow` / `ResolvedElement` **未派生** `Serialize` / `Deserialize`（铁律 8）
- [ ] `CapabilityMatrix::validate()` 覆盖 4 条不变量，且**每条都有负向用例**（ADR-0019 N1）
- [ ] 三个 trait 形状齐备（`PlatformService` / `WindowProvider` / `UiAutomationProvider`）
- [ ] `crates/platform/api/README.md` 含职责 / 边界 / 不变量 / 已知限制
- [ ] `protocol/capability-matrix*` 的 Q2 悬空引用已修（Q3 未裁决则留 DRIFT）
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿；`cargo test -p assistant-platform-api` 全绿
- [ ] `cargo test -p assistant-core arch::` 仍全绿（本卡**不**改 arch 断言；若它因本卡变红 → 说明越界，停下）
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
      （⚠ `refscan` 是**既有 FAILED 基线 151 error**，见 PL-058 —— 如实记录，不得为它放宽判据）
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`
- [ ] 无任何 Out of scope 的文件被修改

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-platform-api
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- refscan
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-016　`platform/api`：统一 trait + `CapabilityMatrix` + `TargetDescriptor` + `NormalizedPoint` + `Fingerprint` 类型
【目标】新建 `crates/platform/api`（crate 名 `assistant-platform-api`）= 铁律 7 的唯一平台入口：
        纯类型 + 三个 trait 的**形状**，**零第三方依赖**
【write scope】仅：`crates/platform/api/**`、`protocol/capability-matrix*`
        （另有 3 处**连带**改动，已登记为 DRIFT-016-1 / DRIFT-016-2，见 §5）
【铁律】铁律 7（core 不得调平台 API）/ 铁律 8（句柄不得跨进程）/ 铁律 1（无静默失败）/
        铁律 10（契约先行）+ 漂移触发器 ⑥（零 `#[allow]`）
【禁止】任何平台实现（Win32 / UIA / AT-SPI / CDP）、策略判定算法、Tool ↔ capability 绑定、
        `unsafe`、`#[allow]`、新增第三方依赖（含 `async-trait`）、改 `crates/core/**`
【验收】`cargo fmt --all --check` → 0 diff｜`cargo clippy --all-targets -- -D warnings` → exit 0｜
        `cargo test --workspace` → **28 target / 514 passed / 0 failed**｜
        `cargo test -p assistant-core arch::` → 5 passed｜
        `xtask hygiene / docscan / card-check / memory-counts / adr-index / check-ledger / check-migrations` → PASSED｜
        `xtask refscan` → FAILED（**既有 151 error**，PL-058）
【依赖】TASK-011（`crates/protocol` 的 schema + `ErrorCode` 已落地）→ 已核对 LEDGER：Done
【疑问】Q1 / Q2 / Q3 三项待裁决已由人类 2026-09-24 授权「按建议默认执行」：
        Q1 = 原生 RPITIT（零依赖，不引 `async-trait`）；Q2 = 悬空引用改指架构 v2 §13.1.2；
        Q3 = 本卡只实现「运行时探测结果」语义、**不动** `title`、改名 `CapabilityCatalog` 留 ADR。
        除此之外无疑问。
```

### 2. 实际改动文件

**新建（本卡主体，20 个文件，全部在 `crates/platform/api/**` 内）**

- `Cargo.toml`（24 行）：`assistant-platform-api`，`[dependencies]` 只有 `serde`（已登记，本卡是新使用方）与工作区内的 `assistant-protocol`
- `README.md`（72 行）：职责 / 边界 / 不变量 / **测试** / 已知限制
- `src/lib.rs`（68 行）：模块头（职责 / 边界 / 5 条不变量 / 典型用法）+ 重导出
- `src/error.rs`（101 行）：`PlatformError` / `PlatformResult`（错误必带 `ErrorCode`）
- `src/geometry.rs`（246 行）：`CoordinateSpace` / `NormalizedPoint` / `PhysicalPoint` + `exact_i32_from_f64`
- `src/fingerprint.rs`（84 行）：`Fingerprint`（`sha256:` + 64 位小写 hex）
- `src/target.rs`（417 行）：`TargetDescriptor` / `SelectorCandidate` / `ResolutionPolicy` / `OnNotFound` / `OnAmbiguous`
- `src/capability.rs`（273 行）：`ResourceAccess` / `SideEffect` / `RiskLevel` / `ApprovalRequirement` / `CapabilityEntry`
- `src/matrix.rs`（370 行）：`ProbeContext` / `CapabilityMatrix` / `ChannelState` / `Degradation` / `CapabilityMatrixError` / `validate()`
- `src/handle.rs`（104 行）：`ResolvedWindow` / `ResolvedElement` / `LocalHandleId`（**不透明句柄**）
- `src/traits/mod.rs`（35 行）+ `session.rs`（148 行）+ `window.rs`（276 行）+ `ui.rs`（501 行）：三个 trait 的形状与参数类型
- `tests/common/mod.rs`（27 行）：`ok_or_fail`（**不使用 `expect` / `unwrap` / `panic!`** 的取 `Ok` 值工具）
- `tests/capability_matrix.rs`（242 行）：4 条不变量逐条正 / 负用例 + `ProbeContext` 读取侧不漂移
- `tests/target_descriptor.rs`（157 行）：候选链 / 解析策略 / `validate()` 正负用例
- `tests/geometry.rs`（147 行）：坐标校验、换算舍入、**`i32` 边界**（含越界一格）
- `tests/fingerprint.rs`（78 行）：前缀 / 长度 / 大小写 / 非 hex
- `tests/handles_not_serializable.rs`（81 行）：铁律 8 的机器校验（扫 `src/handle.rs` 源码，含 2 个负向样本）

**修改**

- `protocol/capability-matrix/capability-1.0.json`：`description` 去掉悬空引用 `TASK-103 (revised):`（**Q2**）；`title` 与全部字段**未动**
- `Cargo.toml`（根）：`[workspace]` 的 `members` 加 `crates/platform/api`、`exclude` 加 `crates/platform`（**DRIFT-016-2**）
- `docs/DEPENDENCIES.md`：`serde` 行的「使用方」列加 `crates/platform/api`（**DRIFT-016-1**）
- `Cargo.lock`：新 crate 的依赖解析（构建产物）
- `docs/PARKING_LOT.md`：+2 行（**PL-064** = `CapabilityMatrix` 一名两物 / 改名留 ADR；**PL-065** = `MEMORY.md` §1 快照落后于事实）
- `MEMORY.md`：「各文件当前规模」表同步（facts 162 / 111、pitfalls 208 / 102）
- `docs/memory/facts.md`：+3 条；`docs/memory/pitfalls.md`：+2 条（见 §8）
- `LEDGER.md`：本卡一行（见提交）

### 3. 验收输出摘要

- `cargo fmt --all --check` → **0 diff**（退出码 0）
- `cargo clippy --all-targets -- -D warnings` → **exit 0**（`-p assistant-platform-api --all-targets` 单独跑同样 exit 0）
- `cargo test -p assistant-platform-api` → **7 target / 45 passed / 0 failed**（lib 2 + capability_matrix 14 + fingerprint 6 + geometry 11 + handles_not_serializable 3 + target_descriptor 8 + doctest 1）
- `cargo test --workspace --no-fail-fast` → **28 target / 514 passed / 0 failed**（TASK-016 之前 = 21 / 469；差额 = 本 crate 的 7 target / 45 tests）
- `cargo test -p assistant-core arch::` → **5 passed**（本卡未改 `crates/core/**`，断言未动）
- `xtask hygiene` → **PASSED**（98 文件 / **0 error** / 3 warning = 既有 3 个 `xtask` 超长文件；TASK-016 之前是 80 文件 / 0e / 3w）
- `xtask docscan` → **PASSED**（160 / **0e** / 572w）—— 本卡填完 9 节执行记录后，「整节为空」类 Warning 自然减少（581 → 572）
- `xtask card-check` → **PASSED**（91 / 0e / 49w）
- `xtask memory-counts` → **PASSED**（8 / 0e / 0w）
- `xtask adr-index` → **PASSED**（25 / 0e / 0w）
- `xtask check-ledger` → **PASSED**（0e / 0w）
- `xtask check-migrations` → **PASSED**（0e / 0w）
- `cargo run -p xtask -- verify-schemas` → **PASSED**（5 schema / 0e）
- `cargo run -p xtask -- codegen --check` → **PASSED**（5 文件 / **0 drift**）—— Q2 只改 `description`，而 `render_capability()` 是**固定模板**（不读 `description`），故生成物无需重生成
- `cargo deny check` → **advisories ok, bans ok, licenses ok, sources ok**（exit 0）
- ⚠ `xtask refscan` → **FAILED（151 error / 0 warning）**：与 TASK-016 之前的 **151** 逐字相同 = **既有基线、本批新增 0**（PL-058 跟踪，不得为它放宽判据）

### 4. DoD 逐条核对

- [x] card 标题声明的能力可被测试用例覆盖：4 个类型 + 3 个 trait + 句柄约束都有对应测试文件（§2）
- [x] `crates/platform/api` 落地且 `[dependencies]` **无第三方 crate**：只有 `serde`（Approved，本卡补登「使用方」）与工作区内的 `assistant-protocol`；**无 `async-trait`**（Q1 裁决 = 原生 RPITIT）
- [x] `TargetDescriptor` / `NormalizedPoint` / `Fingerprint` / `CapabilityMatrix` 四类型齐备，公共 API **100% 有文档注释**（`missing_docs` 是 workspace warn，`-D warnings` 下 exit 0 = 机器证据）
- [x] `ResolvedWindow` / `ResolvedElement` **未派生** `Serialize` / `Deserialize`：`tests/handles_not_serializable.rs` 扫源码断言（+ 2 个负向样本证明扫描器真会报）
- [x] `CapabilityMatrix::validate()` 覆盖 4 条不变量，且**每条都有负向用例**（ADR-0019 N1）：`tests/capability_matrix.rs` 里空列表 / 空白 id / 重复 id / L3 用 auto / L5 非 forbidden / L1 被 forbidden 各一条
- [x] 三个 trait 形状齐备：`PlatformService` / `WindowProvider` / `UiAutomationProvider`（`Send + Sync`，全部返回 `Result`，带超时参数）
- [x] `crates/platform/api/README.md` 含职责 / 边界 / 不变量 / 已知限制（另加「测试」一节）
- [x] `protocol/capability-matrix*` 的 Q2 悬空引用已修（`title` 与字段未动；Q3 的改名已留 PL-064 + §7）
- [x] `cargo fmt --all --check` 0 diff
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [x] `cargo test --workspace` 全绿（28 / 514 / 0）；`cargo test -p assistant-platform-api` 全绿（7 / 45 / 0）
- [x] `cargo test -p assistant-core arch::` 仍全绿（5 passed；本卡未改 arch 断言）
- [x] `xtask hygiene / memory-counts / adr-index / docscan / card-check` 全部 PASSED（`refscan` = 既有 151 error 基线，如实记录）
- [x] LEDGER.md 追加一行；新增 3 条 FACT + 2 条 PITFALL（§8）
- [ ] **无任何 Out of scope 的文件被修改** → **不成立**，有 2 项已登记的连带改动：**DRIFT-016-1**（`docs/DEPENDENCIES.md` 1 行）与 **DRIFT-016-2**（根 `Cargo.toml` 的 `[workspace]`）。两者都不是"顺手改"，而是**不做就无法满足其它硬约束**，详见 §5；请人类裁决是否接受。

### 5. 偏差

**DRIFT-016-1（超出 write scope：`docs/DEPENDENCIES.md`）**

- **现象**：本卡的 `Cargo.toml` 把 `serde` 加为**新使用方**，而 `docs/DEPENDENCIES.md` 的 `serde` 行「使用方」列只写了 `crates/protocol`。
- **影响**：`docs/DEPENDENCIES.md` 登记规则 2 明写「同一个 crate 被多个包使用时，**使用方**列写全部」；规则 1 明写「**先登记，后引入**」。不补这一列，登记表就与实际依赖不符（下次读表的人会以为 `serde` 只有 `crates/protocol` 在用）。
- **建议 / 已做**：把「使用方」列改为 `crates/protocol` / `crates/platform/api`（标注 2026-09-24、TASK-016）。**只改这一格**，不动版本 / 批准人 / 日期。
- **为什么不等裁决**：登记规则 1 要求「同批」；若留到下一张卡，就形成"已引入但未登记"的窗口，那正是规则要防的状态。
- **可否回退**：一行回退，且回退后 `serde` 行重新变成不完整登记。

**DRIFT-016-2（超出 write scope：根 `Cargo.toml` 的 `[workspace]`）**

- **现象**：`members` 原本是 `["xtask", "crates/protocol", "crates/*", "apps/*"]`。新建 `crates/platform/api` 后 `crates/*` 会命中 `crates/platform`（**中间层目录、无 `Cargo.toml`**）→ `cargo metadata` 直接失败：`failed to load manifest for workspace member crates\platform`。
- **影响**：不改就**无法构建**本卡要求的 crate（`crates/platform/windows` 是 TASK-017 / 018 / 040 的落点，所以 `crates/platform/` 这个中间层目录会长期存在，不是一次性问题）。
- **建议 / 已做**：`members` 显式加 `crates/platform/api`、`exclude` 加 `crates/platform`。**未**改任何 lint / profile / 依赖段。
- **更彻底的替代**（未做，建议留给 Orchestrator）：把 `members` 从 glob 改成**显式列举**（glob 与"中间层目录"天然冲突）。
- **可否回退**：两行回退，但回退后 workspace 无法加载。

**Q1 / Q2 / Q3 裁决的落地方式（人类 2026-09-24 授权「按建议默认执行」）**

- **Q1**：三个 trait 用 **RPITIT**（`-> impl Future<Output = …> + Send`），**不引 `async-trait`**。代价是 trait **不是 `dyn`-compatible**（写进 `README.md`「已知限制」与 `src/traits/mod.rs` 模块头）；若 TASK-017 需要 `dyn`，届时按需裁决（仍不引依赖）。
- **Q2**：`capability-1.0.json` 的 `description` 删掉 `TASK-103 (revised):`（该卡号在 `tasks/` 下不存在 = 悬空引用）。**未动** `title` 与任何字段。
- **Q3**：本卡的 Rust 类型取「运行时探测结果」语义；把能力**标识目录**改名 `CapabilityCatalog` 要改 schema `title` = 改契约 → **本卡不动**，已提 **PL-064**（含建议的 ADR 范围）。

**本 crate 的「零 `#[allow]`」是硬约束下的实现选择（不是放宽，也不是新规矩）**

- 卡片 Out of scope 明写「任何 `unsafe`、任何 `#[allow]` 放宽（漂移触发器 ⑥）」，而 `[lints]` 继承 workspace 的 `clippy::expect_used / unwrap_used / panic = deny`。仓库既有 11 个测试文件的做法是在**文件头**写 `#![allow(...)]`（`AGENTS.md` §5.3「`tests/` 内可 allow」）。
- 本卡选择**更严的一条**：全 crate（含 `tests/`）**一个 `#[allow]` 都不写** —— 测试改用 `tests/common/mod.rs::ok_or_fail`（先 `assert!(is_ok)` 再 `let Ok(v) = … else { unreachable!() }`），断言优先比较**整个 `Result`**。
- 由此产生的唯一"非显然代码"是 `src/geometry.rs::exact_i32_from_f64`：`f64 → i32` 若用 `as` 会①**饱和**（`1e9 as i32 == i32::MAX`，静默失败）②被 `clippy::cast_possible_truncation`（pedantic）拦下；而标准库**没有** `TryFrom<f64> for i32`（只有 `as` 与 `unsafe` 的 `to_int_unchecked`，后者被本卡禁止）。故改用**整数域逐位合成**（`f64::from(u32)` 精确 + 比较 + 减法），越界 / NaN / ±∞ 一律 `None` → `TargetNotFound`。**若人类更偏好**「`as` + 范围检查 + 一行窄 `#[allow(clippy::cast_possible_truncation)]`」，替换点是这一个函数（见 §9 关注点 1）。

**人类裁决（2026-09-24 chat：「按你的建议去改」+「授权你去改动」）—— 两条 DRIFT 均被接受**

- **DRIFT-016-1 接受**：`docs/DEPENDENCIES.md` 的 `serde` 行「使用方」列保持 `crates/protocol` / `crates/platform/api`（登记规则 1「先登记后引入」+ 规则 2「使用方写全部」）。**不回退**。
- **DRIFT-016-2 接受**：根 `Cargo.toml` 的 `[workspace]` 保持 `members` 含 `crates/platform/api`、`exclude` 含 `crates/platform`。**不回退**（回退 = workspace 无法加载）。
- **§4 末条「无 Out of scope 文件被修改」**：两条 DRIFT 被人类追认后，其**实质**（不静默扩范围 —— 两处改动都在本 §5 事先登记、且只改必要的那 1 / 2 行）成立；按**字面**仍记为「不成立」，两处如实并列。

**本卡遗留项闭环（2026-09-24，同批 PR）**

- **PL-064 → 已关闭**：**ADR-0042** 落地 —— 能力命名三分（`CapabilityCatalog` = 标识目录 / `CapabilityMatrix` = 运行时探测结果 / `CapabilityEntry` = 风险·审批元数据），`protocol/capability-matrix/capability-1.0.json` 的 `title` 已改名。**实测零代码影响**（codegen 的 `render_capability()` 不读 `title`、`verify_schemas` 不校验 `title`）。
- **PL-065 → 已关闭**：`MEMORY.md` §1 的「下一步 ②」段由手抄逐卡进度改为**指针**，并修掉 4 处已过期事实（下一张卡 / arch 测试条数 / PL-047 归属 / GATE-0 状态）。
- **PL-066 → 已关闭**：`AGENTS.md` §11.1 新增 `plans/*` 头部「当前进度」句一行 + §11.2 第 2 步扩写（**依据 = ADR-0041 D5 既有授权**）；`plans/stage-1-pilots.md` 的进度句与 TASK-016 状态标记同步更新（**条目正文与排期一字未动**）。
- **PL-067 → 已关闭**：4 个 schema（audit-event / envelope / error-codes / tool-schema）的 `description` 前缀 `TASK-103 (revised): ` 已删（**不读 `description`** → 生成物一字未变）。

**§9 三条「给审阅者的关注点」的规范复核（人类 2026-09-24 指示：「全部按照规范去解决；如果失败的返回值是合理的，那么就需要保留」）**

| # | 关注点 | 复核结论（逐条读码 + 读 spec） | 处理 |
|---|---|---|---|
| 1 | `CapabilityMatrix::validate()` 的 4 条不变量与负向用例 | 逐条比对 `docs/spec/capability-matrix.md` §4：①列表非空 + id 非空且唯一 ②风险不得降级（`RiskLevel::escalate`）③`L3+ ⇒ required` / `L5 ⇒ forbidden`（L1/L2 只要求「不是 forbidden」）④Resource / SideEffect 的枚举性由类型 + `parse` 保证。**8 个负向用例全部存在**（`test_validate_rejects_empty_capability_list` / `…_duplicate_and_blank_ids` / `…_l3_without_required_approval` / `…_l5_that_is_not_forbidden` / `…_l1_declared_as_forbidden` / `test_risk_escalate_rejects_downgrade` / `test_resource_access_parse_rejects_vague_words` / `test_side_effect_parse_requires_explicit_value`）。**失败返回值逐条都是「矩阵不可信」的正当失败** | **全部保留**（无改动） |
| 2 | `src/geometry.rs::exact_i32_from_f64`（不用 `as`、不写 `#[allow]`） | 规范侧：铁律 1（`as` 越界**饱和** = 静默失败）、漂移触发器 ⑥（禁 `#[allow]` 放宽）、本卡 Out of scope（禁 `unsafe`）。实现侧：只用 `f64::from(u32)`（精确）+ 比较 + 减法，越界 / 非整数 / NaN / ±∞ 一律 `None`，由调用方转 `TargetNotFound`；`i32::MIN` 单独特判。**没有更规范的替代**（std 无 `TryFrom<f64> for i32`） | **保留**（无改动） |
| 3 | 不透明句柄 `ResolvedWindow` / `ResolvedElement` 的**源码扫描**断言（铁律 8） | 为什么不是类型断言：Rust 没有稳定的「未实现某 trait」断言（negative impl 不稳定）。扫描器先断言**真的读到了句柄文件**（防恒真，铁律 1），再按剥注释后的代码查 5 个禁词；**含 2 个负向样本**（合成 `#[derive(Serialize)]` 句柄、`use serde::Serialize`，ADR-0019 N1） | **保留**（无改动） |

> **注**：本节只做**复核**，未改动任何生产代码 —— 三条关注点的结论都是「现状已符合规范」，故 `crates/platform/api/**` 在本批**零改动**。

### 6. 更合理做法

- **`f64 → i32`**：长期看最干净的是**项目级**只留一个受测的转换工具（本卡的 `exact_i32_from_f64` 已经是候选），否则 DPI / 截图 / 动画等后续卡会各自发明一套。若人类愿意开一个窄 `#[allow]`，可换成 3 行 `as` + 范围检查。
- **`tests/` 的 allow 惯例**：本卡选了"零 allow"，与仓库既有 11 个测试文件（文件头 `#![allow]`）不一致。**二选一都合理**，但应**统一**：要么在 `AGENTS.md` §5.3 明确"测试文件允许文件头 allow"（现状事实上如此），要么推广本卡的 `ok_or_fail` 写法。本卡未改 `AGENTS.md`（Orchestrator-only）。
- **根 `Cargo.toml` 的 `members`**：glob + 中间层目录是结构性冲突，显式列举更稳。
- **`CapabilityMatrix` 的序列化形状**：`probe` 是嵌套对象，而架构 v2 §13.1.2 的示例 JSON 把 `probed_at` / `platform` / `session` 摊在同一层。本卡只定义 **Rust 类型**（wire schema 归 `crates/protocol`，由 `protocol/**/*.json` 生成），故**未**为对齐示例形状而改类型；等 TASK-017 / TASK-021 有真实消费方时再定。
- **`src/target.rs` 417 行**：略超 `AGENTS.md` §5.3 的**软**上限 400（硬上限 600；`xtask hygiene` 的门限是 600，故 0 warning）。拆成两个模块会把一族内聚的选择器类型切开，故本卡**刻意**保留（`tests/target_descriptor.rs` 的注释已说明这个取舍）。

### 7. 遗留问题

1. **Q3 的 ADR 未开**：能力**标识目录**改名 `CapabilityCatalog` 需要 ADR（改 schema `title` + `docs/spec/capability-matrix.md` + `docs/spec/naming.md` §7）→ 已提 **PL-064**。
2. **`dyn` 兼容性**：RPITIT 的代价，等 TASK-017 有真实消费方时按需裁决（`#[trait_variant]` 或手写 `Pin<Box<dyn Future>>`，仍不引依赖）。
3. **`xtask refscan` 基线仍红**（151 error）→ PL-058，与本卡无关，**未**放宽判据。
4. **`MEMORY.md` §1 快照落后于事实**（TASK-014 / 015 / 016 三批 Done 都没回写「下一步」段）→ 已提 **PL-065**（§1 叙述属 Orchestrator 职责，本卡只同步了规模表）。
5. **`tests/` 的 allow 惯例与 `AGENTS.md` §5.3 的措辞**：本卡选了零 allow；若项目决定统一为"测试文件允许文件头 allow"，应同步 `AGENTS.md`（属 Orchestrator-only）。

### 8. 新增长期记忆

**`docs/memory/facts.md`（+3 条）**

1. `f64 → i32` 在标准库**没有** checked 转换（`TryFrom<f64> for i32` 不存在），`as` 会饱和且被 `clippy::cast_possible_truncation` 拦下 → allow-free 写法 = 整数域逐位合成（本卡 `exact_i32_from_f64` 是可用落点）。
2. 集成测试（`tests/*.rs`）与 lib 一样继承 workspace `[lints]`（restriction 类 lint 的 per-item `#[allow]` 无效，只有 **crate 级**属性覆盖得住）→ 这是仓库 11 个测试文件都在文件头写 `#![allow(...)]` 的原因；本 crate 选择零 allow（`tests/common/mod.rs::ok_or_fail`）。
3. **新基线**：`cargo test --workspace` = 28 target / 514 passed / 0 failed（+7 target / +45 tests 全部来自 `crates/platform/api`）。

**`docs/memory/pitfalls.md`（+2 条）**

1. doctest 里的 `map_err(Into::into)?` 会在 `?` 的目标类型确定前报 `E0282` / `E0283` → 写具体的 `map_err(PlatformError::from)?`；**`no_run` 的 doctest 仍会被编译**，`cargo clippy --all-targets` 也覆盖不到它。
2. `docs/PARKING_LOT.md` 主表是 **5 列**（`日期 | 提出者 | 事项 | 相关卡 | 处置`），新增行漏后两列 → `xtask docscan` 判 `doc/table-broken`（Error 级、直接 FAILED）；判据是「数据行 cell 数 ≠ **分隔行** cell 数」。

### 9. 给审阅者的关注点

1. **`src/geometry.rs::exact_i32_from_f64`（最高风险项）**：为满足「零 `#[allow]` + pedantic deny + 无 `TryFrom<f64> for i32`」，`f64 → i32` 改成整数域逐位合成。已测边界：`i32::MIN` / `i32::MAX` 精确通过、`2^31` 与 `-2^31-1` 报 `TargetNotFound`、`±1e300` 报错、`±0` 通过、半数远离零（`1.5 → 2`、`-1.5 → -2`）。**请重点复核**：是否有未覆盖的输入形态（例如次正规数、`-0.0`、`f64::MAX` 附近的舍入）。若认为不值得为"零 allow"付这个复杂度，替换点就是这一个函数。
2. **公共 API 的形状变更**：`CapabilityMatrix::new` 由 8 个参数改为 `(ProbeContext, channels, capabilities, degradations)`，并**新增公共类型 `ProbeContext`** —— 这是 `AGENTS.md` §5.3「函数参数 ≤ 6」的必然结果，也是"探测上下文是同一时刻的原子事实"的语义表达。请确认新类型名与字段（`probed_at` / `platform_os` / `session_locked` / `session_remote` / `session_headless`）是否符合 §13.1.2 的口径。
3. **两处连带改动（DRIFT-016-1 / DRIFT-016-2）**：`docs/DEPENDENCIES.md` 1 行 + 根 `Cargo.toml` 2 行。两者都是"不做就无法满足其它硬约束"（登记规则 1 / workspace 无法加载），请裁决接受或回退。
4. **`tests/` 的零 allow 策略**：与仓库既有 11 个测试文件的文件头 `#![allow]` 惯例不一致（本卡选了更严的一条）。若人类要求统一惯例，回退成本 = 每个测试文件加一行文件头 allow。
5. **`src/target.rs` 417 行**：略超软上限 400（硬上限 600，hygiene 门限 600 → 0 warning）。刻意保留一族内聚类型，请确认可接受。

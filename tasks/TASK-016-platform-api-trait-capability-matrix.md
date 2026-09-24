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

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点

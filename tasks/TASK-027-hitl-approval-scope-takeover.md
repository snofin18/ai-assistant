# TASK-027　`hitl`：审批请求 + 授权范围（四维+TTL）+ 用户接管 + 暂停恢复 + 差异预览数据准备

- 状态：**Done**（2026-09-26）
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：021,022　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：021,022　**预估**：M　**难度**：M
- **write scope**：`crates/hitl/**`（新 crate `assistant-hitl`）；经人类 2026-09-26 明确授权，
  PL-082 的 ADR 路线同时允许改 `docs/adr/0048-*`、`docs/spec/audit-event.md`、
  `protocol/audit-event/audit-event-1.0.json`、`xtask/src/render.rs`、
  `crates/protocol/**`、`crates/policy/src/decision.rs` 及其投影测试、`docs/adr/README.md`、
  `docs/memory/decisions.md`、`docs/DEPENDENCIES.md`（仅依赖使用方行）
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）、`docs/wbs-overview.md` §6（DoD）
- **上位约束**：架构 v2 §10（HITL）/ §8.4（暂停、接管）/ §12.4（污点降权）；
  `docs/spec/audit-event.md`（ADR-0048 后的 `PolicyDecision`）；铁律 1 / 2 / 3 / 4 / 6 / 9 / 10

**目标**

新建 `crates/hitl`（`assistant-hitl`），把策略的“需要确认”决策转成可验证的审批请求，
实现四维授权范围 + TTL + 使用次数的确定性校验，提供接管前后指纹重同步与暂停/恢复
状态衔接，并准备可交给 UI 的差异预览数据。`hitl` 只做审批与接管领域逻辑，不执行工具、
不访问平台 API、不持久化、不渲染 UI。

**In scope（本卡交付物）**

| # | 交付物 | 依据 |
|---|---|---|
| 1 | `crates/hitl/{Cargo.toml,README.md,src/**,tests/**}`；新 crate `assistant-hitl` | gov §5.4、阶段 1 DoD |
| 2 | 审批请求模型：工具、目标、effect、风险、可逆性、摘要、TTL、授权选项、diff 数据，全部输入先校验 | 架构 v2 §10.2 / §10.4 |
| 3 | 协议确认投影适配：消费 ADR-0048 后的 `PolicyDecision`；无条件 allow / deny 不能冒充审批请求 | ADR-0048、PL-082 |
| 4 | 四维授权：subject / tool / target / effect；五种 scope 语义；每次授权必须有正 TTL 与有限使用次数 | 架构 v2 §10.3、批次表验收要点 |
| 5 | 高风险硬约束：`High` / `Critical` 或 `L3Irreversible` 只允许 `once`；禁用范围 fail-closed | 铁律 6、架构 v2 §10.3 |
| 6 | `ApprovalOutcome::{Approved,Denied,TimedOut}`；到期边界确定，批准只能选择请求声明的范围并收紧 TTL | 批次表“审批超时行为明确” |
| 7 | 接管：记录接管基线指纹；交还时重新读取当前指纹并显式标记必须重新解析目标 / 比较计划，绝不假设未变化 | 架构 v2 §10.6 |
| 8 | 暂停 / 恢复：只调用 `assistant-task-engine` 的显式状态迁移；步骤边界语义仍归 task-engine | 架构 v2 §8.4、TASK-022 |
| 9 | diff 数据准备：文本行级增删、字段前后值、文件摘要、UI 步骤预览、不可逆二次确认标记；不做 UI 渲染 | 架构 v2 §10.4 |
| 10 | 正反测试：协议确认字段缺失、高风险非 once、TTL 到期、范围扩权、接管未激活、交还重同步、diff 超预算 | ADR-0019 N1、gov §5.5 |

**Out of scope（做了算漂移）**

- UI 组件、Tauri IPC 方法、审批卡片视觉与 i18n → TASK-029 / 030
- 真实工具执行、策略重新求值、租约获取、撤销执行、后置断言执行
- 用户接管检测的平台实现（全局钩子 / EventTap / portal）→ 平台层后续卡
- 审批状态持久化、策略面板、授权热加载/一键撤销 UI → TASK-032 / 后续
- DLP 脱敏、指令来源归因、注入复核 → TASK-050~054
- 修改 `ErrorCode` 分类、架构 v2 的五种 scope 名称、policy DSL 既有允许范围
- 新增第三方依赖、`unsafe`、`#[allow]` 放宽 lint、后台线程或真实计时器

**必须遵守**

1. **契约先行**：先落 ADR-0048 + `docs/spec/audit-event.md` + schema/codegen，再改 policy 投影，最后实现 `hitl`。
2. **无静默失败**：缺失 scope、缺失 `show_diff`、非法 scope、过期、过期边界、超权范围、接管未激活都必须返回带 `ErrorCode` 的错误或显式 `TimedOut`。
3. **策略唯一放行点**：`hitl` 只处理“policy 已要求确认”的结果，不自行把 deny 改成审批、不重新解释业务权限。
4. **高风险**：`High` / `Critical` / `L3Irreversible` 只允许 `once`，`max_uses` 必须恰为 1；不得自动把 `persistent` 降级成其他范围。
5. **接管不猜测**：交还时始终要求重新解析目标与重算指纹；即使字符串相等，也要返回“需要重新同步”的显式结果。
6. **暂停边界**：`hitl` 只委托 task-engine；执行中的 Step 不中途强停，不复制 task-engine 的状态表。
7. **纯函数/注入时间**：审批 TTL、过期、diff 预算都接收 `now_ms` 或显式上限；禁止读取系统时间、环境变量或文件系统。
8. **注释与命名**：用 `Target` / `Step` / `Tool` / `Fingerprint` 等受控词；公共 API 100% 文档注释，模块读写边界和错误语义。
9. **规模**：单文件 ≤ 400 行（软）/ 600（硬），函数 ≤ 80 行，参数 ≤ 6；超限先拆模块。
10. **禁止 drive-by**：发现审计/存储/UI 缺口只记 `PARKING_LOT` 或卡 §7，不在本卡顺手改。

**步骤**

1. 落 ADR-0048、更新审计 spec/schema/codegen 与 policy 无损投影，跑协议 + policy 专项测试。
2. 新建 `assistant-hitl` 骨架与 README，先写标识符 / 错误码映射 / 协议确认适配测试。
3. 实现四维授权、scope、TTL、使用次数与高风险范围校验，逐个跑专项测试。
4. 实现审批请求与 `Approved` / `Denied` / `TimedOut` 结果，覆盖到期边界、超权、缺失 diff。
5. 实现 diff 数据准备（含文本 LCS 上限失败），覆盖增删改、确定性、超预算不静默截断。
6. 实现暂停/恢复与接管/交还协调器，复用 task-engine 状态迁移并验证交还必须重同步。
7. 跑全部验收命令；填卡 §1~§9、更新 LEDGER / PLAN / README / plans / memory，开 PR 并等 CI。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-hitl
cargo test -p assistant-core arch::
cargo llvm-cov -p assistant-hitl --fail-under-lines 75
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo deny check
```

**完成定义（DoD）**

- [ ] ADR-0048 已落盘并被 `adr-index` 识别；`docs/spec/audit-event.md` 与审计 schema 同步。
- [ ] `PolicyDecision` 可无损承载五种 scope + `show_diff`；旧的最小 allow / deny 形状仍可反序列化。
- [ ] `hitl` 能把确认决策转成审批请求；无条件 allow / deny / 缺失 scope / 缺失 `show_diff` 均明确拒绝。
- [ ] 四维授权各字段都参与匹配；五种 scope 有正反测试；授权必须带 TTL 与有限次数。
- [ ] `High` / `Critical` / `L3Irreversible` 只允许 `once` 且 `max_uses = 1`；禁用值不静默删改。
- [ ] 审批在 `now_ms >= expires_at_ms` 返回 `TimedOut`；批准不能选用未声明 scope、不能扩大 TTL。
- [ ] 接管交还始终返回“需重新解析/重新同步”，并明确标识接管前后指纹是否变化。
- [ ] 暂停 / 恢复通过 task-engine 的合法迁移完成；非法状态返回结构化错误。
- [ ] diff 数据覆盖文本增删改、字段变化、文件摘要、UI 步骤、不可逆二次确认；超预算返回错误而非截断。
- [ ] `crates/hitl/README.md` 含职责 / 边界 / 不变量 / 已知限制。
- [ ] `cargo fmt --all --check` 0 diff；workspace clippy 退出码 0；workspace tests 全绿。
- [ ] `cargo test -p assistant-core arch::`、`cargo llvm-cov -p assistant-hitl --fail-under-lines 75` 通过。
- [ ] `verify-schemas / codegen --check / hygiene / memory-counts / adr-index / check-ledger / card-check / docscan` 全部 PASSED；`refscan` 不新增 PL-058 基线错误。
- [ ] `cargo deny check` 全 ok；无新增第三方依赖，仅登记既有 `serde` / `thiserror` 的新使用方。
- [ ] LEDGER / PLAN 当前状态块 4 行 / README 三处 / `plans/stage-1-pilots.md` 进度句与完成标记已同步。
- [ ] 无任何 Out of scope 文件被修改；PL-082 已闭环或剩余风险明确记录。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-027 hitl：审批请求 + 授权范围（四维+TTL）+ 用户接管 + 暂停恢复 + 差异预览数据准备
【目标】新建 assistant-hitl，把确认决策转成可审批请求，确定性执行四维授权/TTL/次数校验、暂停与接管状态衔接，并只准备 UI 可用的 diff 数据。
【write scope】crates/hitl/**；人类授权 PL-082 ADR 路线后同步 docs/adr/0048-*、docs/spec/audit-event.md、protocol/audit-event、xtask render、crates/protocol、crates/policy 投影及测试；另含本卡记录区与 §11 规定文件。
【铁律】1 无静默失败；2 审批输入与策略结果先校验；3 policy 唯一放行点；4 写操作须有 postcondition；6 高风险禁区；9/10 不扩范围、契约先行。
【禁止】不做 UI/IPC/真实工具/平台调用/持久化；不把 deny 改成审批；不静默降级高风险范围；不加第三方 crate、unsafe、allow 或放宽 lint。
【验收】fmt / workspace clippy / workspace tests / hitl 专项 / core arch / hitl 覆盖率 / verify-schemas / codegen --check / hygiene / memory-counts / adr-index / check-ledger / card-check / docscan / cargo deny。
【依赖】TASK-021、TASK-022 已 Done（已核对 LEDGER）。
【疑问】PL-082 必须按 ADR 路线；采用 ADR-0048 扩展 PolicyDecision 的两个可选字段，保持 schema 1.0 兼容。
```

### 2. 实际改动文件

- 新增 `crates/hitl/**`：`Cargo.toml`、README、`src/{lib,error,identifiers,authorization,approval,diff,control}.rs`、5 个集成测试文件。
- 新增 `docs/adr/0048-policy-decision-confirmation-projection.md`。
- 更新 `docs/spec/audit-event.md`、`protocol/audit-event/audit-event-1.0.json`、`xtask/src/render.rs`、`crates/protocol/src/{generated/audit_event.rs,lib.rs}`、`crates/policy/src/decision.rs`、`crates/policy/tests/protocol_projection.rs`、`crates/policy/README.md`。
- `Cargo.lock`：新增 workspace member 与依赖边。
- 同步 `LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`MEMORY.md`、`docs/memory/{facts,pitfalls,decisions}.md`、`docs/PARKING_LOT.md`、`docs/DEPENDENCIES.md`、`docs/adr/README.md`。

### 3. 验收输出摘要

```text
cargo fmt --all --check → PASS
cargo clippy --all-targets -- -D warnings → PASS
cargo test --workspace → 371 passed / 0 failed / 2 ignored
cargo test -p assistant-hitl → 18 passed / 0 failed（approval 5 + authorization 4 + control 4 + diff 3 + identifiers/errors 2）
cargo test -p assistant-core arch:: → 5 passed
cargo llvm-cov -p assistant-hitl --fail-under-lines 75 → 725 lines / 160 missed / 77.93%
verify-schemas → 5 parsed / 0 error / PASSED
codegen --check → 5 files / 0 drift / PASSED
hygiene → 248 files / 0 error / 4 warning（既有基线）
memory-counts → PASSED
adr-index → 32 files / 0 error / PASSED
check-ledger / card-check / docscan → PASSED（warning 均既有基线或本卡完成记录后收敛）
cargo deny check → advisories / bans / licenses / sources 全 ok
refscan → 151 error（PL-058 既有基线，本卡新增 0）
非宿主 clippy → 不适用 ADR-0045：hitl 传递依赖 storage 的 libsqlite3-sys / zstd-sys，带 C 依赖
```

### 4. DoD 逐条核对

- [x] ADR-0048、audit spec 与 schema 同步，`adr-index` 通过。
- [x] `PolicyDecision` 无损承载五种 scope + `show_diff`；旧最小形状可反序列化。
- [x] 审批请求只从 confirmation 决策构造；deny / 无条件 allow / 缺字段均显式拒绝。
- [x] subject / tool / target / effect 四维参与匹配；五种 scope、TTL 与次数均有测试。
- [x] High / Critical / L3Irreversible 只允许 `once` 且 `max_uses=1`；禁用范围不静默删改。
- [x] `now_ms >= expires_at_ms` 返回 `TimedOut`；批准不能扩 scope / TTL / 次数。
- [x] 接管交还始终要求重同步，并区分指纹是否变化。
- [x] 暂停 / 恢复使用 task-engine 合法迁移。
- [x] diff 覆盖文本、字段、文件、UI 步骤、不可逆；超预算返回错误。
- [x] README 含职责 / 边界 / 不变量 / 已知限制。
- [x] fmt / clippy / workspace tests / arch / hitl 覆盖率通过。
- [x] xtask 文档门禁与 `cargo deny check` 通过；`refscan` 未新增 baseline。
- [x] LEDGER / PLAN / README / plans / memory 已同步。
- [x] 无新增第三方 crate；`thiserror` 只新增使用方。
- [x] “无任何 Out of scope 文件被修改”经人类明确授权后成立：PL-082 的契约文件由人类明确授权（见 §5 DRIFT-027-2），且 PL-082 已闭环（ADR-0048）。【人类 2026-09-26 确认可打勾】

### 5. 偏差

**DRIFT-027-1（卡片状态行由 Ready → Done）**

- 现象：卡面正文区的 `- 状态：` 行由 Implementer 更新。
- 影响：与 ADR-0031 的正文只读字面冲突，与 PL-073 既有未决项同型。
- 处置：沿用 TASK-015~026 的既有做法；其余正文仅在人类明确授权“代 Orchestrator 展开”时补全。

**DRIFT-027-2（PL-082 授权扩展 write scope）**

- 现象：实现前确认协议 `PolicyDecision` 无法承载 confirmation，按人类授权走 ADR 路线，修改 ADR/spec/schema/codegen/policy。
- 影响：超出任务卡最初的 `crates/hitl/**` 字面范围，但属于 `DRIFT-021-1` / PL-082 的明确契约前置工作。
- 处置：先落 ADR-0048 与 spec/schema，再改 codegen 与 policy 投影，最后实现 hitl；旧 schema 1.0 保持兼容。

**非宿主 clippy 不适用**

- `hitl → task-engine → storage` 会传递编译 C 依赖 `libsqlite3-sys` / `zstd-sys`，按 ADR-0045 不在目标平台门禁适用范围内；已在 LEDGER 记录。

### 6. 更合理做法

- 把 `PolicyDecision` 的 confirmation 判据固定为“非空 `scope_options` 是否存在”，不增加同义布尔字段。
- 审批请求与授权 grant 的字段保持私有，只暴露 getter 与受校验的 `issue` 入口，防止外部结构体字面量绕过风险/次数校验。
- 文本 diff 使用有界 LCS；超预算直接失败，避免 UI 展示伪装完整的局部 diff。
- 接管交还返回“是否变化”只作为计划复核提示；“必须重新解析目标”始终为真。

### 7. 遗留问题

- 审批状态、授权列表、撤销历史和策略面板尚未持久化；本卡只提供领域对象。
- 真实 UI 渲染、i18n、来源归因、DLP 脱敏与参数修改交互归后续卡。
- `non-host clippy` 对含 `task-engine` 传递依赖的 `hitl` 不适用；若以后要覆盖，需先消除 C 依赖或按平台拆分 crates。
- PL-078（生成协议类型无构造器）仍存在；本卡测试通过 serde JSON 构造协议 fixture，未在 hitl 重复定义协议类型。

### 8. 新增长期记忆

- FACT：`assistant-hitl` 的授权基线（四维 + TTL + 次数、五 scope 边界、高风险 once、18 测试 / 77.93% 覆盖）。
- PITFALL `[supersedes:2026-09-25 PolicyDecision 有损]`：确认投影现已无损，但 `allow=true` 仍不等于直接执行；HITL 必须检查 scope/show_diff 并维持高风险收敛。
- PITFALL：审批 / 授权关键对象不得公开字段，否则可绕过构造器校验。

### 9. 给审阅者的关注点

1. **高风险收敛**：检查 `ApprovalRisk::validate_scope_options`、`AuthorizationRequest::validate_for_risk`、`AuthorizationGrant::issue` 与审批 resolve 是否任一路径允许 High/Critical/L3 获得非 `once` 或多次使用。
2. **协议兼容与无损**：检查 `PolicyDecision` 新增字段是否保持旧 JSON 可读，以及 policy projection 是否真正保留 scope / `show_diff`。
3. **接管语义**：检查交还在指纹相同时是否仍返回 `requires_target_resolution=true`，以及 task-engine resume 失败时 takeover 记录是否保留。

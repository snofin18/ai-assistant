# TASK-050　`dlp`：三档出域策略（`local_only`/`redacted`/`full`）+ 逐应用与逐内容类型覆盖 + 脱敏规则 + endpoint 白名单 + **降级不静默**

- 状态：**Done**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：014,026　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：014,026　**预估**：M　**难度**：M
- **write scope**：`crates/dlp/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1c（1c）、`docs/wbs-overview.md` §6（DoD）

**目标**

`dlp`：三档出域策略（`local_only`/`redacted`/`full`）+ 逐应用与逐内容类型覆盖 + 脱敏规则 + endpoint 白名单 + **降级不静默**。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/dlp/**`

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-050 `dlp`：三档出域策略 + 逐应用/逐内容类型覆盖 + egress destination 白名单 + 降级不静默
【目标】在 `crates/dlp` 内交付纯逻辑出域策略解析与 fail-closed 判定。
【write scope】仅：`crates/dlp/**`；按预授权执行 ADR 0007、卡面记录、进度热点与轮次报告收口。
【铁律】1 无静默失败；2 输入先校验；3 策略引擎保持唯一放行点；9 不静默扩大范围；10 契约先行。
【禁止】不新增依赖/crate/顶层目录；不改 schema/IPC/ErrorCode/既有公共 trait；不操作真实 GUI；不把审计写库硬接进 DLP。
【验收】`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`、xtask 十一项门禁；专项覆盖默认档、覆盖解析、白名单、local_only 不可用、边界与拒绝路径。
【依赖】014、026 Done；M3 本地模型选型未决，本轮按调用方注入可用性处理。
【疑问】无新增；本地模型选型仍留给既有 M3。

### 2. 实际改动文件

- `crates/dlp/src/egress.rs`（新增纯逻辑策略模块，551 行）
- `crates/dlp/src/egress/tests.rs`（新增 egress 专项测试，287 行）
- `crates/dlp/src/lib.rs`、`crates/dlp/README.md`
- `docs/adr/0007-egress-policy-tiers-and-resolution.md`、`docs/adr/README.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`LEDGER.md`
- 本卡记录区、`docs/automations/2026-10-07-round-1.md`

### 3. 验收输出摘要

已完成首轮专项证据：

```text
cargo test -p assistant-dlp                                  -> 22 passed / 0 failed
cargo fmt --all --check                                      -> EXIT 0
cargo clippy -p assistant-dlp --all-targets -- -D warnings   -> EXIT 0
cargo clippy --all-targets -- -D warnings                    -> EXIT 0（仅既有 unknown-lint warning）
cargo test --workspace                                       -> EXIT 0
x86_64-unknown-linux-gnu clippy -p assistant-dlp             -> EXIT 0
aarch64-apple-darwin clippy -p assistant-dlp                 -> EXIT 0
```

```text
xtask hygiene                -> 0 error / 108 warning / PASSED
xtask memory-counts          -> 0 error / PASSED
xtask adr-index              -> 0 error / PASSED
xtask refscan                -> 0 error / PASSED
xtask docscan                -> 0 error / 318 warning / PASSED
xtask card-check             -> 0 error / PASSED
xtask check-ledger           -> 0 error / PASSED
xtask check-comments         -> 0 error / 70 warning / PASSED
xtask verify-schemas         -> PASSED
xtask codegen --check        -> 0 drift / PASSED
xtask check-migrations       -> 0 error / PASSED
PR #258 pull_request run 37498359372 -> 11/11 SUCCESS, pending=0, failed=0
merge commit                          -> 51bbb27
```

### 4. DoD 逐条核对

- [x] 三档策略、逐应用/逐内容类型覆盖、egress destination 白名单与 `local_only` fail-closed 均有专项测试。
- [x] `cargo fmt --all --check` EXIT 0。
- [x] `cargo clippy --all-targets -- -D warnings` EXIT 0。
- [x] `cargo test --workspace` EXIT 0。
- [x] xtask 十一项门禁全 PASSED。
- [x] LEDGER merge-hash 回填：PR #258 / merge `51bbb27`。

### 5. 偏差

TASK-050 原目标是“策略变更写审计”，但 `crates/dlp` 的 ADR-0071 边界禁止 IO/持久化。本轮按 ADR 0007 实现为返回不可变 `EgressPolicyChange`；宿主仍未接线到 audit/storage，属本卡 write scope 之外的后续装配工作，已在记录区保留而不伪装为已写库。

流程偏差：首笔 `docs/adr/README.md` 编辑发生在 guard acquire 之前；随后已对其余热点文件先 acquire 再写。无并发写者，未发生 lost update。

范围记录：总 diff 1003 行，超过 400 行软预算。超出部分主要来自 ADR 正文、纯逻辑类型与 22 个正负向测试；行为面仍限定在 `crates/dlp/**`，未新增依赖或公共跨进程契约。

### 6. 更合理做法

`EgressDestinationId` 是 canonical lowercase 字符串；策略解析保持纯函数，不引入 serde/TOML/regex。M3 的本地模型选型未决时，只注入“是否可用”，不让 DLP 猜 provider。

### 7. 遗留问题

- `EgressPolicyChange` 的 audit/storage 接线需要在装配层另立小卡；本卡不直接依赖 audit/storage。
- `xtask refscan` 的 `ADR_BARE_PENDING` 仍硬编码 0007；本轮以路径引用绕开误报，动态登记表读取仍归既有 PL-099。

### 8. 新增长期记忆

无新增 FACT/PITFALL/REJECTED；决策本身已落 ADR 0007。

### 9. 给审阅者的关注点

1. 覆盖解析是否符合架构 §12.6.1：应用覆盖可替换默认，内容类型覆盖只能收紧。
2. egress destination 标识的上限、canonical lowercase 与默认拒绝是否足够保守。
3. `EgressPolicyChange` 只产出审计记录、不写库，是否符合 ADR-0071 的 DLP 边界。

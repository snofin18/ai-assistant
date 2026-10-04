# TASK-232　四轮自动化审计收口（TASK-224 状态行 + 状态行规则补强 + PL-108）

- 状态：**Done（2026-10-04；四轮自动化审计收口：`TASK-224` 滞后状态行按实改 Done、状态行比对规则补强（`正文只读 ≠ 状态行只读`；PR 贴比对为必做）、storage 轮遗留转 `PL-108`；零代码改动 —— 门禁与 PR 为证）**
- 阶段：1　子阶段：1a 补救 / 治理　批次：治理池　依赖：TASK-224、TASK-228、TASK-230、TASK-231（四轮产物）、TASK-227、TASK-229（规则来源）
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/memory/pitfalls.md`（2026-10-03 状态行硬规则）、`docs/PARKING_LOT.md`、`tasks/TASK-224-single-write-channel-lock-queue.md`

---

## 目标（一句话）

把 2026-10-03 23:20 → 2026-10-04 06:50 那**四轮自动化**的审计发现收干净：一处卡状态行滞后、
一条规则的执行漏洞、一条只在孤儿 memory 里的遗留项。

## 背景（为什么现在做）

四轮产物都已并入 main 并逐项复核通过（见 §3），审计发现三处**非产物性**问题：

1. **`TASK-224` 卡状态行仍是 `Ready（提案已记录…本轮不实现）`**，而 LEDGER 两次记 Done、ADR-0066 已 Accepted、
   `xtask write` 已实现并合并（PR #208 / `919f2fa`）→ 账面与事实不一致。
2. **状态行硬规则被「写明原因」分支绕开**：TASK-224 轮在 PR 里做了比对、发现不一致，但把原因写成
   「正文区只读」后就没改。规则的本意是「真正无法在本批修正时才写明原因」；而**状态行本身是可维护元数据**
   （本仓库既有惯例：TASK-215~231 的 Done 状态行均由实现方写）。另外 TASK-231 轮**没有在 PR 描述里贴比对结果**
   （规则第 ④ 步缺失），虽未造成事实不一致，但说明该步骤缺少执行约束。
3. **一条真实遗留只躺在孤儿 memory 里**：TASK-230 轮的 automation memory 记着
   「binary 装配层仍缺 production `SessionStore` adapter（`ConversationRecord`/`ConversationMessageRecord` → 核心 `SessionSnapshot`）；
   现在是 `MemorySessionStore` 在顶着」——该 automation 已注销，这条应该进停车位而不是留在 `.codex` 目录。

## write scope

- `tasks/TASK-224-single-write-channel-lock-queue.md`（**仅状态行**）
- `tasks/TASK-232-four-round-automation-audit-closeout.md`（本文件）
- `docs/memory/pitfalls.md`（**仅追加**规则补强）、`MEMORY.md`（**仅规模表**）
- `docs/PARKING_LOT.md`（**仅追加** `PL-108`）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（本卡标记 + 当前进度句）

## In scope

- 把 `TASK-224` 状态行按实改成 Done，并写明依据（ADR-0066 / PR #208 / `919f2fa`）。
- 追加一条 pitfall 补强：① 「写明原因」不得用于可修的滞后（`正文只读 ≠ 状态行只读`）；
  ② PR 描述里的状态行比对是**必做项**，缺失即视为未执行该规则。
- 把 TASK-230 的遗留转成 `PL-108`（生产 `SessionStore` 适配器），并注明来源是孤儿 automation memory。
- 在本卡记录区留下**四轮审计结论**（每轮 merge hash、专项证据、规则执行情况）。

## Out of scope（做了算漂移）

- 改任何 `crates/**` / `apps/**` / `xtask/**` 代码或断言；改 `docs/spec/**`、`AGENTS.md`、gov 文档。
- 实现 `PL-108`（本卡只开条目）；删除 `.codex` 下任何文件（包括那份 orphan memory —— 它是本卡结论的原始证据）。
- 改写 LEDGER 既有行、改写任何卡的分界线以上正文（除 `TASK-224` 的状态行这一行）。

## 必须遵守

- **铁律 1 / 9**：状态与事实一致；只改 write scope；零代码改动。
- ADR-0028：写热点文件前 `guard acquire`，写完立刻 `guard release`。
- 本卡自身必须遵守它要补强的那条规则：提交前逐张比对状态行与 LEDGER，并把结果贴进 PR。

## 验收命令

```powershell
cargo fmt --all --check
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments / verify-schemas
```

并附：`TASK-224` 状态行 diff（仅 1 行）、pitfall 补强条目全文、`PL-108` 行、四轮审计表（§3）。

## 完成定义（DoD）

- [ ] `TASK-224` 状态行改为 Done（依据 ADR-0066 / PR #208 / `919f2fa`），其余正文未动。
- [ ] pitfall 补强条目落档（「写明原因」不得绕开可修的滞后；PR 贴比对是必做项）。
- [ ] `PL-108` 已登记（生产 `SessionStore` 适配器仍缺），并注明来源。
- [ ] 本卡记录区含四轮审计结论（merge hash + 专项证据 + 规则执行情况）。
- [ ] 全套门禁绿；`crates/**`、`apps/**`、`xtask/**` 零改动。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-232 四轮自动化审计收口
【目标】把四轮自动化（TASK-228/224/230/231）审计出的三处账面/流程问题收干净：TASK-224 状态行、
        状态行规则的「写明原因」漏洞、storage 轮只躺在孤儿 memory 里的遗留项
【write scope】仅：TASK-224 卡（仅状态行）、本卡、docs/memory/pitfalls.md（追加）、MEMORY.md（规模表）、
              docs/PARKING_LOT.md（追加）、LEDGER / PLAN(当前状态块) / README(三处) / plans(标记+进度句)
【铁律】1 状态必须与事实一致；9 只改 write scope（**零代码改动**）；10 不改 spec/AGENTS（规则只落 pitfalls）
【禁止】改 crates/**、apps/**、xtask/** 代码或断言；改 docs/spec/**、AGENTS.md、gov；实现 PL-108；
        删除 .codex 下任何文件（那份 orphan memory 是本卡结论的原始证据）
【验收】fmt / test --workspace / xtask 九项 + check-migrations；TASK-224 状态行 diff 仅 1 行；
        本卡自带状态行 ↔ LEDGER 比对并贴进 PR
【依赖】TASK-224 / 228 / 230 / 231 均已合并（ec3ea3b / 919f2fa / 8408e66 / 2df4248）
【疑问】无（三处发现都属账面/流程，不需要人类裁决范围）
```

### 2. 实际改动文件

- `tasks/TASK-224-single-write-channel-lock-queue.md`（**仅状态行 1 行**：`Ready（…本轮不实现）` → `Done（…PR #208 / 919f2fa）`）
- `tasks/TASK-232-four-round-automation-audit-closeout.md`（本卡）
- `docs/memory/pitfalls.md`（追加 1 条规则补强）、`MEMORY.md`（pitfalls 规模表 → 274 行 / 163 条）
- `docs/PARKING_LOT.md`（追加 `PL-108`）
- `plans/stage-1-pilots.md`（当前进度句 + 1 条进度行 + 批次表/卡片表各 2 行 + `TASK-228` 完成标记）、
  `LEDGER.md`、`PLAN.md`（当前状态块）、`README.md`（三处）

### 3. 四轮审计结论与验收输出

**四轮产物落点（复核用）**

| 轮次 | 卡 / 来源 | 实现 PR | merge | 我方独立复核证据 | 状态行比对（新规则） |
|---|---|---|---|---|---|
| 23:20 | TASK-228（`PL-106`） | #206 | `ec3ea3b` | `tools/acceptance-report` 5 passed；卡 §4 三例负向：全 skip 退 **1**、缺 `status` 退 **2**、`status=unknown` 退 **2** | PR 有比对 ✓ |
| 01:50 | TASK-224（ADR-0066） | #208 | `919f2fa` | `cargo test -p xtask` **438 passed**（含写通道占用/超时/锁释放负向） | PR 有比对，但选了「写明原因」（`正文只读`）→ **状态行未改**（本卡修正） |
| 04:20 | TASK-230（`PL-092`） | #210 | `8408e66` | `cargo test -p assistant-storage` 多二进制全绿；`check-migrations` PASSED（迁移 `0005` 已登记） | PR 有比对 ✓ |
| 06:50 | TASK-231（`PL-074`） | #212 | `2df4248` | `-p assistant-platform-api` 与 `-p assistant-platform-windows` 全绿（含 `test_drag_endpoints_use_each_declared_coordinate_space`） | PR **未贴**比对 ✗（无事实不一致，但第 ④ 步缺失） |

**本轮本地门禁（main `e95a5d5` 起）**：`cargo fmt --all --check` EXIT 0；`cargo clippy --all-targets -- -D warnings` EXIT 0；
`cargo test --workspace` EXIT 0；xtask `hygiene` / `memory-counts` / `adr-index` / `refscan` / `docscan` /
`card-check` / `check-ledger` / `check-comments` / `verify-schemas` / `check-migrations` / `codegen --check` 全 PASSED。

**三处发现（已处置）**

1. `TASK-224` 状态行滞后 → 本卡改为 `Done`（依据 ADR-0066 + PR #208 + `919f2fa`），正文其余部分未动。
2. 规则漏洞 → `docs/memory/pitfalls.md` 追加补强（`正文只读 ≠ 状态行只读`；不一致默认必须本批改；PR 贴比对为必做项）。
3. storage 轮遗留 → `docs/PARKING_LOT.md` 新增 `PL-108`（生产 `SessionStore` 适配器仍缺，装配层仍用内存 store），
   来源注明是 TASK-230 轮的孤儿 automation memory。

### 4. DoD 逐条核对

- [x] `TASK-224` 状态行改为 Done（依据 ADR-0066 / PR #208 / `919f2fa`），其余正文未动。
- [x] pitfall 补强条目落档（「写明原因」不得绕开可修的滞后；PR 贴比对是必做项）。
- [x] `PL-108` 已登记并注明来源（孤儿 automation memory）。
- [x] 本卡 §3 含四轮审计表（merge hash + 独立复核证据 + 规则执行情况）。
- [x] 全套门禁绿；`crates/**`、`apps/**`、`xtask/**` 零改动（`git diff --stat` 为证）。

### 5. 偏差

- 1. **规则执行参差**：四轮里 3 轮在 PR 贴了状态行比对，`TASK-231` 轮未贴；`TASK-224` 轮贴了却用
  「正文只读」当理由没改。两者都已在本卡 §3 如实记录，并通过 pitfall 补强收紧规则（第 ④ 步改为必做）。
- 2. **`PL-108` 的来源特殊**：它不是从卡面或仓库文档里发现的，而是从**已注销 automation 的 memory 文件**
   （`.codex\automations\ai-assistant-task-round-0420-storage-session-api\memory.md`）里读到并转抄进来的；
  该目录按本卡 Out of scope **未删除**（它是结论的原始证据）。
- 3. 其余无偏差：本卡**零代码改动**，`cargo test --workspace` 只作回归确认。

### 6. 更合理做法

规则应当**默认收敛到"改"**：状态行是实现方可维护的元数据，只有卡面语义与事实真的冲突（需要人类裁决范围）
才允许"写明原因暂不改"；并且「把比对结果贴进 PR」必须是可核查的必做步骤 —— 否则规则会退化成"看起来执行了"。

### 7. 遗留问题

- `PL-108`（新）：storage 层的会话/消息持久化 API 已就位，但**生产 `SessionStore` 适配器仍缺**
  （`MemorySessionStore` 仍在顶替）——建议立装配卡接线，并补装配级跨重启证据。
- `TASK-041`（截图/脱敏）仍受"新 crate 未批准"阻塞（`PL-102`）；`TASK-042`（视觉验证）依赖 041 未解封。
- `PL-060` 剩余 2 条 hygiene 规则仍需先裁决 ADR（相似度阈值 / 顶层目录白名单）。

### 8. 新增长期记忆

- PITFALL（已落 `docs/memory/pitfalls.md`）：状态行硬规则补强 —— `正文只读 ≠ 状态行只读`；
  「写明原因」只适用于真正的语义冲突；PR 贴比对为必做项。

### 9. 给审阅者的关注点

1. **`PL-106` 的残余风险是否真的关闭**：TASK-228 把 skip/fail 变成结构化状态（退 1 / 退 2 可机器区分），
   但真机用例仍不在 CI 跑 —— 以后判断真机结论请以结构化记录 + 汇总输出为准，而不是"测试是否变红"。
2. **规则的第 ④ 步没有机器校验**：`card-check` 只查卡，不查 PR 描述；若要让"PR 贴比对"变成硬门禁，
   需要改 `AGENTS.md`/gov（前者需 ADR）——本卡刻意没动。
3. **`PL-108` 是真实缺口**：`SessionStore` 目前只有 `MemorySessionStore` 在用；请确认接线优先级
   （它影响"跨重启持久化"这条生产承诺）。

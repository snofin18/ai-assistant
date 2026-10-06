# TASK-244　机器派生计数收口（PL-022）

- 状态：**Done（2026-10-06；PR #243 首轮 CI `37393595320` 11/11 SUCCESS；等待 merge hash）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-241、ADR-0072（均 Done）
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/adr/0075-machine-derived-governance-counts.md`、`docs/governance-ai-agent-execution.md` §5.1 / §5.4、
  `.github/workflows/ci.yml`、`docs/PARKING_LOT.md` PL-022、`xtask/src/deferred.rs`、`xtask/src/doccheck.rs`、`xtask/src/main.rs`

---

## 目标（一句话）

让 `xtask` 从 `gov §5.4` 表格派生 hygiene 规则总数，并机器校验 `gov §5.1` 门禁编号集合与 `ci.yml` 的显式门禁标记集合一致。

## 背景（为什么现在做）

PL-022 的机器派生余项仍开放：`TOTAL_HYGIENE_RULE_COUNT` 硬编码为 13，`gov §5.1` 与 `ci.yml`
也缺少机器化的一一映射校验。ADR-0072 已冻结“动态派生值只指向唯一事实源”的原则；本卡把它
落到 xtask 的可执行门禁，不再靠维护者记得同步数字或人工审阅 workflow。

## write scope

- `tasks/TASK-244-derived-counts.md`（本卡）
- `docs/adr/0075-machine-derived-governance-counts.md`（新 ADR）
- `docs/adr/README.md`（仅登记表、下一可用编号）
- `docs/memory/decisions.md`（仅追加 ADR-0075 决策）
- `docs/governance-ai-agent-execution.md`（§5.1 / §5.4 的派生与标记表述）
- `.github/workflows/ci.yml`（仅增加 `# gov-gate: <id>` 注释标记）
- `xtask/src/deferred.rs`、`xtask/src/doccheck.rs`、`xtask/src/main.rs`
- `docs/PARKING_LOT.md`（仅追加 PL-022 闭环行）
- `LEDGER.md`（仅追加）
- `PLAN.md`（仅当前状态块）
- `README.md`（仅状态行、当前阶段、最近进展）
- `plans/stage-1-pilots.md`（仅完成标记与当前进度句）
- `docs/automations/2026-10-05-round-5.md`（本轮留痕）

## In scope

- ADR-0075 Accepted，登记表与 `decisions.md` 同步。
- `gov §5.4` 表格行数成为 hygiene 规则总数唯一事实源；删除硬编码总数。
- `ci.yml` 每个门禁步骤 / 作业增加恰好一个 `# gov-gate: <id>` 标记。
- `xtask` 解析并校验两套门禁编号集合；不一致为 Error / exit 1。
- 纯函数负向单测覆盖缺失、重复、不可解析和 negative implemented count。
- 同步 LEDGER、PLAN、README、plans 与轮次报告，追加 PL-022 闭环。

## Out of scope（做了算漂移）

- 不把 gov #9 覆盖率或 #11 `cargo doc` 软门禁转硬。
- 不新增 crate、顶层目录、第三方依赖、`#[allow]` 或 `unsafe`。
- 不放宽 / 删除既有门禁或断言；不自动改写文档 / workflow。
- 不改产品代码、公共 trait / schema / ErrorCode。
- 不碰在飞轮次的 `crates/capture`、`crates/verify`、`crates/platform/windows` 写 scope。

## 必须遵守

- **铁律 1 / 2 / 9 / 10**：无静默失败；文档输入不可信；不扩范围；契约先行。
- **ADR-0019**：新增机器判据必须带负向验证。
- **ADR-0072**：数量不手抄，只从事实源派生或指向事实源。
- **ADR-0028**：热点文件写前取锁，写完立即释放。
- **用户预授权**：本轮按最优方案自决；ADR 可代为 Accepted。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p xtask
cargo run -p xtask -- hygiene
cargo run -p xtask -- --list-deferred
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- check-migrations
```

负向验证：临时删除 / 重复一个 `# gov-gate` 标记或移除 `gov §5.4` separator，命令必须以 exit 1 失败并指出不一致。

## 完成定义（DoD）

- [ ] ADR-0075 Accepted，登记表与 `decisions.md` 同步，`adr-index` 0E0W。
- [ ] `TOTAL_HYGIENE_RULE_COUNT` / `IMPLEMENTED_HYGIENE_RULE_COUNT` 硬编码已删除。
- [ ] `gov §5.1` 与 `ci.yml` 标记集合在真实仓库一致，负向样本证明不一致会失败。
- [ ] `cargo test -p xtask` 含正负样本并通过；全 workspace 门禁全绿。
- [ ] 热点文件均先 guard 再写；`guard status` 最终 NONE。
- [ ] PL-022 仅追加闭环行；LEDGER / PLAN / README / plans 与本卡同批同步。
- [ ] PR #243 首轮 pull_request CI `37393595320` 11/11 SUCCESS，`MERGEABLE` + `CLEAN` + base=main；merge hash 待合并后回填。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-244 机器派生计数收口（PL-022）
【目标】hygiene 总数从 gov §5.4 派生，gov §5.1 与 ci.yml 标记集合机器校验。
【write scope】本卡、ADR-0075、ADR 登记表、decisions、gov、ci.yml、xtask deferred/doccheck/main、
              PARKING_LOT、LEDGER、PLAN、README、plans、轮次报告。
【铁律】1 无静默失败；2 文档不可信；9 不扩范围；10 契约先行；ADR-0019 负向验证。
【禁止】不转硬 #9/#11；不新增 crate / 顶层目录 / 依赖 / allow / unsafe；不放宽判据；不碰在飞 PR 写 scope。
【验收】fmt / clippy / test workspace / test xtask / xtask 十一门禁全 exit 0；坏标记与坏表格必须 exit 1。
【依赖】TASK-241 / ADR-0072 已 Done；ADR-0074 被在飞分支占用，本轮选 0075。
【疑问】无；按预授权直接推进。
```

### 2. 实际改动文件

- `docs/adr/0075-machine-derived-governance-counts.md`：新增 Accepted ADR，冻结计数事实源、CI 标记映射和失败判据。
- `docs/adr/README.md`：登记 0075，下一可用编号改为 0076。
- `docs/memory/decisions.md`：追加 ADR-0075 决策条目。
- `docs/governance-ai-agent-execution.md`：§5.4 表述改为运行时派生。
- `.github/workflows/ci.yml`：仅增加 18 个 `# gov-gate: <id>` 注释标记，不改任何命令或触发语义。
- `xtask/src/deferred.rs`：删除 `TOTAL_HYGIENE_RULE_COUNT` / `IMPLEMENTED_HYGIENE_RULE_COUNT`；新增纯解析、集合校验和负向单测。
- `xtask/src/doccheck.rs`：读取治理文档与 CI workflow，调用纯函数派生计数。
- `xtask/src/main.rs`：`hygiene` / `--list-deferred` 使用派生计数；不一致按 exit 1 报告。
- `MEMORY.md`：同步 `decisions.md` 规模行。
- `docs/PARKING_LOT.md`：仅追加 PL-022 闭环行。
- `plans/stage-1-pilots.md`：新增 TASK-244 条目行。
- `LEDGER.md`：追加本卡 InProgress 事件行。
- `docs/automations/2026-10-05-round-5.md` 与 `2026-10-05-report.md`：记录本轮产物、门禁与未合并 WIP。
- `tasks/TASK-244-derived-counts.md`：本卡。
- 未改产品代码、公共 trait / schema / ErrorCode、依赖、crate 或顶层目录。

### 3. 验收输出摘要

- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0；只有仓库既有 `clippy::assert_is_empty` unknown-lint 提示。
- `cargo test --workspace` → EXIT 0。
- `cargo test -p xtask` → 463 passed / 0 failed。
- `cargo run -p xtask -- hygiene` → PASSED，scanned=367，0E/105W；输出 `gov §5.4 共 13 项，已实现 13 项，未实现 0 项`。
- `cargo run -p xtask -- --list-deferred` → `gov §5.4 的 13 项卫生规则已全部实现`。
- `cargo run -p xtask -- memory-counts` → PASSED，8 files，0E/0W。
- `cargo run -p xtask -- adr-index` → PASSED，scanned=62，0E/0W。
- `cargo run -p xtask -- refscan` → PASSED，scanned=707，0E/0W。
- `cargo run -p xtask -- docscan` → PASSED，scanned=319，0E/336W。
- `cargo run -p xtask -- card-check` → PASSED，scanned=138，0E/34W。
- `cargo run -p xtask -- check-ledger` → PASSED，plan_date / ledger_last_date = 2026-10-06。
- `cargo run -p xtask -- check-comments` → PASSED，scanned=367，0E/69W。
- `cargo run -p xtask -- verify-schemas` → PASSED，5/5。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift。
- `cargo run -p xtask -- check-migrations` → PASSED，5 files / 5 registry entries。
- `cargo deny check` → advisories / bans / licenses / sources 全 ok。
- 负向 canary（临时最小仓库删掉 CI 的 `# gov-gate: 2`）：
  `xtask: 治理计数错误：gov §5.1 与 ci.yml 的 # gov-gate 集合不一致：gov=2 项，ci=1 项；ci 缺失=[2]；ci 额外=[]`
  → `negative-canary-exit=1`。
- PR #243；首轮 pull_request CI run `37393595320` = 11/11 SUCCESS；merge hash 待合并后回填。

### 4. DoD 逐条核对

- [x] ADR-0075 Accepted，登记表与 `decisions.md` 同步，`adr-index` 0E0W。
- [x] `TOTAL_HYGIENE_RULE_COUNT` / `IMPLEMENTED_HYGIENE_RULE_COUNT` 硬编码已删除。
- [x] `gov §5.1` 与 `ci.yml` 标记集合在真实仓库一致；缺失 / 重复 / 坏表格 / 负数下界负向样本均已覆盖。
- [x] `cargo test -p xtask` 与 workspace / xtask 门禁全绿。
- [x] 热点文件均先 guard 再写；本轮结束前再做最终 `guard status`。
- [x] PL-022 仅追加闭环行；LEDGER / plans 已同批同步，PLAN / README 将在状态翻转提交同步。
- [x] PR #243 首轮 pull_request CI `37393595320` = 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main；merge hash 待合并后回填。

### 5. 偏差

- 无功能偏差。改动行数超过 400 行的建议预算：本轮目标要求 PL-022 的 ADR、双事实源解析、正负样本和状态同步完整落地，故按全量收口交付并在此记录超预算原因。
- ADR-0074 已被两个在飞分支使用；本轮取 0075，并在登记表备注避让原因。

### 6. 更合理做法

- 用显式 `# gov-gate` 标记让 CI 承接关系成为可解析映射，而不是尝试按原始 `name:` 数量猜测；这样既支持一个门禁由多个步骤表达，也能精确报出缺失 / 重复 / 额外编号。

### 7. 遗留问题

- 若未来门禁拆到多个 workflow 或改为结构化清单，需要按 ADR-0075 的重新评估条件扩展解析范围。
- PR #243 首轮 CI 已完成；最终状态翻转提交仍需再跑 CI，merge hash 待合并后回填。

### 8. 新增长期记忆

- ADR-0075 决策已追加到 `docs/memory/decisions.md`。

### 9. 给审阅者的关注点

- 标记只表达“CI 哪个步骤承接 gov 门禁”，不是第二份数量；集合一致性由 xtask 机器校验。
- 在飞 TASK-042 / TASK-041-B 分支都使用 ADR-0074，本轮以 0075 避让；合并时以 main 登记表为准。

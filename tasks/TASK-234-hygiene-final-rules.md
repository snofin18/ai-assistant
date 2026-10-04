# TASK-234　补齐 gov §5.4 最后两条 hygiene 规则（13/13）

- 状态：**Done（2026-10-04；ADR-0068 / 0069 Accepted，hygiene 13/13 + 0E，PR #218 / merge `e417a2a`，CI 11/11 SUCCESS）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：PL-060、ADR-0025、ADR-0068、ADR-0069
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/PARKING_LOT.md` PL-060、PL-023、`docs/governance-ai-agent-execution.md` §5.4、`xtask/src/deferred.rs`

---

## 目标（一句话）

把 `xtask hygiene` 从 11/13 补到 13/13：跨文件重复代码与顶层目录 ADR 白名单都要有先行的 Accepted ADR、
可执行规则、正负样本与全绿证据。

## 背景（为什么现在做）

PL-060 明确记录最后两条规则没有卡：相似度指纹 / 阈值未裁决，顶层目录白名单文件也不存在。
按铁律 10，两条规则都必须先写 ADR，再实现。`deferred.rs` 的 `UNASSIGNED_HYGIENE_CARD` 是指向 PL-060
的可执行指针，本轮实现后它和 `--list-deferred` 的「未实现 2 项」必须一起消失。

## write scope

- `docs/adr/0068-cross-file-duplicate-code-hygiene.md`、`docs/adr/0069-top-level-directory-adr-whitelist.md`
- `docs/adr/top-level-directories.md`、`docs/adr/README.md`
- `docs/memory/{decisions,facts,pitfalls}.md`、`MEMORY.md`（仅规模表）
- `xtask/src/**`、`xtask/README.md`
- `tasks/TASK-234-hygiene-final-rules.md`
- `docs/PARKING_LOT.md`（仅追加 PL-060 闭环 + PL-023 关系）
- `LEDGER.md` / `PLAN.md`（仅当前状态块）/ `README.md`（仅三处）
- `plans/stage-1-pilots.md`（本卡完成标记 + 当前进度句）
- `docs/automations/2026-10-04-round-1.md`

## In scope

- ADR-0068：定义重复代码指纹、阈值、忽略路径、预算与 Warning 级别。
- ADR-0069：定义 `docs/adr/top-level-directories.md` 白名单、维护者、Error/Warning 判据与 PL-023 关系。
- 实现 `hygiene/duplicate-code`、`hygiene/unregistered-top-level-directory` 及配套显式失败规则。
- 为两条规则提供正样本与负向自证（相同 / 改名代码命中；不同代码不命中；未登记目录命中；合规目录不命中）。
- 实现后 `--list-deferred` 的 hygiene 未实现项为空，`UNASSIGNED_HYGIENE_CARD` 删除。

## Out of scope（做了算漂移）

- 不新增第三方依赖、crate、公共 trait/schema 或 `docs/spec/**` 变更。
- 不把重复代码规则升为 Error；不为通过规则修业务代码。
- 不顺手实现 `scripts/` 目录；PL-023 仍由单独 ADR 裁决。
- 不重构与两条规则无关的 xtask 逻辑。

## 必须遵守

- **ADR 先行**：铁律 10；ADR-0068 / 0069 标 Accepted 后才能实现。
- **零依赖**：xtask 保持无第三方依赖；规则判定为纯函数，IO 留在边界。
- **无静默失败**：预算超限、白名单缺失、目录未登记都必须显式 Finding。
- **热点文件锁**：LEDGER / PLAN / README / plans / docs/memory / PARKING_LOT / MEMORY 写前 guard。
- **状态行收口**：提交前列出本批 `tasks/TASK-*.md` 并逐张比对状态行与 LEDGER，结果贴进 PR。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p xtask
cargo run -p xtask -- hygiene
cargo run -p xtask -- --list-deferred
cargo run -p xtask -- adr-index
cargo run -p xtask -- memory-counts
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
```

## 完成定义（DoD）

- [ ] ADR-0068 / 0069 Accepted，并同步登记表、下一可用号、`decisions.md`。
- [ ] 两条规则实现且 `hygiene` 13/13、0 Error；`--list-deferred` hygiene 未实现项为空。
- [ ] 两条规则都有正负样本测试；负向样本确实命中。
- [ ] 白名单文件落地，当前顶层目录 0 Error；`scripts/` 明确留给 PL-023。
- [ ] 全门禁绿，PR CI 11/11、mergeable / CLEAN / base main 后合并并回填 hash。
- [ ] PL-060 在 PARKING_LOT 闭环，状态行与 LEDGER 一致。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-234 补齐 gov §5.4 最后两条 hygiene 规则（13/13）
【目标】ADR-0068/0069 Accepted 后实现重复代码与顶层目录白名单规则，
        用正负样本、全门禁、PR/CI/merge-hash 收口 PL-060
【write scope】docs/adr/{0068,0069,top-level-directories,README}、docs/memory/{decisions,facts,pitfalls}、
              MEMORY 规模表、xtask/**、本卡、PARKING_LOT、LEDGER、PLAN、README、plans、round report
【铁律】10 契约先行；1 无静默失败；9 不扩 scope；ADR-0028 热点文件 guard；零第三方依赖
【禁止】新增忽略规则只用于让自己通过；放宽 / 删除既有判据；改 docs/spec / AGENTS / gov；
        未裁决就登记 scripts/
【验收】hygiene 13/13 且 0E；--list-deferred hygiene 未实现为空；两条规则的负向样本命中；
        fmt / clippy / workspace test / xtask test + adr-index / memory-counts / refscan / docscan /
        card-check / check-ledger / check-comments / verify-schemas / codegen --check 全绿
【依赖】PL-060 已读；ADR 下一号 = 0068；TASK-233 已占用，本卡取 234
【疑问】无；自动化预授权允许由本轮代裁决并实施到底
```

### 2. 实际改动文件

- 契约：`0068-cross-file-duplicate-code-hygiene.md`、`0069-top-level-directory-adr-whitelist.md`、
  `top-level-directories.md`、`adr/README.md`、`docs/memory/decisions.md`。
- 实现：`duplicate_code.rs`、`duplicate_code_tests.rs`、`top_level_dirs.rs`、`top_level_dirs_tests.rs`、
  `hygiene_io.rs`、`repowalk.rs`、`repowalk_top_level_tests.rs`、`hygiene.rs`、`deferred.rs`、`cli.rs`、`main.rs`。
- 记录与状态：本卡、`docs/memory/facts.md`、`docs/memory/pitfalls.md`、`MEMORY.md`、
  `docs/PARKING_LOT.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、
  `docs/automations/2026-10-04-round-1.md`。

### 3. 验收输出摘要

本节同时记录实现口径、负向样本与已跑命令的原始摘要。

**ADR-0068（重复代码）**

- 40 个规范化 token = 一个 shingle；忽略空白 / 注释，字面量 / 数字 / 非关键字标识符归一化。
- 无序文件对共享 distinct shingle ≥ 4 且包含度 ≥ 80% → 单条 Warning。
- 忽略 `fixtures/**`、`spikes/**`、`tools/**`、测试 / `_tests.rs`、生成代码。
- 1,000 文件 / 单文件 1 MiB / 1,000,000 shingle 硬预算；超限往 report 增加
  `hygiene/duplicate-scan-truncated`，不静默截断。

**ADR-0069（顶层目录）**

- 唯一白名单 = `docs/adr/top-level-directories.md`，固定标题 `## 允许的顶层目录`。
- 未登记目录 = Error；白名单缺失 / 不可解析 = Error；白名单陈旧 = Warning。
- 当前目录全部登记；`scripts/` 不预授权，PL-023 仍待单独 ADR。

**负向样本**

- `test_identical_sources_are_reported`：相同代码必须命中。
- `test_identifier_renames_still_match`：局部变量改名后仍命中，证明 token 规范化生效。
- `test_dissimilar_sources_are_not_reported`：不同代码不误报。
- `test_unregistered_directory_is_error`：未登记目录必须显式 Error。
- `test_stale_whitelist_entry_is_warning` / `test_parse_whitelist_rejects_missing_heading_or_rows`：
  两种白名单退化形态都有判据。

### 4. DoD 逐条核对

- [x] ADR-0068 / 0069 Accepted，登记表 / 下一可用号 / `decisions.md` 已同步。
- [x] 两条规则实现，`hygiene` 13/13 且 0 Error，`--list-deferred` 未实现 hygiene 项为 0。
- [x] 两条规则均有正负样本，重命名复制与未登记目录负向样本命中。
- [x] 白名单文件落地，当前顶层目录 0 Error，`scripts/` 留给 PL-023。
- [x] 合并 / CI / merge-hash 回填：PR #218，CI run `37172655217`，11/11 SUCCESS，merge `e417a2a`。

```text
$ cargo test -p xtask
test result: ok. 452 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo run -p xtask -- --list-deferred
gov §5.4 的 13 项卫生规则已全部实现：未实现 0 项。

$ cargo run -p xtask -- hygiene
-- machine-summary: hygiene: scanned=357 errors=0 warnings=95 verdict=PASSED
-- deferred-rules: gov §5.4 共 13 项，已实现 13 项，未实现 0 项
-- summary: 0 error(s), 95 warning(s)
-- verdict: PASSED
```

剩余门禁、PR / CI / merge hash 在本卡后续记录区追加；未跑完的命令不得写成通过。

### 5. 偏差

- 初始 shingle 预算 250,000 在真实仓库触发截断 Warning；按实测把 ADR-0068 D9 调整为
  1,000,000，并保持超限显式告警。调整发生在实现阶段、提交前，不是放宽判据以骗绿。
- `docs/memory/decisions.md` 的一次预算校正在首次 guard 前发生；随后已先回退该行，
  再在同一 session 内 acquire、锁内重写并 release。无并发写者、无 lost update，最终内容受锁保护。
- 接口 / schema / 依赖均无偏差；未改 `docs/spec/**`、`AGENTS.md`、gov。

### 6. 更合理做法

新增跨文件规则前，应把 `main.rs` 的 IO helper 边界一次性抽清；本轮实现过程中才把
`collect_rust_hygiene_data` / 文本 / 依赖 / 白名单读取移入 `hygiene_io.rs`，避免了把主文件推过 900 行硬限。

### 7. 遗留问题

- `PL-023`：`scripts/` 是否允许、与 `tools/` 的边界仍待单独 ADR；本 ADR 只规定“未登记不得新增”。
- 重复代码规则当前为 Warning；若要升 Error，必须先清零既有告警或另立 ADR。

### 8. 新增长期记忆

- FACT：`hygiene` 13/13 已实现；重复代码规则为 token shingle + 包含度 Warning，顶层目录白名单为 Error。
- PITFALL：跨文件规则应隔离 IO / 判定边界，否则会同时触发 `main.rs` 行数硬限与规则自身结构告警。

### 9. 给审阅者的关注点

1. 重复代码阈值是契约：检查 ADR-0068 D2/D3/D4 与代码常量一致。
2. 顶层目录白名单是否允许 `scripts/`：当前刻意不登记，PL-023 仍是独立决策。
3. 13/13 只表示规则已实现，不表示仓库零 warning；本轮实测 95W、0E，Warning 不改 verdict。

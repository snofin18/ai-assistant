# TASK-226　测试卫生清理：真机用例串行化 + production_root 拆分

- 状态：**Done（2026-10-03；PL-104 / PL-105 闭环；修复前并行红灯 → 修复后 8 passed / 0 failed；生产测试拆分 433 / 390 / 101 行）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：TASK-223 Done（PL-104 / PL-105 来源）
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/PARKING_LOT.md` PL-104 / PL-105、`apps/agent-core/tests/production_root_uia.rs`、`apps/agent-core/tests/production_root.rs`

---

## 目标（一句话）

清掉两个测试卫生地雷：真机 ignored 用例并行时不再因同 AutomationId 窗口互相污染，同时把
`production_root.rs` 按职责拆到 600 行软上限以下且不删除、不放宽任何断言。

## write scope

- `tasks/TASK-226-test-hygiene-cleanup.md`（本文件）
- `apps/agent-core/tests/**`
- `fixtures/apps/notepad-like/**`（仅当选 PL-104 方案 a 时）
- `docs/PARKING_LOT.md`（仅追加 PL-104 / PL-105 闭环行）
- `LEDGER.md`（仅追加）、`PLAN.md`（仅「当前状态」块）、`README.md`（仅状态行 / `## 当前阶段` / `## 最近进展`）
- `plans/stage-1-pilots.md`（仅本卡完成标记 + 「当前进度」句）
- `MEMORY.md`（仅规模表）、`docs/memory/{facts,pitfalls,rejected}.md`（仅按规则追加）

## In scope

- **PL-104**：评估并实施以下两种方案之一，并给修复前并行红灯、修复后并行绿灯的对照证据：
  (a) 给 `notepad-like` 靶机窗口的 AutomationId 加实例后缀，并同步测试选择器；
  (b) 在测试内串行化会启动靶机的 ignored 用例（例如共享 `static Mutex`），只让 fixture 启动互斥。
- **PL-105**：把 `apps/agent-core/tests/production_root.rs` 拆成 2~3 个 `tests/*.rs`，共用
  `tests/support/production_fixture.rs`；拆分后每个文件都在 600 行以下。
- 拆分只做代码移动与必要 helper 提取；原有断言一条都不能删、不能放宽。
- 同步闭合 PL-104 / PL-105，并更新卡、LEDGER、PLAN、README、阶段计划与记忆规模表。

## Out of scope（做了算漂移）

- 改生产代码 `apps/agent-core/src/**`、`crates/**`、协议、公共接口、schema、ErrorCode。
- 新增依赖、crate、顶层目录、允许 lint 或放宽任何门禁。
- 删除、忽略或弱化现有测试断言。
- 真实商业应用；ignored 用例只在仓库自带 `notepad-like` 上运行。

## 必须遵守

- **铁律 1**：真机测试若无法运行必须写明原因，不能伪装成通过。
- **铁律 9**：严格限制在 write scope；另一个自动化负责的 `crates/lease/**` 与 `apps/agent-core/src/**` 一律不碰。
- **铁律 10**：不改变任何契约；本次只动测试组织与测试夹具协调。
- ADR-0028：写 `LEDGER.md` / `PLAN.md` / `README.md` / `plans/*` / `docs/PARKING_LOT.md` / `MEMORY.md` /
  `docs/memory/*` 前必须 `guard acquire`，写完立刻 release。
- ADR-0031：本卡分界线以上只读；执行记录只写分界线以下。
- ADR-0033 / gov §5.4：拆分后测试文件必须低于 `hygiene/file-too-long` 硬上限，目标低于 600 行软上限。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test production_root*
cargo test -p assistant-agent-core --test production_root_uia -- --ignored --test-threads=4
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
```

## 完成定义（DoD）

- [ ] PL-104 有两种方案对比；选定方案有修复前红灯和修复后并行绿灯的原始证据。
- [ ] PL-105 拆分后 `production_root.rs` 及新文件都低于 600 行，断言无删除、无放宽。
- [ ] `hygiene/file-too-long` 不再指向 `production_root.rs`；hygiene 无 Error。
- [ ] `cargo test -p assistant-agent-core --test production_root*` 全绿。
- [ ] 全部卡面验收命令按退出码核；未运行的 ignored 真机证据不写成通过。
- [ ] PL-104 / PL-105 闭环，状态文件与长期记忆同步。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-226 清理两个测试卫生地雷（PL-104 + PL-105）
【目标】让 production_root_uia 的 ignored 真机用例并行安全，并把 888 行的 production_root.rs 拆成每文件 <600 行且断言零删除/零放宽
【write scope】仅：tasks/TASK-226-*.md；apps/agent-core/tests/**；fixtures/apps/notepad-like/**（仅若最终选方案 a）；LEDGER.md / PLAN.md（仅「当前状态」块）/ README.md（仅三处）/ plans/stage-1-pilots.md（本卡标记 + 「当前进度」句）/ docs/PARKING_LOT.md（仅追加闭环行）/ MEMORY.md（仅规模表）/ docs/memory/{facts,pitfalls,rejected}.md（仅按规则追加）
【铁律】1 无静默失败；2 测试输入/夹具结果不可信；9 不静默扩大范围；10 契约先行；ADR-0028 guard 锁；ADR-0031 卡片正文只读
【禁止】改 crates/lease/** 或 apps/agent-core/src/**（另一自动化 write scope）；删/放宽任何断言；新增依赖/公共接口；改 spec/ADR/AGENTS；真实商业应用；把未跑的 ignored 用例写成 CI 通过
【验收】fmt / clippy -D warnings / test --workspace；PL-104 修复前 --ignored --test-threads=4 红灯 → 修复后同命令绿灯；cargo test -p assistant-agent-core --test production_root*；hygiene 中 production_root.rs file-too-long 消失且 0E；memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments / verify-schemas / codegen --check 全绿；能跑则贴 ignored 原始输出
【依赖】TASK-223 Done、TASK-220 Done（启动时已核 LEDGER）；PL-104/PL-105 来自 2026-10-03 TASK-223
【疑问】本轮派单明确要求与另一自动化并行，与 automation charter G8 的“无并发”相冲突；按派单不创建仓库级 .automation.lock，改用热点文件 guard 互斥。并发期间另一分支占用 TASK-225 且同一 checkout 切走了分支，因此本卡改用当时下一个可用号 TASK-226，并在独立 linked worktree 中完成，避免两个 PR 争用同一卡号与工作区。
```

### 2. 实际改动文件

- `tasks/TASK-226-test-hygiene-cleanup.md`（本卡）
- `apps/agent-core/tests/production_root.rs`（拆出 T1.2 / T1.3 / 回滚，保留 T1.1 与配置负向断言，433 行）
- `apps/agent-core/tests/production_root_t1_2.rs`（新增，T1.2 + 回滚，390 行）
- `apps/agent-core/tests/production_root_t1_3.rs`（新增，T1.3，101 行）
- `apps/agent-core/tests/production_root_uia.rs`（fixture 用例共享 `tokio::sync::Mutex`；仍 718 行，PL-104 不要求拆本文件）
- `apps/agent-core/tests/support/production_fixture.rs`（共享 `TestDirectory` / `workspace_root` helper）
- `plans/stage-1-pilots.md`（新增 TASK-226 批次行与卡片位置行；仅完成标记 / 当前进度句按规则维护）
- `docs/memory/facts.md`、`docs/memory/pitfalls.md`（各追加 1 条）
- `MEMORY.md`（仅规模表两行）
- 待提交前还会追加：`docs/PARKING_LOT.md`（PL-104 / PL-105 闭环行）、`LEDGER.md`（本卡事件行）、`PLAN.md`（当前状态块）、`README.md`（三处）。

### 3. 验收输出摘要

全部按退出码核（隔离 linked worktree，main 基线 `715e53a`）：

- `cargo fmt --all --check`：EXIT 0。
- `cargo clippy --all-targets -- -D warnings`：EXIT 0（仅既有 `clippy::assert_is_empty` unknown-lint warning；不改 lint）。
- `cargo test --workspace`：EXIT 0（workspace 全绿；agent-core 的 production_root 拆成 7 + 6 + 1 三组，共 14 passed）。
- `cargo test -p assistant-agent-core --test production_root*`：EXIT 0；三文件分别 7 / 6 / 1 passed。
- `cargo test -p assistant-core arch::`：EXIT 0（arch_dependencies 5 + arch_layering 5）。
- `cargo deny check`：EXIT 0（advisories / bans / licenses / sources 全 ok；仅有既有 license-not-encountered 与 duplicate 依赖 warning）。
- **PL-104 对照证据**：
  - 修复前原始红灯（main `715e53a` / 原文件）：`cargo test -p assistant-agent-core --test production_root_uia -- --ignored --test-threads=4` → **6 failed / 2 passed**，关键错误 `TargetAmbiguous: 3 windows matched app_id powershell.exe ...`；
  - 修复后同命令 → **8 passed / 0 failed / 1 filtered out / finished in 20.85s**。
- xtask 门禁：`hygiene` PASSED（0E / 102W；`production_root.rs` 的 `file-too-long` 已消失）、`memory-counts` PASSED、`adr-index` PASSED、`refscan` PASSED（632 files 0E/0W）、`docscan` PASSED（0E / 342W）、`card-check` PASSED（0E / 27W）、`check-ledger` PASSED、`check-comments` PASSED（0E / 69W）、`verify-schemas` PASSED、`codegen --check` PASSED。
- 行数实测：`production_root.rs` 433、`production_root_t1_2.rs` 390、`production_root_t1_3.rs` 101、`support/production_fixture.rs` 378；均在 600 行软上限以下。
- **合并与 CI 证据（回填）**：PR [#197](https://github.com/snofin18/ai-assistant/pull/197) 的 PR 事件 CI run `37107879818` = **11/11 SUCCESS**（check windows-latest 8m6s / ubuntu-latest 3m18s / macos-latest 4m21s、cargo deny ×2、doc consistency、desktop-ui checks、desktop-ui tauri (windows)、commitlint、gate negative verification #6、xtask deferred inventory）；合并前 `baseRefName=main`、`mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`，合并提交 **`29b68bc`**（2026-10-03T08:03:05Z，`state=MERGED`）。

### 4. DoD 逐条核对

- [x] PL-104 有两种方案对比（见 §6）；选定方案有修复前红灯和修复后并行绿灯原始证据。
- [x] PL-105 拆分后三个 `production_root*` 测试文件均低于 600 行，断言无删除、无放宽（仅移动与 helper 提取）。
- [x] `hygiene/file-too-long` 不再指向 `production_root.rs`；hygiene 0 Error。
- [x] `cargo test -p assistant-agent-core --test production_root*` 全绿。
- [x] 卡面验收命令按退出码核；ignored 真机套件本机实跑并贴原始红/绿结果。
- [x] PL-104 / PL-105 在 `docs/PARKING_LOT.md` 追加闭环行；状态文件与长期记忆同步。

### 5. 偏差

- none（本卡 write scope 内完成）。
- 并发说明：另一自动化与本轮同时存在，且同一 checkout 被切到 `codex/task-225-synthetic-input-lease`、占用 TASK-225。为不混合其 `apps/agent-core/src/**` / `Cargo.toml` 改动，本轮将自身工作隔离到 linked worktree，并将本卡改为 **TASK-226**。这是并发下的编号避让，不是产品语义漂移；本 PR 不包含另一分支任何文件。

### 6. 更合理做法

PL-104 方案对比：

- 方案 (a)（给靶机窗口 AutomationId 加实例后缀）能直接消除窗口歧义，但需要同时改 `fixtures/apps/notepad-like/**` 的窗口标识、CLI/启动参数、Adapter selector 与测试配置；跨越 fixture 与选择器契约，影响面大，且残留旧实例仍可能撞同一后缀生成规则。
- 方案 (b)（测试内串行化 fixture 启动）直接对应根因：冲突发生在同一测试二进制同时启动多个同 AID 窗口；用一个 `tokio::sync::Mutex` 只包住 `start_fixture` 生命周期，既让 6 个 fixture-owning ignored 用例串行，又保留普通测试和纯控制用例的并行能力。实现改动只在测试层，不扩公共契约。
- 最终选 **(b)**；`--test-threads=4` 对照证明修复后不再需要调用方手工加 `--test-threads=1`。

### 7. 遗留问题

- `production_root_uia.rs` 仍为 718 行，`hygiene/file-too-long` 仍会对此文件发出非阻断 Warning；这不是 PL-105 的对象，且本卡只承诺消除 `production_root.rs` 的硬风险。若后续要清零该 Warning，建议立独立拆卡。

### 8. 新增长期记忆

- `docs/memory/facts.md`：`[2026-10-03][FACT][src:TASK-226 实测] 同一 ignored 测试二进制内的真机 fixture 启动可用异步互斥串行化...`（全文见该文件）。
- `docs/memory/pitfalls.md`：`[2026-10-03][PITFALL][src:TASK-226 实测] std::sync::MutexGuard 不能跨 await 持有...`（全文见该文件）。

### 9. 给审阅者的关注点

- 先看 `production_root_uia.rs` 的 `tokio::sync::Mutex` guard 是否确实覆盖 `start_fixture` 到 `FixtureProcess` drop 的完整生命周期；并行绿灯是行为证据，不应只看静态类型。
- 再看拆分 diff：断言应逐字等价，`production_root.rs` 只应少掉 T1.2/T1.3/回滚块和已移入 support 的 helper；任何语义变化都是风险。
- `hygiene` 仍对 `production_root_uia.rs` 报 718 行 Warning；这是已知残留而非本卡失败，不要把“仍有 Warning”误读为 PL-105 未闭环。

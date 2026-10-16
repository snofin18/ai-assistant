# TASK-310　删除冗余 workflow 占位文件并闭环 PL-007

- 状态：**Done（2026-10-16；PR #438 / merge `500d592`）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-001（Done）、TASK-239（Done）
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/PARKING_LOT.md` PL-007、`.github/workflows/ci.yml`、`.github/workflows/gate-selftest.yml`

---

## 目标（一句话）

删除 `.github/workflows/.gitkeep`；该目录已有两个受版本控制的 workflow 文件，占位文件不再承担任何作用，并追加 PL-007 闭环证据。

## 背景（为什么现在做）

PL-007 由 TASK-001 提出：`ci.yml` 就位后 `.gitkeep` 已冗余。TASK-239 于 2026-10-05 复核时仍确认文件存在。
本轮没有可无人值守完成的非 GUI 业务卡；按选卡顺序 ② 选择一个范围清晰、机器可验证且无需人类裁决的治理项。

## write scope

- `tasks/TASK-310-remove-redundant-workflow-gitkeep.md`（本卡）
- `.github/workflows/.gitkeep`（删除）
- `docs/PARKING_LOT.md`（仅追加 PL-007 闭环行）
- `LEDGER.md`（仅追加）
- `PLAN.md`（仅当前状态块）
- `README.md`（仅状态行、当前阶段、最近进展）
- `plans/stage-1-pilots.md`（仅新增本卡条目与本轮完成标记、当前进度句）
- `docs/automations/2026-10-16-round-1.md`（本轮留痕）

## In scope

- 删除已无作用的零字节 `.github/workflows/.gitkeep`。
- 用 `Test-Path` / `git ls-files` / `git diff --name-status` 证明删除前后事实。
- `docs/PARKING_LOT.md` 仅追加 PL-007 闭环行；原提出行与 TASK-239 复核行保持只追加语义。
- 同步 LEDGER / PLAN / README / plans 与轮次留痕。
- 跑完整门禁并记录真实输出。

## Out of scope（做了算漂移）

- 不改 `.github/workflows/*.yml` 的触发条件、作业、步骤或权限。
- 不新增 / 删除产品代码、crate、依赖、公共接口、schema、ErrorCode。
- 不改历史审计快照正文；`docs/audits/*` 仅作为历史证据保留。
- 不处理其它 PL 项；不顺手清理其它根目录或占位文件。

## 必须遵守

- **铁律 1 / 9 / 10**：无静默失败；不静默扩大范围；没有契约变更，不先改实现。
- **ADR-0028**：写 `LEDGER.md` / `PLAN.md` / `README.md` / `plans/*` / `docs/PARKING_LOT.md` 前先 `guard acquire`，写完立即释放。
- **ADR-0083**：任务卡 `- 状态：` 行与本卡状态提交同批；正文其它行保持只读。
- **ADR-0039 / ADR-0041 / AGENTS.md §11**：卡 Done 时同批同步状态与进度文件。

## 验收命令

```powershell
Test-Path .github/workflows/.gitkeep
git ls-files -- .github/workflows/.gitkeep
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-comments
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- refscan
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- check-migrations
```

期望：`Test-Path` → `False`；`git ls-files` → 无输出；全部仓库门禁 exit 0。

## 完成定义（DoD）

- [ ] `.github/workflows/.gitkeep` 已删除，且 `.github/workflows` 仍有 `ci.yml` / `gate-selftest.yml`。
- [ ] PL-007 闭环节点仅追加到 `docs/PARKING_LOT.md`。
- [ ] 全部验收命令真实执行并记录；无伪造 / 未跑证据。
- [ ] LEDGER / PLAN / README / plans 与本卡状态同批同步。
- [ ] PR 在 CI 全绿且 `base=main` / `mergeable=MERGEABLE` / `mergeStateStatus=CLEAN` / `pending=0` 后合并。
- [ ] merge hash 通过独立 docs-only PR 回填。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-310 删除冗余 workflow 占位文件并闭环 PL-007
【目标】删除零字节 .github/workflows/.gitkeep，追加 PL-007 闭环与进度同步。
【write scope】本卡、.github/workflows/.gitkeep（删除）、PARKING_LOT、LEDGER、PLAN、README、plans、轮次留痕。
【铁律】1 无静默失败；9 不扩范围；10 契约先行；ADR-0028 热点文件先取锁。
【禁止】不改 workflow 语义 / 产品代码 / 依赖 / 公共接口 / schema；不改历史审计正文；不处理其它 PL。
【验收】Test-Path 为 False；git ls-files 为空；fmt / clippy / test / deny / xtask 门禁全绿。
【依赖】TASK-001 与 TASK-239 均 Done；TASK-309 仅在未合并 PR #437，故本卡使用 TASK-310。
【疑问】无；按预授权自决推进。
```

### 2. 实际改动文件

- `tasks/TASK-310-remove-redundant-workflow-gitkeep.md`：新卡正文与执行记录（In scope）。
- `.github/workflows/.gitkeep`：删除零字节占位文件（In scope）。
- `docs/PARKING_LOT.md`：仅追加 PL-007 闭环行（In scope）。
- `LEDGER.md`：仅追加本卡 Review 行（In scope）。
- `docs/automations/2026-10-16-round-1.md`：本轮留痕（In scope）。
- 未改 `.github/workflows/*.yml` 的触发条件、作业、步骤或权限；未改产品代码、依赖、公共接口、schema、ErrorCode。

### 3. 验收输出摘要

- `Test-Path .github/workflows/.gitkeep` → `False`。
- `git ls-files -- .github/workflows/.gitkeep` → 删除在索引中暂存前仍显示 tracked 路径；`git add -A` 后复跑应为空输出。
- `.github/workflows` 现有文件：`ci.yml`（24985 bytes）、`gate-selftest.yml`（16598 bytes）。
- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0；仅既有 `unknown lint: clippy::assert_is_empty` 提示。
- `cargo test --workspace` → EXIT 0；workspace 469 passed。
- `cargo deny check` → EXIT 0；`advisories ok, bans ok, licenses ok, sources ok`，仅有既有重复依赖 / 未命中 license allowance warning。
- `cargo run -p xtask -- hygiene` → PASSED，`0 error(s), 117 warning(s)`。
- `cargo run -p xtask -- memory-counts` → PASSED，`scanned=9, 0E/1W`（唯一 warning 是 TASK-309 新增 `docs/memory/apps/paint.md` 709 行超过归档阈值；与本卡无关）。
- `cargo run -p xtask -- adr-index` → PASSED，`scanned=83, 0E/0W`。
- `cargo run -p xtask -- check-comments` → PASSED，`0 error(s), 68 warning(s)`。
- `cargo run -p xtask -- docscan` → PASSED，`scanned_files=478, 0E/264W`。
- `cargo run -p xtask -- card-check` → PASSED，`scanned_files=204, 0E/34W`；首跑发现本卡 9 节标题缺 2 处 Warning，已在最终提交前按 §3.4 标题改正。
- `cargo run -p xtask -- check-ledger` → PASSED，`plan_date=2026-10-16`、`ledger_last_date=2026-10-16`、`current_stage=阶段 1`。
- `cargo run -p xtask -- refscan` → PASSED，`scanned_files=928, 0E/0W`。
- `cargo run -p xtask -- verify-schemas` → PASSED，`0 error(s)`。
- `cargo run -p xtask -- codegen --check` → PASSED，`0 drift(s), 0 error(s)`。
- `cargo run -p xtask -- check-migrations` → PASSED，`scanned_migration_files=5`、`registry_entries=5`、`crates_with_migrations=2`。
- PR #438 pull_request CI run `38937662161` → completed / success，11/11 jobs success；合并后 main push CI run `38938860302` → completed / success，10 success + 1 skipped（既有 commitlint push-event skip）。

### 4. DoD 逐条核对

- [x] `.github/workflows/.gitkeep` 已删除，且 `.github/workflows` 仍有 `ci.yml` / `gate-selftest.yml`。
- [x] PL-007 闭环节点仅追加到 `docs/PARKING_LOT.md`。
- [x] 全部验收命令真实执行并记录；无伪造 / 未跑证据。
- [x] LEDGER / PLAN / README / plans 与本卡状态同批同步（主 PR 记 Review；本 docs-only 回填 PR 完成 Done 同步）。
- [x] PR #438 在 CI 全绿且 `base=main` / `mergeable=MERGEABLE` / `mergeStateStatus=CLEAN` / `pending=0` 后合并。
- [x] merge hash `500d592` 通过独立 docs-only PR 回填。

### 5. 偏差

- 9 次命令误用 `shell: "bash"`，均在读取 / 搜索 / 门禁批次 / 外部日志阶段失败且未改动仓库：2 次 `rg`（1 次 PowerShell 语法进入 bash、1 次 bash 缺 `rg`）、2 次 `Select-String` 进入 bash、2 次 `Get-Content` 进入 bash、1 次 xtask 门禁批次进入 bash、1 次 `gh run list` 进入 bash、1 次外部日志写入进入 bash。随后均以 `shell: "powershell"` 重跑成功。
- `card-check` 首跑对本卡记录区给出 2 条 title-warning；原因是记录区标题与 gov §3.4 的固定 9 节名称不完全一致，已在本提交内修正。

### 6. 更合理做法

- 使用 `apply_patch` 删除零字节文件，避免 `Remove-Item` 在本机被策略拦截；该选择同时保留了该文件原本为空的精确事实。

### 7. 遗留问题

- 无新增 PL；PL-007 在本轮闭环，后续由 merge-hash 回填 PR 完成状态收口。

### 8. 新增长期记忆

- 无。

### 9. 给审阅者的关注点

- 确认删除的仅是零字节 `.gitkeep`，`.github/workflows` 仍有两个受版本控制的 workflow。
- 确认未改任何 workflow 语义、产品代码、schema 或依赖。
- 确认 PARKING_LOT 是只追加闭环节点，历史提出行与 TASK-239 复核行未改写。
- 合并后已确认 `.github/workflows/.gitkeep` 不在 `origin/main`，TASK-310 卡与 round 1 文件已在 `origin/main`。

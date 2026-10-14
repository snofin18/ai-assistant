# TASK-290　gov §9.2 明确 LEDGER 的 `pending` commit 占位与追加式回填（章程 §2 P4/P8）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：无（PL-010 已现场复核）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`docs/governance-ai-agent-execution.md`、`docs/PARKING_LOT.md`、`tasks/TASK-290-gov-ledger-pending-commit-convention.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-14-round-5.md`
- **关联**：`docs/PARKING_LOT.md` PL-010、`LEDGER.md` 头部只追加规则、ADR-0031 / ADR-0083

**目标**

闭环 PL-010：在 `docs/governance-ai-agent-execution.md` §9.2 的 LEDGER 模板与说明中
明确 `commit` 列允许在提交前写 `pending`，并规定真实 commit / PR / merge hash 只能通过
追加新事件行回填，禁止改写既有行。零产品代码改动，不新增依赖，不放宽任何门禁。

**背景**

`LEDGER.md` 自 2026-09-17 起已经实际使用 `pending` 表示“事件已发生、但提交哈希或合并
哈希在写入时不可预知”；当前仓库仍保留这些历史行，并用后续追加行回填。PL-010 指出
`gov §9.2` 模板缺少该约定，导致“只追加、不改写”与“首条台账需要真实 commit”看似冲突。
本轮只把已存在的实践写成模板契约，不改历史行，也不新增机器门禁。

**步骤**

1. 在 `gov §9.2` 模板中加入一条 `pending` 示例行。
2. 在模板后写明占位与回填规则：提交前可写 `pending`；哈希已知后追加新行，旧行不改。
3. 在 `docs/PARKING_LOT.md` 追加 PL-010 闭环行，原开放行保持不动。
4. 同批同步任务卡状态、`LEDGER.md`、`PLAN.md`、`README.md` 与阶段进度。
5. 运行文档硬门禁与仓库基线门禁，把原始输出摘要写入执行记录。

**DoD**

- [ ] `gov §9.2` 明确 `pending` 合法且只追加回填；模板含一条可直接模仿的示例
- [ ] 只改文档、任务卡与状态同步文件；产品代码 / schema / ADR / spec / 依赖 / CI 零改动
- [ ] `docs/PARKING_LOT.md` 以追加行闭环 PL-010，原行不改
- [ ] `check-ledger` / `hygiene` / `docscan` / `card-check` / `check-comments` / `refscan` / `memory-counts` / `adr-index` 全绿
- [ ] `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace` 全绿

**验收命令**

```powershell
rg -n -F "pending" docs/governance-ai-agent-execution.md
rg -n -F "PL-010" docs/PARKING_LOT.md
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-comments
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- check-migrations
cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录

### 1. 约束回执

【任务】TASK-290 PL-010：`LEDGER.md` 的 `pending` commit 口径
【目标】把提交前 `pending` 占位和哈希已知后的追加式回填写进 `gov §9.2`
【write scope】仅本卡列出的文档、任务卡与状态同步文件
【铁律】无静默失败；不扩 scope；台账只追加；不改测试或门禁判据
【禁止】产品代码 / schema / ADR / spec / 依赖 / CI；真实 GUI / 网络 / 凭据
【验收】见上；【依赖】无（PL-010 已现场复核）；【疑问】无

### 2. 实际改动文件

- `docs/governance-ai-agent-execution.md`（§9.2 两行模板改为 `pending` 占位 +
  追加式回填示例；不新增行，避免 `refscan` 的 ADR 豁免行号漂移）
- `docs/PARKING_LOT.md`（仅追加 PL-010 闭环行，原开放行不改）
- `tasks/TASK-290-gov-ledger-pending-commit-convention.md`
- `LEDGER.md`（追加 round 5 Review；哈希 / merge hash 仍用追加行回填）
- `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`（状态同步）
- `docs/automations/2026-10-14-round-5.md`（轮次产物）

### 3. 验收输出摘要

- `rg -n -F "pending" docs/governance-ai-agent-execution.md`：命中模板示例行。
- `rg -n -F "PL-010" docs/PARKING_LOT.md`：命中新增闭环行。
- `cargo run -p xtask -- check-ledger`：0E/0W，PASSED。
- `cargo run -p xtask -- hygiene`：0E/118W，scanned=417，PASSED。
- `cargo run -p xtask -- check-comments`：0E/67W，scanned=417，PASSED。
- `cargo run -p xtask -- memory-counts`：0E/0W，scanned=9，PASSED。
- `cargo run -p xtask -- adr-index`：0E/0W，scanned=80，PASSED。
- `cargo run -p xtask -- refscan`：修正等行数编辑后 0E/0W，scanned=884（含本 round 文件），PASSED。
- `cargo run -p xtask -- docscan`：0E/264W，scanned=446（含本 round 文件），PASSED。
- `cargo run -p xtask -- card-check`：0E/34W，scanned=185，PASSED。
- `cargo run -p xtask -- verify-schemas`：5 schemas，0E，PASSED。
- `cargo run -p xtask -- codegen --check`：0 drift / 0E，PASSED。
- `cargo run -p xtask -- check-migrations`：5 files / 5 entries，0E/0W，PASSED。
- `cargo fmt --all --check`：EXIT 0。
- `cargo clippy --all-targets -- -D warnings`：EXIT 0（仅既有 unknown-lint 警告）。
- `cargo test --workspace`：EXIT 0；workspace 469 passed + doctests 全绿。
- `cargo deny check`：advisories / bans / licenses / sources 全 ok，EXIT 0。
- `cargo run -p xtask -- replay --suite core`：2 steps / 5 changes / 0 mismatch，PASSED。
- `pnpm lint` / `pnpm typecheck` / `pnpm test`：均 EXIT 0；node 78 passed、vitest 3 passed。
- 非宿主纯 Rust clippy：`assistant-verify` 的 `x86_64-unknown-linux-gnu` 与
  `aarch64-apple-darwin --all-targets -D warnings` 均 EXIT 0。

### 4. DoD 逐条核对

- `gov §9.2` 已明确 `pending` 合法且只追加回填；模板含可直接模仿的占位 / 回填两行：满足。
- 只改文档、任务卡与状态同步文件；产品代码 / schema / ADR / spec / 依赖 / CI 零改动：满足。
- `docs/PARKING_LOT.md` 以追加行闭环 PL-010，原行不改：满足。
- `check-ledger` / `hygiene` / `docscan` / `card-check` / `check-comments` / `refscan` /
  `memory-counts` / `adr-index` 全绿：满足；其余仓库门禁亦全绿。

### 5. 偏差

- `refscan` 初次因新增模板行导致 ADR 豁免注册表的既有行号偏移而红。按“只做行号维护、
  不放宽判据”的原则尝试先改登记表，但该文件属 `docs/adr/*` 只读面；改为**等行数**编辑
  `gov §9.2`（只替换原两行模板，不新增行），再跑 `refscan` 从 `1 error` 回到 0E/0W。
- 本轮有 11 次 `exec_command` 误用默认 bash（`Get-Content` / `cargo` 等 not found，另有一次
  PowerShell 循环被按 bash 解析）；均在下一调用改用 `shell: "powershell"` 重跑并确认，
  没有产生仓库垃圾文件，也没有把未跑命令写成通过。该偏差逐条留在 round 5 报告。

### 6. 更合理做法

- 对会被 `refscan` 做行号豁免的文档，优先做等行数替换而不是追加段落；若必须新增行，
  应把 ADR 豁免登记表纳入同一卡 write scope，并在同批更新行号。

### 7. 遗留问题

- 无新的治理或功能遗留；PR 合并后仍需按本卡定义的 `pending` 口径追加 merge hash 回填行。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED；PL-010 的结论已写入 `gov §9.2` 与
  `docs/PARKING_LOT.md`。

### 9. 给审阅者的关注点

1. `gov §9.2` 的两行示例是否足够明确：提交前写 `pending`，取得真实哈希后只追加新行。
2. 是否接受用等行数编辑替代新增段落，以保持 `refscan` 豁免行号稳定。

### 10. 合并回填与诚实校正

- 主 PR：**#388**；merge hash：**4e9bce6**；pull_request run `38580201376`：
  **11/11 SUCCESS**；合并前 base=main、mergeable=MERGEABLE、mergeStateStatus=CLEAN。
- 偏差计数校正：主 PR 记录时写 11 次 bash 误用；合并、回填与清理期间又发生 18 次（含
  `Start-Sleep` / `$env:` / 多行非 ASCII 替换脚本在默认 bash 下失败），故诚实口径为
  **29 次**。其中一次 bash 展开在仓库根留下 4 个 0 字节非 ASCII 文件，已逐个核对为
  工作区内未跟踪文件后用 `[IO.File]::Delete` 清除；所有失败均立即以 PowerShell 重跑，
  无未跑即报通过。

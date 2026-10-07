# TASK-255　退役 charter / automation 文档中的 `docs/nightly/logs/` 残留引用（PL-057）

- 状态：**Done**
- 阶段：1　子阶段：**治理**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`docs/PARKING_LOT.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-07-round-7.md`、`docs/automations/2026-10-07-report.md`、`tasks/TASK-255-retire-nightly-log-path-references.md`
- **关联**：ADR-0054、ADR-0078、PL-057、2026-10-05 停车位复核

**目标**

验证 PL-057 所记录的现行文档残留是否仍存在：若不存在，按机器证据闭环 PL-057，
不改写历史台账，不新增 ADR，也不创建旧日志目录。

**背景**

PL-057 记录章程里有 3 处 `docs/nightly/logs/` 引用。后续 ADR-0051 已改名目录，
ADR-0078 已裁定不设立 `docs/nightly/logs/`，ADR-0054 已把自动化留痕改为“先落 PR 再自删”。
本轮需要以当前 live 文档为事实源复核，而不是重复旧结论。

**步骤**

1. 扫描 PL-057 指定的 `docs/automation-charter.md`，确认不再引用旧日志目录；
   已显式标为 fallback-only 的 `scheduler-acceptance-test.md` 不计入 live 对象。
2. 确认 `docs/nightly/logs/` 不存在、`docs/automations/` 存在且承载现行轮次产物。
3. 区分 live 文档与只追加历史记录；`docs/PARKING_LOT.md` 与历史审计快照中的旧文本不回改。
4. 在 `docs/PARKING_LOT.md` 追加 PL-057 闭环行，在 `LEDGER.md` 追加事件行。
5. 同步 `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`，补 round-7 与本运行日报告。

**DoD**

- [ ] PL-057 指定的 production live scope `docs/automation-charter.md` 对 `docs/nightly/logs` 为 0 命中；`scheduler-acceptance-test.md` 已显式标为 fallback-only，自含证据文本不计入
- [ ] `docs/nightly/logs/` 不存在；`docs/automations/` 存在
- [ ] `docs/PARKING_LOT.md` 追加 PL-057 闭环行，原历史行不改
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同步 TASK-255 Done
- [ ] round-7 与 2026-10-07 阶段报告补充段已落地
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask` 全部现行硬门禁 PASSED

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- memory-counts; cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan; cargo run -p xtask -- docscan; cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger; cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas; cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations
rg -n -F "docs/nightly/logs" docs/automation-charter.md
if exist docs/nightly/logs (exit 1) else (echo LIVE_PATH_ABSENT)
rg -n -F "PL-057" docs/PARKING_LOT.md
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-255 退役旧日志目录引用
【目标】验证 PL-057 已无 live 对象，并以机器证据闭环
【write scope】仅：本卡正文所列文件
【铁律】无静默失败；不得静默扩大范围；历史记录只追加；热点文件先 guard
【禁止】改产品代码 / schema / IPC / ErrorCode；新增依赖 / crate / 顶层目录；改 CI / lint；操作真实 GUI
【验收】见「验收命令」→ live scope 0 命中 + xtask 硬门禁 + fmt/clippy/test 全绿
【依赖】ADR-0054 / ADR-0078 已 Accepted；PL-057 复核开放
【疑问】无

### 2. 实际改动文件

- `tasks/TASK-255-retire-nightly-log-path-references.md`（新增）
- `docs/PARKING_LOT.md`（追加 PL-057 闭环行）
- `LEDGER.md`（追加 TASK-255 事件行）
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`（状态同步）
- `docs/automations/2026-10-07-round-7.md`（新增）
- `docs/automations/2026-10-07-report.md`（追加 round 5~7 收口补充段）

### 3. 验收输出摘要

- `rg -n -F "docs/nightly/logs" docs/automation-charter.md` → 0 命中，退出码 1。
- `if exist docs/nightly/logs` → `LIVE_PATH_ABSENT`；`if exist docs/automations` →
  `AUTOMATIONS_PATH_EXISTS`。
- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0（仅工具链既有未知 lint warning）。
- `cargo test --workspace` → EXIT 0；xtask 469 passed / 0 failed。
- `xtask hygiene` → 0E/100W；`memory-counts` / `adr-index` / `refscan` / `check-ledger` /
  `verify-schemas` / `codegen --check` / `check-migrations` → 0E/0W。
- `docscan` → 0E/273W；`card-check` → 0E/34W；`check-comments` → 0E/71W；全部 PASSED。
- PR #280 的 pull_request run `37685893175` → **11/11 SUCCESS**；合并前
  `baseRefName=main`、`mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`、`state=MERGED`；
  merge hash = `1a7037f`。

### 4. DoD 逐条核对

- production live 章程 0 命中：满足。
- 旧目录不存在、现行目录存在：满足。
- PARKING_LOT 追加 PL-057 闭环行且历史行不改：满足。
- LEDGER / PLAN / README / plans 同步：满足。
- round-7 与运行日报告补充段落地：满足。
- Rust 与 xtask 门禁全绿：满足（§3）。

### 5. 偏差

- 宿主 PowerShell 5.1 / 7（含 `-NoProfile`）均在执行正文前挂起；本轮如实记录后改用非 bash 的
  `cmd.exe` 外层执行。未因此跳过验收，也未把未运行命令写成已通过。
- guard 首次 acquire 时嵌套引号被外层 shell 拆坏，误建 6 把词级锁；立即逐个 release 并复核
  `guard status`，随后用无空格 intent 重新获取正确 5 个热点锁。
- PR #275 提示为 CLEAN，实查为 `mergeStateStatus=DIRTY`、`mergeable=CONFLICTING`；本轮不处理。

### 6. 更合理做法

PL-057 已由后续 ADR-0051 / ADR-0054 / ADR-0078 在事实上修复；本轮最合理的动作是验证并关闭
过期停车位，而不是新增 ADR 或重新改章程。

### 7. 遗留问题

- TASK-044 / `DRIFT-044-1` / `PL-113` 的真实 Paint GUI 验收仍为人工阻塞。

### 8. 新增长期记忆

无新 FACT / PITFALL / REJECTED；本轮只关闭过期停车位记录。

### 9. 给审阅者的关注点

1. 核对 production live 章程确实 0 命中，fallback-only 清单与历史记录未被误当作 live 对象。
2. 核对 PARKING_LOT 只追加，既有 PL-057 原行未被改写。
3. PR #275 与本轮无关，仍按 Paint GUI 缺口保持 open / CONFLICTING。

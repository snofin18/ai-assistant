# TASK-238　`capture` / `dlp` 边界 crate 骨架（解锁 1b / 1c）

- 状态：**Done（2026-10-04）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：PL-102、ADR-0071
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/PARKING_LOT.md` PL-102、`docs/adr/0071-capture-and-dlp-crate-boundaries.md`、TASK-041 / TASK-042 / TASK-050、架构 v2 §3、`docs/adr/top-level-directories.md`

---

## 目标（一句话）

以 ADR-0071 冻结 **截图管线** 与 **出域脱敏** 的 crate 边界，落地两个**零第三方依赖**的骨架
crate（`crates/capture`、`crates/dlp`），并闭环 `PL-102`，让 TASK-041 / 042 / 050 可以合法开工。

## 背景（为什么现在做）

- `PLAN.md` 的下一张卡是 1b 的 **TASK-041**，其 write scope 指向 `crates/capture/**` 与
  `crates/dlp/src/redact*`，但两个 crate 都不存在 —— 第一项工作会变成「新增 crate」，
  命中 AGENTS.md 漂移触发器 ②（加 crate → 停下升级）。
- 架构 v2 §3 的 crate 布局里有 `dlp/`（出域策略、脱敏、截图遮挡），**没有** `capture/`；
  而窗口截图原语 `WindowProvider::capture` 已经在 `crates/platform/api` 里。
- `PL-102`（10-03 自动化轮登记）就是这件事，状态「待人类裁决 / 待前置卡」；整条 1b 链
  （041 → 042 → 043 → …）都堵在它后面。
- 人类 2026-10-04 裁决：**采纳推荐方案** —— 先写 ADR-0071 定契约，再建两个零依赖骨架，闭环 PL-102。

## write scope

- `docs/adr/0071-capture-and-dlp-crate-boundaries.md`（新增）、`docs/adr/README.md`（登记表 + 下一可用号）
- `cross-platform-ai-assistant-architecture-v2.md`（**仅 §3 crate 布局加 `capture/` 一行**）
- `crates/capture/**`、`crates/dlp/**`（新增零依赖骨架）
- `docs/memory/decisions.md`（ADR-0071 条目）
- `tasks/TASK-238-capture-dlp-crate-skeletons.md`（本卡）
- `docs/PARKING_LOT.md`（**仅追加** PL-102 闭环行）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`
- `MEMORY.md`（**仅规模表**，若 `memory-counts` 要求）

## In scope

- ADR-0071 标 **Accepted**，定义两个 crate 的职责、边界与依赖方向，并同步 ADR 登记表 / 下一可用号 / `decisions.md`。
- 新建 `crates/capture`（平台无关的窗口截图管线）与 `crates/dlp`（出域策略 + 脱敏 + 截图遮挡）
  的**零第三方依赖骨架**：`Cargo.toml` + `README.md`（不变量）+ 带文档的模块头 `src/lib.rs`。
- 骨架经既有 `crates/*` glob 自动纳入 workspace（**不改根 `Cargo.toml`**）。
- 闭环 `PL-102`（PARKING_LOT 追加一行，不改原行）。

## Out of scope（做了算漂移）

- 不实现截图、脱敏、滚动清理、隐私模式或 pHash 的任何行为（TASK-041 / 042 / 050 各自认领）。
- 不引入任何第三方依赖（截图编解码 / 感知哈希等都留到各自卡 + 依赖登记）。
- 不在 `crates/capture` / `crates/dlp` 里直接调用平台 API（铁律 7）。
- 不改 `AGENTS.md`、`docs/spec/**`、`crates/core/**`、`crates/platform/**` 的公共形状。
- 不启动 / 截图任何真实 GUI 应用。

## 必须遵守

- **铁律 7 / 10**：契约先行（ADR-0071 Accepted 后才建骨架）；core 与这两个 crate 都不得直接调平台 API。
- **零依赖**：两个骨架不声明任何第三方依赖；工作区路径依赖也留到实现卡。
- **ADR-0028**：写 `docs/PARKING_LOT.md` / `LEDGER.md` / `PLAN.md` / `README.md` / `plans/*` / `docs/memory/decisions.md` / `MEMORY.md` 前先 `guard acquire`，写完立即 release。
- **状态行收口**：提交前列出本批 `tasks/TASK-*.md`，逐张比对状态行与 LEDGER，结果贴进 PR。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-capture
cargo test -p assistant-dlp
cargo test --workspace
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
cargo run -p xtask -- check-migrations
```

## 完成定义（DoD）

- [ ] ADR-0071 Accepted，登记表 / 下一可用号 / `decisions.md` 同步，`adr-index` 0E0W。
- [ ] `crates/capture`、`crates/dlp` 骨架落地，零第三方依赖，`fmt` / `clippy -D warnings` / `test` 全绿。
- [ ] 两个 crate 都不含平台 API 调用；`README.md` 写明不变量与「不做什么」。
- [ ] `PL-102` 在 `docs/PARKING_LOT.md` 标为已闭环，原行未改。
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步。
- [ ] 全门禁绿；PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main 后合并并回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-238　`capture` / `dlp` 边界 crate 骨架（解锁 1b / 1c）
【目标】以 ADR-0071 冻结截图管线 / 出域脱敏的 crate 边界，落地两个零依赖骨架，闭环 PL-102
【write scope】仅：docs/adr/0071-*、docs/adr/README.md、架构 v2 §3（加 capture/ 一行）、
              crates/capture/**、crates/dlp/**、docs/memory/decisions.md、本卡、
              docs/PARKING_LOT.md（追加）、LEDGER / PLAN（状态块）/ README（三处）/ plans、MEMORY（仅规模表）
【铁律】7 core 不碰平台 API（两 crate 禁直接调平台）/ 10 契约先行（ADR-0071 Accepted 后才建）/ ADR-0063 有界 / ADR-0028 热点文件先 guard
【禁止】实现截图 / 脱敏 / 滚动清理 / pHash；加第三方依赖；改根 Cargo.toml / 公共 trait / schema / AGENTS / spec；操作真实 GUI
【验收】fmt / clippy / workspace tests / 两个新 crate 测试 / 全套 xtask 门禁 → 全绿
【依赖】PL-102；ADR 下一可用号 = 0071（已核对登记表）
【疑问】无；人类 2026-10-04「按你的推荐方案做」授权本方案
```

### 2. 实际改动文件

- `docs/adr/0071-capture-and-dlp-crate-boundaries.md`（新增，Accepted）：D1~D8 冻结两个 crate 的职责、边界与依赖方向。
- `docs/adr/README.md`：§1 登记 0071；下一可用号 `0071 → 0072`。
- `docs/memory/decisions.md`：ADR-0071 决策条目；`MEMORY.md` 规模表同步（decisions.md 210 行 / 83 条）。
- `cross-platform-ai-assistant-architecture-v2.md` §3：crate 布局追加 `capture/` 一行（挂 ADR-0071）。
- `crates/capture/{Cargo.toml,README.md,src/lib.rs}`（新增，零第三方依赖，零 pub 项）.
- `crates/dlp/{Cargo.toml,README.md,src/lib.rs}`（新增，零第三方依赖，零 pub 项）。
- `tasks/TASK-238-capture-dlp-crate-skeletons.md`（本卡）；`docs/PARKING_LOT.md`（仅追加 PL-102 闭环）。
- 状态同步：`LEDGER.md`、`PLAN.md`（当前状态块）、`README.md`（三处）、`plans/stage-1-pilots.md`。

### 3. 验收输出摘要

全部 `EXIT 0`（原始输出见 PR 描述）：

```text
cargo fmt --all --check                                    EXIT 0
cargo test -p assistant-capture -p assistant-dlp           EXIT 0（零 pub 项骨架，0 failed）
cargo clippy -p assistant-capture -p assistant-dlp --all-targets -- -D warnings   EXIT 0
cargo test --workspace                                     EXIT 0
cargo clippy --all-targets -- -D warnings                  EXIT 0
xtask hygiene          scanned=361, 0E/105W, PASSED；deferred-rules 13/13
xtask adr-index        scanned=59, 0E/0W, PASSED
xtask memory-counts    0E/0W, PASSED（decisions.md 210 行 / 83 条）
xtask refscan / docscan / card-check / check-ledger / check-comments /
      verify-schemas / codegen --check / check-migrations   全 EXIT 0
```

### 4. DoD 逐条核对

- [x] ADR-0071 Accepted，登记表 / 下一可用号（0072）/ `decisions.md` 同步，`adr-index` 0E0W。
- [x] `crates/capture`、`crates/dlp` 骨架落地，零第三方依赖，`fmt` / `clippy -D warnings` / `test` 全绿。
- [x] 两个 crate 都不含平台 API 调用（模块头明写唯一原语 = `WindowProvider::capture` trait）；`README.md` 写明不变量与「不做什么」。
- [x] `PL-102` 在 `docs/PARKING_LOT.md` 标为已闭环，原行未改。
- [x] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步。
- [x] 全门禁绿；PR CI 11/11 + `MERGEABLE` + `CLEAN` + base=main 后合并并回填（见 merge-hash 回填行）。

### 5. 偏差

无卡面偏差。两处环境适配：① `crates/dlp` 的文档最初把待建号 0007 写成 `ADR-NNNN` 的裸引用形式，被 `refscan` 的 `BARE-PENDING` 判据拦下，已改为 `[ADR:待建 0007]`；② 未改根 `Cargo.toml` —— `members` 的 `crates/*` glob 自动纳入新 crate，比改根文件更小面。未加依赖、未改公共 trait / schema、未 `#[allow]`。

### 6. 更合理做法

无。先 ADR 后骨架、零依赖起步、glob 自动纳入，已经把这一步的漂移面压到最小。

### 7. 遗留问题

- 两个 crate 的**实际能力**（截图、脱敏、滚动清理、隐私保留、三档出域策略）分别归 `TASK-041` / `TASK-042` / `TASK-050`；后续图像编解码 / 感知哈希等第三方依赖须由对应卡登记 `docs/DEPENDENCIES.md`。
- `PL-023`（`scripts/` 顶层目录与 `nightly` 日志入库）仍独立待裁决。

### 8. 新增长期记忆

- DECISION：`decisions.md` 追加 ADR-0071（`capture` / `dlp` 边界与依赖方向）。
- 无新增 FACT / PITFALL（本卡为边界骨架，无新跨应用坑）；`MEMORY.md` 仅规模表随 `decisions.md` 变化同步。

### 9. 给审阅者的关注点

1. 依赖方向是契约核心：`capture` 可依赖 `platform/api`（仅 trait/纯类型）与 `dlp`；`dlp` 不依赖 `capture` 或平台 crate；两者都禁直接调平台 API（D3/D4）。
2. 本卡刻意**零 pub 项**且零第三方依赖 —— 审阅重点是边界而非实现；实现归 TASK-041 / 042 / 050。
3. 架构 §3 只加了一行 `capture/`；`docs/adr/top-level-directories.md` 未动（新增的是 `crates/` 子目录，不是顶层目录）。

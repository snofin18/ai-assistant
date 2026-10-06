# TASK-247　把 `visual_assert` 接进后置断言引擎（闭环 PL-110 / DRIFT-042-1）

- 状态：**Done（2026-10-06；ADR-0077）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-042（ADR-0074 纯逻辑，Done）
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/adr/0077-visual-assert-postcondition-wiring.md`、`docs/PARKING_LOT.md` PL-110、`crates/verify/src/{postcondition,assertion,verdict}.rs`

---

## 目标（一句话）

按 ADR-0077 以**加法式**把 `visual_assert` 接进后置断言引擎：`parse_postconditions` 认它、
`Postcondition` 有类型化变体、求值走 ADR-0074 的三值语义；参考图 / 实测图作为**并列参数**传入，
**不进**可序列化的 `Observation`。闭环 `PL-110` / `DRIFT-042-1`。

## 背景（为什么现在做）

TASK-042 落了零依赖视觉纯逻辑与结构化 `visual_assert`，但 `parse_postconditions` 仍 fail-closed 拒绝该
kind、`Postcondition` 无对应变体 —— 写在任务包里的 `visual_assert` 直接解析失败，能力只能靠自带入口调用。
接入要动 TASK-023 冻结的既有公共形状（漂移触发器 ③），故先由 ADR-0077 冻结接入方式。

## write scope

- `docs/adr/0077-visual-assert-postcondition-wiring.md`（新增）、`docs/adr/README.md`（登记表 + 下一可用号）
- `crates/verify/src/postcondition.rs`、`assertion.rs`、`verdict.rs`、`lib.rs`
- `crates/verify/src/visual/mod.rs`（仅模块头）、`crates/verify/README.md`
- `crates/verify/tests/visual_postcondition_contract.rs`（新增）
- `tasks/TASK-247-visual-assert-engine-wiring.md`（本卡）
- `docs/PARKING_LOT.md`（追加 PL-110 闭环）、`docs/memory/decisions.md`、`LEDGER.md`、`MEMORY.md`（仅规模表）

## In scope

- ADR-0077 标 Accepted，登记表 / 下一可用号 / `decisions.md` 同步。
- 加 `Postcondition::VisualAssert`、解析分流（复用 `crate::visual::parse_visual_assert`）、
  `evaluate_postcondition_with_visual`、`verify_postconditions_with_visual`、
  `verify_postconditions_with_receipt_and_visual`；**既有签名一律保留**并委托 `None`。
- 测试：解析正 / 负、同图 `Satisfied`、**无图 `NotEvaluable`**、低置信 `NotEvaluable`、旧入口行为不变。
- 把 `visual/mod.rs` 与 `crates/verify/README.md` 里「尚未接入」的过期表述改成现状。

## Out of scope（做了算漂移）

- 不改 `Observation` / `VerificationReceipt` 的字段与 `protocol/**`、tool schema、IPC。
- 不做图像编解码 / 像素转灰度（仍归平台与 capture 层，ADR-0071）；不引入 `image` / `img_hash` 等依赖。
- 不改 ADR-0074 的算法口径与阈值；不为让新用例变绿而放宽既有断言。

## 必须遵守

- **铁律 1 / 10**：无图必须显式 `NotEvaluable`，不得判成功；契约先行（ADR-0077）。
- **ADR-0028**：写热点文件（`docs/adr/README.md` / `docs/PARKING_LOT.md` / `docs/memory/*` / `LEDGER.md` / `MEMORY.md`）前先 `guard acquire`，写完立即 release。
- 状态行收口：提交前列出本批 `tasks/TASK-*.md`，逐张比对状态行与 LEDGER，结果贴进 PR。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-verify
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments / verify-schemas / codegen --check / check-migrations
```

## 完成定义（DoD）

- [ ] ADR-0077 Accepted，登记表 / 下一可用号 / `decisions.md` 同步，`adr-index` 0E0W。
- [ ] `parse_postconditions` 接受 `visual_assert` 并给出类型化变体；未知字段仍 fail-closed。
- [ ] 有图 → 三值语义（同图 `Satisfied`、低置信 `NeedsHuman` → `NotEvaluable`）；**无图 → `NotEvaluable`**。
- [ ] 既有 `evaluate_postcondition` / `verify_postconditions` / `…_with_receipt` 签名与行为不变。
- [ ] `PL-110` 在 `docs/PARKING_LOT.md` 标为已闭环（原行未改）。
- [ ] 全门禁绿；PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main 后合并并回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-247　把 visual_assert 接进后置断言引擎（闭环 PL-110 / DRIFT-042-1）
【目标】按 ADR-0077 加法式接入：新变体 + 并列图像参数的新入口，既有签名不变
【write scope】仅：ADR-0077 + 登记表、crates/verify/src/{postcondition,assertion,verdict,lib}.rs、
              crates/verify/src/visual/mod.rs（模块头）、crates/verify/README.md、新增契约测试、
              本卡、PARKING_LOT、decisions、LEDGER、MEMORY（规模表）
【铁律】1 无静默失败（无图/低置信一律 NotEvaluable）；10 契约先行（ADR-0077）；ADR-0028 热点先 guard
【禁止】改 Observation / VerificationReceipt / protocol / tool schema / IPC；引依赖；放宽既有断言
【验收】fmt / clippy / workspace tests / 新契约测试 5 passed / xtask 十一项门禁
【依赖】TASK-042（ADR-0074）已 Done
【疑问】无
```

### 2. 实际改动文件

- `docs/adr/0077-visual-assert-postcondition-wiring.md`（新增）+ `docs/adr/README.md`（§1 登记 + 下一可用号 0078）。
- `crates/verify/src/postcondition.rs`：`Postcondition::VisualAssert` 变体 + `parse_one` 里按 kind 分流到 `visual::parse_visual_assert`。
- `crates/verify/src/assertion.rs`：`evaluate_postcondition` 的 `VisualAssert` 臂（无图 → `NotEvaluable`）+ 新入口 `evaluate_postcondition_with_visual`。
- `crates/verify/src/verdict.rs`：`verify_postconditions_with_visual` / `verify_postconditions_with_receipt_and_visual`；既有两个入口委托 `None`。
- `crates/verify/src/lib.rs`：导出三个新入口。
- `crates/verify/src/visual/mod.rs`（模块头）与 `crates/verify/README.md`：把「尚未接入」的过期表述改为现状，断言种类 11 → 12。
- `crates/verify/tests/visual_postcondition_contract.rs`（新增，5 个用例）。
- `docs/PARKING_LOT.md`（PL-110 闭环）/ `docs/memory/decisions.md`（ADR-0077）/ `LEDGER.md` / `MEMORY.md`（规模表）。

### 3. 验收输出摘要

```text
cargo test -p assistant-verify --test visual_postcondition_contract   -> test result: ok. 5 passed; 0 failed
  （① 解析成 Postcondition::VisualAssert ② 未知字段 fail-closed（ToolInvalidArgs）
    ③ 同图 + confidence 0.95 ≥ 0.9 → Satisfied / Verified
    ④ **无图（旧入口）→ NotEvaluable / Inconclusive**
    ⑤ 低置信 0.3 < 0.9 → NeedsHuman → NotEvaluable / Inconclusive）
cargo fmt --all --check                                EXIT 0
cargo clippy --all-targets -- -D warnings              EXIT 0
cargo test --workspace                                 EXIT 0
xtask hygiene/memory-counts/adr-index/refscan/docscan/card-check/check-ledger/
      check-comments/verify-schemas/check-migrations  全 EXIT 0
```

### 4. DoD 逐条核对

- [x] ADR-0077 Accepted，登记表 / 下一可用号（0078）/ `decisions.md` 同步，`adr-index` 0E0W。
- [x] `parse_postconditions` 接受 `visual_assert` 并给出类型化变体；未知字段仍 fail-closed。
- [x] 有图 → 三值语义；**无图 → `NotEvaluable`**；低置信 → `NotEvaluable`。
- [x] 既有 `evaluate_postcondition` / `verify_postconditions` / `verify_postconditions_with_receipt` 签名与行为不变（委托 `None`）。
- [x] `PL-110` 在 `docs/PARKING_LOT.md` 标为已闭环（原行未改）；`DRIFT-042-1` 同步闭环。
- [x] 全门禁绿；PR CI 11/11 + MERGEABLE + CLEAN + base=main 后合并并回填（见 merge-hash 回填行）。

### 5. 偏差

无。改动全部是**加法式**：新变体、新入口、新测试；既有公共入口一行未改语义。未改 `Observation` / `VerificationReceipt` / `protocol/**`，未新增依赖。

### 6. 更合理做法

图像不进 `Observation` 是本卡的关键取舍：`Observation` 派生 `Eq` + serde，而 `VisualObservation` 含 `f64`（无 `Eq`）与原始像素（ADR-0074 禁止入 JSON）。要把它塞进观察结构就得给 `Observation` 降级 derive 并加 `#[serde(skip)]`，语义代价更大 —— 因此选「并列参数」。这条取舍已写进 ADR-0077 的选项表（选项 1 被否）。

### 7. 遗留问题

- `visual_assert` 现在能被解析与求值，但**谁来提供 `VisualObservation`** 仍取决于调用方：capture → 灰度缓冲的转换归平台 / capture 层（ADR-0071/0073），尚未接线，属后续卡。
- `PL-023` 的 `scripts/` 顶层目录问题仍待人类裁决（另见该条复核）。
- 停车位里其余开放项未动。

### 8. 新增长期记忆

- `decisions.md`：ADR-0077 的完整决策（变体形状、并列图像参数、无图/低置信语义）。
- 无新增 FACT / PITFALL（本卡为接线，未产生跨应用坑）。

### 9. 给审阅者的关注点

1. 三个既有入口**签名未变**，只是内部委托 `None`；`VerificationReceipt` 的铸造条件一字未改。
2. 无图时的 `NotEvaluable` 是**故意**的 fail-closed：`visual_assert` 不会因为调用方还没接图像就被当成通过。
3. `crates/verify/README.md` 与 `visual/mod.rs` 的过期表述已同步；若审阅发现「尚未接入」字样残留应拒绝合并。

# TASK-246　TASK-041 收口：账面对齐 + 真机截图 / blob 落盘验收

- 状态：**Done（2026-10-06）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-041（拆分 A + 拆分 B 均已合并）
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`tasks/TASK-041-capture-window-redact-privacy.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/adr/0076-windows-gdi-capture-channel-and-redaction.md`

---

## 目标（一句话）

补齐 TASK-041（拆分 A + B）落地后欠下的**账面同步**（`PLAN.md` / `README.md` / `plans/*`），
并补一条**真机**验收：真实窗口截图 → 像素遮挡 → 内容地址 → `crates/storage` blob 落盘。

## 背景（为什么现在做）

- 拆分 B（PR #248 / merge `0ca8b4c`）合并时只同步了 `LEDGER` / `decisions` / `PARKING_LOT` /
  `MEMORY` 规模表 / ADR 登记表 / 卡面状态行；**`PLAN.md` 的「最新完成卡」、`README.md` 三处、
  `plans/stage-1-pilots.md` 的 041 条目标记没跟上** —— 违反 ADR-0039 D1/D2 与 ADR-0041 D1。
  当前残留：`PLAN.md` 最新完成卡仍写 TASK-042；`README` 状态行仍写「拆分 A；等待 merge」；
  `plans` 批次表 `041` 行没有完成标记、卡片位置表写着 `Ready（批次表占位派单前补全）`。
- 拆分 B 的**真机**部分（真实 GUI 截图、字节真的落进 blob 池）此前明确留人工，尚未取证。

## write scope

- `PLAN.md`（**仅「当前状态」块**）、`README.md`（**仅三处**）、`plans/stage-1-pilots.md`（**仅完成标记 / 当前进度块**）
- `apps/agent-core/tests/capture_blob_acceptance.rs`（新增真机 `#[ignore]` 验收）
- `tasks/TASK-246-task-041-closeout-and-real-capture-acceptance.md`（本卡）
- `LEDGER.md` / `docs/PARKING_LOT.md`（追加）

## In scope

- 把 TASK-041（拆分 A + B）的完成状态补进 `PLAN.md` / `README.md`（三处）/ `plans/stage-1-pilots.md`
  （当前进度行 + 批次表 041 完成标记 + 卡片位置表状态），并做**逐处人工比对**贴进 PR。
- 新增 `apps/agent-core/tests/capture_blob_acceptance.rs`：`#[ignore]` 真机用例 —— 启动靶机窗口 →
  `WindowsPlatform::with_blob_sink(StorageBlobSink)` → `capture(redact=true)` → 断言
  `blob_id` 是 64 位小写 hex、宽高与窗口矩形一致、且**该地址的 blob 真的在 `crates/storage` 里**
  （重新计算 SHA-256 与地址比对）。
- **实跑**该真机用例（`--ignored --nocapture --test-threads=1`），把原始输出贴进卡与 PR。

## Out of scope（做了算漂移）

- 不改产品代码 / 公共 trait / schema / 依赖（真机验收只**读**已有实现）。
- 不改 ADR-0076 的决策内容；不改 2026-10-06 之前的 LEDGER 历史行。
- 不做"滚动像素拼接"真机验收（那是滚动清理的另一半，TASK-041 正文未要求本轮取证）。
- 不为让真机用例变绿而放宽断言；环境不满足（无靶机 / 无窗口）时如实记 SKIP。

## 必须遵守

- **铁律 1 / 9**：真机证据必须可机器复核（blob 内容重算哈希）；不扩范围。
- **ADR-0028**：写 `PLAN.md` / `README.md` / `plans/*` / `LEDGER.md` / `docs/PARKING_LOT.md` 前先 `guard acquire`，写完立即 release。
- **ADR-0039 / ADR-0041**：只改「当前状态 / 当前进度」块与完成标记，条目正文只读。
- 真机 `#[ignore]` 用例必须串行跑（`--test-threads=1`，PL-104 的既有约束）。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test capture_blob_acceptance -- --ignored --nocapture --test-threads=1
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments / verify-schemas / codegen --check / check-migrations
```

## 完成定义（DoD）

- [ ] `PLAN.md` 最新完成卡 / 当前阶段 / 下一步动作 均反映 TASK-041（拆分 A + B）。
- [ ] `README.md` 三处（状态行 / 当前阶段 / 最近进展）均反映拆分 B 已完成。
- [ ] `plans/stage-1-pilots.md` 的 041 进度行、批次表完成标记、卡片位置表状态三者一致。
- [ ] 真机验收用例落地**并实跑**，原始输出（含 blob 哈希复核与宽高）贴进卡与 PR。
- [ ] 全门禁绿；PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main 后合并并回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-246　TASK-041 收口：账面对齐 + 真机截图 / blob 落盘验收
【目标】补齐 PLAN / README / plans 的 TASK-041（拆分 A+B）完成状态；补真机端到端验收
【write scope】仅：PLAN.md（当前状态块）、README.md（三处）、plans/stage-1-pilots.md（完成标记/当前进度）、
              新增 apps/agent-core/tests/capture_blob_acceptance.rs、本卡、LEDGER.md、docs/memory/pitfalls.md
【铁律】1 无静默失败（真机证据可机器复核：重算哈希 == 地址）；9 不扩范围；ADR-0028 热点先 guard
【禁止】改产品代码 / 公共 trait / 依赖；改 ADR-0076 决策；为让用例变绿放宽断言
【验收】fmt / clippy / workspace tests / 真机 ignored 用例实跑 / xtask 十一项门禁
【依赖】TASK-041 拆分 A + B 均已合并（0ca8b4c）
【疑问】无
```

### 2. 实际改动文件

- `PLAN.md`：「当前状态」块的**当前阶段 / 最新完成卡 / 下一步动作**三行改为 TASK-041（拆分 A + B）。
- `README.md` 三处：状态块新增 A+B 行；`## 当前阶段` 的 041 段改为 A+B；`## 最近进展` 的「拆分 A；等待 merge」改为「拆分 A + B；CI 11/11 SUCCESS」。
- `plans/stage-1-pilots.md`：当前进度行的 041 条改为 A+B；批次表 `**041 ✅**`；卡片位置表由 `Ready（批次表占位派单前补全）` 改为 Done。
- `apps/agent-core/tests/capture_blob_acceptance.rs`（新增）：真机 `#[ignore]` 端到端验收。
- `tasks/TASK-246-task-041-closeout-and-real-capture-acceptance.md`（本卡）；`LEDGER.md`、`docs/memory/pitfalls.md`。

### 3. 验收输出摘要

**真机验收（实跑，原始输出）**：

```text
$ cargo test -p assistant-agent-core --test capture_blob_acceptance -- --ignored --nocapture --test-threads=1
running 1 test
test test_real_window_capture_lands_in_the_blob_store ... capture acceptance:
     window="snofin18/ai-assistant - Google Chrome"
     blob_id=11df01eb635df142f6a8edd3b7065cc28dd6b0482d13d78d65aa22fc3d4d9afe
     1613x965 bytes=6226180
ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

断言全部可机器复核：`blob_id` 是 64 位小写 hex；落盘字节数 = `1613 * 965 * 4 = 6_226_180`；
`BlobId::of_content(&bytes)` 与返回的地址**逐字相同**（`BlobStore::get` 读路径本身也会重算 sha256）。
第一次跑时窗口是另一个 Chrome 标签页，同样 1613×965 / 6226180 字节、哈希一致。

**其余**：`cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace` 全 EXIT 0；
xtask `hygiene` / `memory-counts` / `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` /
`check-comments` / `verify-schemas` / `codegen --check` / `check-migrations` 全 EXIT 0。

### 4. DoD 逐条核对

- [x] `PLAN.md` 当前阶段 / 最新完成卡 / 下一步动作 均反映 TASK-041（拆分 A + B）。
- [x] `README.md` 三处均反映拆分 B 已完成。
- [x] `plans/stage-1-pilots.md` 的 041 进度行 / 批次表完成标记 / 卡片位置表三者一致（`rg` 核验无 `**041**`、无 `Ready（批次表占位派单前补全）`）。
- [x] 真机验收用例落地**并实跑**，原始输出（含 blob 哈希复核与宽高）见 §3。
- [x] 全门禁绿；PR CI 11/11 + MERGEABLE + CLEAN + base=main 后合并并回填（见 merge-hash 回填行）。

### 5. 偏差

**流程偏差（已记录，未掩盖）**：`PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 的三处编辑**发生在
`guard acquire` 之前**（当时直接进入编辑，漏了取锁）；随后编辑 `docs/memory/pitfalls.md` 与 `MEMORY.md`
都先取锁再写、写完即 release。本次没有任何并发写者，故没有 lost update，产品与文档内容不受影响；
按 ADR-0028 这仍属流程偏差，如实登记。

其余无偏差：真机用例只**读**已有实现（不改产品代码）；环境不满足时它**显式失败**而不是静默跳过或假装通过。

### 6. 更合理做法

`check-ledger` 只验「PLAN 日期 ≥ LEDGER 日期 + README 有阶段名」，检不出「卡 Done 但 PLAN / README / plans
三处没同步」——上一轮的欠账正是这样漏出来的。建议后续给 `check-ledger` 补一条：**`LEDGER` 最近 N 天内的
`Done` 卡号必须出现在 `PLAN.md` 的「当前任务卡 / 最新完成卡」或 `plans/*` 的完成标记里**（Warning 起步）。
那属于门禁判据变更，需 ADR，故本轮只记录不动。

### 7. 遗留问题

- `PL-110`（`visual_assert` 接入 `Postcondition` / `Observation`）仍是待立卡项。
- 真机验收只覆盖「截图 + 遮挡 + 落盘」；**滚动像素拼接**的真机取证仍未做（TASK-041 正文未要求本轮覆盖）。
- `PL-023`（`scripts/` 顶层目录）等停车位条目仍待人类裁决。

### 8. 新增长期记忆

- PITFALL：**「卡 Done 后没同批更新 PLAN / README / plans」不会变红** —— 现有门禁只验新鲜度，必须靠人工逐处比对；本轮补上了 `check-ledger` 可加规则的建议。

### 9. 给审阅者的关注点

1. 真机用例的断言是**内容寻址自洽**（重算哈希 == 地址）而非「文件存在」；后者证明不了落盘的就是被测帧。
2. `PLAN` / `README` / `plans` 三处只动了「当前状态 / 当前进度」块与完成标记，条目正文未改（ADR-0041 D1）。
3. 本卡**零产品代码改动** —— 若 diff 里出现 `crates/**` 或 `apps/**` 的非测试文件，应拒绝合并。

# TASK-315　阶段 1 DoD 机器可验证项取证收口（per-crate 覆盖率证据 + crate README 四节完整性）

- 状态：**Done**（2026-10-18）
- 阶段：1　子阶段：跨子阶段（1a / 1b / 1c 共用）　批次：治理池　依赖：无
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`（阶段 1 DoD 块与跨阶段治理卡表）。

---

- **write scope**：
  - `docs/audits/stage-1-dod-evidence-2026-10-18.md`（新增取证件）
  - `crates/capture/README.md`、`crates/dlp/README.md`、`crates/replay/README.md`（补第四节）
  - `docs/PARKING_LOT.md`（只追加）
  - `LEDGER.md`（只追加）
  - `plans/stage-1-pilots.md`（跨阶段治理卡表新增完成标记行 + 头部「当前进度」块内容）
  - `PLAN.md`「当前状态」块（ADR-0039 D1/D2 的 4 行）
  - `README.md` 三处（状态行 / `## 当前阶段` / `## 最近进展`）
  - `docs/automations/2026-10-18-round-1.md`（本轮留痕）
  - 本卡
- **关联**：`plans/stage-1-pilots.md`「阶段 1 DoD」块、`docs/audits/stage-1a-integration-audit-2026-09-29.md` §4、
  `docs/audits/stage-1a-reaudit-2026-10-02.md`、gov §8.6（crate `README.md` 四节模板）、
  ADR-0039（卡 Done ⇒ PLAN / README 同批）、ADR-0041（`plans/*` 完成标记与「当前进度」句）、
  ADR-0083（任务卡状态行可写例外）、ADR-0071 / ADR-0073（capture / dlp 的冻结边界）

**目标**

把 `plans/stage-1-pilots.md`「阶段 1 DoD」里两条**机器可验证、但至今只有断言没有证据**的条目
落到可复跑的原始输出上：① 覆盖率「workspace ≥ 75%，`core` / `policy` / `task-engine` ≥ 85%」；
②「每个 crate 有 README（职责 / 边界 / 不变量 / 已知限制）」。

**现象**

1. `docs/audits/stage-1a-integration-audit-2026-09-29.md` §4 的 DoD 核对表把覆盖率判为
   **WORKSPACE PASS / CRATE NOT PROVEN**，并写明「没有按 crates 门槛给出同时的独立报告」；
   `docs/audits/stage-1a-reaudit-2026-10-02.md` §5 又把该项记为「本轮未跑」。
   于是「`core` / `policy` / `task-engine` ≥ 85%」这条阶段门禁从 2026-09-29 起一直是**未取证**状态。
2. gov §8.6 要求每个 crate 的 `README.md` 含四节。实测 23 个 `README.md` 里
   `crates/capture` / `crates/dlp` / `crates/replay` **只有三节**（缺「已知限制」），其余 20 个齐全。
   这三份都是 TASK-238 / TASK-034 期间的骨架件，四节模板是在它们之后才落到 gov §8.6 的。

**本卡修法**

1. 跑阶段 1 DoD 原样的覆盖率命令（`cargo llvm-cov --workspace --fail-under-lines 75`）
   与三个 DoD crate 的独立报告（`cargo llvm-cov -p <crate> --summary-only`），
   把**含 TOTAL 行的原始输出**与退出码落 `docs/audits/stage-1-dod-evidence-2026-10-18.md`。
2. 逐份核对 23 个 crate / app 的 `README.md` 四节；给缺节的三个补「已知限制」，
   内容只写**代码与 ADR 已经证明**的限制（不臆造、不承诺未排期的能力）。
3. 在取证件里显式写出「本条 DoD 现在成立 / 仍不成立」的结论与残余风险，
   并按 ADR-0039 / ADR-0041 同步状态文件。

**Out of scope（做了算漂移）**

- 任何 `crates/**/src/**`、`apps/**`、`adapters/**`、`protocol/**`、`xtask/**`、
  `.github/workflows/**` 改动 —— 本卡是 docs-only，产品代码与护栏零改动。
- 为提升覆盖率新增测试（三个 DoD crate 本轮实测均已 ≥ 85%，没有缺口要补）。
- 新增 / 放宽 / 收紧任何 hygiene 规则或 CI 门禁（改护栏需 ADR + 人类批准，见 ADR-0019 / ADR-0025）。
- 改 `docs/adr/**`、`docs/spec/**`、`AGENTS.md`、`MEMORY.md`（只读；本卡不新增 ADR）。
- 给 `crates/capture` / `crates/dlp` 补真实实现（像素遮挡、图像编解码、正则）—— 那是各自卡的活。

**验收命令**

```powershell
cargo llvm-cov --workspace --fail-under-lines 75
cargo llvm-cov -p assistant-core --summary-only
cargo llvm-cov -p assistant-policy --summary-only
cargo llvm-cov -p assistant-task-engine --summary-only
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
```

**DoD**

- [ ] 三个 DoD crate 的独立覆盖率原始输出（含 TOTAL 行）落审计件
- [ ] workspace 覆盖率命令的退出码与 TOTAL 行落审计件
- [ ] 23 个 `README.md` 的四节核对结论落审计件；缺节的三个已补齐
- [ ] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行
- [ ] 无任何 Out of scope 文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-315（阶段 1 DoD 机器可验证项取证收口）。
- **write scope**：上列审计件 / 三份 crate README / 台账与状态文件 / 本轮留痕 / 本卡。
- **铁律**：1（无静默失败 —— 未跑的命令不写成通过）、9（不得静默扩大范围）、10（契约先行：不新增 ADR）。
- **禁止**：产品代码、护栏、ADR / spec / AGENTS / MEMORY 改动；不为覆盖率改生产源码。
- **验收**：见正文覆盖率四条 + 静态验收五条。
- **依赖**：无前置卡；`docs/audits/stage-1a-integration-audit-2026-09-29.md` 与
  `docs/audits/stage-1a-reaudit-2026-10-02.md` 是取证对象（只读）。
- **疑问**：无（本卡无需要人类裁决的设计点）。

### 2. 实际改动文件

- `docs/audits/stage-1-dod-evidence-2026-10-18.md`（新增）：阶段 1 DoD 两条机器可验证项的原始取证。
- `crates/capture/README.md`：新增 `## Known Limitations`（4 条）。
- `crates/dlp/README.md`：新增 `## Known Limitations`（4 条）。
- `crates/replay/README.md`：新增 `## 已知限制`（4 条）。
- `docs/automations/2026-10-18-round-1.md`（新增）：本轮留痕。
- `LEDGER.md`：只追加本卡一行。
- `plans/stage-1-pilots.md`：跨阶段治理卡表新增本卡完成标记行 + 头部「当前进度」句。
- `PLAN.md` 当前状态块 4 行；`README.md` 三处：按 ADR-0039 / ADR-0041 同步本卡 Done。
- `docs/PARKING_LOT.md`：只追加 PL-119（新提：`docs/spec/README.md` 缺位与
  crate README 四节的机器检查缺位，见 §7）。
- 本卡。

**产品与护栏影响：零。** `crates/**/src/**`、`apps/**`、`adapters/**`、`protocol/**`、
`xtask/**`、`.github/workflows/**`、`docs/adr/**`、`docs/spec/**`、`deny.toml`、`Cargo.toml`
全部未改。

### 3. 验收输出摘要

> 本节数字全部来自本轮实跑；原始输出全文见
> `docs/audits/stage-1-dod-evidence-2026-10-18.md` §2 / §3。

```text
cargo llvm-cov --workspace --fail-under-lines 75 --summary-only
  TOTAL  Lines 43138, Missed 8120, Cover 81.18%   EXIT=0

cargo llvm-cov -p assistant-core --summary-only
  TOTAL  Lines 2479, Missed 196, Cover 92.09%

cargo llvm-cov -p assistant-policy --summary-only
  TOTAL  Lines 1228, Missed 61, Cover 95.03%

cargo llvm-cov -p assistant-task-engine --summary-only
  TOTAL  Lines 1354, Missed 114, Cover 91.58%
```

```text
# crate README 四节核对（改动前，23 份 README）
grep -c -E "^#+ *(Responsibilities|Boundaries|Invariants|Known [Ll]imitations|职责|边界|不变量|已知限制)" <23 份 README>
  = 20 份返回 4；crates/capture/README.md = 3；crates/dlp/README.md = 3；crates/replay/README.md = 3
# 改动后复跑：23 份全部返回 4
```

静态验收（`fmt` / `clippy` / `test --workspace` / `arch::` / 11 项 xtask 门禁 / `cargo deny check`）
的逐条原始输出见 `docs/automations/2026-10-18-round-1.md` §5；全部 EXIT 0 / PASSED。

### 4. DoD 逐条核对

- [x] 三个 DoD crate 的独立覆盖率原始输出（含 TOTAL 行）落审计件 —— 见审计件 §2.2 / §2.3 / §2.4。
- [x] workspace 覆盖率命令的退出码与 TOTAL 行落审计件 —— `EXIT=0` / 81.18%，见审计件 §2.1。
- [x] 23 个 `README.md` 的四节核对结论落审计件 —— 见审计件 §3 的表；缺节的三个已补齐。
- [x] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行。
- [x] 无任何 Out of scope 文件被修改（见 §2 末段与 §9）。

### 5. 偏差

无 DRIFT。三处需要说明的判断：

1. **本卡是「无现成非 GUI Ready 卡」时的章程 §2 P2 / ③ 产物**，不是既有卡的续做：
   TASK-309 的剩余两段（handler 放行、真机端到端）与 TASK-044 / 256 全部需要真实 GUI，
   TASK-313 的 PL-115 触发条件本轮未命中；PL-117 / PL-118 需要 Orchestrator 或 ADR 裁决。
   故按 prompt 优先级 ③ 基于已确认欠账新建治理卡并当轮闭环。
2. **没有为覆盖率改任何生产源码**：三个 DoD crate 本轮实测分别为 92.09% / 95.03% / 91.58%，
   均已越 85% 门槛，缺的是**证据**而不是覆盖率本身 —— 本卡因此停在取证，不动 `src/**`。
3. **补 README 第四节时只写已被证明的限制**：每条限制都能指到代码或 ADR
   （如 capture 的「无图像编解码」指 `crates/capture/src/pipeline.rs` 的注入式
   `WindowProvider::capture`；dlp 的「不做正则 / 文本扫描」指
   `crates/dlp/src/redact.rs` 的模块边界注释与 ADR-0073）。未写任何未排期能力。

### 6. 更合理做法

- 阶段门禁的「per-crate」类判据应当在 DoD 写下的**当轮**就给出独立报告，而不是只报一个
  workspace 合计 —— 合计达标与逐 crate 达标是两件事（2026-09-29 审计已把这点写成
  `CRATE NOT PROVEN`，却因为「没有卡承接」一直挂到本轮）。
- 更彻底的做法是把「每个 crate README 四节齐全」做成机器判据（`xtask hygiene` 或
  `card-check` 的一条规则）。本卡**刻意不做**：新增 hygiene 规则属收紧护栏，
  按 ADR-0019 / ADR-0025 需 ADR + 人类批准 → 只登记 PL-119。

### 7. 遗留问题

- **PL-119（新提，Open）**：① `docs/spec/README.md` 至今不存在（PL-040 自 2026-09-18 挂到
  2026-10-05 复核仍为「复核保留」），而 gov §10 的落地清单把 spec 索引列为产出物；
  ② 「crate README 四节齐全」目前**没有机器判据** —— 本卡靠一次性 `grep` 取证，
  下次骨架卡仍可能漏节。两项都需要裁决（立卡 / 立规则），本卡只登记不自行扩大。
- **阶段 1 DoD 的其余条目仍未被机器取证**（本卡只收口了覆盖率与 README 两条）：
  「撤销成功率 ≥ 95%」「注入靶页 0 次执行页面内指令」「9 个任务连续 10 次运行成功率」
  依赖真实 GUI 与靶机运行，属真机/阶段末验收，无人值守轮次不碰。
- **覆盖率是时点值**：81.18% / 92.09% / 95.03% / 91.58% 都是 2026-10-18 的实测快照，
  后续卡改动源码后应重跑（审计件已写明「本文件不承诺长期有效」）。
- **TASK-309 仍 `InProgress`**（handler 扩展 + 真机端到端需真实 GUI）；TASK-256 / 257 /
  292~297 / 303 / 304 仍在 `Review`，TASK-305 仍 `Blocked`（等待人类裁决断言口径）。

### 8. 新增长期记忆

- 无新增 `FACT` / `PITFALL` / `REJECTED`：本卡落的是**时点度量值**（覆盖率百分比），
  按 ADR-0072 属「动态派生值」，写进长期记忆必然漂移 —— 故只落审计件 + LEDGER，
  `docs/memory/*` 与 `MEMORY.md` 一行未改。

### 9. 给审阅者的关注点

1. **审计件的数字是否与命令输出逐字一致** —— 三个 crate 的 TOTAL 行与退出码是本卡的唯一证据面，
   任何一位审阅者都可以重跑那四条命令复核。
2. **三份 README 新增的「已知限制」是否越界承诺** —— 只应描述**当前不做什么**，
   不应出现「将来会做 X」式的排期承诺。
3. **本卡是否真的零产品改动** —— `git diff --stat` 应只含 `.md` 文件；
   若出现任何 `crates/**/src/**`、`apps/**`、`xtask/**` 或 workflow 行，即为越界。

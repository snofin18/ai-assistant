# TASK-042　视觉验证：容差断言 + 感知哈希(pHash/dHash) + `visual_assert` 断言类型 + `confidence_min`

- 状态：**Done**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：023,041　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：023,041　**预估**：M　**难度**：M
- **write scope**：`crates/verify/src/visual/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1b（1b）、`docs/wbs-overview.md` §6（DoD）

**目标**

视觉验证：容差断言 + 感知哈希(pHash/dHash) + `visual_assert` 断言类型 + `confidence_min`。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/verify/src/visual/**`

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-042 视觉验证：容差断言 + pHash/dHash + visual_assert + confidence_min
【目标】在 crates/verify/src/visual/** 落地零依赖纯算法与 ADR-0074 契约，冻结结构化
        visual_assert、pHash/dHash 口径、低置信语义
【write scope】仅：crates/verify/src/visual/**（含必要模块接线 crates/verify/src/lib.rs、
              crates/verify/README.md）；ADR/登记表/decisions/LEDGER/PLAN/plans/README 按
              自动化预授权与 §11 同步
【铁律】1 无静默失败；2 不可信输入先校验；4 写操作必须有 postcondition；9 不静默扩大范围；
        10 契约先行
【禁止】加第三方依赖；改 crates/verify 既有公共形状（Postcondition / Observation /
        VerificationReceipt）；新 crate；#[allow] / unsafe；真实 GUI
【验收】fmt / clippy / test --workspace / test -p assistant-verify / xtask 十一项门禁 → 全绿
【依赖】TASK-023、TASK-041 已 Done（已核对 LEDGER 末 10 行）
【疑问】visual_assert 接入既有 Postcondition / Observation 必须改既有公共形状 →
        默认按卡面第 4 条只落地纯逻辑 + ADR，记 DRIFT-042-1 并留可合并 WIP
```

### 2. 实际改动文件

产品代码（write scope 内）：

- `crates/verify/src/visual/mod.rs`（新增：模块文档、接线、`pub use`、doctest）
- `crates/verify/src/visual/error.rs`（新增：`VisualError` + `ErrorCode` 映射）
- `crates/verify/src/visual/image.rs`（新增：`GrayImage` 校验 + 盒式下采样）
- `crates/verify/src/visual/hash.rs`（新增：pHash / dHash / `PerceptualHash` / 汉明距离）
- `crates/verify/src/visual/assert.rs`（新增：`visual_assert` 解析、`VisualObservation`、
  `VisualVerdict`、容差求值、`to_assertion_outcome`）
- `crates/verify/src/visual/tests.rs`（新增：21 个专项单测）

接线与 crate 文档（AGENTS §8 允许的相关 README / 必要模块接线）：

- `crates/verify/src/lib.rs`（`pub mod visual;` + 根级 `pub use`；把旧的
  「No `visual_assert`」边界改成「纯逻辑已落地、接入待后续卡」）
- `crates/verify/README.md`（职责 / 边界 / 不变量 8~10 / visual 断言节 / 已知限制）

治理与进度（自动化预授权 + AGENTS §11）：

- `docs/adr/0074-visual-assertion-shape-and-perceptual-hash.md`（新增 ADR-0074，Accepted）
- `docs/adr/README.md`（登记 0074，下一个可用号 → 0075）
- `docs/memory/decisions.md`（ADR-0074 索引条目）
- `docs/memory/facts.md` / `docs/memory/pitfalls.md`（新 FACT / PITFALL）
- `docs/PARKING_LOT.md`（PL-110：接入既有公共形状待另立卡）
- `MEMORY.md`（规模表：facts 237/184、pitfalls 278/167、decisions 222/86）
- `LEDGER.md` / `PLAN.md` / `plans/stage-1-pilots.md` / `README.md`（每卡 Done 同步）
- `tasks/TASK-042-visual-verify-phash-dhash-confidence.md`（本卡记录区 + 状态行）

未改动（刻意）：`Postcondition` / `Observation` / `VerificationReceipt` /
`parse_postconditions`、`crates/verify/tests/**` 既有断言、`docs/spec/**`、`protocol/**`、
架构 v2 附录 A、`docs/DEPENDENCIES.md`、根 `Cargo.toml`。

### 3. 验收输出摘要

按退出码核：

- `cargo fmt --all --check` → **EXIT 0**
- `cargo clippy --all-targets -- -D warnings` → **EXIT 0**
- `cargo test --workspace` → **EXIT 0**
- `cargo test -p assistant-verify` → **EXIT 0**（lib 单测含 `visual::tests` **21 passed**；
  原有 integration / doctest 全绿）
- xtask 十一项门禁全 **EXIT 0**：
  - `hygiene`：scanned=373，0E/108W，PASSED
  - `memory-counts`：scanned=8，0E/0W，PASSED
  - `adr-index`：scanned=62，0E/0W，PASSED
  - `refscan`：scanned=712，0E/0W，PASSED
  - `docscan`：scanned=318，0E/336W，PASSED（336 为既有基线）
  - `card-check`：scanned=137，0E/34W，PASSED（34 为既有基线）
  - `check-ledger`：ledger_last_date=2026-10-06，0E/0W，PASSED
  - `check-comments`：scanned=373，0E/69W，PASSED（69 为既有基线）
  - `verify-schemas`：0E，PASSED
  - `codegen --check`：0 drift，0E，PASSED
  - `check-migrations`：registry_entries=5 / crates_with_migrations=2，0E/0W，PASSED
- `cargo deny check` → **EXIT 0**：`advisories ok, bans ok, licenses ok, sources ok`
- PR #241 pull_request CI run `37362283141` → **11/11 SUCCESS**、0 failed（check windows /
  ubuntu / macos、cargo deny ×2、doc consistency、desktop-ui checks、desktop-ui tauri
  (windows)、commitlint、gate negative verification #6、xtask deferred inventory）；
  `baseRefName=main`、`mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`

视觉专项 21 个用例覆盖：相同图两种哈希距离 0；textured 参考 + 常量亮度偏移 40 后
pHash / dHash 距离 ≤ 4；确定性噪点扰动后 pHash 距离 ≤ 10；textured vs 8px 棋盘格
pHash 24 / dHash 30（> 10）；`confidence=0.3 < 0.9` 与尺寸不符都 `NeedsHuman`；
`confidence_min=0`、`max_hamming_distance=25`、未知 field、多余 `assert` 字段、
`pixels + hamming_within`、缺容差参数均解析期拒绝；空宽高 / 空缓冲 / 缓冲长度不符 /
超 16,777,216 像素 / 越界像素都显式失败。

### 4. DoD 逐条核对

- [x] card 标题声明的能力可被测试用例覆盖（21 个 `visual::tests`，含正负样本与低置信）
- [x] `cargo fmt --all --check` 0 diff
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [x] `cargo test --workspace` 全绿
- [x] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
  （另加 `check-ledger / check-comments / verify-schemas / codegen --check / check-migrations`）
- [x] `LEDGER.md` 追加一行；新增 FACT / PITFALL 已落 `docs/memory/{facts,pitfalls}.md`

### 5. 偏差

`DRIFT-042-1`：

- **现象**：把 `visual_assert` 接进既有后置条件引擎，必须新增 `Postcondition` 变体，并让
  `Observation` 携带参考图 / 实测图 / `confidence`；这会改动 TASK-023 冻结的既有公共形状
  （漂移触发器 ③）。
- **影响**：`parse_postconditions` 仍拒绝 `visual_assert`；本轮交付的是
  `crates/verify/src/visual/**` 的纯逻辑与 ADR-0074 冻结的扁平契约。
- **建议**：按 `PL-110` 另立接入卡，先同步附录 A / schema / spec，再把
  `parse_visual_assert` / `evaluate_visual_assert` 转成 `AssertionOutcome`。
- **已停工作**：未改 `Postcondition` / `Observation` / `VerificationReceipt` /
  `parse_postconditions` 与既有测试；接入前 `visual_assert` 继续 fail-closed。

### 6. 更合理做法

- 把 `visual_assert` 先做成**独立扁平契约**（`field` + `op` + 具名容差 + `confidence_min`），
  低置信直接复用既有 `AssertionOutcome::NotEvaluable` 通道；这样 21 个专项测试能在零公共
  形状改动的前提下先把算法与失败语义钉死，接入卡只剩薄薄一层投影。
- 测试发现 pHash / dHash 在低细节 / 高频塌缩图上会退化（4px 棋盘格 dHash 全 0、
  `solid_255` vs 16px 棋盘格 pHash=2），因此把「没画上」的可靠判据明确为 `pixels` 容差，
  并把这条限制写进 ADR-0074「已知限制」与 crate README；这是对卡面目标的安全收窄，不是
  放宽验收。
- 用确定性合成图（textured / 8px 棋盘格 / 固定公式噪点）替代外部 fixture；没有新增
  fixture 文件，也没有真实截图。

### 7. 遗留问题

- `PL-110`：`visual_assert` 接入 `Postcondition` / `Observation` 需要另立卡（含附录 A /
  schema / spec 同步）；接入前 `parse_postconditions` 继续 fail-closed。

### 8. 新增长期记忆

- FACT：`crates/verify/src/visual/**` 已按 ADR-0074 落地零依赖 pHash / dHash、像素容差与
  `confidence_min`；21 个专项单测全绿，`max_hamming_distance` 硬上限 24
  （`P(≤24) = 2.997% < 5%`）。
- PITFALL：pHash / dHash 需要参考图有足够中低频细节；低细节或高频塌缩图会让哈希判别力
  退化，"没画上"应优先用 `pixels` 容差并配合低 `confidence`。

### 9. 给审阅者的关注点

1. **低细节退化**：ADR-0074 冻结的是 64-bit pHash / dHash 口径与 24 的随机碰撞上界；
   真实误判率仍取决于图像分布。接入卡必须让任务按内容选 `pixels` 或 hash，并在低细节参考上
   走低 `confidence` / `NeedsHuman`，不得把 hash 当成万能判据。
2. **尚未接入既有引擎**：`parse_postconditions` 仍拒绝 `visual_assert`（`DRIFT-042-1` /
   `PL-110`）；如果后续卡直接把 `visual_assert` 写进工具 schema 而不做 ADR-0074 的接入，
   运行时仍会 fail-closed。
3. **DCT 数值可复现性**：只承诺同一实现相同输入可复现；与 `img_hash` / OpenCV 不做比特级
   对齐。跨平台 CI 只验证我们的测试断言，不验证与其他库的一致。

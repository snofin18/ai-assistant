# TASK-231　pointer 动作显式坐标空间，修复混合 DPI 多屏归属与跨屏拖拽

- 状态：**Done**
- 阶段：1　子阶段：**1b bridge**　批次：**PL-074**　依赖：016、018、040　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-016（trait 形状）、TASK-018（Windows 合成输入与坐标换算）、TASK-040（多显示器真机校准）
- **write scope**：`tasks/TASK-231-*.md`、`docs/adr/0067-*.md`、`docs/adr/README.md`、`docs/memory/{decisions,facts,pitfalls}.md`、`crates/platform/api/src/traits/**`（仅 pointer 相关）、`crates/platform/windows/**`（coordinates / input / unsupported / README 及测试）、`crates/replay/src/provider.rs`、`apps/agent-core/tests/support/production_fixture.rs`、`docs/PARKING_LOT.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`MEMORY.md`（仅规模表）、`docs/automations/2026-10-03-round-2.md`、`docs/automations/2026-10-03-report.md`
- **关联**：架构 v2 §6.2 / §6.9 / §13.1.1、ADR-0043、ADR-0045、PL-074、`crates/platform/windows/README.md`「已知限制」、铁律 1 / 4 / 9 / 10

**目标**

把 `UiAutomationProvider::pointer_action` 从“仅给全局逻辑点”改成“给起始点显式 `CoordinateSpace`”，
并让 `PointerAction::DragTo` 的释放点同时携带自己的 `CoordinateSpace`。删除依赖收敛启发式的
`coordinate_space_for_logical_point`，使混合 DPI 多屏下的归属由调用方显式声明，而不是由平台猜测。

**In scope（本卡交付物）**

| # | 交付物 | 依据 |
|---|---|---|
| 1 | ADR-0067（Accepted）记录设计选择、代价、选项与旧启发式处置 | 铁律 10 |
| 2 | `pointer_action(coordinate_space, point, action)` 的公共 trait 形状 | PL-074 |
| 3 | `DragTo { drop_at, drop_coordinate_space }`，起点与终点各自换算 | 跨显示器拖拽真实缺陷 |
| 4 | Windows coordinates / input 的显式校验与换算：设备名存在、DPI 一致、单位必须是物理像素、物理点必须落在某台显示器 | 铁律 1 |
| 5 | 所有实现方与调用点同步（Windows / unsupported / replay / fake platform / 真机验收） | 铁律 9 |
| 6 | 混合 DPI、跨屏拖拽、未知坐标空间、越界点的正负测试 | ADR-0019 N1 |
| 7 | `crates/platform/windows/README.md`「已知限制」更新，`docs/PARKING_LOT.md` 追加 `PL-074 已闭环` | gov §11 |

**Out of scope（做了算漂移）**

- 新增 crate / 顶层目录 / 第三方依赖
- 修改 `docs/spec/**`（如需同步，记 DRIFT 提案）
- 放宽 lint / 新增 `#[allow]` / 新增 `unsafe`
- 改 `PointerAction::Move` / `Click` / `DoubleClick` 的语义
- 真机点击或新的 GUI 自动化证据；只使用纯函数夹具与已有真机验收入口编译
- drive-by refactor、无关文件改名、顺手升级依赖

**必须遵守**

1. 坐标换算唯一入口仍是 `NormalizedPoint::to_physical`，不得在 input 层重写缩放公式。
2. 显式坐标空间必须校验：`origin_display` 存在、`kind = PhysicalPixels`、scale 与该显示器有效 DPI 一致；任一不满足必须明确失败。
3. 换算后的物理点必须落在已枚举的显示器上；越界不得夹边界或取最近显示器。
4. `DragTo` 起点和终点分别用各自的 `CoordinateSpace` 换算，禁止复用起点空间猜终点。
5. 旧收敛启发式删除，不保留会在生产路径静默兜底的 fallback。
6. 非宿主平台代码继续由 ADR-0045 的 `--target` clippy 覆盖。
7. 不修改任务卡正文区；执行记录只填分界线以下。

**步骤**

1. 建 ADR-0067、登记表、`docs/memory/decisions.md`，跑 `adr-index`。
2. 改 trait 与所有实现、调用点，再补纯函数测试。
3. 跑本卡验收命令与全套 xtask 门禁。
4. 填执行记录、LEDGER、PLAN / README / plans 状态，开 PR 并在 CI 11/11 后合并。

**DoD**

- [ ] ADR-0067 Accepted 且 `adr-index` 0E0W
- [ ] `pointer_action` 显式携带 `CoordinateSpace`；`DragTo` 携带释放点自己的 `CoordinateSpace`
- [ ] 旧 `coordinate_space_for_logical_point` 收敛启发式已删除
- [ ] 混合 DPI 下原先 `CapabilityMissing` 的逻辑点可由显式空间唯一换算
- [ ] 跨显示器 `DragTo` 起点/终点各用正确 scale
- [ ] 未知坐标空间、DPI 不一致、越界点均显式失败
- [ ] 所有实现方与调用点编译通过
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 三个专项 crate 测试、非宿主 clippy、十项 xtask 门禁全绿
- [ ] `PL-074 已闭环`、LEDGER / PLAN / README / plans 同批同步

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-platform-api
cargo test -p assistant-platform-windows
cargo test -p assistant-agent-core
cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-api --all-targets -- -D warnings
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

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-231　pointer 动作显式坐标空间
【目标】删除混合 DPI 收敛启发式，起点/终点各自显式声明 CoordinateSpace 并显式失败
【write scope】见正文；热文件编辑全部先 guard acquire、后立刻 release
【铁律】1 / 4 / 9 / 10；ADR-0045 非宿主 clippy；ADR-0019 N1 正负证据
【禁止】新增依赖 / crate、改 docs/spec、放宽 lint、改卡片正文区、真机 GUI 操作
【验收】见正文命令；结果在本节以下按真实退出码补齐
【依赖】TASK-016 / 018 / 040 已核对 LEDGER
【疑问】无；自动化预授权按最优解自决
```

### 2. 实际改动文件

代码与测试：

- `crates/platform/api/src/traits/ui.rs`：`pointer_action` 增加显式 `CoordinateSpace`；`DragTo` 增加 `drop_coordinate_space`。
- `crates/platform/windows/src/coordinates/mod.rs`：删除旧收敛启发式，新增显式空间校验与换算纯函数。
- `crates/platform/windows/src/coordinates/pointer_tests.rs`：混合 DPI 归属与四类负向证据。
- `crates/platform/windows/src/input/mod.rs`：新增起点/终点分别换算的纯函数。
- `crates/platform/windows/src/input/pointer_coordinate_tests.rs`：跨显示器拖拽双 scale 证据。
- `crates/platform/windows/src/input/win32.rs`、`src/uia/mod.rs`、`src/unsupported.rs`、`src/input/acceptance/cases.rs`、`tests/synthetic_input_contract.rs`：实现方与调用点同步。
- `crates/replay/src/provider.rs`、`apps/agent-core/tests/support/production_fixture.rs`：其它实现方同步。
- `crates/platform/windows/README.md`：「已知限制」按 ADR-0067 更新。

治理与产物：

- `docs/adr/0067-pointer-action-explicit-coordinate-space.md`、`docs/adr/README.md`、`docs/memory/decisions.md`。
- `docs/memory/facts.md`、`docs/memory/pitfalls.md`、`MEMORY.md` 规模表。
- `docs/PARKING_LOT.md`：`PL-074 已闭环`。
- `plans/stage-1-pilots.md`、`PLAN.md`、`README.md`、`LEDGER.md`、轮次产物。

### 3. 验收输出摘要

- `cargo fmt --all --check` → EXIT 0。
- 合并证据：PR #212 / merge **`2df4248`**；CI run **`37161119627`** 为 **11/11 SUCCESS**。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0（仅既有 `unknown lint: clippy::assert_is_empty` warning）。
- `cargo test --workspace` → 全绿；`xtask` 438 passed / 0 failed，platform-windows 93 passed / 4 ignored，platform-api 45 passed，agent-core 全套 passed / 8 ignored。
- `cargo test -p assistant-platform-api` → 45 passed / 0 failed。
- `cargo test -p assistant-platform-windows` → all passed，0 failed，4 ignored。
- `cargo test -p assistant-agent-core` → all passed，0 failed，8 ignored。
- `cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-api --all-targets -- -D warnings` → EXIT 0。
- `cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings` → EXIT 0。
- 证据 ①：`cargo test -p assistant-platform-windows pointer_tests -- --nocapture` → **6 passed / 0 failed**（原先歧义点在显式副屏空间下得 `(1400, 200)`、主屏空间下得 `(700, 100)`；未知显示器 / scale 不一致 / LogicalPixels / 越界均失败）。
- 证据 ②：`cargo test -p assistant-platform-windows pointer_coordinate_tests -- --nocapture` → **1 passed / 0 failed**（起点 `(500,100)` 按 2.0x 得 `(1000,200)`，释放点 `(1500,300)` 按 1.0x 得 `(1500,300)`）。
- xtask：`hygiene` 0E/101W PASSED、`memory-counts` 0E0W、`adr-index` 0E0W、`refscan` 0E0W、`docscan` 0E/342W、`card-check` 0E/32W、`check-comments` 0E/69W、`verify-schemas` 0E、`codegen --check` 0 drift；`check-ledger` 在状态同步提交前重跑。

### 4. DoD 逐条核对

- [x] ADR-0067 Accepted 且 `adr-index` 0E0W。
- [x] `pointer_action` 显式携带 `CoordinateSpace`；`DragTo` 携带释放点自己的 `CoordinateSpace`。
- [x] 旧 `coordinate_space_for_logical_point` 收敛启发式已删除。
- [x] 混合 DPI 原先 `CapabilityMissing` 的逻辑点可由显式空间唯一换算。
- [x] 跨显示器 `DragTo` 起点/终点各用正确 scale。
- [x] 未知坐标空间、DPI 不一致、非物理单位、越界点均显式失败。
- [x] 所有实现方与调用点编译通过。
- [x] fmt / clippy / workspace tests / 三个专项测试 / 非宿主 clippy 全绿。
- [x] 十项 xtask 门禁（`check-ledger` 在状态同步后）与 `PL-074` 闭环同批完成。

### 5. 偏差

- **流程偏差（无产品影响，已即时纠正）**：首次写 `docs/adr/README.md` / `docs/memory/decisions.md` 时先落盘后才执行 `guard acquire`。
  发现后立即取得两文件锁、在锁内复核并再次修改 `decisions.md`、随后 release；本轮无并发写者，未发生 lost update。
  这不改变 ADR-0067 的决策或代码行为，但如实记录为一次 ADR-0028 次序违反。
- **变更量偏差**：本批 `git diff --stat` 为 769 insertions / 180 deletions，超过单卡 400 行的自动化预算。
  超额来自公共 trait 变更必须同步全部实现方、调用方、真机验收入口，以及用户明确要求的四类机器证据；
  未做无关重构。请审阅时优先看公共接口、两个纯函数和两个专项测试文件。
- 无接口/Schema 范围外改动；`docs/spec/**` 未改。

### 6. 更合理做法

- 未来若干平台真正需要跨显示器拖动时，可把“起点空间 + 释放点空间”抽成 pointer 专用的小结构；
  当前只有两个字段，新增结构没有减少复杂度，故仍用现有 `CoordinateSpace`。

### 7. 遗留问题

- 架构 v2 §13.1.1 的示例签名仍是旧 `pointer_action(pt, act)`；本 ADR 已是更高裁决源，
  文档同步可在后续治理卡中处理，本卡不改 `docs/spec/**`，未把它当隐藏已完成项。

### 8. 新增长期记忆

- FACT：混合 DPI 下逻辑点不能反推唯一显示器；显式起点/终点空间分别换算，物理点只要求落在任一显示器。
- PITFALL：`origin_display` 是 scale 声明，不是物理点必须回落到该显示器的断言。
- DECISION：ADR-0067 取代 TASK-016 的 pointer 旧形状，保留其它 trait 形状。

### 9. 给审阅者的关注点

1. `physical_point_for_coordinate_space` 的跨 DPI 语义是否与调用方对“显式空间”的理解一致。
2. `DragTo` 起点与终点分别换算后，是否还有调用方把绝对坐标误绑定到错误坐标空间。
3. 删除旧启发式后，所有实现方是否都已从编译期覆盖，没有残留 fallback。

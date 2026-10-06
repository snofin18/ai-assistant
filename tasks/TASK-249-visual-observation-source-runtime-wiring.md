# TASK-249　`visual_assert` 图像来源与运行时接线

- 状态：**Done（2026-10-06；ADR-0079）**
- 阶段：1　子阶段：1b　批次：1b bridge　依赖：TASK-247、TASK-041
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/adr/0079-visual-observation-source-wiring.md`、`crates/verify/src/visual/assert.rs`、`apps/agent-core/src/runtime.rs`、`apps/agent-core/src/visual_source.rs`

---

## 目标（一句话）

把 TASK-247 留下的「谁来提供 `VisualObservation`」接通：宿主运行时从成功工具信封读取已落盘截图的内容地址与尺寸，经装配层 `storage` 读取 BGRA、转成灰度图，再通过加法式视觉观测入口交给既有 verify 引擎；像素不进入 JSON、IPC 或 `Observation`。

## 背景（为什么现在做）

TASK-247 已把 `visual_assert` 接进后置断言引擎，但 `RuntimeExecutor` 仍只调用 `verify_postconditions_with_receipt`，且没有任何宿主组件把 `ImageRef` 对应的 blob 还原成 `VisualObservation`。因此运行时里的 `visual_assert` 永远只能得到 `NotEvaluable`，能力尚未真正可用。

## write scope

- `docs/adr/0079-visual-observation-source-wiring.md`（新增）
- `apps/agent-core/src/runtime.rs`、`visual_source.rs`（新增）、`lib.rs`
- `apps/agent-core/src/production.rs`、`production_resume.rs`
- `apps/agent-core/tests/runtime_execution.rs`、`apps/agent-core/tests/visual_observation_source.rs`（新增）
- `tasks/TASK-249-visual-observation-source-runtime-wiring.md`（本卡）
- `docs/adr/README.md`、`docs/memory/decisions.md`、`LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`（仅状态同步）

## In scope

- ADR-0079 冻结宿主视觉观测的信封形状、blob 校验、BGRA→灰度口径与加法式运行时入口。
- `ObservationCollector` 新增带默认实现的 `observe_visual`，既有实现零改动。
- `RuntimeExecutor::verify_and_commit` 在有视觉观测时调用 `verify_postconditions_with_receipt_and_visual`。
- 新增 `StorageVisualObservationCollector`：只读装配层 `DatabaseHandle`，按 `BlobId` 读取参考图与实测图，验证尺寸与内容地址，转灰度并构造 `VisualObservation`。
- 生产装配根在 `plan_task` / `resume_plan` 使用该 collector。
- 测试覆盖：同图视觉断言可铸 receipt；坏 blob、尺寸不符、半组图像、非法置信度显式失败；无视觉字段时行为与旧路径一致。

## Out of scope（做了算漂移）

- 不改 `platform/api` trait、`protocol/**`、tool schema、IPC、`Observation`、`VerificationReceipt`。
- 不引入图像编解码、OCR、模板匹配或第三方依赖。
- 不在 JSON 中内联像素或哈希；不把平台句柄带出宿主进程。
- 不实现 Paint Adapter 或具体绘制任务。

## 必须遵守

- **铁律 1 / 2 / 8**：blob 地址与尺寸都当不可信输入校验；任一字段缺失或损坏显式失败；像素只在宿主进程内流转。
- **ADR-0063**：读取与灰度缓冲受既有 `MAX_IMAGE_PIXELS` 约束，不新增无界状态。
- **ADR-0077**：运行时调用 `*_with_visual`，旧入口行为保持不变。
- **ADR-0028**：写热点文件前先 `guard acquire`，写完立即 release。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-agent-core --test runtime_execution --test visual_observation_source
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments / verify-schemas / codegen --check / check-migrations
```

## 完成定义（DoD）

- [ ] ADR-0079 Accepted，登记表 / `decisions.md` / `adr-index` 同步。
- [ ] `ObservationCollector` 的旧实现无需改动仍可编译，`visual_assert` 在有图时走三值语义。
- [ ] blob 读取、BGRA→灰度、`VisualObservation` 构造均有正负测试。
- [ ] 生产 `plan_task` / `resume_plan` 使用 `StorageVisualObservationCollector`。
- [ ] 全门禁绿；PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main 后合并并回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤，
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-249 visual_assert 图像来源与运行时接线
【目标】host blob -> BGRA -> GrayImage -> VisualObservation -> *_with_visual
【write scope】apps/agent-core/src/**、apps/agent-core/tests/**、ADR-0079 与状态同步文件
【铁律】1 无静默失败；2 不可信输入先校验；8 像素不跨进程；10 契约先行；12 资源有界
【禁止】改 protocol / tool schema / IPC / Observation / VerificationReceipt；内联像素；新增依赖
【验收】专项测试 + fmt / clippy / workspace tests / xtask 十一项门禁
【依赖】TASK-247、TASK-041 已 Done
【疑问】无
```

### 2. 实际改动文件

- `docs/adr/0079-visual-observation-source-wiring.md`（新增，Accepted）+ `docs/adr/README.md`（登记 0079 / 下一可用号 0080）。
- `apps/agent-core/src/runtime.rs`：`ObservationCollector::observe_visual` 带默认实现；`verify_and_commit` 收集可选 `VisualObservation` 并调用 `verify_postconditions_with_receipt_and_visual`。
- `apps/agent-core/src/visual_source.rs`（新增）：`StorageVisualObservationCollector`、信封 `visual_observation` 元数据解析、装配 `DatabaseHandle` blob 读取、BGRA→Rec.709 灰度、`VisualObservation` 构造。
- `apps/agent-core/src/lib.rs`：导出新 collector 与字段常量。
- `apps/agent-core/src/production.rs` / `production_resume.rs`：生产 `plan_task` / `resume_plan` 注入 `StorageVisualObservationCollector`；`drive_steps` 泛化为任意 `ObservationCollector`。
- `apps/agent-core/tests/runtime_execution.rs`：运行时视觉有图提交 / 无图 `NotEvaluable` 两条契约测试；既有 collector 测试改为显式 `visual: None`。
- `apps/agent-core/tests/visual_observation_source.rs`（新增）：7 条来源契约测试（同图、缺失 blob、尺寸不符、半组对象、未知字段、无视觉字段、错误类型）。
- `docs/memory/decisions.md`（ADR-0079）、`MEMORY.md`（decisions 242 行 / 91 条）、`LEDGER.md`、`PLAN.md` / `README.md` / `plans/stage-1-pilots.md`（状态同步）。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --test runtime_execution --test visual_observation_source` → **15 passed / 0 failed**。
- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0（仅仓库既有 `clippy::assert_is_empty` unknown-lint 提示）。
- `cargo test --workspace` → EXIT 0（含新的运行时 / 来源专项）。
- xtask 十一项门禁全 EXIT 0：`hygiene` / `memory-counts` / `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` / `check-comments` / `verify-schemas` / `codegen --check` / `check-migrations`。

### 4. DoD 逐条核对

- [x] ADR-0079 Accepted，登记表 / `decisions.md` / `adr-index` 同步。
- [x] `ObservationCollector` 既有实现无需改代码仍可编译，`visual_assert` 有图时走三值语义。
- [x] blob 读取、BGRA→灰度、`VisualObservation` 构造均有正负测试。
- [x] 生产 `plan_task` / `resume_plan` 均使用 `StorageVisualObservationCollector`。
- [ ] 全门禁绿已在本地完成；PR CI / merge hash 回填待远程流程完成后追加。

### 5. 偏差

无 DRIFT。新增 `ObservationCollector::observe_visual` 是对 binary 层 trait 的加法式扩展，已先由 ADR-0079 冻结；未改 `platform/api` / `protocol/**` / tool schema / IPC / `Observation` / `VerificationReceipt`，未新增依赖或 crate。

**Scope note**：本卡单 PR diff 为 886 insertions / 18 deletions，高于 400 行软预算；其中约 500 行是来源与运行时契约测试，其余为 ADR / 卡面 / 状态同步。实现仍是一个不可拆分的闭环任务：只做来源读取与运行时接线，没有顺带实现 Paint Adapter 或真实截图工具。

### 6. 更合理做法

继续沿用 ADR-0077 的“并列参数”方向，而不是把像素或 `GrayImage` 塞进 `Observation`：运行时只多收集一个可选视觉观测，普通步骤仍得到 `None`，因此既有 Notepad 路径不受影响；存储读取与颜色转换留在宿主装配层，verify 继续保持零依赖纯逻辑。

### 7. 遗留问题

Paint Adapter / 具体截图工具仍需在自己的成功信封中提供 `visual_observation.reference` / `observed` / `confidence` 元数据；本轮只把来源契约与运行时入口接通，不实现 Paint 工具或真实图像采集。若未来要支持区域截图或参考图锚点选择，另开卡并按 ADR-0079 D3/D4 扩展契约。

### 8. 新增长期记忆

`docs/memory/decisions.md`：新增 ADR-0079 条目，记录宿主视觉来源、信封形状、BGRA→灰度口径与失败语义。未新增 FACT / PITFALL。

### 9. 给审阅者的关注点

1. `visual_observation` 只携带 `blob_id` / `width` / `height` / `confidence`，审阅时请确认没有任何像素或哈希被序列化。
2. `StorageVisualObservationCollector` 对参考 / 实测成对性、blob 地址、尺寸和置信度都 fail-closed；缺图只在信封完全没有视觉字段时返回 `None`，半组对象不得被当成“无图”。
3. 生产非视觉步骤仍走 `observe_visual = None`，请重点确认 Notepad `plan_task` / `resume_plan` 的既有行为没有被改成新的成功条件。

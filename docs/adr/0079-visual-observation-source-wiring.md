# ADR-0079　`visual_assert` 图像来源与运行时接线

状态：**Accepted**（2026-10-06，按用户 2026-10-06「继续 TASK-247」授权）　日期：2026-10-06　
Supersedes：—　Superseded by：—
关联：ADR-0063、ADR-0071、ADR-0073、ADR-0074、ADR-0076、ADR-0077、
`crates/verify/src/visual/assert.rs`、`apps/agent-core/src/runtime.rs`、
`tasks/TASK-247-visual-assert-engine-wiring.md`

## 背景（为什么现在要决定）

TASK-247 已按 ADR-0077 把 `visual_assert` 接进后置断言引擎，并保留了旧入口的 fail-closed 行为。
但调用方仍缺少一条确定路径回答两个问题：

1. 参考图与实测图从哪里来；
2. 运行时在哪一层把已落盘的截图变成 `VisualObservation`，并且不把像素放进 JSON、IPC 或 `Observation`。

当前 `RuntimeExecutor` 只调用 `verify_postconditions_with_receipt`，因此任务包中的 `visual_assert`
即使已被解析，运行时也只能得到 `NotEvaluable`。这不是算法缺口，而是来源与接线缺口。

## 决策（一句话）

**宿主运行时通过 `ObservationCollector::observe_visual` 获取可选视觉观测；生产装配层的
`StorageVisualObservationCollector` 只读取工具信封里的 blob 元数据，凭 `BlobId` 从
装配拥有的 `storage` 读取 BGRA，按固定整数 luma 口径转灰度并构造 `VisualObservation`，
再由 `RuntimeExecutor` 调用 `verify_postconditions_with_receipt_and_visual`。**

## 决策细化

| # | 内容 |
|---|---|
| **D1** | `ObservationCollector` 新增带默认实现的 `observe_visual(&PlanStep, &ToolEnvelope) -> Result<Option<VisualObservation>, RuntimeExecutionError>`；默认返回 `Ok(None)`。既有 collector 不需改代码，普通步骤行为与 TASK-247 前完全一致。 |
| **D2** | `RuntimeExecutor::verify_and_commit` 先收集普通 `Observation`，再收集可选 `VisualObservation`，最后统一调用 `verify_postconditions_with_receipt_and_visual`。无图时传 `None`，仍由 ADR-0077 的 `NotEvaluable` 语义处理。 |
| **D3** | 图像来源只通过成功 `ToolEnvelope.data.visual_observation` 传递一个对象，形状固定为：`reference` 与 `observed` 两个图像描述符，以及 `confidence`。每个描述符只含 `blob_id`、`width`、`height`。禁止内联像素、哈希或平台句柄。 |
| **D4** | `blob_id` 必须是 `BlobId::parse` 接受的 64 位小写 hex 内容地址；`width` / `height` 必须为正且可无损转为 `u32`。参考与实测必须成对出现；只出现一个或对象结构不完整都属于显式观测错误。 |
| **D5** | 读取 blob 必须经装配拥有的 `DatabaseHandle`，不能在平台层私开连接。读取结果必须满足 `width * height * 4 == bytes.len()`；丢失、损坏或尺寸不符一律显式失败，绝不补零、截断或返回默认图。 |
| **D6** | BGRA→灰度使用固定整数 Rec.709 luma：按行主序处理每个 4 字节像素，忽略 alpha，`gray = (77 * R + 150 * G + 29 * B + 128) >> 8`。该口径足够稳定、无浮点分叉，且由同一测试固定。alpha 不参与合成，因为当前 Windows 截图通道产出的是不透明窗口表面。 |
| **D7** | 灰度图只通过既有 `GrayImage::new` 构造，继续受 `MAX_IMAGE_PIXELS = 16_777_216` 上限约束。`VisualObservation::new` 继续校验置信度在 `[0.0, 1.0]`；`confidence = 0` 在既有 `confidence_min > 0` 下会得到 `NeedsHuman`，不是成功。 |
| **D8** | 不改 `platform/api`、`protocol/**`、tool schema、IPC、`Observation` 或 `VerificationReceipt`；不引入图像编解码或第三方依赖；不新增 crate。 |
| **D9** | 生产装配在 `plan_task` 与 `resume_plan` 两处注入 `StorageVisualObservationCollector`，保证首次执行和审批恢复走同一条视觉来源路径。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 把 `GrayImage` 或字节数组塞进 `Observation` / 工具信封 | ❌ 否决 | 违反 ADR-0074 / ADR-0077 的「像素不进可序列化结构」，也会把不可信巨量输入带进模型与 IPC 边界。 |
| 2 | 在平台层直接返回灰度图，或扩展 `WindowProvider::capture` | ❌ 否决（本轮） | 改动公共 trait 并让平台层承载 verify 专用语义；当前 `ImageRef` 已是持久 blob 引用，宿主层完全可以在不扩公共形状的前提下完成来源接线。 |
| 3 | 只在 `crates/verify` 内实现 blob 读取与颜色转换 | ❌ 否决 | verify 是零依赖纯逻辑 crate；读取 storage 会把持久化与 IO 依赖反向压进断言引擎。 |
| 4 | **宿主层 `StorageVisualObservationCollector` 读取 blob 元数据并转灰度，运行时通过加法式 `observe_visual` 注入（本 ADR）** | ✅ 采纳 | 保持平台 / verify / protocol 形状不变；消费已落盘的截图证据；像素只在宿主进程内流转；既有运行路径零破坏。 |

## 影响

- `apps/agent-core` 新增 `visual_source` 模块与 `StorageVisualObservationCollector`。
- `RuntimeExecutor` 增加一次可选视觉观测收集，并改调既有 `verify_postconditions_with_receipt_and_visual`。
- `production.rs` / `production_resume.rs` 使用新 collector；普通非视觉步骤仍走 `None`。
- 新增测试固定信封形状、BGRA→灰度公式、blob 失败模式和运行时 receipt 铸造条件。
- 不改 `crates/verify`、`crates/platform/api`、`crates/storage`、`protocol/**` 或任何第三方依赖。

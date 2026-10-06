# ADR-0077　把 `visual_assert` 接进后置断言引擎（加法式接入）

状态：**Accepted**（2026-10-06，按用户 2026-10-06 预授权代为裁决）　日期：2026-10-06　Supersedes：—　Superseded by：—
关联：ADR-0074、ADR-0047、ADR-0063、`crates/verify/src/{postcondition,assertion,verdict}.rs`、`crates/verify/src/visual/**`、`tasks/TASK-042-visual-verify-phash-dhash-confidence.md`、`docs/PARKING_LOT.md` PL-110

## 背景（为什么现在要决定）

TASK-042 已按 ADR-0074 落地零依赖的视觉纯逻辑（`GrayImage` / pHash / dHash / 结构化 `visual_assert` / `confidence_min`），
但**没有接进既定后置断言引擎**：`parse_postconditions` 仍拒绝 `visual_assert`（fail-closed），
`Postcondition` 没有对应变体，`Observation` 也不携带参考图 / 实测图 / 置信度。结果是这条能力只能被
自带入口调用，写在工具/任务包的 postcondition 里会直接解析失败 —— 记在 `PL-110` / `DRIFT-042-1`。

接入要动 TASK-023 冻结的既有公共形状（漂移触发器 ③），因此必须先冻结接入方式。

## 决策（一句话）

**以「加法式」接入：新增 `Postcondition::VisualAssert` 变体与 `evaluate_postcondition_with_visual` /
`verify_postconditions_with_visual` 入口（既有签名一律保留并委托 `None`）；参考图 / 实测图 / 置信度
**不进 `Observation`**，而是作为**并列参数**传入 —— 像素因此永不进入可序列化的观察结构。**

## 决策细化

| # | 内容 |
|---|---|
| **D1** | `Postcondition` 新增变体 `VisualAssert { assertion: crate::visual::VisualAssert }`。变体形状直接复用 ADR-0074 冻结的结构化断言，**不新增字段、不引入表达式语言**（ADR-0047）。 |
| **D2** | `parse_postconditions` 认出 `"kind": "visual_assert"` 时，**直接复用** `crate::visual::parse_visual_assert`（同一处解析、同一套判据），错误映射为既有 `VerifyError::MalformedPostcondition { index, reason }`；未知字段 / 缺字段 / 越界阈值的拒绝原因保持原样，**不新造 ErrorCode**。 |
| **D3** | **图像不进 `Observation`**。`Observation` 派生了 `Eq` + `Serialize/Deserialize`，而 `VisualObservation` 含 `f64` 置信度（无 `Eq`）与原始像素（ADR-0074 明写「不在 JSON 内联像素」）。因此新增**并列参数**：`evaluate_postcondition_with_visual(postcondition, observation, visual: Option<&VisualObservation>)`，`VisualAssert` 只从它取图。 |
| **D4** | 既有入口**不破坏**：`evaluate_postcondition` / `verify_postconditions` / `verify_postconditions_with_receipt` 签名不变，内部等价于 `visual = None`；`VisualAssert` 在无图时返回 `NotEvaluable`（→ 既有归约得 `Inconclusive` / `VerifyFailed`），**绝不判成功**。 |
| **D5** | 有图时：调用 ADR-0074 的 `evaluate_visual_assert`，把三值 `VisualVerdict` 映射为既有 `AssertionOutcome`（`to_assertion_outcome()`）；低置信 / 尺寸不符 → `NeedsHuman` → `NotEvaluable`。求值错误（`VisualError`）同样落 `NotEvaluable` 并带原因，**不 panic、不默认通过**。 |
| **D6** | 归约层同样加法式：新增 `verify_postconditions_with_visual(...)` 与 `verify_postconditions_with_receipt_and_visual(...)`；既有 `verify_postconditions*` 委托 `None`。`VerificationReceipt` 的形状与铸造条件（只有全 `Satisfied` 才能铸造）不变。 |
| **D7** | 本轮**不改** IPC / tool schema / `protocol/**` / `Observation` / `VerificationReceipt` 的字段；不新增依赖；不引入图像编解码（像素转灰度仍归平台 / capture 层，ADR-0071）。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 把 `VisualObservation` 放进 `Observation` 的新字段 | ❌ 否决 | `Observation` 派生 `Eq` + serde；`f64` 无 `Eq`，且像素入可序列化结构直接违反 ADR-0074「不在 JSON 内联像素」。要这样做就得给 `Observation` 降级 derive 并加 `#[serde(skip)]`，改动面与语义代价都更大。 |
| 2 | 只加变体、把求值留给调用方自己调 `visual::*` | ❌ 否决 | 等于 `parse` 认得、`verify` 不认：postcondition 会被静默降级成 `NotEvaluable` 装饰，正是铁律 1 要防的形态。 |
| 3 | **加法式接入：新变体 + 并列 `visual` 参数的新入口，旧入口委托 `None`（本 ADR）** | ✅ **采纳** | 既有调用方零改动；写在 postcondition 里的 `visual_assert` 真的能被求值；像素不进可序列化结构；与 ADR-0074 的「低置信不得单独判成功」一致。 |

## 影响

- `crates/verify/src/postcondition.rs`：+1 变体、+1 解析分支（复用 `visual::parse_visual_assert`）。
- `crates/verify/src/assertion.rs`：+1 求值路径 + 新入口 `evaluate_postcondition_with_visual`；旧入口保持。
- `crates/verify/src/verdict.rs`：+2 加法式入口；旧入口保持，`VerificationReceipt` 不变。
- 新增测试：`visual_assert` 的解析（正/负）、有图求值三值、**无图 → `NotEvaluable`**、旧入口行为不变。
- **不新增依赖、不新增 crate、不改 `protocol/**`；`PL-110` / `DRIFT-042-1` 由此闭环。**

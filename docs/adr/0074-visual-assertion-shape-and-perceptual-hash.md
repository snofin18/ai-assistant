# ADR-0074　`visual_assert` 结构化形状与 pHash / dHash 口径

状态：**Accepted**（2026-10-06，按用户 2026-10-06 预授权代为裁决）　日期：2026-10-06　
Supersedes：—　Superseded by：—
关联：ADR-0047、ADR-0063、ADR-0071、ADR-0073、
`cross-platform-ai-assistant-architecture-v2.md` §7.4 与附录 A `#/$defs/assertion`、
`crates/verify/src/visual/**`、`tasks/TASK-042-visual-verify-phash-dhash-confidence.md`

## 背景（为什么现在要决定）

架构 v2 §7.4 的断言表列了 `visual_assert`，附录 A 只给了它一个自由数字字段
`confidence_min`，没有冻结比较的字段、算子、容差与感知哈希口径。TASK-023 在实现
`crates/verify` 时把 `visual_assert` 解析期拒绝并指向 TASK-042；TASK-042 必须先把形状
和算法口径冻结，否则不同会话会各自发明一套「像不像」的判据。

同时有两条既有约束：ADR-0047 永久否决自由字符串 `assert`（结构化 `field` + `op` +
具名字段），本轮预授权又要求零第三方依赖（不引 `image` / `img_hash` 等 crate）。
因此 TASK-042 的纯算法必须在内存灰度缓冲上自己实现，且不能借「表达式语言」绕开结构化。

另外，`crates/verify` 的既有公共形状 `Postcondition` / `Observation` /
`VerificationReceipt` 是 TASK-023 冻结的：把 `visual_assert` 接进它们需要新增
`Postcondition` 变体并让 `Observation` 携带参考图与置信度，属于「改既有公共形状」
（漂移触发器 ③）。本轮不许做，只把纯算法与契约落到 `crates/verify/src/visual/**`，
接入另立卡（见「影响」与 `DRIFT-042-1`）。

## 决策（一句话）

**`visual_assert` 只有结构化形状**：`kind` + `field` + `op` + 该算子专属的具名容差/阈值
参数 + 必填 `confidence_min`；感知哈希固定为「32×32 盒式下采样 → 8×8 DCT 中位阈值
pHash」与「9×8 盒式下采样 → 相邻像素差分 dHash」两种 64-bit 口径；低置信结果一律标注
`NeedsHuman`，映射到既有三值求值里的 `NotEvaluable`，**永不进入 `Satisfied`**，因此也
永远拿不到 `VerificationReceipt`。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | **JSON 形状固定为扁平结构化字段**，无自由字符串、无表达式：`kind`（必须 `visual_assert`）、`field`、`op`、`confidence_min`，以及 `op` 专属的具名参数。`parse_visual_assert` 对未知字段、缺字段、类型错误、字段/算子不匹配一律在解析期拒绝（`VerifyError` → `ErrorCode::ToolInvalidArgs`）。 |
| **D2** | **`field` 闭集**：`pixels`（逐像素比较）、`phash`、`dhash`。不引入 OCR / 模板匹配 / 区域选择；那些需要平台与图像编解码，归后续平台卡，不是本 ADR 的范围。 |
| **D3** | **`op` 闭集与参数**：① `pixels` + `mean_abs_diff_within` + `max_mean_abs_diff`（整数 `0..=255`，平均绝对像素差上限）；② `pixels` + `changed_ratio_within` + `pixel_delta_threshold`（整数 `0..=255`，单个像素记为「变化」的阈值）+ `max_changed_ratio`（`0.0..=1.0`）；③ `phash` / `dhash` + `hamming_within` + `max_hamming_distance`（整数 `0..=24`）。其它组合解析期拒绝。 |
| **D4** | **`confidence_min` 必填且严格大于 0**：取值 `(0.0, 1.0]`。`0.0` 被拒绝，因为置信度为 0 的观测会「永远达标」，等于把低置信兜底悄悄关掉（铁律 1 的静默失败面）。无隐式默认值（沿用 TASK-023 的 invariant 5）。 |
| **D5** | **感知哈希口径（pHash）**：输入灰度 8-bit → 32×32 盒式平均下采样 → **不做 `alpha(u)·alpha(v)` 归一化的 raw 2D DCT-II** 取左上 8×8（64 个系数，含 DC）→ 以排序后第 31/32 个系数（0-based）的平均值作为中位阈值 → 系数 **严格大于**中位阈值置 1，否则置 0。位序 = `垂直频率 v * 8 + 水平频率 u`，bit 0 = 最低有效位 = `(u=0, v=0)`。 |
| **D6** | **感知哈希口径（dHash）**：输入灰度 8-bit → 9×8 盒式平均下采样 → 每行 8 次比较「左像素 **严格大于** 右像素」置 1 → 8 行 × 8 位 = 64-bit。位序 = `row * 8 + column`，bit 0 = 最低有效位 = 第 0 行第 0 次比较。 |
| **D7** | **汉明距离与误判率判据**：距离 = 两个 64-bit 哈希异或后的 `count_ones()`。`max_hamming_distance` 硬上限 **24**：在「两张无关图产生独立均匀 64-bit 哈希」的零假设下，`P(distance ≤ 24) = 2.997%`（精确二项尾和，`sum_{k=0..24} C(64,k) / 2^64`），满足阶段 DoD 的误判率 < 5%；推荐的更严取值是 pHash ≤ 10（`9.98e-7 %`）与 dHash ≤ 12（`2.28e-5 %`）。阈值必须由任务卡显式写出，禁止隐式默认。 |
| **D8** | **输入是显式的「宽 / 高 / 像素缓冲」**：`GrayImage::new(width, height, pixels)` 校验宽高非零、缓冲长度等于 `width * height`、非空、像素总数不超过 `16_777_216`（ADR-0063 的有界纪律）；越界 / 尺寸不符 / 空缓冲 / 超上限一律显式错误，绝不补零、绝不截断伪造结果。 |
| **D9** | **低置信不得单独判成功**：评估返回三值 `VisualVerdict`（`Satisfied` / `Falsified` / `NeedsHuman`）。`actual.confidence < confidence_min`、或参考图与实测图尺寸不一致时返回 `NeedsHuman`；`VisualVerdict::to_assertion_outcome()` 把 `NeedsHuman` 映射为 `AssertionOutcome::NotEvaluable`。既有 `verify_postconditions` 把 `NotEvaluable` 归入 `Inconclusive`（`ErrorCode::VerifyFailed`），只有全 `Satisfied` 才能铸造 `VerificationReceipt` —— 本 ADR 不改收据类型，只保证视觉结果走同一条非成功通道。 |
| **D10** | **参考与实测的载体**：参考图与实测图都由调用方（Host / 平台）放入 `VisualObservation`，不在 `visual_assert` JSON 里内联像素或哈希。理由：像素是巨大的不可信输入，内联会让工具 schema 膨胀并诱导模型编造参考图；参考必须来自可审计的截图证据或任务包锚点。 |
| **D11** | **本轮不改既有公共形状**：不新增 `Postcondition` 变体、不改 `Observation`、不改 `VerificationReceipt`、不改 `parse_postconditions` 的既有行为，也不改 `crates/verify/tests/**` 既有断言。`visual` 模块是新的纯逻辑入口，接入既有 postcondition 引擎另立卡；本轮记 `DRIFT-042-1`。 |
| **D12** | **零第三方依赖**：pHash / dHash / 盒式下采样全部用 `std` 自己实现，不引 `image` / `img_hash` / `fast_image_resize` 等 crate，不新增 `docs/DEPENDENCIES.md` 行；不改根 `Cargo.toml`、不 `#[allow]`、不 `unsafe`。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 引入 `image` + `img_hash` 等 crate，直接用现成实现 | ❌ 否决（本轮） | 触发漂移触发器 ①（第三方依赖）：需要先写 ADR、登记 `docs/DEPENDENCIES.md`、审许可证与传递依赖；而本轮目标恰恰是零依赖纯算法。感知哈希本身只有几十行数学，先自己实现可把供应链与口径一次性冻结。 |
| 2 | 让 `visual_assert` 接受自由字符串（如 `similar_to(reference, 0.9)`） | ❌ 否决 | 与 ADR-0047 直接冲突：自由字符串 = 表达式语言（lexer / parser / evaluator / scope）= 新抽象层，且模型无法枚举可写形式。结构化 `field` + `op` + 具名参数已能表达全部本轮需要的判据。 |
| 3 | 把 `visual_assert` 直接塞进 `Postcondition` 枚举并让 `Observation` 携带图片 | ❌ 否决（本轮） | 改 TASK-023 冻结的既有公共形状（漂移触发器 ③），且会让 `Observation` 从纯文本 / 指纹结构膨胀成图片容器。本轮先落纯算法与契约，接入由后续卡按本 ADR 做。 |
| 4 | 不做感知哈希，只做逐像素容差 | ❌ 否决 | 目标应用存在抗锯齿、半透明、主题差异与缩放，逐像素相等必然不稳定（`target-apps-feasibility.md` §3 已指出）。pHash/dHash 正是为这种「轻微扰动仍命中、明显不同不命中」设计的。 |
| 5 | **结构化字段 + 自写 pHash/dHash + 独立 `VisualObservation`（本 ADR）** | ✅ **采纳** | 零依赖、零既有公共形状改动，先冻结判据与失败语义；后续接入只需把 `parse_visual_assert` / `evaluate_visual_assert` 的结果转成既有的 `AssertionOutcome`。 |

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

- 新增 `crates/verify/src/visual/**`：`GrayImage`、`pHash` / `dHash`、汉明距离、容差断言、
  `VisualObservation` / `VisualVerdict` / `parse_visual_assert`，及专项正负样本测试。
- `crates/verify/src/lib.rs` 只做模块接线与 `pub use`；`crates/verify/README.md` 更新边界与
  「visual 模块已落地、postcondition 接入待后续卡」的已知限制。
- `docs/adr/README.md` 登记 0074 并推进「下一个可用编号」；`docs/memory/decisions.md`
  追加 ADR-0074 条目；`LEDGER.md` / `PLAN.md` / `plans/stage-1-pilots.md` / `README.md`
  按每卡 Done 的同步规则更新。
- **本轮不改**：架构 v2 附录 A / §7.4、`docs/spec/**`、`protocol/**`、`crates/verify` 的
  既有公共类型与既有测试、`docs/DEPENDENCIES.md`。附录 A 的真实形状同步（把
  `field` / `op` 的 `visual_assert` 专用取值与阈值参数写进 schema）归后续接入卡，
  属于漂移触发器 ③，必须先有卡与 spec 变更。
- `DRIFT-042-1`：`visual_assert` 接入 `Postcondition` / `Observation` 需要改既有公共形状，
  本轮按卡面第 4 条停在纯逻辑，留可合并 WIP。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 自己实现的 DCT 与主流库有数值差异 | 口径固定在 D5（下采样尺寸、DCT 类型、中位阈值、位序），专项测试用确定性合成图；不做跨库数值对齐的承诺，只承诺同一实现对相同输入可复现。 |
| `max_hamming_distance` 被写成很大的值，导致「什么都算命中」 | D3 硬上限 24，解析期拒绝更大值；D7 给出随机图对 `P(≤24) = 2.997% < 5%` 的机器可复算判据。 |
| 低置信结果被调用方当成成功 | D9 把 `NeedsHuman` 映射为 `NotEvaluable`，接进 `verify_postconditions` 后是 `Inconclusive` + `VerifyFailed`，结构上拿不到 receipt。 |
| 参考图内联进 JSON 后被模型编造 | D10 明确参考图只从 `VisualObservation`（截图证据 / 任务包锚点）来，JSON 只写判据。 |
| 大图造成内存 / CPU 放大 | D8 像素总数上限 16,777,216；下采样先把长边压到 32 / 9，再算 DCT，无缓存、无全局状态。 |

## 已知限制

- **pHash / dHash 需要参考图有足够的中低频细节。** 当参考图接近单色，或纹理细到在 32×32 / 9×8
  盒式下采样后塌缩成均匀灰（例如 4px 棋盘格），64 个低频系数里只有少数有能量，中位阈值落进
  浮点噪声区，哈希的判别力会显著下降。TASK-042 的专项测试固定了这条边界：**「没画上」（参考有
  细节、实测接近单色）应优先用 `pixels` + `mean_abs_diff_within` / `changed_ratio_within`
  判定**，pHash / dHash 只作为轻微扰动下的容差通道；调用方对低细节参考图也不得给出高
  `confidence`。这是「低置信不得单独判成功」在算法层的配套用法，不是本 ADR 的例外。
- **本轮不计算置信度。** `VisualObservation.confidence` 由调用方提供（截图质量、定位可靠性、
  遮挡 / 运动状态等）；本 ADR 只保证「低于 `confidence_min` 一定变成 `NeedsHuman`」，不承诺
  调用方给出的置信度本身正确。
- **不承诺跨库数值对齐。** 只承诺同一实现（同一份 `crates/verify/src/visual/**`）对相同输入
  可复现；与 `img_hash` / OpenCV 等其他实现之间不做比特级对齐。

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `cargo test -p assistant-verify` 覆盖：相同图命中、亮度 / 噪点轻微扰动在容差内命中、
   明显不同图不命中、低置信 `NeedsHuman`、尺寸不符 `NeedsHuman`、空缓冲 / 尺寸不符 /
   超大图显式失败、未知字段与字段算子错配解析期拒绝；
2. `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` /
   `cargo test --workspace` 与 xtask 十一项门禁全绿；
3. `cargo run -p xtask -- adr-index` 绿灯（登记表 ↔ ADR 文件 ↔ `decisions.md` 三方一致）；
4. **何时重新评估**：需要 OCR / 模板匹配 / 区域选择、需要把参考哈希内联进任务包、
   或需要接入 `Postcondition` / `Observation` 时 → 新开卡；若新卡要改本 ADR 的 JSON
   形状或阈值上限，必须先写取代本 ADR 的新 ADR（D11 / ADR-0047 的永久性）。

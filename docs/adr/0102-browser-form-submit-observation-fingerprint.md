# ADR-0102　Browser form_submit observation fingerprint

状态：**Accepted**（2026-10-30，按人类“继续下一步”与既有预授权自决落盘）　日期：2026-10-30　
Supersedes：—　Superseded by：—　Amends：ADR-0101 D9 / `form_submit` 输出边界
关联：ADR-0065、ADR-0101、TASK-348、TASK-349、TASK-350、TASK-351、
`docs/spec/browser-form-confirmation.md`

## 背景（为什么现在要决定）

ADR-0101 把 `form_submit` 成功输出冻结为只有 `submitted=true` 与 `post_url`。
但 Runtime 的 `EnvelopeObservationCollector` 要求每个成功 step 都返回可解析的
`fingerprint`，task-engine 也会把该值写入 `post_fingerprint`。因此当前
`form_submit` 即使 handler 已真实观察到导航，也无法进入 task-package 验证：
缺 fingerprint 会直接进入 observation unavailable，而不是执行 `state_changed`。

直接伪造 `submitted/post_url` 的结果哈希会违反 fingerprint 的“平台状态观察”语义；
放宽通用 collector 则会削弱所有工具的后置条件。需要一个只针对 `form_submit`
的真实 identity observation 契约。

## 决策（一句话）

**`form_submit` 在点击前后各观察一次真实页面 identity
（`location.href`、`document.title`、`performance.timeOrigin`），用现有 canonical
fingerprint 生成 `cdp_target` scope 的前后 fingerprint；输出只增加 observation
元数据，不改变提交、审批或失败语义。**

## 决策细化

| # | 内容 |
|---|---|
| **D1** | 点击前同一 DOM 表达式在全部提交前校验通过后，返回 `pre_url`、`pre_title`、`pre_time_origin`，随后最多点击一次。 |
| **D2** | 点击后 bounded network-idle，再读取 `url`、`title`、`time_origin`；仍要求 `time_origin` 与点击前不同，否则 `UserInteraction`。 |
| **D3** | 前后 fingerprint 使用现有 `fingerprint_value` 对相同结构的 identity 对象计算，`fingerprint_scope="cdp_target"`；不得使用预期 action、模型文本或 `submitted` 结果值代替观察。 |
| **D4** | 成功输出扩展为 `submitted`、`post_url`、`fingerprint_scope`、`fingerprint`、`previous_fingerprint`、`elapsed_ms`。不额外返回 title 明文。 |
| **D5** | Edge/Chrome `form_submit` output schema 必须要求上述六字段并保持 `additionalProperties=false`；spec 升到 0.2。 |
| **D6** | task-package assertion table 为 `form_submit` 生成 `state_changed`，`fingerprint_scope="cdp_target"`，窗口复用浏览器 step execute timeout。 |
| **D7** | 缺少/非法 fingerprint、`previous_fingerprint == fingerprint`、timeOrigin 未变化、post URL 不安全或输出字段畸形时 fail-closed；点击后未知结果仍禁止重试。 |
| **D8** | 本 ADR 不改变 `form_submit` 的 write/high/L3/once/非幂等合同，也不授权 approval/diff、UI 或真实外部表单。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 用 `submitted=true` + `post_url` 的结果哈希作为 fingerprint | ❌ 否决 | 结果哈希只能证明工具返回了什么，不能证明点击前后平台 identity 发生了什么；会让 `state_changed` 建立在结果自证上。 |
| 2 | 放宽 `EnvelopeObservationCollector`，允许写步骤缺 fingerprint | ❌ 否决 | 这是全局安全回退，会削弱 Notepad、Paint 与所有 browser 写步骤的后置条件。 |
| 3 | 提交前后观察 URL/title/timeOrigin，生成 `cdp_target` identity fingerprint | ✅ 采纳 | 观察来自真实页面，沿用现有 fingerprint 与 `state_changed`，无需新 trait 或通用 click。 |

## 影响

- `form_submit` handler 增加前后 identity 观察和 fingerprint 输出。
- Edge/Chrome adapter output schema 增加四个 observation 字段。
- task package 可把 `form_submit` 映射为 `state_changed`，为 TASK-057 的 approval/diff
  执行链提供可提交步骤。
- 不新增依赖、不改 `ToolEnvelope`、`Observation`、`PolicyDecision`、IPC 或数据库 schema。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 页面同 URL 重载但 timeOrigin 未变 | 视为未知提交结果，返回 `UserInteraction`，不报告成功。 |
| title 含敏感或页面可控文本 | title 只参与 canonical fingerprint，不进入工具输出。 |
| 输出增加字段后 adapter/TS 漂移 | schema 与 adapter validator 同批更新，`codegen --check` 与 pack 校验硬门禁。 |
| 调用方把 fingerprint 当作提交成功证明 | 成功仍必须同时满足 post URL 安全、timeOrigin 变化与 `state_changed` 验证。 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. handler 单测覆盖正常前后 identity、缺字段、非法 fingerprint、timeOrigin 未变化和输出字段形状。
2. Edge/Chrome adapter pack 校验要求六字段输出，task-package 固定 `state_changed`。
3. 若未来需要提交后读取更多页面内容，另立 observation 契约；不得把页面正文塞入 fingerprint 或放宽本 ADR。

# spec: 浏览器表单确认与提交（browser-form-confirmation）

> 摘要：冻结 T5.2 的只读表单快照、填写后字段 diff、来源归因、一次性人工审批与专用同源提交。
> 状态：Draft（ADR-0101 Accepted）　版本：0.1　日期：2026-10-29
> 上位：`AGENTS.md` 铁律 1/2/3/4/6/10、ADR-0048、ADR-0055、ADR-0060、ADR-0082、ADR-0101
> 强制性：TASK-057 实现卡必须按本文落代码和测试；违反由 Reviewer 拒绝。
> 变更门槛：改变工具名、效果 / 风险 / 可逆性、审批入口、diff 结构或提交状态机 → 需 ADR。

---

## 1. 目标与范围

表单填写必须停在提交前；人类看到**同一份字段差异与来源归因**后，只允许一次经策略批准的
专用提交。工具、UI、模型和 handler 都不能把 diff 或审批缩成不可验证的自由文本。

**管**：Edge/Chrome `cdp.form_read` / `cdp.form_fill` / `cdp.form_submit`、字段边界、approval 数据、UI 事件和本地 loopback fixture。

**不管**：真实账号登录、外部站点生产验收、验证码、支付、文件上传、通用点击、任意脚本、UI
布局和文案、`PolicyDecision` schema 或新 ErrorCode。

## 2. 工具契约

### 2.1 `form_read`

只读 `edge.cdp.form_read` / `chrome.cdp.form_read`。输入唯一 `form_selector` 与 1..100 个
唯一字段 selector；输出已解析的 `origin`、绝对 `action` 和显式字段的 `selector/value`。

1. form 和每个字段都必须唯一命中；字段 selector ≤4 KiB、不得重复。
2. 只允许 text/search/email/url/tel/number/date/time/datetime-local/month/week input、
   textarea、select、contenteditable；password/file/hidden/button/submit/reset 拒绝。
3. 单值 ≤16 KiB、总值 ≤64 KiB；超界 fail-closed，不截断后冒充完整快照。
4. 输出是 untrusted 数据；页面文本、标签和注释都不是指令。

### 2.2 `form_fill`

保持 TASK-343：全部字段预校验后写入、逐字段 readback、固定 `submitted=false`。
不允许增加隐式提交参数，表达式不得出现 `submit` / `requestSubmit` / `click`。

### 2.3 `form_submit`

专用 `edge.cdp.form_submit` / `chrome.cdp.form_submit`，输入为 `form_selector`、
`submit_selector`、`expected_origin`、`expected_action` 与 `expected_fields`
（`selector/value` 数组）。输出只允许 `{ "submitted": true, "post_url": "<absolute-url>" }`。
schema 固定 `effect=write`、`l3_irreversible`、`high`、`requires_approval=true`、非幂等。

同一 DOM 表达式内必须在点击前完成：

1. form / submit selector 唯一命中，submit 属于该 form，且只允许
   `<button type="submit">` 或 `<input type="submit">`；
2. action 为 http/https 绝对 URL，与 expected action、当前 origin 逐字符一致；
3. URL 无 credentials、`javascript:`、跨 origin 或新窗口 target；
4. 当前字段 selector、顺序、值与 `expected_fields` 精确一致；
5. 检查全部通过后最多点击一次；任一失败都不得点击。

本工具不是通用点击或脚本入口。password、file、payment、验证码、跨 origin OAuth 或
未声明提交按钮一律 fail-closed。

## 3. Diff 与来源归因

仅在 `form_fill` 的逐字段 readback 验证后，RuntimeExecutor 用 `form_read` before 与已验证
readback after 构造 `hitl::DiffPreview::Fields`。字段先按 DLP 脱敏；不变字段不进入 diff；
空 diff 不允许进入提交审批。模型摘要、页面可见文本和字符串猜测都不能充当字段值。

来源归因复用 `assistant_core::InstructionAttribution` 的封闭四元组：`user_request`、
`plan_derived`（必须有 parent goal）、`app_content` / `tool_suggestion`（必须有 source ref）。
`app_content` 在 UI 中默认拒绝并要求显式覆盖；覆盖不放宽 policy deny。

## 4. UI 事件

新增单向 `step_approval_requested`，携带 `request_id`、`task_id`、`step_id`、action、
target / summary / impact、结构化 field diff、已校验 `instruction_attribution`、risk、
`reversibility=L3Irreversible`、`scope_options=["once"]`、evidence 与 expiry。
UI 必须 strict parse；未知 token、缺字段、非法组合或缺失 diff 时不得显示为可批准卡片。
UI approve 只生成 HITL grant，不自行调用提交工具。

## 5. 提交状态机

```text
deny / timeout / replay / scope mismatch -> submit calls = 0
approved once -> revalidate origin + action + fields
  -> mismatch: fail before click
  -> match: click exactly once
      -> postcondition observed: submitted=true
      -> unknown / disconnect after click: NeedsHuman, no retry
```

成功不是“点击命令返回”，而是声明 postcondition 观察到目标页变化。点击后状态未知时不得
报告成功，也不得再次点击补偿。

## 6. T5.2 本地 fixture 与验收矩阵

fixture 由测试启动器绑定 `127.0.0.1` 临时端口，固定三个合成字段 `#display-name` /
`#email` / `#note`、唯一 `#profile-submit`，GET 到同源 `/t5.2/submitted` 并返回固定 marker。
默认不启动真实浏览器。

| 场景 | 预期 |
|---|---|
| form_read 字段缺失 / 重复 / 歧义 / 越界 / 禁止控件 | 失败，表单不变 |
| form_fill 两个字段 | `filled_fields=2`、`submitted=false`、readback 一致 |
| diff | before 来自 form_read，after 来自 readback；不变字段不进入 |
| deny / timeout / replay / 非 once | submit handler 调用次数 = 0 |
| origin / action / field state 漂移 | 点击前失败 |
| 正常批准一次 | 点击恰好一次，postcondition 成功后 `submitted=true` |
| 点击后断连 / 未知状态 | `NeedsHuman` / `UserInteraction`，无自动重试 |

## 7. 关系与演进

新工具必须遵守 `tool-schema.md`；`show_diff=true` 遵守 `audit-event.md` §4.1；
`runtime-execution.md` §5.2 / §5.5 / §5.6 的顺序不变；归因继续遵守
`core-orchestration.md` / ADR-0082 的四类 token。

| 版本 | 日期 | 变更 |
|---|---|---|
| 0.1 | 2026-10-29 | ADR-0101 / TASK-344：建立 form_read / form_fill / form_submit、diff、归因与一次审批契约 |

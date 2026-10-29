# ADR-0101　浏览器表单确认与提交契约

状态：**Accepted**（2026-10-29，人类确认 TASK-344 契约方案）　日期：2026-10-29　
Supersedes：—　Superseded by：—
关联：ADR-0048、ADR-0055、ADR-0060、ADR-0082、TASK-056、TASK-057、TASK-343、
`docs/spec/browser-form-confirmation.md`、`docs/spec/runtime-execution.md`

## 背景（为什么现在要决定）

TASK-343 已把 `edge|chrome.cdp.form_fill` 做成有界、唯一命中、写后逐字段回读且固定
`submitted=false` 的生产 handler。TASK-057 仍需完成“填写后展示 diff 与来源归因，
再由人类确认提交”的闭环，但现有工具集没有可信字段快照工具，也没有语义化的表单提交
工具；`ApprovalRequest`、UI approval card 与 audit metadata 之间也尚未冻结归因传递形状。

若直接给模型一个通用 `click`，或让 `form_fill` 自行选择并点击提交按钮，就会把
point-of-no-return 动作藏在普通写步骤里，无法证明“人类看到的是同一份表单状态”。因此
必须先冻结工具分层、diff 来源、归因投影、审批范围和提交后失败语义，再实现 TASK-057。

## 决策（一句话）

**T5.2 采用“只读快照 → 有界填写 → 字段级 diff + 已校验来源归因 → 高风险 once 审批 →
专用同源表单提交”的分阶段链路；`form_fill` 永不提交，模型可见工具集永不暴露通用点击。**

## 决策细化

| # | 内容 |
|---|---|
| **D1** | T5.2 的确定性基线只使用绑定到 `127.0.0.1` 的本地 loopback 表单 fixture，使用三个合成字段；外部站点、真实账号、真实个人信息、密码、文件上传、支付或验证码不得进入自动验收。 |
| **D2** | 保留既有 `edge / chrome.cdp.form_fill` 的职责与 TASK-343 输出合同。新增只读 `edge / chrome.cdp.form_read` 与高风险 `edge / chrome.cdp.form_submit`；三者必须分步，`form_fill` 不得调用 `submit` / `requestSubmit` / `click`。 |
| **D3** | `form_read` 只接受唯一 form selector 与 1..100 个唯一字段 selector，返回该表单已解析的 origin、action 和字段当前值。字段只允许文本类控件、textarea、select 与 contenteditable；password、file、hidden、button、submit、reset 等一律拒绝。selector ≤4 KiB，单值 ≤16 KiB，总值 ≤64 KiB。 |
| **D4** | diff 由 RuntimeExecutor 使用“`form_read` 的 before 快照 + `form_fill` 的有界请求值”构造 `hitl::DiffPreview::Fields`。after 值只能来自已验证的 `form_fill` readback；不得由模型摘要、页面可见文本或字符串猜测补齐。diff 值按 DLP 规则预脱敏。 |
| **D5** | 来源归因在 approval 步骤前由一个经校验的 `assistant_core::InstructionAttribution` 提供。`user_request` 不得携带额外归因元数据；`plan_derived` 必须有父目标；`app_content` / `tool_suggestion` 必须有来源引用。`app_content` 在审批 UI 中必须标红、默认拒绝并要求显式覆盖，且该覆盖不放宽 policy deny。 |
| **D6** | 新增 UI 事件 `step_approval_requested`，携带 task / step / action、结构化 diff、已校验归因、risk、reversibility、scope options、evidence 与 expiry。UI 对事件执行 strict parse；未知 token、缺字段或非法组合一律拒绝显示为可批准卡片。 |
| **D7** | `form_submit` 的 schema 固定为 `effect=write`、`risk_level=high`、`reversibility=l3_irreversible`、`requires_approval=true`、`idempotent=false`。Policy/HITL 只允许 `once`，plan UI 必须标注 point-of-no-return；不得产生 `this_task`、`this_app_session`、`persistent` 或无人值守授权。 |
| **D8** | `form_submit` 输入必须同时给出唯一 form selector、唯一 submit selector、预期 origin、预期 action 与预期字段状态。执行前必须在同一 DOM 表达式内重读并逐字段精确比较；任一项不匹配、歧义、跨 origin、非 http/https action、带 URL credentials、`javascript:`、新窗口 target 或状态超界时，必须在点击前 fail-closed。批准本身不是提交参数的替代品。 |
| **D9** | `form_submit` 只允许触发一个属于目标 form 的 `button[type=submit]` 或 `input[type=submit]`，每次调用最多点击一次。成功只能在观察到声明 postcondition 后返回；点击后若结果不确定，必须进入 `NeedsHuman` / `UserInteraction`，禁止自动重试或再次点击。 |
| **D10** | audit metadata 必须记录已校验的 `instruction_attribution`、审批 scope、diff 摘要与 form origin/action；不得记录密码、文件内容、Cookie、凭据或未脱敏字段值。审计写失败时不得报告提交成功。 |
| **D11** | 本 ADR 只冻结契约。新增工具 schema、UI IPC 事件、handler、fixture 与真实 Edge 人工验收必须由后续实现卡按本文落地；在实现卡完成前不得把 TASK-057 标为 Done。 |

## 工具边界

### `form_read`

- 只读，无副作用；输出按 untrusted 输入处理。
- 返回范围仅限显式请求的字段状态，不返回整页 DOM、Cookie、隐藏字段或页面脚本内容。
- 无匹配、歧义、超界或字段类型不允许时返回既有 `ToolInvalidArgs` / `TargetNotFound` /
  `TargetAmbiguous` 语义，不返回部分快照冒充成功。

### `form_fill`

- 继续遵守 TASK-343：全部字段预校验后才写入，写后逐字段 readback。
- 永不提交、永不点击；不增加隐式“填完顺手提交”参数。
- 输出保持 `filled_fields` 与 `submitted=false`。

### `form_submit`

- 输入：`form_selector`、`submit_selector`、`expected_origin`、`expected_action`、
  `expected_fields`。
- 输出：`submitted=true` 与提交后的 `post_url`；失败不得返回成功占位值。
- 提交前核对字段状态，提交后核对目标页是否出现确定性变化；不得用计时等待代替
  postcondition。

## 提交状态机

```text
read_before
  -> fill
  -> verify_readback
  -> prepare_field_diff + validate_attribution
  -> request_approval(once, point_of_no_return)
      -> denied / timed out / replay: submit calls = 0
      -> approved once: consume grant exactly once
          -> revalidate origin + action + field state
              -> mismatch: fail before click
              -> match: click exactly once
                  -> postcondition observed: submitted=true
                  -> result unknown: NeedsHuman, no retry
```

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 扩展 `form_fill`，填写后自动点击提交并用一个确认参数放行 | ❌ 否决 | 把普通 medium 写入与 L3 不可逆提交混成一个步骤，diff 与审批发生前字段状态可能已漂移，且无法独立证明提交调用次数为 0 或 1。 |
| 2 | 暴露通用 `edge.cdp.click`，由模型按 selector 或坐标点击 | ❌ 否决 | 通用点击无法约束同源、form ownership、提交控件类型和字段 TOCTOU；会把任意页面点击面扩大给模型。 |
| 3 | 使用 `form_read` + 既有 `form_fill` + 专用 `form_submit` 三段式（本 ADR） | ✅ 采纳 | 每段职责可独立验证；diff 有可信来源；审批只覆盖一次不可逆提交；无需通用点击或新平台 trait。 |

## 影响

- 新增 `docs/spec/browser-form-confirmation.md` 作为工具、diff、归因、UI 事件和负向测试
  的实现契约。
- `docs/spec/core-orchestration.md` 的 InstructionOrigin 边界改为“可经 ADR-0101 投影到
  审批事件 / audit metadata”，但仍不判定权限。
- 后续实现卡需要新增 Edge/Chrome tool schema、handler、registry、task package postconditions、
  UI IPC event、approval model 映射、loopback fixture 与 eval 验收。
- 不新增平台 trait、通用点击工具、第三方依赖或数据库 schema。
- TASK-057 在后续实现卡落地前保持 Blocked；TASK-344 不修改产品代码或可执行 schema。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 人类批准的是旧字段状态，点击时页面已被改为别的内容 | `form_submit` 在同一表达式内重读并精确比较预期字段、origin、action；不匹配在点击前失败。 |
| UI 展示 diff 后，调用方用未经校验的页面文本替换真实字段 | diff 只从 `form_read` 结果与已验证 readback 生成；页面文本、模型摘要不得充当字段值。 |
| 来源被错误默认成 `user_request` | 归因 token 采用封闭四元组并复用 `InstructionAttribution` 校验；未知 token / 元数据组合 fail-closed。 |
| `app_content` 覆盖或点击后状态未知被误当成放行 | 覆盖不放宽 policy deny；点击边界后 `NeedsHuman` / `UserInteraction`，禁止自动重试与重复点击。 |
| 新工具被复用于真实支付、密码或文件上传 | tool handler 对字段类型和 form 内容做白名单；外部真实站点不在 T5.2 自动验收范围，人工验收也必须遵守黑名单。 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. 规范门禁：`adr-index` 登记 0101；`docscan` / `refscan` / `card-check` / `check-ledger`
   全部 PASSED；实现卡必须覆盖正负合同：未批准、拒绝、超时、replay、跨 origin、歧义 selector、
   字段类型不允许、字段状态漂移、点击次数 0/1、点击后未知状态不自动重试。
2. 真实 Edge 人工验收只针对本地 loopback fixture；先完成离线 fixture / 脚本化 ToolBus，
   再执行人类在当前会话确认的单次提交。
3. **何时重新评估**：出现需要多步表单、跨源 OAuth、文件上传、支付、验证码、真实账号
   登录，或需要通过通用点击完成提交时，另立 ADR；不得扩展本契约的隐式参数绕过。

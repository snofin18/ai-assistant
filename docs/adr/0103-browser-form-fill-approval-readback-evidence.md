# ADR-0103：Browser form_fill approval readback evidence

状态：**Accepted**（2026-10-31；人类“继续下一步”授权按建议路线推进）
Supersedes：无
Superseded by：无
Amends：ADR-0101 D4 / `form_fill` 到 approval preview 的内部证据通道
关联：ADR-0101、ADR-0102、TASK-352、TASK-353、`DRIFT-352-1`
落点：`docs/spec/browser-form-confirmation.md`

## 背景：为什么需要新 ADR

`form_fill` 已在 handler 内完成逐字段写入、readback 和一致性校验，但 ToolBus 的成功
`data` 只暴露 `filled_fields` / `submitted` 与 fingerprint 元数据。adapter output schema
还是 `additionalProperties=false`，所以 Runtime 无法取得逐字段 before/after，也无法按
ADR-0101 D4 用“form_fill readback after”构造 approval diff。该缺口已记录为
`DRIFT-352-1`。

直接扩展模型可见 ToolOutput 会把字段原值暴露给模型；在 fill 后再做一次 `form_read`
又违反“after 只来自 form_fill readback”。两者都不采用。

## 决策：一句话

**保持 `form_fill` 公共 data/schema 不变，把成功填写后的逐字段 readback 作为审批专用
证据写入 `ToolEnvelope.evidence.extra` 的固定命名空间；Runtime 一次性消费并生成
approval preview，原始值永不进入模型、日志、审计、持久化或任务输出。**

## 决策细节

| # | 决策 |
|---|---|
| **D1** | 固定证据键为 `assistant.browser_form_fill.approval_readback.v1`。只允许出现在 `evidence.extra`，不得混入 `data`、`source`、`metrics` 或 `error.details`。 |
| **D2** | 证据值只含 `form_origin`、`form_action` 与 1..100 个 `fields[{selector,before,after}]`。禁止 requested、matched_by、DOM、title、Cookie、页面文本、模型摘要或凭据。 |
| **D3** | `form_fill` 必须要求每个字段都属于同一个 `HTMLFormElement`，并用该 form 的绝对 action 与当前 origin 生成证据；无 form、跨 form、无效 action 或超界时整次 fill 失败且不写证据。 |
| **D4** | 证据只在全部字段 readback 校验成功后附加；失败、部分写入、未知控件或超界一律无成功证据。公共 output schema 仍只有 `filled_fields` / `submitted` 的模型可见语义。 |
| **D5** | Runtime 对证据做严格解析：精确一个命名空间键、无未知字段、字段数/selector/value/总量沿用 4 KiB / 16 KiB / 64 KiB 边界、origin/action 仅 http/https。 |
| **D6** | approval preview 必须把证据与同一 `form_read` before 匹配：origin/action 相同、selector 顺序和数量一致、before 值与 before 快照一致。任一不一致 fail-closed。 |
| **D7** | 证据是一次调用、一次性消费的进程内数据。不得被序列化进 `BrowserTaskResult.outputs`、模型消息、日志、audit event、DB、错误详情或 replay 文件；消费后不得复用。 |
| **D8** | `FormApprovalPreview` 只能从 before 快照与上述证据构造；原始字段值不得进入 `DiffPreview::Fields`。当前最小 redactor 将每个非空值替换为 `[redacted:<utf8-bytes>]`，空值保留空串；未来文本 DLP 只能收紧或等价替换，不得放宽。 |
| **D9** | 证据缺失、重复、畸形、跨 form、origin/action 漂移、字段漂移或 redactor 失败时，不生成可批准 preview，也不请求 once grant。 |
| **D10** | 本 ADR 不改变 `form_submit` 的 write/high/L3/once/非幂等合同，不实现 UI `step_approval_requested`，不消费 grant，不启动真实浏览器。 |
| **D11** | 后续实现只可扩展 `browser_form_fill` 的内部证据构造、`browser_task_approval` 的 preview 和 `browser_task_runtime` 的消费/测试；不新增 Tool schema、protocol 字段、DB schema 或 ErrorCode。 |

## 数据形状

```json
{
  "assistant.browser_form_fill.approval_readback.v1": {
    "form_origin": "http://127.0.0.1:8766",
    "form_action": "http://127.0.0.1:8766/t5.2/submitted",
    "fields": [
      {
        "selector": "#display-name",
        "before": "",
        "after": "Ada"
      }
    ]
  }
}
```

该 JSON 只存在于 `ToolEnvelope.evidence.extra` 的内部生命周期中。`fields.before` /
`fields.after` 是 handler 实际 readback 的 untrusted 页面值，不是模型参数或摘要。

## 拒绝方案

| 方案 | 结论 | 理由 |
|---|---|---|
| 扩展 `form_fill` 公共 output data/schema | ❌ 否决 | 会把逐字段原值送入模型可见面，并扩大公共 schema；需要另一次完整契约迁移。 |
| fill 后再做一次 `form_read` 作为 after | ❌ 否决 | 违反 ADR-0101 D4 的“after 只来自 form_fill readback”，也增加新的 TOCTOU 窗口。 |
| 新增跨 crate 公共 readback API | ❌ 否决 | 只为 approval 增加公共 API，扩大攻击面；现有 evidence envelope 已提供受限内部通道。 |
| 在 evidence 中携带 DOM/title/页面文本 | ❌ 否决 | 超出审批最小字段集，增加敏感数据泄露和持久化风险。 |

## 影响

- `form_fill` 行为收紧为“所有字段属于同一 form”，否则失败。
- `browser_task_runtime` 可以读取 evidence，但不得把 evidence 放进 outputs。
- `browser_task_approval` 可据此生成 redacted field diff 和归因预览。
- 不需要修改 protocol schema、ToolOutput data、adapter pack、DB 或 ErrorCode。
- TASK-352 从契约阻塞恢复为 Ready；实现仍需独立验收。

## 验证方式

1. 构造器专项测试覆盖证据形状、跨 form、origin/action 漂移、selector 顺序、重复/缺失证据和超界。
2. 断言 `BrowserTaskResult.outputs` 只包含 data 派生值，不含 evidence 命名空间。
3. 断言 `DiffPreview::Fields` 只含 redacted 值，原始 before/after 不出现。
4. 文档门禁：`adr-index`、`refscan`、`docscan`、`card-check`、`check-ledger` 全 PASSED。

## 何时重新讨论

需要把 approval readback 持久化、跨进程传递、展示真实字段明文、扩展支持字段类型或引入
通用 evidence 数据通道时，必须重新提案；不得把本 ADR 当作扩大 evidence 用途的通用授权。

# assistant-hitl

## 职责

`assistant-hitl` 把“策略要求人工确认”转换成可验证的审批请求，并提供：

- 四维授权范围（subject / tool / target / effect）+ TTL + 有限使用次数；
- `once` / `this_step_pattern` / `this_task` / `this_app_session` / `persistent`
  五种授权范围的确定匹配；
- 审批批准、拒绝与超时结果；
- 用户接管基线与交还后的指纹重同步报告；
- 暂停 / 恢复对 `assistant-task-engine` 显式状态迁移的协调；
- 文本、字段、文件、UI 步骤与不可逆动作的差异预览数据。

## 边界（不做什么）

- 不执行工具、不获取租约、不创建撤销锚点、不验证后置条件。
- 不调用平台 API，不检测真实用户输入，不渲染 UI。
- 不读取系统时间；审批 TTL 与接管时刻都由调用方注入。
- 不持久化审批或授权；本 crate 只提供确定性的领域对象与转换。
- 不新增策略规则：policy 仍是唯一放行点；deny 不能借 `hitl` 变成审批。

## 不变量

1. **确认判据唯一**：只有 ADR-0048 的 `allow = true` + 非空 `scope_options`
   可以创建审批请求；无条件 allow 与 deny 都必须明确拒绝。
2. **高风险收敛**：`High` / `Critical` 或 `L3Irreversible` 只允许 `once`，
   且 `max_uses` 必须为 1；禁用范围不得静默删除或降级。
3. **授权不可扩权**：批准的 scope 必须属于请求声明，TTL 不得大于请求上限，
   使用次数不得大于请求上限。
4. **TTL 硬边界**：`now_ms >= expires_at_ms` 一律过期；时间回退返回结构化错误。
5. **接管不猜测**：交还后必须重新解析目标、重算指纹，并比较接管前后指纹；
   即使指纹相同也不能直接假设状态未变化。
6. **diff 不静默截断**：超过显式预算返回错误，UI 层不得收到伪装完整的局部 diff。

## 已知限制

- 审批状态、授权列表与撤销历史尚未持久化；持久化与策略面板由后续卡接线。
- UI 渲染、i18n、来源归因与 DLP 脱敏不属于本 crate。
- 真实接管检测由平台层提供；本 crate 只接收调用方给出的接管事件与指纹。
- 审批响应暂不支持“修改参数后重试”；该交互可由后续卡在相同请求模型上扩展。

## 相关文档

- `cross-platform-ai-assistant-architecture-v2.md` §8.4 / §10；
- `docs/spec/audit-event.md`（ADR-0048）；
- `tasks/TASK-027-hitl-approval-scope-takeover.md`。

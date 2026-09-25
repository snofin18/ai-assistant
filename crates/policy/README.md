# assistant-policy

TASK-021 的**唯一策略放行点**：用纯函数表达白名单、风险分级、默认拒绝与
参数校验。输入是不可信模型参数和已解析的执行上下文，输出是显式策略决策。

## 职责

- 用 DSL v0 表达架构 v2 §12.2 的五条示例规则，并对规则集做结构校验。
- 对 `EvaluationContext` 执行 deny 优先的确定性判定；无命中时返回 `default_deny`。
- 提供路径、URL、文本、数值和正则复杂度的纯函数护栏。
- 把完整的 `Decision` 投影为 `assistant_protocol::PolicyDecision` 供审计使用。

## 边界（不做什么）

- **不执行动作、不做 IO**：不读文件、不解析 DNS、不发网络请求、不读取时钟或随机数。
- **不做审批流程**：只返回 `AllowWithConfirmation`；审批请求、授权范围与用户接管归
  TASK-027 的 `crates/hitl`。
- **不做污点传播或 DLP**：只读取上下文的 `tainted` / `egress` 字段。
- **不做持久化、热加载或 UI 编辑**：规则集由调用方在内存中构造或从 JSON 载入。
- **不替代平台解析**：路径的符号链接解析结果必须由调用方作为输入提供。

## 不变量

1. **默认拒绝**：空规则集、无规则命中或解析失败都不得产生隐式放行；加载失败必须由
   调用方映射为拒绝。
2. **安全底线不可绕过**：`L3Irreversible + unattended` 无条件拒绝；污点上下文中的
   `L3Irreversible`、`High`、`Critical` 动作无条件拒绝，自定义规则不能覆盖。
3. **deny 优先**：任一命中的 deny 规则压过所有 allow / confirmation 规则。
4. **判定是纯函数**：同样输入重复求值必须得到同样输出；判定路径没有 IO、时钟、随机数
   或全局可变状态。
5. **不支持的语法一律拒绝**：DSL 未知键、URL 特殊 authority、路径编码穿越、
   正则 lookaround / 回引 / 嵌套量词都直接失败，绝不“不认识就放行”。
6. **每个拒绝可解释**：deny 决策一定包含非空 `rule_id` 与非空可读 `reason`。

## DSL v0

DSL v0 使用 JSON（不引入 TOML 依赖）。所有五条架构示例规则如下：

```json
{
  "default": { "effect": "deny" },
  "rules": [
    {
      "id": "allow_read_low_risk",
      "when": { "effect": "read", "risk_level": ["low"] },
      "effect": "allow"
    },
    {
      "id": "confirm_medium_write",
      "when": {
        "effect": "write",
        "risk_level": ["medium"],
        "reversibility": ["L0_undo_stack", "L1_snapshot", "L2_compensating"]
      },
      "effect": "allow_with_confirmation",
      "confirmation": { "scope_options": ["once", "this_task"], "show_diff": true }
    },
    {
      "id": "block_irreversible_unattended",
      "when": { "reversibility": ["L3_irreversible"], "unattended": true },
      "effect": "deny",
      "reason": "irreversible actions are forbidden while unattended"
    },
    {
      "id": "block_after_untrusted_content",
      "when": { "tainted": true, "risk_level": ["high"] },
      "effect": "deny",
      "reason": "high-risk actions are forbidden after untrusted content"
    },
    {
      "id": "deny_sensitive_egress",
      "when": { "target_app": ["hr_system", "finance"], "egress": "cloud_model" },
      "effect": "deny",
      "reason": "sensitive application content must not leave the machine"
    }
  ]
}
```

## 已知限制

- `Reversibility` 与 `Effect` 目前唯一定义在本 crate，沿用架构 v2 §9.1 与资源操作分类；
  待 TASK-024 决定是否收敛为协议生成类型。
- `assistant_protocol::PolicyDecision` 只有 `allow` / `rule_id` / `reason`，无法承载
  `AllowWithConfirmation` 的 `scope_options` / `show_diff`。本 crate 保留完整决策，
  协议投影是明确的审计摘要；扩展协议需走 ADR（`DRIFT-021-1`）。
- URL 校验器是保守子集，不接受 IPv6、userinfo、fragment 或 non-ASCII host；未支持的
  合法 URL 形态会 fail-closed。
- 正则校验器不执行匹配，只对静态结构做保守判据；它不是完整正则语法检查器，无法证明
  任意允许模式一定线性时间。
- 路径校验要求调用方提供平台解析后的绝对路径；未提供或提供错误解析结果时，本 crate
  无法单独发现符号链接逃逸。

## 相关文档

`cross-platform-ai-assistant-architecture-v2.md` §8.7 / §9.1 / §12.2 / §12.3 / §12.4、
`docs/spec/error-codes.md`、`docs/spec/capability-matrix.md`、
`docs/spec/tool-schema.md`、`tasks/TASK-021-policy-whitelist-risk-default-deny.md`。

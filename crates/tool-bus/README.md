# assistant-tool-bus

TASK-020 的**工具通道**：in-process MCP server + client（`rmcp`）、JSON Schema 输入校验、
统一返回信封、工具集指纹与动态挂载。

## 职责

- 用 MCP（`tools/list` / `tools/call`）表达内置工具：Agent Core 是 MCP client，内置工具是
  **同进程**的 MCP server（架构 v2 §5.4 —— MCP 是唯一工具协议）。
- 按 `ToolSchema.input` 做 draft-07 **子集**校验；不合法 → `ErrorCode::ToolInvalidArgs`，
  **不进** handler（铁律 1 / 4）。
- 所有返回一律走 `assistant_protocol::ToolEnvelope`：`ok` / `tool` / `task_id` / `step_id` /
  `data` / `untrusted` / `source` / `truncated` / `evidence` / `metrics` / `error` 字段齐全
  （架构 v2 §5.3）。
- 超 `max_bytes` 时按契约截断并显式标注 `truncated.occurred = true` + `reason` +
  `original_bytes`；截不动就让调用**失败**（fail-closed），绝不静默放过超限载荷。
- 计算工具集指纹（SHA-256，与挂载顺序无关），提供 `toolset.list` / `toolset.search` 两个
  元工具，单次挂载 > 40 个工具时产出结构化告警 + 未串链审计事件（架构 v2 §5.5）。

## 边界（不做什么）

- **不做任何放行判断**（铁律 3）：白名单 / 风险分级 / 默认拒绝 / 审批决策全部归
  `crates/policy`（TASK-021）。
- 不接真实 Adapter 工具（TASK-035 起）；本 crate 只用测试用内置工具证明链路可用。
- 不加载 / 不治理外部 MCP server（stdio、streamable-http 一律没有接入，属阶段 3）。
- 不做结果缓存、不做 token 预算、不写审计库（审计链由 `crates/audit` 持有）。
- **不自建第二套**信封 / 错误码 / 工具 schema：全部复用 `assistant_protocol`（铁律 10）。

## 不变量

1. **参数校验发生在 handler 之前**：不符合 `ToolSchema.input` 的调用直接返回
   `ErrorCode::ToolInvalidArgs`，**永不进入 handler**。
2. **只要到达 handler，返回路径上一定有信封**：成功、工具失败、参数非法三种结果都是同一个
   `assistant_protocol::ToolEnvelope` 形状。
3. **截断不静默**：载荷超 `max_bytes` 一定带 `truncated.occurred = true` + `reason` +
   `original_bytes`；无法按契约截断时让调用**失败**，绝不放过超限载荷。
4. **指纹只由「真正挂载的工具」决定**：同一集合任意挂载顺序 → 同一指纹；集合里任何一个
   字节变化 → 指纹变化。
5. **单次挂载的工具数 > 40 必须留下告警**：挂载报告里带结构化 `ToolsetOversizeWarning` +
   未串链的 `AuditEvent`，禁止只打一行日志。
6. 零 `unsafe`、零 `#[allow]`（`tests/` 由测试文件自行声明例外，依据 AGENTS.md §5.3）。

## 已知限制

- **JSON Schema 只实现 draft-07 的子集**（`SUPPORTED_KEYWORDS`，21 条）：`type`（含类型数组）/
  `enum` / `const` / `properties` / `required` / `additionalProperties` / `minProperties` /
  `maxProperties` / `items`（**仅**单 schema 形式）/ `minItems` / `maxItems` / `minLength` /
  `maxLength` / `minimum` / `maximum` / `exclusiveMinimum` / `exclusiveMaximum` / `allOf` /
  `anyOf` / `oneOf` / `not`。
- **判据方向 = 「不支持即拒绝」（fail-closed），且拒绝必须可读**：拒绝消息带**标签 + 一句理由 +
  schema 文档内 JSON pointer**（如 `/properties/name`），四类**可区分**、各有机器可读入口：
  - **永久放弃** `REJECTED_FOREVER_KEYWORDS`（12 条）—— `$ref` / `definitions` / `pattern` /
    `patternProperties` / `format` / `if`-`then`-`else` / `dependencies` / `additionalItems` /
    `contentEncoding` / `contentMediaType`；理由 + 替代方案见该表与 `docs/memory/rejected.md`
    （替代一律是「`enum` / `const` 收窄取值」或「**由 handler 校验值**」，见本条最后一段）
  - **尚未实现** `REJECTED_FOR_NOW_KEYWORDS`（4 条）—— `multipleOf` / `uniqueItems` /
    `contains` / `propertyNames`：都是**纯断言、零依赖**，缺的只是实现（可以有卡）
  - **其它方言** `NON_DRAFT07_KEYWORDS`（15 条）—— 2019-09 / 2020-12 的关键字
    （`prefixItems` / `$defs` / `unevaluated*` / `dependent*` / `minContains` …）→ 请降到 draft-07
  - **完全不认识** —— 兜底拒绝，提示这是拼写错误而不是「永久放弃」
- **纯注解关键字忽略但不拒绝**（`ANNOTATION_KEYWORDS`，9 条）：`title` / `description` /
  `default` / `examples` / `$comment` / `deprecated` / `readOnly` / `writeOnly` / `$id`。
- **`$schema` 不是注解**：缺省 或 draft-07 = 放行，声明**别的方言 = 拒绝** —— 否则我们会用
  draft-07 的语义去校验一份 2020-12 的文档（`items` 在 2020-12 里才是单 schema 形式）。
- **校验器管形状，handler 管值的安全**：本 crate **不**做注入防护（不拼 shell、参数化查询、
  路径规范化都归 handler / policy）。把语言关键字或 SQL 注入词做进关键字黑名单既不合规范
  （JSON Schema 关键字是固定闭集）也是 **fail-open** → 已否决（`docs/memory/rejected.md`）。
- **`assistant_protocol` 的生成类型是 `#[non_exhaustive]` 且多数没有构造器**（`Source` /
  `Truncation` / `Evidence` / `Metrics` / `EnvelopeError` / `AuditEvent` / `ToolSchema`）→
  信封、工具声明、超限审计事件一律经 serde 组装（`DRIFT-020-3` / `PL-078`）。
- **in-process only**：两边跑在**同一进程、同一 tokio 运行时**，传输是
  `tokio::io::duplex` 的内存管道（无 socket、无子进程、无网络）。外部 MCP server 的
  stdio / streamable-http 传输未接入（阶段 3）。
- **不做响应缓存**：`rmcp` client 的响应缓存默认关闭（打开会让 `tools/list` 看到上一次的
  清单，直接破坏「指纹反映当下」这条不变量）。
- **元工具名是两段式例外**（`toolset.list` / `toolset.search`），与
  `docs/spec/tool-schema.md` §4 不变量 1 的 `<app>.<domain>.<action>` 三段式冲突 →
  `DRIFT-020-4` / `PL-080`；例外是**闭集**，其余非三段式名字一律拒绝。
- 本 crate **不做任何放行判断**：`call_tool` 不会因为工具风险级而拒绝调用（铁律 3）。

相关：架构 v2 §5.2 / §5.3 / §5.4 / §5.5 / §11.4 / §12.4、`docs/spec/tool-schema.md`、
`docs/spec/envelope.md`、`docs/spec/error-codes.md`、`docs/spec/audit-event.md`、
`tasks/TASK-020-tool-bus-mcp-rmcp-server.md`。

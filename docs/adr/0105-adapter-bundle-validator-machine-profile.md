# ADR-0105：Adapter bundle validator 机器契约

状态：**Accepted**（2026-11-18，人类要求按 TASK-381 建议继续下一步）
日期：2026-11-18
Clarifies：ADR-0104
Supersedes：无
Superseded by：无
关联：ADR-0021、ADR-0047、ADR-0055、ADR-0062、ADR-0095、TASK-380、TASK-381、
TASK-383
落点：`docs/spec/adapter-bundle.md`、`docs/spec/tool-schema.md`、
`docs/spec/capability-matrix.md`、`docs/spec/naming.md`

## 背景

ADR-0104 已批准 Adapter 六件套的目标 owner、安全语义和拒绝类别，但 TASK-381 证明其中仍缺少
validator 直接实现所需的机器契约：App Map projection 没有可重放生成输入，selector JSON 与
运行时模型之间没有完整 mapping，Tool postcondition/ErrorCode/capability 没有生成目录入口，
rollback anchor 与 interrupt action 没有封闭对象联合。

如果直接从现有四套 pack 提取形状，会把迁移中的临时字段固化成新契约；如果由 validator 内置
第三份枚举，又会偏离既有 Verify/ErrorCode/Capability 单一事实源。因此需要一个只澄清机器形状、
不改变 ADR-0104 安全决策的补充 ADR。

## 决策

一句话：**ADR-0104 的 D1-D6 语义保持不变；validator 只消费
`docs/spec/adapter-bundle.md` 定义的 canonical JSON、typed references、generated catalogs 和
封闭判别联合，任何缺字段、未知字段、悬空引用、错误 tag、重复键/ID 或歧义优先级均失败。**

### D1：canonical parse 与路径

- validator 必须拒绝重复 JSON object key、非法 UTF-8、未知版本、绝对路径、`.`、`..`、
  重复规范化目标、canonicalize 后越界和同一文件占用多个 bundle slot。
- 路径比较使用 canonical filesystem target；finding 中仍保留 authored relative path。
- 整数字段不接受浮点数或字符串强转；布尔字段不接受 `0/1`。

### D2：作者维护 projection manifest，生成器只做确定性投影

- 富 `app_map.json` 新增必填顶层 `projection_manifest`。
- manifest 的 `entries[]` 是 projection 内容的唯一作者输入。生成器不运行模型、不按标题猜测、
  不自动摘要，只复制、排序、计算 token estimate 并序列化。
- 每个 entry 必须含 `id/title/content/source_reference/token_estimate`；每个 source reference
  必须可解析到 bundle 或 `docs/memory/apps/**` 的明确锚点。
- 富 App Map 除 `schema_version/app_id/version_range/projection_manifest` 外的每个顶层知识域，
  必须至少被一个 entry source reference 覆盖，或在 `omissions[]` 中给出非空原因；否则失败。
- projection 固定为 `version/app_id/entries`。entries 按 `id` UTF-8 字节序升序；
  `token_estimate = max(1, ceil(content Unicode scalar count / 4))`。
- generator 输出 UTF-8 无 BOM、2 空格缩进、LF、固定对象键序、无尾随空白、单个末尾 LF；
  validator 重新生成后与已提交 projection 逐字节比较。

### D3：selector JSON 与运行时模型一一 mapping

- `uia_candidate_chain_v1` 的 target 只允许 `scope=window|element`；candidate typed value 的
  discriminator 必须与 `kind` 同名。未知 kind、错配 value、重复 candidate/target id 均失败。
- UIA policy 使用结构化 `on_not_found`，映射到运行时 `OnNotFound`；歧义固定
  `error_and_ask`，不得配置降级。
- 文本/结构路径类 candidate 必须 `locale_dependent=true` 且 `fallback_only=true`；
  primary/fallback 先分组，再按 effective score 降序、id 字节序升序。
- `allow_fallback_entry=true` 仅在该 target 至少一个 candidate 且全部为 fallback 时允许。
- `shell_target_v1` 不使用 selector policy 或 UIA candidate；role、class_name、automation_id、
  supports、probe_status 使用 spec 的闭集，且不承载 DOM/CSS/XPath/JavaScript identity。

### D4：共享 catalog 是 validator 的唯一枚举来源

- bundle 接受 ToolSchema 1.0 与 1.1 的迁移窗口；`none_readonly` 仅允许 ToolSchema 1.1 +
  `effect=read`。
- Capability ID 只从 `protocol/capability-matrix/capability-1.0.json` 读取；ErrorCode 只从
  `protocol/error-codes/error-codes-1.0.json` 读取。
- Postcondition 的 kind/字段/类型由现有 assertion 契约生成 catalog；validator 不得手写第二套
  kind 列表。所有 envelope postcondition 必须能映射到 catalog kind，并显式携带合法 ErrorCode。
- read tool 必须至少有一个可执行 `state_unchanged`；write tool 必须至少有一个非
  `state_unchanged` 的可执行 postcondition。空数组、重复项和仅描述性字符串失败。
- envelope 必须 `additionalProperties=false`；`requires_approval/idempotent/tags` 在 Adapter
  声明中必须显式写出，不能依赖 schema 默认值。

### D5：rollback coverage 使用 capture/restore/post-verify 判别联合

- 每个 write tool 恰有一行 coverage；read tool 不得有 coverage。
- `recipe` 行必须含非空 ordered `anchor_strategy`。每个 anchor 精确为
  `level/mechanism/capture/restore/post_verify`。
- `L0` 只用 `mechanism=undo_stack`；`L1` 只用 `content_snapshot|shadow_copy`；
  `L2` 只用 `mechanism=compensating_action`。
- `capture` 和 `restore` 必须引用 declared tool 或 spec 列出的 builtin；只写自然语言字符串失败。
- 每个 anchor 的 `post_verify` 必须是 canonical Postcondition；失败必须发布 incident 并要求
  人工处理。
- `L3` 只允许 `no_recovery` + 非空 reason，且 tool 必须 high/critical、
  `requires_approval=true`、不得 unattended。
- L0 可按数组顺序附带 L1 fallback；L1/L2 不得伪装成其他 level 的恢复方式。

### D6：interrupt grammar 使用显式 priority 和 typed action

- 每条 interrupt 必须有唯一 id、正整数 priority、非空 match、typed action、classification、
  requires_human；人工动作必须声明 resume_after。
- priority 降序决定首个规则；priority 相同且 match 可同时命中即歧义失败，不按文件顺序猜。
- `title_candidates/name_candidates` 每项必须显式 `{value, locale_dependent:true}`，且文本不能
  成为唯一 match；还必须同时存在 class/automation 类非文本条件。
- action 只允许 `cancel/dismiss_explicitly/cancel_unless_declared_by_tool/delegate_to_tool/
  ask_human` 的 typed object；delegate 与 accept 集合中的 tool 必须解析到同一 bundle。
- unsaved three-way、credential、captcha、consent、overwrite、untrusted input 和
  unsupported network feature 必须 `requires_human=true`。
- 固定 sleep、等待自行消失、危险 save/discard/replace/accept fallback 与历史
  `delegate_to_task_*` 永久拒绝。

## 兼容与实施边界

- 本 ADR 不修改 ToolSchema JSON Schema、generated Rust/TS、runtime loader、Policy 或现有 pack。
- 下一张 validator 卡必须先落 canonical parser、catalog generation 和 report-only findings，
  再对现有四包输出缺口；不得自动改写 pack。
- 现有 pack 在新 validator 接入前保持原状；迁移由 Notepad/Paint 与 Edge/Chrome 两张独立卡完成。

## 后果

正面：

- Projection 不再需要模型总结或启发式抽取。
- selector、rollback、interrupt 都有可枚举、可做负向测试的封闭对象。
- capability、ErrorCode、postcondition 只有一个生成/引用入口。
- validator 与现有 pack 迁移之间的责任边界可机器审计。

代价：

- 所有现有 `adapter.toml` 仍需补 `[bundle]`；富 App Map 需补 projection manifest。
- 现有 selector policy、tool postcondition 标签、rollback 与 interrupt 形状需迁移。
- generated catalog 和相关 codegen 会在下一张实现卡中扩大规模，必须继续使用版本窗口。

## 验证方式

1. `docs/spec/adapter-bundle.md` 对 D1-D6 均给出 exact fields、closed values、mapping 和拒绝条件。
2. ADR 登记、`docs/memory/decisions.md` 与 ADR 文件一致。
3. 文档门禁全绿，`git diff` 不包含 `protocol/**`、`adapters/**`、`crates/**`、`apps/**`、
   CI 或依赖。
4. 下一张 validator 卡的测试必须覆盖 spec 第 10 节的完整负向矩阵。

## 何时重新讨论

新增 selector model、改变 projection manifest 的权威性、允许无 manifest 的语义摘要、增加
interrupt action/classification、扩大 ToolSchema 兼容窗口、改变 postcondition catalog 来源，
或允许 `no_recovery` 用于 L0-L2 时，必须新 ADR。

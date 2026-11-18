# spec: Adapter bundle validator 机器契约

> 摘要：定义 `adapter.toml` 六件套在 validator 中的 canonical JSON、引用、判别联合和
> 运行时 mapping。
> 状态：Accepted，ADR-0105 澄清 ADR-0104，版本：1.0，日期：2026-11-18。
> 上位：`AGENTS.md` §2/§5、ADR-0047、ADR-0055、ADR-0062、ADR-0095、ADR-0104、ADR-0105。
> 强制性：违反本文 = validator finding + merge block；实现代码仍由后续卡交付。

---

## 1. 范围与术语

本文只定义机器可实现的 Adapter bundle 方言和 mapping，不改变 ADR-0104 的安全决策。下列词均为
规范词：

- **MUST**：不满足即 fail-closed。
- **MUST NOT**：出现即拒绝。
- **MAY**：只有本文列出的可选字段。
- **finding**：validator 对 authored path、稳定 code 和 message 的只读报告，不自动改写文件。

`bundle root` 是 `adapter.toml` 所在目录。所有 JSON 文件按 UTF-8 解析；BOM 只允许出现在输入
读取层并被剥离，canonical byte comparison 永远不包含 BOM。

## 2. Canonical JSON 与引用

### 2.1 解析规则

- MUST reject duplicate object keys，不能采用“后者覆盖前者”。
- MUST reject trailing comma、NaN、Infinity、注释和非法 UTF-8。
- 整数字段只接受 JSON integer；布尔字段只接受 `true/false`。
- schema object 一律 `additionalProperties=false`；未知字段必须报告 authored JSON pointer。
- 多个文本字段的内容 MUST reject NUL；generated projection 另外拒绝 `\r` 和首尾空白。
- authored JSON 的对象键序不参与语义；只有本文明确要求 byte comparison 的 generated 文件
  才使用固定键序。

### 2.2 引用语法

`source_reference` MUST 使用以下之一：

```text
bundle:app_map#<json-pointer>
bundle:selectors#<json-pointer>
bundle:tools#<json-pointer>
bundle:rollback#<json-pointer>
bundle:interrupts#<json-pointer>
memory:docs/memory/apps/<app>.md#<anchor>
```

`<json-pointer>` 为空或以 `/` 开头；解析后必须存在且指向非 null 值。`memory` 引用只允许
`docs/memory/apps/<app>.md`，anchor 必须非空。未知 scheme、绝对文件路径、`..` 或悬空引用拒绝。

## 3. D1 Bundle envelope

`adapter.toml` MUST 含：

```toml
schema_version = 1

[adapter]
app_id = "com.vendor.app"

[bundle]
app_map = "app_map.json"
app_map_projection = "memory/app_map.v1.json"
app_map_projection_model = "core_app_map_v1"
selectors = "selectors/targets.json"
tools = "tools/tools.json"
rollback = "rollback/recipes.json"
interrupts = "interrupts/interrupts.json"
```

`[bundle]` MUST 恰好包含七个键：六个 path slot 和 `app_map_projection_model`。七个 path 值均相对
bundle root，使用 `/`，不得为空、绝对或包含 `.`/`..` segment。六个 target MUST:

1. 存在、可读、可解析；
2. canonical filesystem target 唯一且位于 root 内；
3. 顶层 `app_id` 与 `[adapter].app_id` 逐字相等；
4. 使用本文要求的 version 字段。

validator 先完成路径 containment，再读取 JSON；任何路径失败时不得继续调用目标文件解析器。

## 4. D2 App Map projection

### 4.1 富 App Map

`app_map.json` MUST 含：

```json
{
  "schema_version": 1,
  "app_id": "com.vendor.app",
  "version_range": ">=1 <2",
  "projection_manifest": {},
  "identity": {},
  "commands": [],
  "ui_map": {},
  "workflows": [],
  "known_pitfalls": [],
  "state_machine": {},
  "undo_capability": {}
}
```

应用可按已有知识模型增加其他顶层域，但每个域都要被 projection entry 的 source reference 覆盖，
或列入 omissions。

### 4.2 Projection manifest

`projection_manifest` MUST 恰好包含：

```json
{
  "model": "core_app_map_v1",
  "entries": [
    {
      "id": "identity",
      "title": "Notepad identity",
      "content": "One-line canonical knowledge.",
      "source_reference": "bundle:app_map#/identity",
      "token_estimate": 12
    }
  ],
  "omissions": [
    {
      "source_reference": "bundle:app_map#/commands",
      "reason": "Commands are executable declarations and do not enter core memory."
    }
  ]
}
```

规则：

- `model` MUST 为 `core_app_map_v1`。
- `entries` MUST 非空；`id` 全局唯一，匹配 `^[a-z][a-z0-9_]*$`。
- `title/content` MUST 非空、单行、无首尾空白、无 NUL/CR/LF。
- `content` MUST 是作者维护的 projection 内容；MUST NOT 由模型运行时摘要或按关键词猜测。
- 每个 entry 至少一个 `source_reference`；本文为简化 JSON 用一个字符串，但 MUST 命中引用语法。
- omissions 可为空；每项 MUST 有可解析 source_reference 和非空 reason。
- entries 和 omissions 的 source coverage 合并后，MUST 覆盖富 App Map 的每个知识域顶层键；
  例外只有 `schema_version/app_id/version_range/projection_manifest`。

### 4.3 确定性生成

generator 只执行以下步骤：

1. 从 `projection_manifest.entries` 复制五个字段；
2. 计算 `token_estimate = max(1, ceil(Unicode scalar value count(content) / 4))`；
3. 按 `id` UTF-8 byte order 升序稳定排序；
4. 生成 `version=1`、root `app_id` 和 entries；
5. 按 §4.4 序列化并比较字节。

manifest 中 authored token estimate MUST 等于计算值。重复 id 或 byte-tie 排序歧义拒绝。

### 4.4 Canonical projection bytes

projection MUST 精确使用以下对象键序：

```text
root: version, app_id, entries
entry: id, title, content, source_reference, token_estimate
```

序列化规则：

- UTF-8 无 BOM；
- 2 空格缩进；
- LF 换行；
- `:` 后一个空格，`,` 后换行，无尾随空格；
- `/` 与 Unicode 字符不转义；只有 JSON 必需的 quote/backslash/control 才转义；
- 文件末尾恰好一个 LF。

validator 从富 App Map 重新生成 canonical bytes，与已提交 projection 做长度和逐字节比较。
任何差异产生 `PROJECTION_BYTE_DRIFT`。omission 是 manifest 的合法知识声明，不进入 projection
文件；validator 在 finding/report 中按 source_reference 排序输出。

## 5. D3 Selector models

### 5.1 公共闭集

`probe_status` MUST 为 `observed|required|provisional`。

`role` MUST 为以下 UIA/ARIA 风格的 canonical 名称：

```text
Button CheckBox ComboBox Dialog Document Edit Group List Menu MenuBar
MenuItem Pane RadioButton ScrollBar Slider SplitButton Tab Text ToolBar
Tree TreeItem Window
```

新增 role 需要 ADR。`supports` MUST 去重、按 UTF-8 byte order 升序，shell target 每项只能为：

```text
foreground inspect read_text
```

### 5.2 `uia_candidate_chain_v1`

顶层 MUST 恰好包含：

```json
{
  "schema_version": 1,
  "app_id": "com.vendor.app",
  "selector_model": "uia_candidate_chain_v1",
  "app_version_range": ">=1 <2",
  "probe_status": "observed",
  "policy": {
    "on_ambiguous": "error_and_ask",
    "on_not_found": {
      "retry_after_ms": [100, 500],
      "then_escalate": true
    },
    "min_score_to_try": 0.1,
    "max_resolve_ms": 3000
  },
  "targets": []
}
```

- `on_ambiguous` 唯一值 `error_and_ask`。
- `retry_after_ms` 每项 >0，严格递增，最多 5 项；空数组合法。
- `min_score_to_try ∈ [0,1]`；`max_resolve_ms >0`。
- `probe_status` 表示整个 selector pack 的最新证据状态。

target MUST 恰好包含 `id/scope/probe_status/allow_fallback_entry/candidates`：

- `id` 在文件内唯一，匹配 `^[a-z][a-z0-9_]*$`。
- `scope` 为 `window|element`。
- `allow_fallback_entry` 必须显式 bool。
- candidates 非空，candidate id 在 target 内唯一。

candidate MUST 恰好包含：

```json
{
  "id": "primary",
  "kind": "automation_id",
  "value": {"text": "Editor"},
  "score": 0.9,
  "locale_dependent": false,
  "fallback_only": false,
  "ttl_ms": 0
}
```

`kind` 与 value discriminator MUST 一致：

| kind | value object | locale_dependent |
|---|---|---|
| `runtime_id` | `{"text": "..."}` | `false` |
| `automation_id` | `{"text": "..."}` | `false` |
| `ax_identifier` | `{"text": "..."}` | `false` |
| `class_and_role` | `{"class_and_role":{"class":"...","role":"<role>"}}` | `false` |
| `exact_name_and_role` | `{"exact_name_and_role":{"name":"...","role":"<role>"}}` | `true` |
| `role_and_parent` | `{"role_and_parent":{"role":"<role>","parent_id":"<candidate-id>"}}` | `false` |
| `a11y_path` | `{"path":["..."]}` | `true` |
| `title_regex` | `{"text": "..."}` | `true` |
| `name_regex` | `{"text": "..."}` | `true` |
| `exact_name` | `{"text": "..."}` | `true` |
| `visual_anchor` | `{"visual_anchor":{"ocr_text":"...","region":"..."}}` | `true` |

文本和结构路径类（`exact_name_and_role/a11y_path/title_regex/name_regex/exact_name/visual_anchor`）
MUST `locale_dependent=true` 且 `fallback_only=true`。`role_and_parent.parent_id` MUST 同位 target
内更早或同组存在的 candidate id；循环引用拒绝。`runtime_id` MUST 显式 `ttl_ms=0`；其他 kind
的 `ttl_ms` 可省略，出现时 MUST >0。`score` 为有限数且在 `[0,1]`。

`allow_fallback_entry=true` MUST 表示 candidates 全部 fallback；否则拒绝。若 false，则 MUST
至少一个 primary。

### 5.3 UIA runtime mapping

validator/report 必须能按以下确定规则生成 `TargetDescriptor` 预览：

1. `scope=window` 的 candidates 进入 `window_candidates`；`element` 进入
   `element_candidates`；
2. candidate 的 `kind/value/score/locale_dependent/fallback_only/ttl_ms` 字段原样映射；
3. `policy` 映射到 `ResolutionPolicy`；`on_ambiguous=error_and_ask` 映射唯一运行时变体；
4. primary/fallback 分组后，每组按 effective score 降序、candidate id byte order 升序。

mapping 只允许复制和规范化排序，不得推断 unstated candidate。

### 5.4 `shell_target_v1`

顶层 MUST 恰好包含：

```json
{
  "schema_version": 1,
  "app_id": "com.vendor.browser",
  "selector_model": "shell_target_v1",
  "probe_status": "provisional",
  "targets": []
}
```

target MUST 恰好包含 `id/role/probe_status/supports` 及可选
`class_name/automation_id`。`id` 唯一；role/supports 使用 §5.1 闭集；target 至少有一个
`class_name|automation_id` 非文本 identity。`title/name/text` 和 DOM/CSS/XPath/JavaScript
identity 字段 MUST NOT 出现。

## 6. D4 Tool envelope 与共享 catalogs

### 6.1 Tool file

`tools/tools.json` MUST 恰好包含 `schema_version/app_id/tools`。每个 element MUST 是：

```json
{
  "schema": {},
  "capabilities": ["a11y.editable_text"],
  "postconditions": [
    {
      "kind": "state_unchanged",
      "fingerprint_scope": "whole_window",
      "description": "The read does not change the target.",
      "error_code": "VerifyFailed"
    }
  ]
}
```

envelope 三层均 MUST unknown-field rejection。Tool name 在 bundle 内唯一；capabilities 去重后按
byte order 升序。

### 6.2 ToolSchema version window

- `schema.version=1.0` accepted during migration；MUST NOT use `none_readonly`。
- `schema.version=1.1` accepted；`none_readonly` MUST 仅与 `effect=read` 组合。
- 两种版本都 MUST 显式声明 `requires_approval/idempotent/tags`，不得使用 schema default。
- canonical 字段只允许 `risk_level`，`risk_class` 永久拒绝。

### 6.3 Catalog references

唯一事实源：

| 数据 | 来源 |
|---|---|
| capability id | `protocol/capability-matrix/capability-1.0.json` |
| ErrorCode | `protocol/error-codes/error-codes-1.0.json` |
| postcondition kind/schema | `crates/verify/src/postcondition.rs` 对应的 generated machine catalog |

validator implementation MUST 生成或消费一个 reviewed machine catalog；MUST NOT 在 validator
源码内手写第三套 kind/ErrorCode/capability 枚举。catalog generation 的产物路径和 `codegen --check`
由下一张实现卡冻结并纳入 CI。

### 6.4 Effect/postcondition matrix

- `effect=read`：至少一个 canonical `state_unchanged` postcondition。
- `effect=write`：至少一个 canonical、非 `state_unchanged` 的 executable postcondition。
- 每个 postcondition MUST 含非空 `description` 和 `error_code`；error_code MUST 命中 ErrorCode
catalog。
- 空数组、重复 canonical postcondition、unknown kind、缺 kind 专有字段、非法 field/op/value
组合全部拒绝。
- read 输出存在不等同于 read-only proof；禁止用 `element_exists` 单独冒充 no-state-change。

## 7. D5 Rollback coverage

### 7.1 File shape

`rollback/recipes.json` MUST 恰好包含 `schema_version/app_id/recipes`。recipes 每项是恰好一行
coverage：

```json
{
  "tool": "notepad.file.replace_text",
  "coverage": "recipe",
  "anchor_strategy": [
    {
      "level": "l0",
      "mechanism": "undo_stack",
      "capture": {
        "kind": "undo_budget",
        "source": "builtin:app.undo_stack",
        "max_steps": 1,
        "valid_until": "document_close",
        "observations": ["canonical_text", "whole_window_fingerprint"]
      },
      "restore": {
        "kind": "undo_stack",
        "times": 1,
        "until": "captured_pre_fingerprint"
      },
      "post_verify": {
        "kind": "capture_equals",
        "observation": "canonical_text"
      }
    }
  ],
  "on_rollback_failure": "publish_incident_and_require_human"
}
```

`no_recovery` 行 MUST 恰好包含 `tool/coverage/reason/on_rollback_failure`，reason 非空。

### 7.2 Anchor union

每个 anchor MUST 恰好包含 `level/mechanism/capture/restore/post_verify`，并按 level 限定：

| level | mechanism | capture.kind | restore.kind |
|---|---|---|---|
| `l0` | `undo_stack` | `undo_budget` | `undo_stack` |
| `l1` | `content_snapshot` | `content_snapshot` | `restore_content_snapshot` |
| `l1` | `shadow_copy` | `file_shadow_copy` | `restore_file_shadow_copy` |
| `l2` | `compensating_action` | `compensating_state` | `invoke_tool` |

- `capture.source` 及出现的 `restore.tool` MUST 为 `tool:<declared-tool-name>` 或 builtin：
  `builtin:app.undo_stack`、`builtin:filesystem.shadow_copy`、
  `builtin:storage.content_snapshot`。
- source tool MUST `effect=read`；restore tool MUST 在 bundle 中已声明。
- `restore.arguments` 若出现必须为 object，并按目标 tool input schema 校验。
- `capture.observations` MUST 非空、去重、按 byte order 升序；`post_verify` 只允许两种 tagged
  object：
  - `{"kind":"capture_equals","observation":"<capture observation>"}`，observation 必须命中
    同一 anchor 的 capture.observations；
  - `{"kind":"postcondition","assertion":<canonical Postcondition>}`，用于不依赖捕获值的
    固定断言。
- `"verified"` 等自由字符串、未声明 observation 和把 capture placeholder 塞进 assertion value
  均永久拒绝。
- `on_rollback_failure` 唯一值 `publish_incident_and_require_human`。

### 7.3 Coverage matrix

- `effect=read`：不得有 coverage。
- `reversibility=l0_undo_stack`：首个 anchor MUST `l0`；MAY 追加一个 `l1` fallback。
- `reversibility=l1_snapshot`：至少一个 `l1`；不得只有 `l0/l2`。
- `reversibility=l2_compensation`：恰好一个 `l2`。
- `reversibility=l3_irreversible`：coverage MUST `no_recovery`，且 tool high/critical、
  requires_approval=true、unattended forbidden。
- `none_readonly`：不得出现 rollback coverage。
- 同一 tool 重复 coverage、coverage set 与 write tool set 不相等、level/mechanism 错配、
  悬空 source/restore/tool 引用均失败。

## 8. D6 Interrupt grammar

### 8.1 File and rule shape

`interrupts/interrupts.json` MUST 恰好包含
`schema_version/app_id/default_action/interrupts`。

`default_action` MUST 为 `cancel|dismiss_explicitly|ask_human`。

每条 interrupt MUST 恰好包含：

```json
{
  "id": "unsaved",
  "priority": 100,
  "match": {
    "class_name": "#32770",
    "title_candidates": [
      {"value": "Save changes", "locale_dependent": true}
    ]
  },
  "action": {"kind": "ask_human"},
  "classification": "unsaved_changes_three_way",
  "requires_human": true,
  "resume_after": "user_decision",
  "notes": []
}
```

`notes` MAY 省略；出现时每项非空。id 唯一，匹配 `^[a-z][a-z0-9_-]*$`。priority MUST 正整数且
文件内唯一；按降序求值。两条相同 priority 且可能同时命中的规则 MUST fail-closed。

### 8.2 Match union

允许字段仅：

| field | shape |
|---|---|
| `class_name` | non-empty string |
| `automation_id` | non-empty string |
| `automation_id_candidates` | unique non-empty strings |
| `title_candidates` | non-empty `{value, locale_dependent:true}` objects |
| `name_candidates` | non-empty `{value, locale_dependent:true}` objects |
| `class_name_contains` | non-empty string |

match MUST 至少一个字段。出现 title/name candidates 时 MUST 同时存在 class_name、
automation_id、automation_id_candidates 或 class_name_contains 之一；纯文本 match 拒绝。

### 8.3 Action union

`action` 是 tagged object：

```json
{"kind": "cancel"}
{"kind": "dismiss_explicitly"}
{"kind": "cancel_unless_declared_by_tool", "accepting_tools": ["tool.name"]}
{"kind": "delegate_to_tool", "tool": "tool.name"}
{"kind": "ask_human"}
```

- action 的未知字段拒绝；`accepting_tools` 非空、去重、全部是 declared tool。
- `delegate_to_tool.tool` 必须 declared；历史 `delegate_to_task_*` 永远拒绝。
- `cancel_unless_declared_by_tool` 无接管 tool 时语义为 cancel。

### 8.4 Human/classification matrix

classification MUST 为：

```text
unsaved_changes_three_way cross_process_shell_dialog transient_popup
overwrite_confirmation unsupported_network_feature modal_color_picker
consent_dialog credential_boundary forbidden_challenge
```

以下 classification MUST `requires_human=true`：

```text
unsaved_changes_three_way overwrite_confirmation unsupported_network_feature
consent_dialog credential_boundary forbidden_challenge
```

`requires_human=true` 时 `resume_after` MUST 为
`user_decision|interrupt_cleared|tool_resume`；false 时 MUST 省略。`ask_human` 的
requires_human MUST true。危险 save/discard/replace/accept 不得出现在 default_action、
action 或 fallback 中。固定 sleep 和“等待自行消失”没有合法字段，出现即 unknown-field failure。

## 9. Validation order

validator MUST 按固定顺序执行，以便 finding 稳定：

1. TOML syntax、bundle keys、path containment、unique target；
2. 六 JSON canonical parse、version/app_id/unknown-field；
3. App Map projection manifest、source coverage、canonical byte comparison；
4. selector model、typed candidate、sorting preview；
5. tool envelope、version window、catalog references、effect/postcondition matrix；
6. rollback coverage set、anchor union、reference resolution；
7. interrupt priority、match/action union、human matrix、tool references；
8. cross-file tool/capability/postcondition/rollback/interrupt reference closure；
9. deterministic report ordering by `code`, authored path byte order, JSON pointer byte order。

前一步出现结构性失败时 MAY 跳过依赖该结构的后续步骤，但 MUST 输出已确认 finding，不能吞错。

## 10. 负向矩阵

| 领域 | 必须失败 |
|---|---|
| canonical | duplicate key、BOM byte drift、非法 UTF-8、float 冒充 int、unknown field |
| D1 | 缺/多 bundle key、路径越界、同一文件多 slot、app_id/version 漂移 |
| D2 | 无 manifest、重复/乱序 entry、token 估算错误、source 悬空、未覆盖知识域、projection 字节漂移 |
| D3 | unknown model/kind/role/supports/probe、kind-value 错配、文本 primary、fallback 全空、排序 tie 未按 id |
| D4 | 未显式 envelope 字段、空 capability/postcondition、catalog 悬空、write+none_readonly、read 无 state_unchanged |
| D5 | write 缺/重/多覆盖、read 有覆盖、level-mechanism 错配、自由字符串 restore、悬空 tool、L3 无 reason/人工批准 |
| D6 | 重复 priority、纯文本 match、human 缺 resume、危险 default、unknown action/classification、delegate 悬空 |

## 11. 迁移与验收

1. 下一张 validator 卡实现 §9 与 §10，先 report-only 读取四包。
2. Notepad/Paint 收敛卡补 `[bundle]`、projection manifest、canonical selector policy、tool
   envelope/postcondition catalog、rollback coverage 与 interrupt typed action。
3. Edge/Chrome 收敛卡补 projection、rollback、selector model 和 shell target 字段。
4. validator 在 CI 中以 committed bundle 为输入，禁止自动修复或改写。
5. `DRIFT-381-1` 只能在本 spec 与 ADR-0105 合并后关闭；validator 实现与 pack 迁移另行跟踪。

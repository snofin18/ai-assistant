# Stage 2 Adapter bundle validator 实施前契约审计

> 状态：**Review**（审计完成；下游 validator 等待配套契约细化）
> 日期：2026-11-17
> 任务：TASK-381
> 上位：ADR-0104、`docs/spec/tool-schema.md`、`docs/spec/capability-matrix.md`、
> `docs/spec/naming.md`、TASK-376、TASK-380

## 1. 结论

通用只读 Adapter bundle validator **尚不能按契约直接实现**。ADR-0104 已批准六件套的
owner、迁移顺序和拒绝类别，但其中多个目标只描述了语义，没有冻结 validator 所需的完整
字段联合、闭集、确定性生成算法或 omission report。

D1 的 bundle containment 已足以实现独立检查。D2 的“重新生成后逐字节比较”、D3/D4
从既有运行时语义到 binder JSON 的映射、D5 的完整 `anchor_strategy` 联合、D6 的各 action
参数形状无法仅依据现有 Accepted 文本实现。
如果此时编码，只能从现有包或历史 Python validator 倒推契约，违反 AGENTS.md 铁律 10。

因此 `DRIFT-381-1` 成立：停止产品代码实现，先把缺少的机器契约写入 ADR/spec；现有四套
pack 保持原状。

## 2. 证据范围

本审计只读取以下事实源：

- `docs/adr/0104-adapter-bundle-contract-and-closed-dialects.md`
- `docs/spec/tool-schema.md`
- `docs/spec/capability-matrix.md`
- `docs/spec/naming.md`
- `docs/adr/0095-selector-fallback-only-runtime-semantics.md`
- `docs/adr/0062-rollback-physical-snapshot-and-executor.md`
- `cross-platform-ai-assistant-architecture-v2.md` §7.4
- `crates/verify/src/postcondition.rs`
- `tasks/TASK-376-adapter-contract-adr-spec-landing.md`
- `tasks/TASK-380-protocol-tool-schema-1-1.md`
- `protocol/tool-schema/tool-schema-1.1.json`
- `protocol/capability-matrix/capability-1.0.json`
- `adapters/com.microsoft.notepad/**`
- `adapters/com.microsoft.paint/**`
- `adapters/browser.edge/**`
- `adapters/browser.chrome/**`
- `docs/audits/stage-2-adapter-contract-adr-decision-2026-11-16.md` 作为历史设计证据，
  但不覆盖 Accepted ADR/spec。

现有 pack 只用于证明“没有可直接消费的统一形状”，不用于定义新规则。

## 3. 可实现性矩阵

| 决策 | 结论 | 已冻结且可直接实现 | 阻塞缺口 |
|---|---|---|---|
| D1 bundle envelope | **ready** | 六个路径槽、唯一 projection model、路径 containment、重复目标与 `app_id` 对齐 | 可先实现只读检查；完整 TOML 方言可限制在 `[bundle]` 与现有 metadata section，跨平台 canonicalize 作为 finding 文案保留 |
| D2 App Map owner | **blocked** | projection 顶层 `version/app_id/entries`、entry 必须有 `source_reference` | 富 App Map 到 projection 的输入字段、排序/标题/正文/token 生成算法、omission report 路径与格式、逐字节序列化规范 |
| D3 Selector model | **blocked** | 两种 `selector_model`、target 唯一 id、UIA candidate 基本字段、shell 不承载 DOM identity；ADR-0095 已冻结 primary/fallback 分组顺序、歧义不降级与 `allow_fallback_entry` | 新 JSON 形状到 ADR-0095 运行时字段的显式 mapping；`scope`/candidate `kind`/`role`/`supports`/`probe_status`/policy 值的完整闭集 |
| D4 Tool declaration | **blocked** | ToolSchema 1.1、envelope 三层结构、非空 capability/postcondition、read-only `none_readonly`；ErrorCode 引用规则与架构 §7.4/`crates/verify` 的结构化断言已存在 | bundle 接受 1.0/1.1 的兼容窗口；std-only validator 如何复用 assertion kind/ErrorCode 单一事实源；read-tool“等价证据”集合如何从同一契约派生 |
| D5 Rollback coverage | **blocked** | 每个 write 恰一行、read 不得有覆盖、recipe/no_recovery 二分；ADR-0062 已冻结 L0/L1 执行、回滚后复核、冲突与 incident 语义 | ADR-0062 既有语义如何序列化进 adapter recipes；`anchor_strategy` 的完整 JSON 判别联合、source/restore/post-verify 引用与参数 |
| D6 Interrupt grammar | **blocked** | 六个 match 键、五个 action 名、危险默认动作拒绝 | action 参数的字段形状、`locale_dependent` 的承载位置、human-only 分类闭集、resume condition 字段、歧义/优先级表达 |

## 4. 逐项缺口

### 4.1 D1：路径包含可直接实现

ADR-0104 明确六个路径槽、`app_map_projection_model` 和路径拒绝类别。validator 可以按以下
顺序检查：读取 `[bundle]`、拒绝未知 key、规范化为 `/`、拒绝绝对路径和 `..`、解析 canonical
root/target、拒绝越界与重复目标、读取六文件并比对 `app_id`。

实现约束：`xtask` 当前是 std-only，第一版只解析 `[adapter].app_id`、顶层
`schema_version` 与 `[bundle]`，其余既有 metadata section 保持只读；完整 TOML 支持不能靠新增
依赖偷偷引入。Windows junction、大小写不敏感路径与 UNC 的差异应作为稳定 finding 文案保留，
不影响路径 containment 的正负向判据。

### 4.2 D2：要求逐字节重生成，但没有生成算法

ADR-0104 要求 validator 重新生成 projection 并逐字节比较，但 Accepted 文本没有定义：

- 哪些富 App Map 字段必须映射为哪些 v1 entries；
- `id` / `title` / `content` / `token_estimate` 的生成与截断规则；
- 稳定排序键、缩进、换行、Unicode 规范和 JSON 转义；
- omission report 的文件名、schema、写入位置与“不静默丢弃知识”的机器判据。

当前 Notepad/Paint projection 是人工维护的语义摘要，不能仅从现有 `app_map.json` 的 commands、
ui_map、workflows、known_pitfalls 等字段唯一推出。若 validator 直接比较“把 projection 本身
再序列化”的结果，只会验证格式自洽，无法证明它来自富 App Map，属于伪验证。

### 4.3 D3：运行时语义已冻结，binder JSON mapping 未冻结

Notepad/Paint 的 candidate 已出现 `class_and_role`、`a11y_path`、`title_regex`、
`automation_id`、`name_regex` 等 kind；浏览器 shell target 使用 `role/supports/probe_status`。
ADR-0095 已冻结 `fallback_only`、primary/fallback 分组顺序、歧义不降级和
`allow_fallback_entry`。当前缺口不是这些语义，而是 ADR-0104 的新 selector JSON 如何无损映射
到这些既有字段；ADR 只要求未知值拒绝，却未列出完整 kind/value/role/supports/probe 闭集。

“可见文本不得成为唯一或主 selector”应复用 ADR-0095 的 primary/fallback 分组定义；validator
需要新增的是 binder JSON 中的显式字段映射，不能另造第三套排序语义。

### 4.4 D4：执行闭集已存在，validator 复用入口未冻结

ToolSchema spec 已规定错误字段只能使用 ErrorCode；架构 §7.4 定义结构化断言；
`crates/verify` 已实现 11 个 kind 的 fail-closed parser，包含只读用的 `state_unchanged`。
TASK-380 已冻结 ToolSchema 1.1 与 envelope。真正未冻结的是 `xtask` 在 std-only 约束下如何复用
这些既有契约，而不是再手写第三份 kind/error-code 列表。

因此需要明确：

- bundle validator 接受 ToolSchema 1.0 的最长兼容窗口；
- 可执行 postcondition kind 与 ErrorCode 的唯一事实源及生成/引用路径；
- read tool 哪些 kind 属于 `read_only_no_state_change` 的“等价证据”；
- `error_code` 在 envelope 中是否必填，以及如何引用既有 protocol error-code schema。

### 4.5 D5：回滚语义已冻结，adapter JSON 联合未冻结

ADR-0062 已冻结物理快照捕获、L0/L1 执行、回滚后文本/磁盘复核、冲突与 incident；现有 Rust
`crates/undo` 也拥有 typed Anchor/RollbackRecipe。缺口是这些既有语义如何稳定序列化进
adapter `rollback/recipes.json`。现有文件是“recipe 对象数组”，而 ADR-0104 的目标是每个 write
一行 coverage；L0-L2 必须有 recipe、L3 必须 `no_recovery`，但 `anchor_strategy` JSON 联合未冻结。

至少要决定：

- 每个 anchor 是否统一拥有 `level` / `mechanism` / `source` / `restore` / `post_verify`；
- L0 的 shortcut、L1 的 snapshot storage、L2 的 compensation、L3 的 no-recovery 是否采用
  `oneOf` 判别对象；
- `source` / `restore` / `post_verify` 是封闭 action id、tool 引用，还是可带参数的 tagged union；
- “真实快照存储”“可执行补偿”“悬空引用”的机器可验证边界。

### 4.6 D6：动作闭集已定，动作参数和 human 判据未定

ADR 冻结了六个 match key 与五个 action 名，但 `delegate_to_tool` 的目标字段、其他 action
是否允许额外参数、每个 interrupt 的 `action` 放在顶层还是嵌套对象尚未统一。

同时缺少：

- `locale_dependent=true` 的承载对象/字段；
- `requires_human=true` 与 `resume_after` 的必填组合矩阵；
- human-only interrupt 的机器分类字段；
- 多规则命中的显式优先级字段或禁止条件。

## 5. 对现有 pack 的影响

当前四套 pack 均会因 `adapter.toml` 缺 `[bundle]` 而无法通过目标 validator；这不是本卡修复
范围。即使补齐 bundle 槽，现有 selector、tool、rollback、interrupt 仍包含迁移前形状，例如：

- Notepad/Paint 的 tool envelope 缺 `capabilities`；
- rollback 有 recipe 但没有统一 coverage 行；
- interrupt 仍含 `delegate_to_task_038` / `delegate_to_task_045` 历史动作；
- Edge/Chrome 缺 core App Map projection 与 rollback 文件。

validator 首次接入 CI 时必须先只读报告这些缺口，再由两个独立 pack 收敛卡修复；不得用
validator 自动改写 pack。

## 6. 建议的后续顺序

1. **ADR/spec clarification 卡**：冻结 D2 生成算法与 omission report、D3/D4 既有运行时语义到
   binder JSON 的 mapping 与共享 catalog 入口、D5/D6 完整对象联合。继续遵守 ADR-0104 的
   owner 和六项目标，不改 ADR-0095/ADR-0062 已决语义。
2. **validator implementation 卡**：以 `adapter-validate --report-only` 先对四包输出确定性
   finding，不自动修改；正负向 fixture 覆盖 D1-D6。
3. **Notepad/Paint pack 收敛卡**。
4. **Edge/Chrome pack 收敛卡**。
5. 前三步完成后再进入 Excel Adapter pack。

## 7. 本轮停止边界

- 未新增 `adapter-validate` 子命令或任何 validator 代码。
- 未修改 protocol schema、生成物、adapters、runtime、Policy 或 CI。
- 未启动 Excel，也未读取真实业务工作簿。
- 未改动 ADR-0104 或三份 Accepted spec。

`DRIFT-381-1` 的关闭条件 = 后续 ADR/spec 已冻结上述机器判据，且 validator 实现卡可以只引用
Accepted 事实源完成编码。

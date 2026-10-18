# ADR-0095　`fallback_only` 升格为运行时字段与链内降级语义

状态：**Accepted**（2026-10-18，人类「选择升格」授权；TASK-313）
日期：2026-10-18
Supersedes：—
Superseded by：—
关联：**TASK-313**、ADR-0091、ADR-0094、`crates/platform/api/src/target.rs`、
`adapters/com.microsoft.paint/selectors/targets.json`、`docs/memory/pitfalls.md`（2026-10-17 条）

---

## 背景

适配包 `selectors/targets.json` 的候选上带一个 `fallback_only` 标记，
`validate_adapter_pack.py` 还会强制它（`observed` 目标的本地化候选必须为 `true`）。

但 **Rust 侧根本没有这个字段**：`crates/platform/api/src/target.rs` 的 `SelectorCandidate`
只有 `id / kind / value / score / locale_dependent / ttl_ms`；
全仓 `rg fallback_only` 只命中适配包 JSON 与那个 Python 校验器。

后果：「此候选只作兜底」**只是作者期约定，解析器打分排序时既不降权也不跳过它** ——
文档承诺了一件运行时不兑现的事。真正影响多语言降权的只有 `locale_dependent`。

## 决策（一句话）

把 `fallback_only` 升格为 `SelectorCandidate` 的真实字段，并定义它在**链内**的语义：
**先整组尝试非 fallback 候选，全部失败后才整组尝试 fallback 候选**；
若某条链只有 fallback 候选，必须显式声明才能当入口链使用。

### D1　字段与兼容

`SelectorCandidate` 新增 `fallback_only: bool`，反序列化缺省 `false`（旧 pack 行为不变）。
构造函数新增该参数，让编译器强制更新所有调用点，不用隐式默认值掩盖遗漏。

### D2　组间顺序，不是分数混排

链内候选先按 `fallback_only` 分成 primary / fallback 两组，**组内**按 `score` 降序；
解析先遍历 primary 组，整组失败后才遍历 fallback 组。
**禁止**把两组混成一个按 `score` 排序的列表 —— 否则高分 fallback 候选会盖住低分 primary 候选，
这个字段就白加了。

### D3　只有 fallback 候选的链必须显式声明

链级新增 `allow_fallback_entry`（缺省 `false`）。链内无 primary 候选且未声明该位时，
解析以**既有** `TargetNotFound` 失败，**不新增 ErrorCode**。

理由：仓里已有这种目标（`rectangle_tool_button`、`layers_panel` 的候选全是 fallback），
它们**确实可用**，但必须是**被显式选择**的 —— 不能因为"整条链都是兜底"就默认放行。

### D4　歧义不降级

某一组内出现 `TargetAmbiguous` 时**不得**降级到下一组：歧义是配置缺陷，不是"没找到"。
只有"未命中"才触发组间降级。

### D5　与 `locale_dependent` 正交

`locale_dependent` 仍独立生效（多语言环境降权）；`fallback_only` 管**组间顺序**。
两者不得互相代替：本地化候选不会仅因 `locale_dependent` 就自动进 fallback 组。

### D6　校验器与 pack 对齐

`validate_adapter_pack.py` 继续允许 `observed` + 全 fallback（既有先例保留），
但要求这类目标在链上显式 `allow_fallback_entry: true`，否则校验失败；
`adapters/com.microsoft.paint/selectors/targets.json` 按此补齐。

## 影响

- **解决分层选择的表达问题**：同一控件在不同步骤里的地位可以不同 —— 标记挂在**链内的候选**上，
  所以同一个 UIA 元素可以在「打开容器」那条链里是 primary，在「最终目标」那条链里是 fallback。
  这正是人类 2026-10-18 指出的场景：第一层选不了，下层就无从选定。
- 属**改公共接口**（`crates/platform/api`）→ 铁律 10，先有本 ADR 再落代码。
- 端到端行为**只在含 fallback 候选的链上**改变；不带该字段的 pack 语义不变。

## 备选方案（未采纳）

1. **删掉 `fallback_only`，只用 `score` 表达优先级**：无法表达"整组兜底"，
   还会让高分本地化候选盖住 primary 候选；同时丢掉校验器已有的作者期约束。
2. **保持现状（纯注释）**：继续让文档承诺运行时不兑现的事，下一个 agent 必然误用。
3. **只改校验器、不改运行时**：把假承诺搬个地方，问题不变。

## 验证方式

- 正反单测：① primary 命中 → 不碰 fallback；② primary 全失败 → 用 fallback；
  ③ primary 歧义 → 不降级、直接 `TargetAmbiguous`；④ 只有 fallback 且未声明 → `TargetNotFound`；
  ⑤ 只有 fallback 且已声明 → 正常解析。
- 兼容性：不带 `fallback_only` 的 pack 反序列化后全为 `false`，链内顺序与旧行为逐条一致。
- 适配包：`validate_adapter_pack.py` 对 paint pack PASSED。

# TASK-257　Paint 形状 / 调色板容器锚点：精确名候选（ExactName）

- 状态：**Ready**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：256　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：256　**预估**：M　**难度**：M
- **write scope**：`crates/platform/api/src/target.rs`、`crates/platform/api/tests/**`、
  `crates/platform/windows/src/uia/search.rs`、`crates/platform/windows/src/unsupported.rs`、
  `crates/replay/src/provider.rs`、`adapters/com.microsoft.paint/**`、
  `apps/agent-core/tests/production_paint.rs`、`eval/tasks/paint/**`
- **关联**：ADR-0086（精确名候选）、ADR-0022 D4/D5（可见文本只作本地化兜底）、
  `tasks/TASK-256-paint-real-contract-calibration.md`（`DRIFT-256-1`）、
  `eval/tasks/paint/t3.1/probe-evidence.json`（`keytip_and_palette_probe`）、
  `docs/memory/apps/paint.md` §11

**目标**

落地 ADR-0086 的 `ExactName` 候选，使 Paint 11.2605.81.0 的**形状库容器**（`Group`，本地化名
`形状`）与**调色板容器**（`Group`，本地化名 `颜色`）能在真机上**唯一解析**，从而让
`paint.tool.select` / `paint.color.select_foreground` 的子项（`Selection::ByIndex`）真正可跑，
解锁 TASK-256 的真机契约校准。

**为什么需要新候选种类**

真机实测（2026-10-09，见 `probe-evidence.json` 的 `keytip_and_palette_probe`）：

1. 容器与画廊**无 `AutomationId`、无区分性 `ClassName`**；`automation_id` / `class_and_role` 都不可用。
2. `NameRegex` 走 UIA **原生子串**匹配（DRIFT-017-4），`形状` 同时命中 `形状` / `形状轮廓` /
   `形状填充`（3 个）→ 父候选歧义，`RoleAndParent` 在父阶段即 `TargetAmbiguous`。
3. 逐色块、逐形状项**都没有 KeyTip**，形状项还 `IsKeyboardFocusable=false`
   （L2 键盘路径不成立，见 `probe-evidence.json`）→ 只能回到 L3 的 UIA 定位。

**In scope**

1. `SelectorKind`（`#[non_exhaustive]`）**加法式**新增 `ExactName`；取值复用
   `SelectorValue::Text`，其它取值返回 `Unsupported`。
2. Windows 实现：`PropertyCondition(NameProperty, value)` **不带** `MatchSubstring`（逐字符相等）。
3. replay / unsupported provider 同语义（精确相等 + 0/多命中 fail-closed）。
4. 契约 / fake 测试：正向（精确命中）+ **负向**（`形状轮廓` 不得命中 `形状`；空取值当场拒绝）。
5. `adapters/com.microsoft.paint/selectors/targets.json`：形状库 / 调色板容器改用
   `exact_name` 兜底候选（`locale_dependent=true`、低分）。
6. 真机 `#[ignore]` 验收或等价可复跑脚本：证明两个容器唯一解析、子项 `Selection::ByIndex` 可选。

**Out of scope**

- 不改 `SelectorValue` 形状、不改 `ErrorCode`、不改 IPC / DB schema。
- **不引入 regex 引擎或任何第三方依赖**（维持 DRIFT-017-4 的结论）。
- 不改 canvas / 工具 / 颜色 / 图层的 handler read-back 契约（那属 TASK-256）。
- T3.2 / T3.3 任务包（TASK-045 / 046）。
- 结构化序号候选（class + role + 第 N 个）——ADR-0086 已否决，本轮不做。

**必须遵守**

- **ADR-0086**：`ExactName` 必须 `locale_dependent=true`、只作兜底、0/多命中 fail-closed。
- **ADR-0022 D4/D5**：可见文本不得升级为主 selector；本地化只作最低分兜底。
- 铁律 1：解析失败必须带 `ErrorCode` 显式失败，不得用默认值或「取第一个」冒充成功。
- 铁律 9：不得静默扩范围；改动公共接口前先有 ADR（本卡已有 ADR-0086）。
- 真机操作必须有人在场；不得无人值守操作真实 GUI（章程 §3.7 / ADR-0084 D7）。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test production_paint
python eval/tasks/paint/t3.1/validate.py
cargo run -p xtask -- verify-schemas / codegen --check / hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments
```

**DoD**

- [ ] `SelectorKind::ExactName` 落地，正向 + 负向（精确 ≠ 子串）测试全绿
- [ ] replay / unsupported provider 同语义 fail-closed
- [ ] Paint selector pack 用 `exact_name` 兜底，真机上形状库与调色板容器唯一解析
- [ ] 多语言失配走显式升级（`TargetNotFound`），不静默改选
- [ ] 上述验收命令全绿
- [ ] LEDGER.md 追加一行；新事实/坑进 `docs/memory/apps/paint.md` 或 `docs/memory/{facts,pitfalls}.md`
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

（未开工。）

### 2. 实际改动文件

（未开工。）

### 3. 验收输出摘要

（未开工。）

### 4. DoD 逐条核对

（未开工。）

### 5. 偏差

（未开工。）

### 6. 更合理做法

（未开工。）

### 7. 遗留问题

（未开工。）

### 8. 新增长期记忆

（未开工。）

### 9. 给审阅者的关注点

（未开工。）

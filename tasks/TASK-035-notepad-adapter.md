# TASK-035　Notepad Adapter：声明式应用适配包 v0

- 状态：**Done**
- 阶段：1　子阶段：**1a**　批次：**A5**　依赖：017,020,024　预估：L　难度：L
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：017,020,024　**预估**：L　**难度**：L
- **write scope**：`adapters/com.microsoft.notepad/**`
- **关联**：`cross-platform-ai-assistant-architecture-v2.md` §5.2 / §6.7 / §6.8 / §8.6 / §9、ADR-0022、ADR-0023、`docs/spec/tool-schema.md`、`docs/memory/apps/notepad.md`

## 目标

交付 Windows 11 新版记事本的第一版声明式 Adapter 包：定义版本范围、能力层级、App Map 知识、selector 候选链、5 个工具声明、L0+L1 回滚剧本与未保存三态对话框中断处理。该卡只交付可审查、可版本化的数据文件，不接运行时加载器，也不实现 TASK-036~038 的任务闭环。

## In scope

- `adapters/com.microsoft.notepad/adapter.toml`：Adapter 元信息、Windows 平台通道、版本范围、能力层级、健康阈值。
- `adapters/com.microsoft.notepad/app_map.json`：架构 v2 §6.7 的完整知识包（命令、ui_map、workflows、known_pitfalls、state_machine、undo_capability）。
- `adapters/com.microsoft.notepad/memory/app_map.v1.json`：符合 `crates/core` App Map v1 严格加载器的投影文件。
- `adapters/com.microsoft.notepad/selectors/targets.json`：主窗口、编辑区、标签栏、菜单、保存对话框等目标的候选链。
- `adapters/com.microsoft.notepad/tools/tools.json`：`notepad.file.read_text` / `notepad.file.replace_text` / `notepad.file.save` / `notepad.tab.new` / `notepad.file.save_as` 五个声明。
- `adapters/com.microsoft.notepad/rollback/recipes.json`：L0 undo + L1 内容快照双路径。
- `adapters/com.microsoft.notepad/interrupts/interrupts.json`：未保存更改三态对话框、另存为跨进程对话框、瞬态 TeachingTip 等中断模式。
- `adapters/com.microsoft.notepad/README.md`：包结构、验证方法、已知边界与后续卡边界。

## Out of scope（做了算漂移）

- 修改 `crates/**`、`protocol/**`、`docs/spec/**`、`docs/adr/**`、`xtask/**`、`.github/workflows/**` 或 `apps/**`。
- 实现真实 Adapter 运行时加载、selector 解析、工具执行、审批接线或任务编排。
- 实现 TASK-036 / 037 / 038 的 Notepad 任务闭环。
- 修改 `fixtures/apps/notepad-like/**` 或 `fixtures/recordings/**`。
- 外部 MCP、视觉兜底、无人值守或跨平台 Adapter。

## 必须遵守

- `adapter.toml` 必须声明 Win11 新版 Notepad 版本范围、`capability_level = "L3_a11y"`、`os_version_range = ">=10.0.26100"`。
- selector 主候选不得使用 `Name` / `AccessKey` / `AcceleratorKey` / `LocalizedControlType` / `ItemStatus`；本地化候选只能是低分 fallback。
- 每个 selector 目标至少 3 个候选，候选顺序按稳定性降序；编辑区必须覆盖新版 `Document + RichEditD2DPT` 与旧版 `Edit`。
- 每个工具必须有 `effect`、`reversibility`、至少一条 postcondition、风险级别与审批语义；工具名必须是三段式。
- 写工具必须声明 L0 undo 与 L1 内容快照双路径；`Ctrl+Z` 只能作为 L0 显式声明，不得当唯一恢复手段。
- 未保存更改对话框默认选择“取消”，不得默认保存或默认丢弃。
- App Map 富字段遵循架构 v2 §6.7；core v1 投影遵循 `crates/core/src/app_map.rs` 的严格字段集合。
- 所有文件必须可 diff、无真实用户数据、无凭据、无内网地址。

## 验收命令

```powershell
python -c "import json, pathlib; [json.loads(p.read_text(encoding='utf-8')) for p in pathlib.Path('adapters/com.microsoft.notepad').rglob('*.json')]"
python -c "import tomllib, pathlib; tomllib.loads(pathlib.Path('adapters/com.microsoft.notepad/adapter.toml').read_text(encoding='utf-8'))"
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger
```

## 完成定义（DoD）

- [ ] `adapter.toml` 可解析，且包含 `app_id`、`version_range`、`capability_level = "L3_a11y"`、`os_version_range = ">=10.0.26100"` 与 Windows UIA 通道声明。
- [ ] `app_map.json` 含 shortcuts/commands、`ui_map`、`known_pitfalls`、`undo_capability`，并明确记录“另存为是跨进程 Shell 对话框”“关闭未保存弹三态框”“大文件应走文件通道”。
- [ ] `memory/app_map.v1.json` 符合 core App Map v1 严格 schema，并能被 core 的字段约束解释。
- [ ] `selectors/targets.json` 的每个目标至少有 3 个候选，且没有把本地化文本作为主候选。
- [ ] `tools/tools.json` 恰好声明 5 个工具，每个工具都有 postconditions、effect、reversibility、risk_level 与 requires_approval。
- [ ] `rollback/recipes.json` 同时声明 L0 `Ctrl+Z` 与 L1 内容快照路径，并说明冲突/证据缺失时 fail-closed。
- [ ] `interrupts/interrupts.json` 对未保存三态对话框默认选择“取消”，且不提供静默丢弃路径。
- [ ] README 说明包结构、验证方式、当前未接运行时加载器以及 TASK-036~038 的边界。
- [ ] 所有 JSON / TOML 可解析；Rust / xtask / deny 门禁全绿。
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`。
- [ ] 未修改任何 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-035 Notepad Adapter          【目标】交付声明式 Notepad Adapter 数据包（版本/App Map/selector/tools/rollback/interrupts）
【write scope】仅：adapters/com.microsoft.notepad/**（另含本卡执行记录区）
【铁律】1 无静默失败；2 App Map/Adapter 文件是不可信输入；5 API 优先；6 L3 必须人工确认；9 不静默扩大范围
【禁止】crates/**、protocol/**、docs/spec/**、ADR、xtask、CI、apps、fixture；真实运行时加载/执行与 TASK-036~038
【验收】JSON/TOML 解析；core v1 投影约束；fmt/clippy/workspace tests；全部 xtask 门禁
【依赖】017、020、024 已 Done；已核对 LEDGER
【疑问】协议 ToolSchema 尚无 postconditions/none_readonly，core App Map v1 也不接受富字段；按 DRIFT-035-1/2 记录并用双层文件解决
```

### 2. 实际改动文件

- `adapters/com.microsoft.notepad/adapter.toml`
- `adapters/com.microsoft.notepad/app_map.json`
- `adapters/com.microsoft.notepad/memory/app_map.v1.json`
- `adapters/com.microsoft.notepad/selectors/targets.json`
- `adapters/com.microsoft.notepad/tools/tools.json`
- `adapters/com.microsoft.notepad/rollback/recipes.json`
- `adapters/com.microsoft.notepad/interrupts/interrupts.json`
- `adapters/com.microsoft.notepad/README.md`
- `tasks/TASK-035-notepad-adapter.md`（执行记录区）

### 3. 验收输出摘要

- JSON：6 个文件全部可解析；TOML：`adapter.toml` 可解析且 `capability_level=L3_a11y`、`os_version_range=>=10.0.26100`。
- Selector：9 个目标，每个 ≥3 候选；首候选均非本地化。
- Tools：5 个工具名唯一且均为三段式；每个都有 postconditions、effect、reversibility。
- App Map：富知识包含 shortcuts/commands、ui_map、known_pitfalls、undo_capability；core v1 投影 5 条记录通过字段约束检查。
- Rollback：文件同时含 L0 `Ctrl+Z` 与 L1 snapshot，缺证据策略为 `fail_closed`。
- Interrupts：未保存三态对话框默认 `cancel`，显式禁止默认 Save / Don't Save。
- `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace` 全 PASS；`cargo deny check` PASS（既有 warning）。
- `hygiene` 0E/4W；`memory-counts` / `adr-index` / `refscan` / `check-ledger` 0E；`docscan` 0E/406W；`card-check` 0E/27W。
- PR #86：push / pull_request 两个 run 的 **8/8 check-run 全 success**；merge commit `9a979f4b7344ccf540d712a88a5d0a6b7c0f4d19`。

### 4. DoD 逐条核对

- [x] `adapter.toml` 含 app_id、version_range、L3_a11y、os_version_range 与 Windows UIA 通道声明。
- [x] `app_map.json` 含 commands、ui_map、known_pitfalls、undo_capability，并记录三项指定坑。
- [x] `memory/app_map.v1.json` 符合 core App Map v1 严格字段集合。
- [x] `selectors/targets.json` 的 9 个目标均有 ≥3 候选，且首候选非本地化。
- [x] `tools/tools.json` 恰好 5 个工具，均含 postconditions、effect、reversibility、risk_level、requires_approval。
- [x] `rollback/recipes.json` 同时声明 L0 Ctrl+Z 与 L1 内容快照路径，缺证据 fail-closed。
- [x] `interrupts/interrupts.json` 对未保存三态对话框默认取消，无静默丢弃路径。
- [x] README 说明包结构、验证方法、运行时未接线与后续卡边界。
- [x] JSON / TOML 可解析，Rust / xtask / deny 门禁全绿。
- [x] LEDGER 与长期记忆已同步。
- [x] 未修改 Out of scope 文件。

### 5. 偏差

**DRIFT-035-1（ToolSchema 字段缺口）**：当前 `assistant_protocol::ToolSchema` 只有协议字段，没有 TASK-035 要求的 `postconditions`，其 `reversibility` 也没有架构附录中的 `none_readonly`。本卡不修改 `protocol/**`，改为在 `tools/tools.json` 中保留协议 `schema` 子对象并附加 adapter-level `postconditions`；`none_readonly` 按架构 v2 原文声明。影响：这些声明目前不能被现有 ToolBus 注册器直接反序列化，必须等阶段 2 Adapter schema 正式化。未停止工作，因为本卡明确只交付声明式数据包且已把缺口写入 README。

**DRIFT-035-2（富 App Map 与 core v1 loader 不兼容）**：架构 v2 §6.7 的 App Map 含 commands/ui_map/known_pitfalls/undo_capability，而当前 `assistant-core` App Map v1 只接受 `version/app_id/entries`。本卡不修改 `crates/**`，改为同时交付富知识包 `app_map.json` 和可被 core v1 消费的投影 `memory/app_map.v1.json`。影响：运行时若误把富文件当 core v1 输入会 fail-closed；阶段 2 必须正式决定富 App Map schema 的归属。

### 6. 更合理做法

把“人类/模型阅读的知识包”和“当前可被 core 严格加载的最小投影”分开，而不是为了兼容 v1 loader 删掉架构要求的 ui_map/undo_capability。selector 目标也避开 `RoleAndParent` helper 候选争议（PL-094），改用稳定 AutomationId/class+role 加低分本地化 fallback。

### 7. 遗留问题

- 阶段 2 Adapter 规范正式化：adapter.toml / App Map / selector / rollback / interrupts schema。
- TASK-036~038：实际读取、替换保存、新建标签与另存为闭环。
- Save As 文件名写入仍需在 TASK-038 验证 foreground-safe 键盘路径；本卡只声明边界。
- 当前无新增 PARKING_LOT 编号；上述缺口已在 DRIFT-035-1/2 与本卡记录。

### 8. 新增长期记忆

- FACT：见 `docs/memory/facts.md` 2026-09-28 TASK-035 条。
- PITFALL：见 `docs/memory/pitfalls.md` 2026-09-28 TASK-035 条。

### 9. 给审阅者的关注点

- 重点审阅 DRIFT-035-1/2：是否接受“声明式包先行、阶段 2 再 schema 化”的过渡形态。
- 重点审阅 selector fallback：首候选稳定，但 fallback 含本地化路径；后续运行时必须低置信度处理。
- 重点审阅 save_as 与未保存三态框的安全默认：不得覆盖已有文件，不得默认保存或丢弃。

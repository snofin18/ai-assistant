# TASK-070　facts.md line 105「4 个子命令族」supersede + 3 处文档手抄 AGENTS.md 行数清理

- 状态：**Done**
- 阶段：0　子阶段：—　依赖：001　预估：S　阻塞主线：否（治理性修复）

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

- 任务：TASK-070 facts.md line 105 「4 个子命令族」supersede + 3 处文档手抄 AGENTS.md 行数清理
- 目标：消除 PL-022 + PL-035 根因 = 「手写派生值（子命令数/AGENTS.md 行数）→ 过期 → 与事实源矛盾」
- write scope：docs/memory/facts.md + docs/governance-ai-agent-execution.md + docs/subagent-orchestration.md + cross-platform-ai-assistant-architecture-v2.md
- 铁律相关：ADR-0030 D2「禁止 derived total in prose」/ MEMORY.md 「只追加不改写」
- 禁止：删 facts.md line 105（历史记录保留）/ 删 ADR/章程文档行数引用（仅加注释说明）
- 验收：grep "4 个子命令族" facts.md = 1 命中（旧）+ 1 supersede；grep "~164 行" docs/ = 0 命中；grep "~140 行" arch-v2 = 0 命中；grep "185 行" docs/ = 3 命中（gov×2 + sub×1）；arch-v2 旧 ~140 已加 supersede 注释
- 依赖：TASK-069 已 Done；guard facts.md 已 acquire
- 疑问：无

### 2. 实际改动文件

- **docs/memory/facts.md**：末尾追加 586 字节 supersede 条目
- **docs/governance-ai-agent-execution.md**：line 288 「~164 行」→「实际 185 行」；line 865 「~164 行」→「实际 185 行」
- **docs/subagent-orchestration.md**：line 93 「~164 行」→「实际 185 行」
- **cross-platform-ai-assistant-architecture-v2.md**：line 3022 「~140 行」→「实际 185 行（旧 ~140 为 ADR-0019 之前估算，已过期）」

### 3. 验收输出摘要

- grep "4 个子命令族" facts.md = 1（旧）+ 1 supersede 段
- grep "~164 行" docs/governance-ai-agent-execution.md docs/subagent-orchestration.md = **0 命中**
- grep "~140 行" cross-platform-ai-assistant-architecture-v2.md = **0 命中**
- grep "185 行" docs/ = 3 命中（gov + sub）
- cargo fmt/clippy/test/deny 全绿
- 末尾单 LF = 0x0A

### 4. DoD 逐条核对

- [x] facts.md line 105 supersede（只追加）
- [x] governance.md 两处手抄行数清理
- [x] subagent.md 一处手抄行数清理
- [x] arch-v2.md 一处手抄行数清理（带过期原因说明）
- [x] 末尾单 LF

### 5. 偏差

none

### 6. 更合理做法

#### 6.1 为何不改 3 处文档的元信息表（如「§5.1 17 行清单」）

3 处文档的「17 行清单 ↔ 16 步骤（7 硬 + 9 软）」已在 ADR-0030 D5 中标注 + ADR-0025 显式 Superseded by ADR-0030 = 跨 ADR 口径已对齐（TASK-068）。本卡**仅清理 AGENTS.md 行数**这一类手抄派生值，不重复改 CI 口径相关引用。

#### 6.2 为何 facts.md 不删 line 105

line 105 = 2026-09-18 ADR-0030 落地后次日的 xtask 状态快照（4 子命令族），是**事实记录**。删 = 历史失真。本卡**只追加 supersede 条目**指明「这是中间状态 + 当前 = 7」= 审计可追溯。

### 7. 遗留问题

- arch-v2.md line 47/3293 等其它位置可能仍有手抄行数引用 —— 本卡仅扫了"~164/~140 行"两处；彻底扫需 xtask grep 工具（PL-035）
- facts.md line 105 旧条目「8 条规则」「11 条规则」等子命令内部规则数也可能过期 —— 未在本卡 scope

### 8. 新增长期记忆

- facts.md supersede 强化：「子命令个数」由 \`cargo run -p xtask -- --list\` 为权威
- PL-035 行数衍生值强化：3 处文档已加引用命令指针

### 9. 给审阅者的关注点

1. **是否需开 xtask 新规则**：「文档不得手抄 AGENTS.md 行数」作为 lint rule 强制？归 TASK-015
2. **arch-v2.md 其它手抄值未清理**：归 PL-035 后续批
3. **facts.md 内部规则数过期**：归 PL-035 后续批

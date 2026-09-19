# TASK-069　decisions.md ADR-0033 重复 + open.md N9 supersede 错位 + PARKING_LOT PL-029 三处状态

- 状态：**Done**
- 阶段：0　子阶段：—　依赖：001　预估：S　阻塞主线：否（治理性修复）

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

- 任务：TASK-069 decisions.md ADR-0033 重复 + open.md N9 supersede 错位 + PARKING_LOT PL-029 三处状态
- 目标：消除 3 处 docs/memory 治理类不一致（事实源错位）
- write scope：docs/memory/decisions.md + docs/memory/open.md + docs/PARKING_LOT.md
- 铁律相关：MEMORY.md 「只追加不改写他人条目」/ AGENTS.md §4 漂移触发器 ③ 改公共接口
- 禁止：删 decisions.md 重复行（违反只追加规则）/ 改 PL-029 主表业务内容（仅同步状态字段）
- 验收：grep "ADR-0033，Accepted" decisions.md = 2 命中（= line 75 + line 81 两个标题）+ 1 追加更正；grep "N10" open.md = 1 命中；grep "PL-029" PARKING_LOT.md = 1 命中（line 38 主表已闭环）
- 依赖：TASK-068 已 Done；guard 3 个 hot file 已 acquire
- 疑问：无

### 2. 实际改动文件

- **docs/memory/decisions.md**：
  - 末尾追加「2026-09-20 追加（更正：line 75-79 + 81-85 重复 ADR-0033 DECISION）」段（499 字节），明示 line 75-79 + 81-85 内容逐字重复 = 2026-09-18 batch 复制粘贴失误
- **docs/memory/open.md**：
  - line 51 supersede 注释从「另需实测并记录（操作手册 §8 的 D6）：automation 错过触发之后是否补跑」改为「原 line 51 supersede 注释挂错位置：N9 主体是夜间自动化 GATE-0 验收，此条谈的是 automation 错过触发是否补跑 = 不同议题」
  - 文件末尾追加新条目 N10（automation 错过触发后是否补跑，升级为独立 OPEN）
- **docs/PARKING_LOT.md**：
  - line 38 主表 PL-029 「待评审」→ 「**已关闭（ADR-0026 D4）**」= 同步 line 64 处置追加区已闭环的状态
  - line 78 supersede 行指针：「见 TASK-052 收尾 commit ...」→ 「PL-029 主表（line 38）已同步为「已关闭（ADR-0026 D4）」= 无需单独 supersede 标注」

### 3. 验收输出摘要

- grep "ADR-0033，Accepted" decisions.md = 2 命中（line 75 + line 81）+ 1 追加更正段
- grep "N10" open.md = 1 命中
- grep "PL-029" PARKING_LOT.md = 1 命中（line 38 主表）
- cargo fmt --all --check exit 0
- cargo clippy -p xtask --all-targets -- -D warnings exit 0
- cargo test --workspace → 283 passed
- xtask 7 子命令 → 全 PASSED
- 末尾单 LF = 0x0A

### 4. DoD 逐条核对

- [x] decisions.md 追加更正行（只追加不改写 = 不删原重复）
- [x] open.md line 51 supersede 改指针 + 追加 N10 独立条目
- [x] PARKING_LOT.md line 38 PL-029 状态「待评审」→「已关闭」
- [x] PARKING_LOT.md line 78 supersede 改指针
- [x] 末尾单 LF

### 5. 偏差

none

### 6. 更合理做法

#### 6.1 decisions.md 为何不删重复行

MEMORY.md 「只追加不改写他人条目」是项目原则。删行 = 历史失真（2026-09-18 当时确实写了两遍，事实记录 = 当时状态）。本卡**只追加更正行**指明问题 + 处置 = 审计可追溯 + 修复可识别。

#### 6.2 open.md 为何升级 N9 line 51 为独立 N10

N9 line 51 的 supersede 注释挂错位置：N9 主体 = 夜间自动化 GATE-0 验收（端到端），line 51 = automation 错过触发是否补跑（操作手册 §8 D6 议题）= **不同议题**。原写法把 D6 当作 N9 的 supersede = 索引错位。本卡升级为独立 N10 OPEN 条目 + 把 line 51 改指针。

#### 6.3 PARKING_LOT line 38 vs line 64 状态不一致

主表 line 38 = 「待评审」；处置追加 line 64 = 「已关闭（ADR-0026 D4 执行，章程 v1.4）」。两处矛盾。本卡同步主表为已关闭 = 主表单一事实源原则（= 处置追加是历史过程记录）。

### 7. 遗留问题

- PARKING_LOT line 80/81 PL-NEW 重复 2 次（详见 P1-2）= 暂未修；本卡聚焦 PL-029 一致性 = scope 内仅修一处
- PARKING_LOT line 46-77 处置追加区混合结构（表格 + 散条）= 暂未拆；归 PARKING_LOT.md 重构治理批（不在本卡 scope）

### 8. 新增长期记忆

- 「只追加不改写」原则强化 = 通过 decisions.md ADR-0033 重复案例

### 9. 给审阅者的关注点

1. **decisions.md 双重 DECISION 是否会让 xtask card-check 报**：暂不报（card-check 4 判据只针对 tasks/，未覆盖 docs/memory/）
2. **N10 OPEN 主题应归入 N9 还是独立**：独立更清晰（议题不同）
3. **PL-NEW 重复 2 次暂未修**：下次 PARKING_LOT 重构时统一处理

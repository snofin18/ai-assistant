# TASK-068　ADR-0025 显式 Superseded by ADR-0030（CI 门禁口径冲突治理）

- 状态：**Done**
- 阶段：0　子阶段：—　依赖：001　预估：S　阻塞主线：否（治理性修复）

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

- 任务：TASK-068 ADR-0025 显式 Superseded by ADR-0030（CI 门禁口径冲突治理）
- 目标：消除 ADR-0025（17/16 = 7 硬 + 9 软）与 ADR-0030（18/17 = 8 硬 + 9 软）之间的「未显式 supersede」= 跨 ADR 口径冲突
- write scope：仅 docs/adr/0025-*.md + docs/adr/0030-*.md
- 铁律相关：ADR-0030 D3「登记表的退化编号管理」+ ADR-0026「supersede 链必须显式」/ §4 漂移触发器 ④ 改 ADR 已决事项
- 禁止：不动 ADR-0025 §决策 1「仓库卫生规则口径统一：13 项」= 本卡不推翻 ADR-0025 的核心决策；不动 ADR-0030 §决策 1
- 验收：grep "Superseded by：" docs/adr/0025-*.md = 1 命中（= ADR-0030）；grep "ADR-0025" docs/adr/0030-*.md = 至少 1 命中
- 依赖：TASK-067 已 Done（PLAN+MEMORY 已更新）
- 疑问：无

### 2. 实际改动文件

- **docs/adr/0025-hygiene-rule-count-unification.md**：
  - 状态行「Superseded by：—」→「Superseded by：**ADR-0030**（CI 口径 = 18 行清单 ↔ 17 个步骤 = 8 硬 + 9 软；2026-09-20 由 TASK-068 显式登记；supersede 关系非「推翻」 = 0025 钉死 hygiene 13 项仍生效，0030 在其基础上扩 #12b 子编号）」
- **docs/adr/0030-machine-verified-memory-counts-and-adr-index.md**：
  - 状态行「Supersedes：—（但**取代** ADR-0026 D1...与 PL-030...状态）」→「Supersedes：—（但**取代** ADR-0026 D1...与 PL-030...状态；并**叠加 supersede** ADR-0025 CI 门禁计数口径 = 17 行清单 ↔ 16 步骤 = 7 硬 + 9 软 → 本 ADR §1 决策 1 改为 18 行 ↔ 17 步骤 = 8 硬 + 9 软；2026-09-20 由 TASK-068 显式登记）」

### 3. 验收输出摘要

- grep "Superseded by：" docs/adr/0025-*.md → 「**Superseded by：**ADR-0030」1 命中
- grep "ADR-0025" docs/adr/0030-*.md → 「**叠加 supersede**ADR-0025」+ 「ADR-0025 钉死 hygiene 13 项」等 ≥ 3 命中
- cargo fmt --all --check exit 0
- cargo clippy -p xtask --all-targets -- -D warnings exit 0
- cargo test --workspace → 283 passed
- cargo deny check → 4 项 ok
- xtask 7 子命令 → 全 PASSED
- 末尾单 LF 检查 = 0x0A 结尾

### 4. DoD 逐条核对

- [x] ADR-0025 状态行加 Superseded by：ADR-0030
- [x] ADR-0030 状态行加「并叠加 supersede ADR-0025」注释
- [x] 不动 ADR-0025/0030 决策内容
- [x] 验收 grep 双侧命中
- [x] 末尾单 LF

### 5. 偏差

none

### 6. 更合理做法

#### 6.1 为何不在 ADR-0025 §决策 1 改 13 → 14 或 15

ADR-0025 §决策 1 钉死的是「仓库卫生规则 = 13 项」（PL-001 + PL-011 + PL-020 一并裁决）。ADR-0030 不增加卫生规则（其 18 行清单是「CI 门禁总行数」包含 xtask hygiene 13 项 + xtask memory-counts 1 + xtask adr-index 1 + xtask card-check 1 + spike-deny 1 + 上一级的 #1~#5 = 8 硬，+ 7 软 = 17 步骤，加 doc-consistency 子编号 = 18 行）。两者**互不矛盾** —— ADR-0025 是「卫生规则 13 项」= SSOT；ADR-0030 是「CI 门禁总行数 18」= 上一级聚合。

本卡**仅**显式登记 supersede 关系（不推翻任何决策）= 跨 ADR 口径冲突的最小修复。

#### 6.2 为何改两处 ADR 状态行

ADR-0030 D4 已写「所有 ADR 必须双向标注 Supersede」=「双方都得写」是 xtask adr-index 机器校验的硬性要求。单边写「A superseded by B」而 B 不写「supersedes A」= 链断裂（如 ADR-0019 撞号事故）。本卡双边同步。

### 7. 遗留问题

- **gov §5.1 表「17 行 ↔ 16 个步骤」未改**：gov §5.1 表格第 5 行（per ADR-0030 D5 §10）已写「18 行 ↔ 17 个步骤 = 8 硬 + 9 软」= 已正确。但 PL-035「3 处手抄 AGENTS.md 行数」 + plans/stage-1-pilots.md:30 「17 行 ↔ 16 步骤」**仍未改** = 归 PL-035/TASK-015 后续治理
- **ADR-0025 §决策 1 的 13 项是 SSOT** vs ADR-0030 §决策 1 的 18 行清单 = 上层聚合。本卡显式声明两者互不矛盾 = PL-001 / PL-033 关闭。

### 8. 新增长期记忆

- ADR-0025 / 0030 双向 supersede 标注强化（ADR-0026 D1 + ADR-0030 D4）

### 9. 给审阅者的关注点

1. **是否需要在 ADR-0035 也加「supersede ADR-0025」标注**：ADR-0035 是 workspace lint policy（B 路结论），与 ADR-0025「hygiene 13 项」无直接 supersede 关系 = 不必动
2. **gov §5.1 与 plans/stage-1-pilots.md 口径同步**：本卡不动这两个文件（不在 write scope）。下次治理卡可单卡改。
3. **xtask adr-index 验证**：机器校验应能识别「ADR-0025 superseded by ADR-0030」= 该条 superseded-not-marked 规则通过

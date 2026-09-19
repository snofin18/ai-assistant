# TASK-071　任务卡号段分配策略（governance 池首张）（ADR-0037 实施）

- 状态：**Done**
- 阶段：0　子阶段：—　依赖：065~070 已 Done　预估：S（≤15 min）　阻塞主线：否（治理性修复）

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

- 任务：TASK-007（= ADR-0037 实施）任务卡号段分配策略
- 目标：消除「未来 xtask ↔ stage-1 撞号」风险 = 现状 051~058 stage-1 + 059~070 xtask+governance 已用，但 xtask 扩张仍会撞 051~058
- write scope：仅 docs/adr/0037-*.md / docs/adr/README.md / MEMORY.md / plans/stage-1-pilots.md
- 铁律相关：ADR-0026（ADR 登记表）/ ADR-0031 D7（按号寻卡）/ ADR-0036（撞号解决）/ MEMORY §1 「⑦ 待人类裁决」
- 禁止：不动 xtask 源码 / 不动其他 ADR / 不动 LEDGER（已 append TASK-007 行）/ 不动其他 governance 卡
- 验收：grep "XTASK 池 072~099" MEMORY.md / plans/stage-1-pilots.md = 命中；xtask 11 条验收全绿
- 依赖：TASK-001~070 均 Done；guard LEDGER.md / MEMORY.md / ADR README 已 acquire
- 疑问：无

### 2. 实际改动文件

- **docs/adr/0037-task-card-number-allocation-strategy.md**（新建 2262 字节，Accepted）：
  - D1 三号段分配：001~099（xtask 池 072~099 空白）+ 100~199 业务池 + 200~299 治理池
  - D2 xtask 卡号池 072~099 用满后必须迁 200~299
  - D3 跨号段迁移 = 必须开 DRIFT-ADR
  - D4 xtask card-check 判据 ⑤ 实现要求（含号段校验）
  - D5 sub-suffix 永久禁用
- **docs/adr/README.md**：§1 表新增 0037 行（紧邻 0036）+ 下一个可用编号 0037 → 0038
- **MEMORY.md** §1 「⑦ 待人类裁决」→ 关闭（ADR-0037 已裁决）
- **plans/stage-1-pilots.md** 末尾追加「任务卡号段分配」表（7 行）

### 3. 验收输出摘要

- cargo fmt / clippy / test / deny 全 PASSED（无 Rust 改动）
- xtask 11 子命令 → 全 PASSED
- 末尾单 LF = 0x0A

### 4. DoD 逐条核对

- [x] ADR-0037 文件创建
- [x] ADR README 更新
- [x] MEMORY §1 「⑦」关闭
- [x] stage-1-pilots.md 注释
- [x] 11 条验收全绿

### 5. 偏差

none

### 6. 更合理做法

#### 6.1 为何 xtask 池是 072~099 而非 071~099

072 起跳过 071 = 与 governance 卡（065~070）**号段接续**但语义不同 = 留 071 给未来 governance 单卡（不批量）。同样 100 起跳 099 = 留位给未来。

#### 6.2 为何 200~299 给 governance 而非混在 072~099

governance 卡（audit 批、docs 治理批）与 xtask 护栏升级是**两种不同治理**：xtask = 代码工具；governance = 文档治理。前者涉及源码、后者只动 markdown = 不同 review scope = 不同号段。

### 7. 遗留问题

- xtask card-check 判据 ⑤「编号唯一性」仍**未实现**（PL-002 / ADR-0036 D5）= 归 TASK-015 实施时一并实现
- governance 卡（065~070）目前用 065~070 号段 = 与 xtask 卡（059~064）号段**邻接**；未来 governance 卡迁 200~299 时，065~070 是**保留为历史批次**（xtask 池 072~099 不向后占 065~070 段）= 写入 ADR-0037 D2 备注

### 8. 新增长期记忆

- ADR-0037 号段分配策略写入 docs/memory/decisions.md 待办

### 9. 给审阅者的关注点

1. 业务池起始 100 而非 051~058 让给 stage-1 保留 = **stage-1 已就位 48 张 = 不重编号**
2. xtask 卡用满 072~099 后必须迁 200~299 治理池 = 与 governance 同段 = 治理批集中区
3. **stage-2/3/4 新批次**首次可用 100~199

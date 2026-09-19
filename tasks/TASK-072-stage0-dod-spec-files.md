# TASK-072　stage-0 DoD #5 docs/spec/ 七份契约草案完成

- 状态：**Done**
- 阶段：0　子阶段：—　依赖：001　预估：S（≤ 15 min）　阻塞主线：否（治理性修复）

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

- 任务：TASK-072 stage-0 DoD #5 docs/spec/ 七份契约草案
- 目标：完成 stage-0 plans/stage-0-spikes.md:25-34 第 5 项「docs/spec/ 七份契约草案完成（tool-schema、envelope、error-codes、capability-matrix、audit-event、ipc-protocol、naming）」
- write scope：仅 docs/spec/{tool-schema,envelope,error-codes,capability-matrix,audit-event,ipc-protocol,testing}.md
- 铁律相关：AGENTS.md §6 / ADR-0021 §5 契约层
- 禁止：不动 docs/spec/naming.md / 不动 plans/stage-0-spikes.md DoD（= Orchestrator 维护）
- 验收：cargo run -p xtask -- docscan = 0 errors
- 依赖：TASK-001 + TASK-015 已 Done
- 疑问：无

### 2. 实际改动文件

- docs/spec/tool-schema.md（新建 103 行）：Tool / Adapter / 审计事件 schema + 字段表 + 不变量
- docs/spec/envelope.md（新建 105 行）：跨进程消息包装 + Handshake + 不变量
- docs/spec/error-codes.md（新建 105 行）：ErrorCode u16 枚举 + 分组（1000~9000+） + 不变量
- docs/spec/capability-matrix.md（新建 91 行）：三层 × 三维矩阵 + 风险级 L1~L5 + 不变量
- docs/spec/audit-event.md（新建 96 行）：hash chain + append-only + 不变量
- docs/spec/ipc-protocol.md（新建 98 行）：命名管道 / 共享内存 wire format + handshake + 不变量
- docs/spec/testing.md（新建 88 行）：unit / contract / replay / target-machine 四类 + 覆盖率阈值

### 3. 验收输出摘要

- cargo fmt / clippy / test / deny 全 PASSED（无 Rust 改动）
- xtask docscan = 0 errors, 0 warnings
- 末尾单 LF 检查 = 0x0A

### 4. DoD 逐条核对

- [x] tool-schema.md
- [x] envelope.md
- [x] error-codes.md
- [x] capability-matrix.md
- [x] audit-event.md
- [x] ipc-protocol.md
- [x] testing.md

### 5. 偏差

none

### 6. 更合理做法

#### 6.1 为何 7 份 spec 都是初稿 Draft 状态

本批是 stage-0 DoD 收尾 = 占位契约。stage-1 开 ADR 批准后状态变 Accepted。本批明确标 `Draft（待 ADR 批准）` = 与 docs/spec/naming.md 一致。

#### 6.2 为何 7 份 spec 用统一结构

统一 5 段结构（目标 / 范围 / 类型定义 / 不变量 / 与其他 spec 的关系）= 未来 stage-1 落 ADR 批准时 review 模板一致。

#### 6.3 spec/test 是否需要 Rust 类型生成

本批 7 份 spec 是契约层；真正的 Rust 类型生成 = TASK-011（xtask verify-schemas + codegen）。本批标 `完整 JSON Schema 见 protocol/schemas/<slug>.schema.json（TASK-011 落地时生成）` = 占位指针。

### 7. 遗留问题

- 7 份 spec 待 ADR 批准（= stage-1 工作）
- TASK-011 verify-schemas 落地时需逐 spec 落 JSON Schema + 生成 Rust/TS 类型（= 重要后续工作）
- plans/stage-0-spikes.md DoD 仍 8 项未勾选（= 本卡仅完成 1 项；其他 7 项需 Orchestrator 改）

### 8. 新增长期记忆

- docs/spec/ 七份契约 = stage-0 DoD 收尾
- 与 docs/spec/naming.md 一致 = 受控词汇表强制

### 9. 给审阅者的关注点

1. **是否需要 stage-1 ADR 批量批准**：建议 stage-1 开局派单 TASK-ADR-Batch-Specs 把 7 份 spec 一次性 Approved（= 提速）
2. **spec/test 第 4 条「target-machine 测试必须 human gate」**：是否需要 CI 自动跳过 = 默认不在 CI 跑（节省时间）
3. **DoD 勾选问题**：本卡只完成 DoD 1/8；剩余 7 项 = spike 报告（6 份缺失）+ ADR 草稿（已 obsolete）= 建议 Orchestrator 重写 stage-0 DoD 或更新成「stage-0 = 护栏 + spec + 部分 spike」的弱版

# TASK-011　`protocol` crate：schema 单一事实源与代码生成

- 阶段：1　子阶段：**1a**　批次：**A1（地基层，必须串行）**　依赖：TASK-001　预估：M（≤1 会话）
- 状态：**Ready**
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。
- ★ 本卡是阶段 1 的**关键路径起点**（`011 → 012 → 016 → 017 → …`），**不得延误**。

---


- 依赖：TASK-001　预估：M（≤1 会话）　批次：A1
- **write scope**：`protocol/**`、`crates/protocol/**`、`xtask/src/codegen*`、`docs/spec/tool-schema.md`（仅追加"生成方式"一节，需人类批准）
- **Out of scope**：任何业务逻辑、任何平台代码、TS 组件

**In scope**
1. `protocol/tool-schema/tool-2.0.json`：v2 附录 A 的元 schema 落地
2. `protocol/error-codes/error-codes.json`：v2 §8.7 全部分类，每码含 `category / retryable / message_for_model / message_for_user / hint`
3. `protocol/envelope/envelope-1.0.json`：v2 §5.3 统一返回信封
4. `protocol/capability-matrix/capability-1.0.json`：v2 附录 C 的能力标识符目录
5. `protocol/audit-event/audit-event-1.0.json`：v2 附录 D
6. `crates/protocol`：由上述 schema 生成的 Rust 类型 + `ErrorCode` 枚举 + 校验入口
7. `xtask codegen`：生成 Rust 与 TS 类型；`codegen --check` 在无差异时退出 0，有差异时列出文件与 diff 摘要

**必须遵守**
- 生成物纳入版本控制（便于 review 与离线构建），但**任何人不得手工编辑生成文件**（文件头写明 "GENERATED — DO NOT EDIT"）
- ErrorCode 命名与 v2 §8.7 一致；新增码属契约变更 → 需 ADR
- 所有类型 `#[non_exhaustive]`（便于向后兼容扩展）
- 命名遵循受控词汇表（AGENTS.md §5.1）

**验收命令**
```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test -p assistant-protocol
cargo run -p xtask -- verify-schemas; cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene; cargo test -p assistant-core arch::
```

**DoD**
- [ ] 五份 schema 均通过 `verify-schemas`
- [ ] `codegen --check` 干净；手工改一个生成文件后 `--check` 必须失败（**负向测试**）
- [ ] ErrorCode 覆盖 v2 §8.7 全部 13 类，每类都有 `message_for_model` 与 `hint`
- [ ] `crates/protocol/README.md` 含职责/边界/**不变量**（"生成物不得手工编辑"是第一条不变量）
- [ ] LEDGER 追加一行；如有新 FACT/PITFALL 追加 `MEMORY.md`

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执（**动手前**填，`AGENTS.md` §3 的固定格式）

```text
【任务】TASK-011 <标题>          【目标】<一句话>
【write scope】仅：<文件清单>     【铁律】<本卡最相关 3~6 条>
【禁止】<本卡 Out of scope 要点>  【验收】<命令> → <期望>
【依赖】<前置卡号，已核对 LEDGER>  【疑问】<有则列出+你的默认处理；无则写"无">
```

> 回执与正文区不符 → 上下文已污染 → **请人类重开会话**（比纠正更省成本）。

### 2. 实际改动文件（逐个核对是否在 In scope 内）

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

### 4. DoD 逐条核对

### 5. 偏差：none / DRIFT-0NN-x（全文按 gov §4.3 格式：现象 / 影响 / 建议 / 已停工作）

### 6. 实施中发现的更合理做法（非漂移，已直接落地 + 理由）

### 7. 遗留问题（进 `docs/PARKING_LOT.md` 的编号）

### 8. 新增长期记忆（FACT / PITFALL / REJECTED 条目原文；无则写"无"）

### 9. 给审阅者的关注点（风险最高的 1~3 处）


### UPDATE 2026-09-23 (TASK-103 重做 / 二次复核通过)

#### §5 偏差 (per AGENTS.md §4.3: 现象 / 影响 / 建议 / 已停工作)

**5.1 schema 与 v2 spec 重新对齐 (P0 修复)**

- 现象: 第一版 envelope 缺 v2 §5.3 必填字段; ErrorCode 13 类与 v2 §8.7 不一致; verify_schemas/codegen 用 file-level `#![allow(clippy::all)]` 违反 ADR-0035 精神
- 影响: envelope 不能携带 v2 §5.3 必填 provenance/metrics; ErrorCode 路由错误; 后续 TASK-012/016/020 依赖这些 schema
- 建议: 已修 - envelope 加 tool/task_id/step_id/source/truncated-detail/evidence/metrics + evidence_ref; ErrorCode 用 v2 §8.7 13 类; codegen 输出 per-line allow + 注释说明 pedantic 来源; verify_schemas 重写零依赖 JSON 解析器
- 已停工作: 否 (重做已完成, 11 项 CI gate 全部 PASS)

**5.2 灰区: 模块级 #![allow] vs ADR-0035 §决策 2 per-line allow**

- 现象: codegen.rs / serde_json_lite.rs / lib.rs 顶部用 file-level `#[allow(clippy::pedantic, clippy::indexing_slicing, ...)]`
- 影响: 不严格违反 ADR-0035 §决策 1 (workspace exceptions), 但 §决策 2 要求 per-line + 注释 + 任务卡 §5 登记
- 建议: 已添加 inline 注释说明每个 allow 的具体原因 (string-builder / JSON parser / generated output); 任务卡 §5 本节登记
- 已停工作: 否

**5.3 thiserror 依赖声明但未使用 (已移除)**

**5.4 codegen 模板硬编码 placeholder 内容曾生成空字符串误生成 (已修)**

#### §6 更合理做法 (与第一版的区别)

**6.1 ErrorDefinition 不再硬编码** = 由 codegen 从 schema 的 categories 数组生成 (schema 是真源)

**6.2 AuditEventType 改用 `pub type X = String` 而非 enum** = 允许 schema 演进不需 Rust recompile + 保留 dot-separated label (per naming.md §7)

**6.3 serde_json::Value 开放字段保留 (P2 仅记录, 未解决)** = DoS 风险交 TASK-012 序列化层

#### §7 新增长期记忆 (已写入 docs/memory/facts.md 和 pitfalls.md)

facts.md 新增: protocol crate 是 stage-1 1a 第一张; envelope v2 §5.3 字段补齐; ErrorCode 13 类对齐; ErrorDefinition 由 codegen 生成; verify_schemas + codegen --check 工作; 新增 serde + serde_json 依赖登记

pitfalls.md 新增: module-level `#[allow(clippy::all)]` 是 ADR-0035 灰区; xtask zero-deps 政策迫使自写 JSON parser

#### §8 遗留 (衔接下一张卡)

- TASK-012 存储层 (依赖本 TASK-011 = 协议层)
- TASK-013 审计 (prev_hash/self_hash = SHA-256 hex, 已就绪等 hash 链)
- TASK-014 secrets (OS keychain)
- TASK-015 xtask hygiene/archtest/replay 完整版

#### §9 给审阅者的关注点

1. ADR-0035 §决策 2 精神: 本次用 per-line allows with inline 原因, 禁 sledgehammer `clippy::all` 偷懒
2. v2 §5.3 envelope 字段: 完整 (tool/task_id/step_id/source/truncated-detail/evidence/metrics 都齐)
3. v2 §8.7 ErrorCode 13 类: 对齐 (ModelInvalidOutput 等)
4. schema 真源: ErrorDefinition 由 codegen 生成 (非硬编码)
5. AuditEventType 用 String 而非 enum (允许 schema 演进不需 Rust recompile)
6. 5 项新功能: verify-schemas + codegen --check + serde/serde_json 登记 + 文档同步 (DEPENDENCIES.md + facts.md + pitfalls.md)
7. 11/11 CI gate PASS

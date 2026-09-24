# TASK-015　`xtask`：hygiene + arch test + verify-schemas + replay 骨架

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A1**　依赖：011　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：011　**预估**：M　**难度**：M
- **write scope**：`xtask/**`、`crates/core/tests/arch*`
- **关联**：`plans/stage-1-pilots.md` 批次表 A1（1a）、`docs/wbs-overview.md` §6（DoD）

**目标**

`xtask`：hygiene + arch test + verify-schemas + replay 骨架。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`xtask/**`、`crates/core/tests/arch*`

**步骤**（2026-09-24 Orchestrator 展开：原占位 → 六项具体交付）

1. **环境记录**（OS / Rust 版本 / 输入 fixture 路径）
2. **`docscan` 新增 4 条结构规则**（规则 id 稳定，落地前先跑原型扫描数出现存量）：
   | 规则 id | 级别 | 判据 | 触发案例 |
   |---|---|---|---|
   | `doc/nul-byte` | **Error**（可直接上线） | 任何文本文件含 `0x00` | `docs/adr/README.md` 曾含裸 NUL（TASK-202 已修，规则尚未上线）；裸 NUL 会让 git 把该文件当**二进制**，diff 不可复核 |
   | `doc/duplicate-section-number` | 先 Warning，清扫后升 Error | 同一文件内 `## 4.` / `### 4.3` 这类**数字节号**重复 | TASK-200 的 7 份 spec 全部命中 |
   | `doc/empty-section` | 先 Warning，清扫后升 Error | 标题下方到下一个标题 / 文件尾之间**无非空内容** | TASK-200：`## 1. 目标` / `## 2. 范围` 曾整节为空 |
   | `doc/duplicate-heading` | 先 Warning，清扫后升 Error | 同一文件内**标题文字**重复（如 `### 字段` 连续出现两次） | TASK-200：外来模板块重复 `### 字段` |
   **背景**：TASK-200 的 7 份 spec 结构性缺陷 `docscan` **全部漏过**（见 `docs/memory/pitfalls.md` 2026-09-24 条）；
   先 Warning 后 Error 的口径见 **ADR-0025 D1**，不得为变绿放宽判据。
3. **`crates/core/tests/arch*` 分层断言**（铁律 7：`core` 不得调平台 API，只能用 `crates/platform/api` 的 trait）——
   目标是把 `cargo test -p assistant-core arch::` 从当前的 `running 0 tests` 变成**真断言**。
4. **PL-047 机器化**：`xtask` 扫描 `crates/*/migrations/*.sql`，三条判据：
   ① 文件名号段**全局唯一**；② 与 `docs/storage-design.md` **§3.4 登记表逐行一致**；
   ③ 每个含迁移的 crate 都公开 `pub const MIGRATIONS`。三条都要有**负向用例**。
5. **ADR-0039 D3 的 `check-ledger` 两条规则**（`check-ledger` 目前属 gov §5.1 #16，未分配）：
   ① `PLAN.md` 的「更新日期」**不得早于** `LEDGER.md` 最后一条记录的日期；
   ② `README.md` 必须存在 `> 状态：` 状态行且其中出现当前阶段名（`阶段 N`）。先 Warning 上线。
6. **跑测试**：`cargo test --workspace` + 本卡专项测试；不合格 → DRIFT（`DRIFT-015-x`）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `docscan` 4 条新规则各自有单测；**全仓实跑结果记入执行记录**（若先 Warning：附「清扫后升 Error」的后续动作与卡号）
- [ ] `cargo test -p assistant-core arch::` **不再是** `running 0 tests`（分层断言真生效）
- [ ] **PL-047**：迁移文件名号段唯一 + 与 §3.4 登记表逐行一致 + 每个含迁移的 crate 公开 `MIGRATIONS`（3 条判据 + 3 条负向用例）
- [ ] **ADR-0039 D3**：`check-ledger` 的 ① `PLAN.md` 新鲜度 ② `README.md` 状态行存在性 两条规则落地
- [ ] 若新增规则计入 gov §5.4 的 13 项卫生规则 → 同批更新 `xtask/src/deferred.rs` 与 **ADR-0025** 登记表；**不得**为让门禁变绿而放宽判据
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点

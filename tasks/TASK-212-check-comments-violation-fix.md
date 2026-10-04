# TASK-212　修掉 `check-comments` 首次真跑发现的 9 处真实违规

- 状态：**Done（2026-09-30；LEDGER Done + PR #104 / merge `5844326`；check-comments 0 error）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：087
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：TASK-087 §5 **DRIFT-087-1**、`docs/spec/naming.md` §5/§8/§10、ADR-0019

## 目标

把 TASK-087 的 `check-comments` 首次真跑报出的 **9 条真实 Error** 修掉，使该门禁在仓库上为绿，
从而 TASK-087 可以按 ADR-0019「先证明门禁会红，再接入阻断」把它接成 CI 硬门禁。

## In scope

- `crates/policy/src/dsl.rs`、`crates/ipc/src/frame.rs`：补公共 API 文档注释（规则 ③）。
- `crates/platform/windows/src/uia/actions.rs`、`crates/platform/windows/src/uia/tree.rs`：
  给缺说明的 `unsafe` 块补 `// SAFETY:`（规则 ⑤）。
- 本卡记录。

## Out of scope

- 修改上述 crate 的**任何可执行语句或行为**（本卡只加注释）。
- 放宽 `check-comments` 的任何判据（那是"为让门禁变绿而放宽规则"，禁止）。
- TASK-087 的其余项（三类 canary / CI 接入 / ADR-0019 登记表 / gate-selftest 实跑）。

## 必须遵守

- 只加注释：`git diff` 里不得出现非注释行的改动。
- `// SAFETY:` 必须写清"**为什么**这个 `unsafe` 是安全的"，禁止"标准调用"这类无信息文本（naming §8）。
- 公共 API 文档必须含语义与错误语义（naming §5）。
- 不得为了让 `check-comments` 变绿而删除或弱化测试。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- check-comments
cargo run --package xtask -- hygiene
```

## 完成定义（DoD）

- [ ] 9 处违规全部修掉，`cargo run -p xtask -- check-comments` 报 **0 error**。
- [ ] diff 只含注释行（无任何可执行语句改动）。
- [ ] `cargo test --workspace` 全绿。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-212 修掉 check-comments 首次真跑发现的 9 处真实违规
【目标】只加注释清零 9 处真实违规，使 check-comments 可作为 CI 硬门禁接入
【write scope】仅：crates/policy/src/dsl.rs、crates/ipc/src/frame.rs、
               crates/platform/windows/src/uia/{actions,tree}.rs、本卡记录
【铁律】无静默失败；公共 API 文档必须含语义与错误语义；SAFETY 必须说明为什么安全；
        不放宽门禁；不改可执行语句
【禁止】修改任何可执行语句；做 TASK-087 剩余项；改其他 crate/文档
【验收】cargo fmt/clippy/test --workspace、xtask check-comments、xtask hygiene → 全绿
【依赖】TASK-087（PR #104 的 check-comments 首次实跑已产出本卡 9 处违规）
【疑问】无
```

### 2. 实际改动文件

| 文件 | 改动 |
|---|---|
| `crates/policy/src/dsl.rs` | 为 `parse_rule_set` 补语义、确定性、幂等性与错误语义文档 |
| `crates/ipc/src/frame.rs` | 为 `FramePrefix` 补纯值类型语义与边界说明 |
| `crates/platform/windows/src/uia/actions.rs` | 为 6 处 `GetCurrentPatternAs` unsafe 调用补 COM/STA/所有权安全性说明 |
| `crates/platform/windows/src/uia/tree.rs` | 为 `CurrentIsEnabled` unsafe 调用补 BOOL/COM 安全性说明 |

### 3. 验收输出摘要

```text
git diff --check                          → PASS
git diff（4 个 crate 文件）              → 仅 30 行新增注释，0 行可执行语句改动
cargo fmt --all --check                   → PASS
cargo clippy --all-targets -- -D warnings → PASS
cargo test --workspace                    → PASS（0 failed）
cargo run -p xtask -- check-comments      → PASS：scanned=292，0 error / 67 warning
cargo run -p xtask -- hygiene             → PASS：0 error / 4 warning（既有基线）
```

### 4. DoD 逐条核对

- [x] 9 处违规全部修掉，`check-comments` 报 **0 error**。
- [x] diff 只含注释行，无任何可执行语句改动。
- [x] `cargo test --workspace` 全绿。
- [x] 未修改 Out of scope 文件。

### 5. 偏差

none。按人类派单的推荐方案 ① 立本卡修复；未放宽任何规则。

### 6. 更合理做法

无。本卡刻意保持最小 diff：只加文档/SAFETY 注释，不重构、不改格式以外的行为。

### 7. 遗留问题

无本卡遗留。`check-comments` 仍有 67 条 Warning 级存量（缩写/受控词汇/模块头），不是本卡 Error 级阻断项；后续是否清扫需另立治理卡。

### 8. 新增长期记忆

无新增 `docs/memory/*` 条目（修复内容是本仓库一次性真实违规，不形成跨项目长期结论）。

### 9. 给审阅者的关注点

1. `actions.rs` 6 处新增 SAFETY 都是 `GetCurrentPatternAs`：核对是否写清 COM 引用、STA 线程、所有权与缓冲区边界。
2. `dsl.rs` 的错误文档是否完整覆盖 InvalidRuleSet 的全部触发面（JSON/结构/未知字段/枚举）。
3. `tree.rs` 新增说明必须与实际 BOOL 转换路径一致，不能泛化为无信息“标准调用”。

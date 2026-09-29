# TASK-212　修掉 `check-comments` 首次真跑发现的 9 处真实违规

- 状态：**Ready**
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
cargo run -p xtask -- hygiene
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

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点

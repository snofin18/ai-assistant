# TASK-054　xtask render() 返回 Result — 消除 per-line unwrap_used 允许

- 状态：**InProgress**
- 阶段：0　子阶段：—　依赖：001 / 051 / 052 / 053　预估：S　阻塞主线：否
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。

---

- 依赖：TASK-001 / TASK-051 / TASK-052 / TASK-053（均已 Done）　预估：S（≤ 30 min）　难度：S
- **write scope**：
  - `tasks/TASK-054-...md`（本卡文件）
  - `xtask/src/refscan.rs`（改 `render` 签名 + `run` 适配）
  - `README.md`（同步本卡进展）
  - `LEDGER.md`（追加本卡一行）
  - `docs/PARKING_LOT.md`（关闭 PL-NEW 跟进项）
- **Out of scope**：
  - workspace `[lints.clippy]` 配置**不动**（TASK-052 人类裁决 B 路 = 不动 workspace）
  - 其他 xtask 子命令的 `render` / `write` 模式（refscan 独有模式）
  - 新建 ADR（TASK-052 §9 #1 决策已在本卡实施，无需 ADR 路径）

**背景与源流**

- TASK-052 / TASK-053 都遗留一个开放项：render 函数（refscan.rs:292）用 `.expect("...")` 处理 `writeln!` 到 `String` 的 Result，因 `writeln!` 到 `String` 永不失败（仅 OOM），`expect` 等同于 panic。`#[allow(clippy::unwrap_used, clippy::expect_used)]` per-line allow 被加上作 bypass。
- 人类在 TASK-051 / TASK-052 / TASK-053 三轮 review 中反复提出这一处应改成 `render() -> Result<String, std::fmt::Error>`，把错误上抛到 `run() -> Result<u8, String>`。
- 本卡实施 B 路径（人类推荐）：改签名 + 删 per-line allow + 错误上抛。
- 严格遵循 TASK-052 路线 B（纯重构 = 不动 workspace lints = 漂移触发器 ⑥ 0 命中）。

**In scope 清单**

1. `pub fn render(findings: &[Finding]) -> String` → `pub fn render(findings: &[Finding]) -> Result<String, std::fmt::Error>`
2. render 内部 `writeln!(...).expect("...")` → `writeln!(..)?`
3. 删除 render 前的 `#[allow(clippy::unwrap_used, clippy::expect_used)]`
4. `run()` 中 `render(&findings).as_bytes()` → `render(&findings)?.as_bytes()`（用 `?` operator）
5. README.md 同步本卡落地
6. 跑全套 11 条验收 + 写执行记录 9 节 + 更新 LEDGER

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy -p xtask --all-targets -- -D warnings        # exit 0（per-line allow 移除后仍无新警告）
cargo test --workspace                                 # 279 passed（无增删）
cargo deny check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan                            # 行为不变（仍报 151 errors）
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
grep -nE '#!\[allow\(clippy::unwrap_used|#!\[allow\(clippy::expect_used' \
     xtask/src/refscan.rs
# 期望：0 命中（per-line allow 已删）
grep -nE '\.expect\(' xtask/src/refscan.rs
# 期望：仅在 #[cfg(test)] 测试代码里有 .expect()（如有）
```

**DoD**

- [ ] `pub fn render` 返回 `Result<String, std::fmt::Error>`
- [ ] render 内部无 `#[allow(clippy::unwrap_used, clippy::expect_used)]`
- [ ] render 内部无 `.expect("...")` 处理 writeln! Result
- [ ] `run()` 用 `?` operator 上抛 render 错误
- [ ] `cargo clippy -p xtask --all-targets -- -D warnings` exit 0
- [ ] `cargo test --workspace` 全绿（279 passed）
- [ ] 全部 11 条 xtask 验收全绿
- [ ] `README.md` 同步本卡落地（新增 sub-bullet）
- [ ] `LEDGER.md` 追加本卡一行

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope）。 -->

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

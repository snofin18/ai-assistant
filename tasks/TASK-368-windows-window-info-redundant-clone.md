# TASK-368 Windows WindowInfo redundant clone

- 状态：**Review**
- 阶段：1　子阶段：CI 修复　批次：Stage 1 closeout follow-up
- 依赖：TASK-367、CI run `43780062585`
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

## write scope

- `tasks/TASK-368-windows-window-info-redundant-clone.md`
- `tasks/TASK-367-t5-2-ten-run-human-approval.md`
- `crates/platform/windows/src/window/mod.rs`
- `PLAN.md`
- `plans/stage-1-pilots.md`
- `README.md`
- `LEDGER.md`

## 目标

修复 Rust 1.100 stable Clippy 新增 `redundant_clone` 在 Windows `WindowInfo` 列表构造中的
命中，恢复 post-merge main CI，并同步 TASK-367 的最终 shell misuse 审计值。

## In scope

- 移除 `crates/platform/windows/src/window/mod.rs` 中已经拥有所有权的 `record.title.clone()`。
- 运行目标 crate 的 fmt / clippy / tests，再通过 PR 全门禁和 post-merge main CI 验证。
- 将 TASK-367 收口审计的 shell misuse 从 `bash=27` 更正为本轮最终 `bash=29` / `cmd=0`。
- 同步本卡、PLAN、阶段计划、README、LEDGER 的 Done 状态。

## Out of scope

- 不修改公共 API、Tool/UI/DB schema、Policy、依赖或其他生产逻辑。
- 不做顺手重构、不增加 `#[allow]`、不放宽 `clippy -D warnings`。
- 不改写 TASK-367 十轮证据或矩阵结果。

## 必须遵守

1. 只移除被 Clippy 证明多余的 clone，不改变 `WindowInfo` 字段语义或所有权边界。
2. 不以 lint allow、版本降级或 CI 判据修改掩盖问题。
3. PR 必须 11/11 checks，merge 后 main CI 必须 success。
4. shell misuse 必须如实记录，且所有后续命令显式使用 PowerShell。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-platform-windows
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- check-ledger
git diff --check
```

期望：Windows crate 与 workspace clippy 全绿；PR checks 11/11；merge 后 main CI success。

## DoD

- [ ] `redundant_clone` 修复且行为不变。
- [ ] 目标 crate 与 workspace 验证全绿。
- [ ] TASK-367 shell misuse 终值同步为 `bash=29` / `cmd=0`。
- [ ] PR checks 11/11，merge 与 post-merge main CI 证据回填。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-368 Windows WindowInfo redundant clone
【目标】修复 Rust 1.100 Clippy 新增的 redundant_clone 并恢复 main CI
【write scope】仅：TASK-368 卡、TASK-367 终值回填、window/mod.rs、PLAN/plans/README/LEDGER
【铁律】不静默扩 scope；不放宽 lint；不伪造 CI；先验证真实失败原因
【禁止】其他生产重构、公共接口/schema/依赖变更、旧证据改写
【验收】all-targets clippy 通过；PR 11/11；merge 后 main CI success
【依赖】TASK-367 Done；main `7eb2124` 的 Windows Clippy 红门已确认
【疑问】无；人类已批准新建 TASK-368
```

### 2. 实际改动文件

- `crates/platform/windows/src/window/mod.rs`：移除已拥有 `title` 的多余 clone。
- `tasks/TASK-367-t5-2-ten-run-human-approval.md`：把 shell misuse 终值更正为
  `bash=29` / `cmd=0`。
- 本卡：状态、验收与边界记录。

### 3. 验收输出摘要

- GitHub main CI `43780062585` 的 Windows 失败日志确认 Rust 1.100 Clippy 命中
  `crates/platform/windows/src/window/mod.rs:149 redundant_clone`。
- `cargo fmt --all --check`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过；只有仓库既有
  `clippy::assert_is_empty` unknown-lint 警告。
- `cargo test -p assistant-platform-windows`：138 passed、0 failed、5 ignored；
  12/7/4 个集成测试与 1 个 doctest 通过。
- `cargo test --workspace`：通过；xtask 510/510，全部 doctest 通过。
- pre-merge `shell_misuse` 复核：`bash=29` / `cmd=0`，全部由 ADR-0100 hook 重写。

### 4. DoD 逐条核对

- [x] `redundant_clone` 修复且行为不变。
- [x] 目标 crate 与 workspace 验证全绿。
- [x] TASK-367 shell misuse 终值同步为 `bash=29` / `cmd=0`。
- [ ] PR checks 11/11，merge 与 post-merge main CI 证据回填。

### 5. 偏差

无。只改一个已拥有值的 clone；未改公共 API、schema、Policy 或依赖。

### 6. 实施中发现的更合理做法

新 stable Clippy 的 lint 漂移应由真实生产最小修复吸收，不通过 allow、lint 降级或
CI 判据修改绕过。

### 7. 遗留问题

PR、merge 与 post-merge main CI 证据待回填。

### 8. 新增长期记忆

待 PR 收口时判断是否需要记录 stable toolchain lint 漂移坑。

### 9. 给审阅者的关注点

- `record.title` 移动后是否仍保持 `WindowInfo` 字段语义。
- PR 的 Windows Rust 1.100 Clippy 是否真正变绿。
- TASK-367 shell count 是否已由 27 更正为 29。

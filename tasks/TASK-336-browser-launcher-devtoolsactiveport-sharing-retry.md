# TASK-336　Browser launcher DevToolsActivePort sharing retry

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：基础设施修复
- 依赖：TASK-110、TASK-335、`DRIFT-056-8`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`crates/platform/windows/src/browser_launcher.rs`、
  `crates/platform/windows/src/browser_launcher_tests.rs`、
  `crates/platform/windows/README.md`、`docs/memory/facts.md`

## 目标

在既有 launch readiness deadline 内，把 Windows `DevToolsActivePort` 打开时出现的
sharing violation 32 / lock violation 33 视为“尚未 ready”并继续有界轮询，同时让
NotFound、ACL、非法内容和其他 IO 错误保持显式失败。

## In scope

- 仅修改 launcher 对 `DevToolsActivePort` 打开错误的分类与有界等待行为。
- 增加真实 Windows 独占打开文件的回归，证明 sharing violation 返回“尚未 ready”。
- 增加纯错误分类测试，锁定 32 / 33 可重试，其他错误不可重试。
- 更新平台 README 的 launcher 不变量与已知限制。

## Out of scope（做了算漂移）

- 不执行 TASK-056 的真实 Edge 本地 10/10 或外部 `books.toscrape.com` 3/3。
- 不修改公共 trait、Tool schema、ErrorCode、依赖、Policy、CSV 契约或测试断言。
- 不重构 launcher、进程树回收、profile 校验、CDP transport 或其他浏览器模块。
- 不为任意 `PermissionDenied` 或未知 IO 错误建立重试兜底。

## 必须遵守

1. 只将 Windows raw OS error 32 / 33 视为 transient sharing/lock violation。
2. transient 分支必须继续受既有 launch timeout 约束，不能形成无界重试。
3. `NotFound` 继续按尚未 ready 处理；ACL、非法内容、超限和其他 IO 错误必须显式失败。
4. 失败路径继续在返回错误前终止完整 browser 进程树并有界 reap（ADR-0063）。
5. 回归必须使用真实 Windows 独占文件句柄触发 sharing violation，不得只 mock 错误码。

## 验收命令

```powershell
cargo test -p assistant-platform-windows browser_launcher
cargo test -p assistant-platform-windows
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- check-migrations
cargo deny check
```

## 完成定义（DoD）

- [ ] Windows sharing violation 32 / lock violation 33 在 launch deadline 内重试。
- [ ] 真实 Windows 独占打开 `DevToolsActivePort` 的回归返回“尚未 ready”，不是 Fatal。
- [ ] NotFound、ACL、非法内容、超限及其他 IO 错误行为未被放宽。
- [ ] 专项、workspace、arch、schema/codegen、全部文档门禁与 deny 全绿。
- [ ] 不声称 TASK-056 或 Stage 1c 已完成，真实浏览器验收继续留给后续人类会话。
- [ ] LEDGER、计划状态与必要长期事实同步。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-336 Browser launcher DevToolsActivePort sharing retry
【目标】把 Windows sharing/lock violation 纳入既有 deadline 内的 readiness 重试
【write scope】仅：browser_launcher.rs、browser_launcher_tests.rs、platform/windows README、facts
【铁律】1 无静默失败 / 4 写操作有 postcondition / 9 不扩范围 / 12 失败路径有界回收
【禁止】不改公共契约/依赖；不放宽 ACL/非法内容；不跑真实 GUI；不冒充 TASK-056 Done
【验收】launcher 专项、真实独占打开、workspace、arch、schema/codegen、文档门禁、deny → 全绿
【依赖】TASK-110 / TASK-335 Done，DRIFT-056-8 已核对，N_auto=0
【疑问】无
```

### 2. 实际改动文件

- `crates/platform/windows/src/browser_launcher.rs`
- `crates/platform/windows/src/browser_launcher_tests.rs`
- `crates/platform/windows/README.md`
- `docs/memory/facts.md`
- 本卡、`PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`
- `docs/automations/2026-10-27-round-1.md`

### 3. 验收输出摘要

- `cargo test -p assistant-platform-windows browser_launcher`
  → **9 passed / 0 failed / 1 ignored**；真实 Windows 独占打开
  `DevToolsActivePort` 的回归通过。
- `cargo test -p assistant-platform-windows`
  → lib **138 passed / 0 failed / 5 ignored**；CDP 契约 **12/12**、
  handle-discipline **7/7**、synthetic-input **4/4**、doctest **1/1**。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有
  `clippy::assert_is_empty` unknown-lint warning。
- `cargo test --workspace` → exit 0，所有测试套件 failed=0。
- `cargo test -p assistant-core arch::` → **10/10 passed**。
- `verify-schemas` → 5 份 schema、0 error；`codegen --check` → 0 drift / 0 error。
- `hygiene` / `memory-counts` / `adr-index` / `refscan` / `docscan` /
  `card-check` / `check-ledger` / `check-comments` / `check-migrations` → 全部 exit 0。
- `cargo deny check` → advisories / bans / licenses / sources 全部 ok。
- `cargo llvm-cov --fail-under-lines 75` → exit 0，TOTAL 行覆盖 **79.79%**。
- `cargo run -p xtask -- replay --suite core` → 2 steps / 5 changes / 0 mismatch。
- PR [#603](https://github.com/snofin18/ai-assistant/pull/603) →
  [CI run 40479322481](https://github.com/snofin18/ai-assistant/actions/runs/40479322481)
  **11/11 success**；base=main、`MERGEABLE`、`CLEAN`、pending / failed 均为 0。
- merge commit `a4777ae2675b42b441f20160fd3be549b48ddc34`；merge 后
  [main CI run 40480883072](https://github.com/snofin18/ai-assistant/actions/runs/40480883072)
  success（10 success + 1 预期 skipped）。

### 4. DoD 逐条核对

- [x] Windows sharing violation 32 / lock violation 33 在 launch deadline 内重试。
- [x] 真实 Windows 独占打开 `DevToolsActivePort` 的回归返回“尚未 ready”，不是 Fatal。
- [x] NotFound、ACL、非法内容、超限及其他 IO 错误行为未被放宽。
- [x] 专项、workspace、arch、schema/codegen、全部文档门禁与 deny 全绿。
- [x] 未声称 TASK-056 或 Stage 1c 已完成；真实浏览器验收继续留给后续人类会话。
- [x] LEDGER、计划状态与长期事实同步。

### 5. 偏差

- 当前 automation rollout 有 **21** 次 `exec_command` 误填 `bash`：其中 2 次 exit 0
  （Git 状态、`git switch`）、16 次 exit 127、1 次 exit 1、2 次 exit 2；未通过 bash
  写入产品文件或替代任何成功验收。全部误用已实时写入外部 JSONL。
- exit 1 的 PR 创建调用把 PowerShell here-string 与反引号交给 bash，在仓库根生成 81 个
  未跟踪文件，且未创建 PR；逐项验证它们均为根目录普通文件后以 `git clean -f`（不带
  `-d` / `-x`）清理，复验未跟踪文件为 0，已跟踪文件未受影响。2 次 exit 2 调用是
  JSONL 审计/补记命令误由 bash 解析，未产生有效审计结果或仓库写入；另有 1 次
  `card-check` 误由 bash 执行，因 cargo 不在 PATH 而 exit 127，随后以 PowerShell 重跑通过。
- 本卡不执行 TASK-056 真实 Edge 10/10 或外部站点 3/3，因此 TASK-056 仍为 Blocked。

### 6. 更合理做法

- 在单次 `File::open` 的错误点只识别 raw OS error 32 / 33 并返回 pending，不引入新的
  retry 框架；既有 `wait_for_active_port` deadline、process liveness 检查与失败前进程树回收
  全部保持不变。这样竞态会自然收敛，ACL 或其他 IO 错误仍直接失败。

### 7. 遗留问题

- TASK-056 本地仍为 9/10，外部 3/3 未开始；合并后需人类在场重跑完整 TASK-056 验收。

### 8. 新增长期记忆

- FACT：Windows `DevToolsActivePort` 的 raw OS error 32 / 33 是浏览器写句柄尚未释放的
  readiness 竞态，应在既有 launch deadline 内重试；ACL 5、invalid parameter 87 等不得重试。

### 9. 给审阅者的关注点

- 重点检查重试判据是否只匹配 raw OS error 32 / 33，没有把一般 `PermissionDenied` 或其他
  IO 错误纳入重试。
- 真实独占打开测试必须验证 sharing violation 被处理为 pending；确认测试失败路径仍会清理
  临时文件和 browser 进程树。
- 本卡只解除一个 launcher 前置条件，不证明 TASK-056 10/10 或外部验收完成。

# TASK-340　HTML raw-text tokenizer

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：基础设施修复
- 依赖：TASK-330、TASK-338、`DRIFT-056-12`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`apps/agent-core/src/browser_task_csv_html.rs`、
  `apps/agent-core/src/browser_task_csv_tests.rs`

## 目标

为 T5.1 DOM tokenizer 增加 `<script>` / `<style>` raw-text 状态，使脚本或样式正文
中的 `<`、`<\/script>` 等文本不再被误解析为标签，同时保持所有畸形输入 fail-closed。

## 必须遵守

1. 只有大小写不敏感的精确 `</script>` / `</style>` 结束符可退出 raw-text。
2. `<\/script>`、`</scriptx>`、普通 `<div>` 等 raw-text 内部文本必须作为文本忽略。
3. 未找到真实结束符时必须保留未闭合元素错误，不能静默截断 DOM。
4. 不通过字符串替换、正则清洗或通用 HTML5 解析器绕过问题。

## Out of scope

- 不执行真实外部 3/3；修复合并后由 TASK-056 重跑。
- 不改 CSV schema、商品字段、origin、60 行断言、公共 API 或依赖。
- 不解析 JavaScript 或 CSS 内容。

## 验收命令

```powershell
cargo test -p assistant-agent-core browser_task_csv
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

## DoD

- [ ] 脚本中的 `<\\/script>` 作为文本处理，不报非法标签。
- [ ] script/style 大小写不敏感的真实闭合标签可正常结束。
- [ ] `</scriptx>` 等伪闭合不结束 raw-text。
- [ ] 未闭合 script/style 继续显式失败。
- [ ] 上述验收命令全绿；真实外部 3/3 留给 TASK-056。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-340 HTML raw-text tokenizer
【目标】script/style 内容不参与标签解析，且畸形输入继续 fail-closed
【write scope】仅：browser_task_csv_html、browser_task_csv_tests、TASK-340 与状态同步
【铁律】1 无静默失败 / 2 DOM 不可信 / 8 资源有界 / 9 不扩范围 / 10 契约先行
【禁止】不做字符串替换、不改 CSV/公共契约、不放宽未闭合或非法标签断言
【验收】escaped close、script/style、大小写、伪闭合、未闭合回归 + 全量门禁 → 全绿
【依赖】DRIFT-056-12、用户批准
【疑问】无
```

### 2. 实际改动文件

- `apps/agent-core/src/browser_task_csv_html.rs`
- `apps/agent-core/src/browser_task_csv_tests.rs`
- `tasks/TASK-340-html-raw-text-tokenizer.md`
- `PLAN.md` / `plans/stage-1-pilots.md` / `README.md` / `LEDGER.md`
- `docs/memory/pitfalls.md` / `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core browser_task_csv` → **13/13 passed**；
  escaped `<\\/script>`、`</scriptx>`、script/style、大小写闭合和未闭合失败均通过。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有 unknown-lint warning。
- `cargo test --workspace` → **497 passed / 0 failed**，doc-tests 全绿。
- `cargo test -p assistant-core arch::` → **10/10 passed**。
- `verify-schemas` → 5/5；`codegen --check` → 0 drift / 0 error。
- `hygiene` → 0E / 151W；`memory-counts` → 0E / 1W；
  `adr-index` / `refscan` / `check-ledger` / `check-migrations` → 0E。
- `docscan` → 0E / 175W；`card-check` → 0E / 35W；
  `check-comments` → 0E / 75W。
- `cargo deny check` → advisories / bans / licenses / sources all ok。

### 4. DoD 逐条核对

- [x] 脚本中的 `<\\/script>` 作为文本处理，不报非法标签。
- [x] script/style 大小写不敏感的真实闭合标签正常结束。
- [x] `</scriptx>` 等伪闭合不结束 raw-text。
- [x] 未闭合 script/style 继续显式失败。
- [x] 验收命令全绿；真实外部 3/3 留给 TASK-056。

### 5. 偏差

- 无功能漂移。当前 rollout shell misuse 机器审计更新为 **165** 次；
  本卡新增 2 次均为 `Get-Content` / guard release 在 bash 中 command-not-found，
  未执行目标、未写仓库，随后用 PowerShell 重跑。

### 6. 更合理做法

- 使用 tokenizer 状态机而不是字符串替换：进入 script/style 后，只有精确且大小写
  不敏感的闭合标签能恢复普通标签解析；raw-text 内其余字符一律作为有界文本。

### 7. 遗留问题

- TASK-340 合并后由 TASK-056 使用全新 profile 重跑外部
  `books.toscrape.com` 3/3，并保留严格 CSV validator。

### 8. 新增长期记忆

- PITFALL：script/style 的 raw-text 中可能出现 `<\/script>`、伪闭合或其他 `<`；
  tokenizer 必须保持 raw-text 状态，直到真实结束标签。

### 9. 给审阅者的关注点

- 确认只用精确 `</script>` / `</style>` 退出 raw-text，`</scriptx>` 不会误闭合。
- 确认未闭合 raw-text 仍导致显式错误，而不是静默丢弃剩余 DOM。
- 确认没有字符串替换、通用 HTML5 parser、新依赖或公共契约变化。

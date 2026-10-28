# TASK-338　Browser CSV title anchor 去歧义

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：基础设施修复
- 依赖：TASK-330、TASK-056 的 `DRIFT-056-10`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`apps/agent-core/src/browser_task_csv_page.rs`、
  `apps/agent-core/src/browser_task_csv_tests.rs`

## 目标

修正 T5.1 商品卡 DOM 归一化对标题锚点的过度识别：图片容器中的 `a[href]`
不得被当作 title，只有显式标题契约才能建立商品标题和 URL。

## 必须遵守

1. 标题字段只接受显式 `data-field="title"`、`title` 属性或
   `data-product-url` 契约；不得仅凭 `href` 推断 title。
2. 保持商品行、字段完整性、60 行、重复 URL 与 origin 校验的 fail-closed 语义。
3. 增加真实 `books.toscrape.com` 商品卡形状回归：图片链接触发区位于标题链接之前。
4. 增加无显式标题锚点时返回缺失 title 的负向回归，禁止静默选择 href-only 锚点。

## Out of scope

- 不执行真实 Edge 或外部 `books.toscrape.com` smoke；由 TASK-056 在合并后继续。
- 不改 Tool schema、公共 API、ErrorCode、依赖、Policy、CSV 列或 validator。
- 不放宽重复字段、60 行、字段完整性或跨 origin 断言。

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

- [ ] 图片容器中的 href-only 锚点不再占用 title 字段。
- [ ] 真实 books 商品卡形状可提取唯一标题、价格、库存、评分与商品 URL。
- [ ] 无显式标题时显式失败为缺少 title。
- [ ] 既有 60 行 fixture、UTF-8/CSV、origin 与重复字段契约保持全绿。
- [ ] 上述验收命令全绿；真实外部 3/3 留给 TASK-056。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-338 Browser CSV title anchor 去歧义
【目标】阻止图片 href 冒充 title，同时保持严格商品行与 CSV 契约
【write scope】仅：browser_task_csv_page、browser_task_csv_tests、TASK-338 与状态同步
【铁律】1 无静默失败 / 2 DOM 不可信 / 4 可验证 postcondition / 9 不扩范围 / 10 契约先行
【禁止】不改公共 schema/API/ErrorCode/依赖/Policy；不放宽 60 行和字段完整性断言
【验收】真实 DOM 形状正负回归、现有 CSV 测试、workspace 与全量门禁 → 全绿
【依赖】TASK-330、DRIFT-056-10；TASK-056 外部验收证据已核对
【疑问】无
```

### 2. 实际改动文件

- `apps/agent-core/src/browser_task_csv_page.rs`
- `apps/agent-core/src/browser_task_csv_tests.rs`
- `tasks/TASK-338-browser-csv-title-anchor.md`
- `PLAN.md` / `plans/stage-1-pilots.md` / `README.md` / `LEDGER.md`
- `docs/memory/pitfalls.md` / `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core browser_task_csv` → **10 passed**；真实
  books 商品卡形状和 href-only 负向回归均通过。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有
  `clippy::assert_is_empty` unknown-lint warning。
- `cargo test --workspace` → exit 0，workspace **497 passed / 0 failed**，
  doc-tests 全部通过。
- `cargo test -p assistant-core arch::` → **10/10 passed**。
- `verify-schemas` → 5/5 OK；`codegen --check` → 0 drift / 0 error。
- `hygiene` → 0E / 150W；`memory-counts` → 0E / 1W；
  `adr-index` → 0E / 0W；`refscan` → 0E / 0W；
  `docscan` → 0E / 175W；`card-check` → 0E / 35W；
  `check-ledger` / `check-comments` / `check-migrations` → PASSED。
- `cargo deny check` → advisories / bans / licenses / sources all ok；仅有既有
  duplicate / unused-license warning。

### 4. DoD 逐条核对

- [x] 图片容器中的 href-only 锚点不再占用 title 字段。
- [x] 真实 books 商品卡形状可提取唯一标题、价格、库存、评分与商品 URL。
- [x] 无显式标题时显式失败为缺少 title。
- [x] 既有 60 行 fixture、UTF-8/CSV、origin 与重复字段契约保持全绿。
- [x] 验收命令全绿；真实外部 3/3 留给 TASK-056。

### 5. 偏差

- 无功能漂移。当前 rollout shell misuse 最终机器审计更新为 **144** 次；
  本卡新增 4 次：3 次只读 `Get-Content` / `cargo` 在 bash 中 command-not-found，
  未执行目标；1 次 `git commit` 在 bash 中成功，仅创建仓库提交元数据，文件内容来自
  已审查暂存区；随后以 PowerShell 重写同一提交。

### 6. 更合理做法

- href 只能说明节点是链接，不能说明它拥有 title 语义。标题锚点应有显式
  `data-field="title"`、`title` 属性或 `data-product-url` 契约；这样真实页面和
  fixture 共用同一判定，不需要为外部站点增加特殊分支。

### 7. 遗留问题

- TASK-338 合并后由 TASK-056 使用最新 `main` 重跑外部
  `books.toscrape.com` 3/3；每轮仍须三页、60 行并通过严格 CSV validator。

### 8. 新增长期记忆

- PITFALL：books 商品卡图片链接先于标题链接出现时，任何 `a[href]` 都识别为
  title 会导致 duplicate title；href-only 锚点不携带 title 契约，必须忽略。

### 9. 给审阅者的关注点

- 确认 title 识别只移除了 href-only 分支，`data-field`、`title` 和
  `data-product-url` 显式路径仍可工作。
- 确认重复字段、商品 URL、origin、60 行和字段完整性断言没有放宽。
- 真实外部 3/3 不在本卡内执行，合并后由 TASK-056 取证。

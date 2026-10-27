# TASK-330　Browser task DOM 到 T5.1 CSV 规范化器

- 状态：**Done**
- 阶段：1　子阶段：**1c**　批次：治理/基础设施　依赖：TASK-329、TASK-107、TASK-112、TASK-114
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文；以下为执行记录。

---

**目标**

在 `assistant-agent-core` 的私有 `--browser-task-run` 后增加显式 CSV 产出：
复用现有真实执行器完成三页 T5.1 task-package，只有全部步骤 `Committed` 后，才把
`page_1_dom` 到 `page_3_dom` 规范化为 TASK-056 已批准的 60 行 CSV，并以
create-new、sync、读回逐字节校验的边界写出。

本卡只负责 DOM → CSV 的产品后置阶段，不把脚本化测试当成真实 Edge 验收。

**In scope**

- `--csv-output <absolute-new-path>` 与 `--csv-base-url <origin>` 私有 CLI 参数。
- 独立的私有 HTML tokenizer / product-article parser 子模块，避免单个文件超过治理上限。
- 从已提交 runner outputs 读取三页 DOM，不使用页面内指令。
- 支持 TASK-056 离线 fixture 与 `books.toscrape.com` 的公开列表结构：
  `article.product_pod` / `article.product-pod`、标题 anchor、价格、库存与 star-rating。
- 固定 CSV header、60 行、UTF-8 无 BOM、CRLF 与 RFC 4180 quoting。
- 严格拒绝缺页、缺行、重复字段、未知实体、跨 origin URL、重复 URL、非法价格/评分、
  空字段、超出 DOM 字节上限或输出路径已存在。
- JSON 与 CSV 都只创建新文件；任一写出/读回失败都显式失败并清理本流程已创建的文件。
- 自动化测试只使用内存 outputs 与本地静态字符串，不启动浏览器、不访问网络。

**Out of scope**

- 不启动真实 Edge/Chrome，不访问 `books.toscrape.com`，不执行 10/10 或 3/3 smoke。
- 不修改 TASK-056 正文，不把 TASK-056 改成 Done。
- 不实现 click、form fill、submit、download、screenshot 或 browser close provider。
- 不修改公共 trait、Tool schema、ErrorCode、IPC、DB schema、task-package schema、ADR 或 spec。
- 不新增第三方依赖，不新增通用 HTML/表达式引擎。

**必须遵守**

1. DOM 是 `app_content` 不可信输入；解析失败必须显式报错，不能跳过坏行后继续。
2. 只有六步全部提交后才允许生成 CSV；不得为未完成步骤留下成功 CSV。
3. `--csv-output` 和 `--csv-base-url` 必须成对出现，路径为绝对新路径且父目录已存在。
4. CSV 的 header、页/行序号、价格、币种、库存、评分和 URL 必须逐项校验。
5. JSON/CSV 写出后必须读回逐字节一致；失败路径不得静默覆盖。
6. 真实 Edge、专用 profile 与外部 smoke 仍只允许人类在场会话。

**验收命令**

```powershell
cargo test -p assistant-agent-core --bin assistant-agent-core browser_task_csv
cargo test -p assistant-agent-core --bin assistant-agent-core browser_task_runtime
python -m unittest discover -s eval/tasks/edge/t5.1 -p "test_*.py" -v
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

**DoD**

- [ ] 三页 DOM 规范化结果为 header + 60 行，列型与现有 Python validator 完全一致。
- [ ] fixture 的逗号、双引号、非 ASCII、HTML entity 与绝对 URL 均被逐字节匹配。
- [ ] 真实站点常见相对 URL、`product_pod`、`price_color`、`star-rating` 结构有离线测试。
- [ ] 所有负向输入显式失败，不产生成功 CSV。
- [ ] JSON/CSV create-new、读回一致、失败清理有测试。
- [ ] 全量验收通过；TASK-056 保持 Blocked。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。 -->

## 执行记录

### 1. 约束回执

见本会话约束回执。预授权在本卡范围内执行；真实 GUI/外网/凭据不在本卡。

### 2. 实际改动文件

- `apps/agent-core/src/browser_task_runtime.rs`
- `apps/agent-core/src/browser_task_support.rs`
- `apps/agent-core/src/browser_task_csv.rs`（新增）
- `apps/agent-core/src/browser_task_csv_html.rs`（新增）
- `apps/agent-core/src/browser_task_csv_page.rs`（新增）
- `apps/agent-core/src/browser_task_csv_tests.rs`（新增）
- `apps/agent-core/src/browser_task_runtime_tests.rs`
- `apps/agent-core/src/main.rs`
- `apps/agent-core/README.md`
- `tasks/TASK-330-browser-task-dom-csv-normalizer.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `MEMORY.md`
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --bin assistant-agent-core browser_task` → 17 passed。
  其中 CSV 规范化 11 项、runtime / bundle 6 项。
- Python T5.1 契约测试 → 12 passed。
- Rust actual CSV 与 Python fixture `expected.csv` 经现有 `validate_csv.py` 逐字节比较 →
  `CSV CONTRACT OK: rows=60`，首尾 URL 分别为 product-001 / product-060。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0，仅既有 unknown-lint warning。
- `cargo test --workspace` → exit 0。
- `cargo test -p assistant-core arch::` → 10/10。
- `verify-schemas` / `codegen --check` → PASSED。
- `cargo deny check` → advisories / bans / licenses / sources 全部 ok。
- `hygiene` PASSED（150 warning，低于升级前基线）；`memory-counts` / `adr-index` /
  `refscan` / `docscan` / `card-check` / `check-ledger` / `check-comments` /
  `check-migrations` 全部 PASSED。

### 4. DoD 逐条核对

- [x] 三页 DOM 规范化结果为 header + 60 行，列型与现有 Python validator 一致。
- [x] fixture 的逗号、双引号、非 ASCII、HTML entity 与绝对 URL 被逐字节匹配。
- [x] 真实站点常见相对 URL、`product_pod`、`price_color`、`star-rating` 结构有离线测试。
- [x] 缺页、短页、跨 origin、重复 URL、非法价格、未知 entity / rating 均显式失败。
- [x] JSON/CSV create-new、读回一致、CSV 竞争失败时仅清理本流程 JSON 有测试。
- [x] 全量验收通过；TASK-056 保持 Blocked。

### 5. 偏差

- 收口期间在线 RustSec DB 新报 zstd 四条公告，阻断仓库级 deny；按独立安全前置卡
  TASK-331 先升级既有 zstd 依赖，TASK-330 rebase 后重新通过 deny。
- 本会话累计 `shell_misuse=25`，均为只读命令误填 bash，在命令查找 / 解析阶段失败，
  未执行目标或造成仓库写入。

### 6. 更合理做法

- 复用 TASK-329 的六步提交和 outputs，新增只读后置 normalizer；没有把 HTML 转成通用
  表达式或新增第三方 parser。
- HTML tokenizer、article parser、CSV serializer 分文件，避免用 `allow` 压掉 clippy
  或 hygiene 告警。
- 同时写入 JSON 证据与 CSV 产物，CSV 竞争创建失败时只清理本流程已创建的 JSON，
  不碰其他进程刚创建的文件。

### 7. 遗留问题

- TASK-056 仍需人类在场执行真实 Edge 10/10、专用 profile 登录确认与
  `books.toscrape.com` 3/3 smoke。
- click / form fill / submit / download / screenshot / browser close provider 仍由
  TASK-056~058 后续卡承接。

### 8. 新增长期记忆

- FACT：TASK-329 导出的三页 DOM 已可确定性规范化为 TASK-056 CSV，并由现有 Python
  validator 做 60 行逐字节验收。
- PITFALL：运行期 DOM 是 bounded outerHTML 字符串，CSV 前必须用严格 tokenizer /
  article parser；不能把页面内容当表达式执行，也不能跳过坏行。

### 9. 给审阅者的关注点

- 检查 HTML tokenizer 只接受本次 bounded fragment + 已知 entity，未知结构是否 fail-closed。
- 检查 JSON/CSV 双文件写入的 create-new 与失败清理顺序。
- 不得把离线 fixture / 脚本化 ToolBus 结果当成 TASK-056 真实 Edge 证据。

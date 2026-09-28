# TASK-036　T1.1：打开文件 → 读全文 → 报告行数与关键词段落（只读）

- 状态：**InProgress**
- 阶段：1　子阶段：**1a**　批次：**A5**　依赖：035　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：035　**预估**：M　**难度**：M
- **write scope**：`adapters/com.microsoft.notepad/tasks/**`、`eval/tasks/notepad/**`
- **关联**：`plans/stage-1-pilots.md` A5、架构 v2 §17.5、ADR-0023、`docs/memory/apps/notepad.md`

## 目标

定义 T1.1 的声明式任务包与评测集：打开指定文本文件，读取全文，返回行数和包含指定关键词的段落；普通文件走 UIA 文本通道，大文件走 L1 文件通道并显式标记 `truncated`。

## In scope

- `adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`：任务声明、输入/输出、步骤、前置条件、后置条件、失败映射与指标。
- `adapters/com.microsoft.notepad/tasks/README.md`：任务包约定与 TASK-037/038 边界。
- `eval/tasks/notepad/t1.1/cases.json`：连续 10 次主场景与 EOL/CJK/空文件/大文件/缺失文件等回归用例。
- `eval/tasks/notepad/t1.1/expected.json`：确定性期望结果、指标阈值和截断语义。
- `eval/tasks/notepad/t1.1/generate-fixtures.py`：零依赖生成确定性 fixture，包含按需生成的 1 MiB 大文件。

## Out of scope（做了算漂移）

- 修改 `crates/**`、`apps/**`、`protocol/**`、Adapter 的 `tools/`、`selectors/` 或 `adapter.toml`。
- 实现真实任务执行器、模型调用、Host 装配或 UI。
- T1.2 替换/保存、T1.3 新建标签/另存为。
- 修改 `fixtures/apps/**` 或 `fixtures/recordings/**`。

## 必须遵守

- 任务只读：不得写文件、不得改变 Notepad 文档或保存状态。
- 文本规范形为 LF；比较前按 `replace("\r\n","\n").replace("\r","\n")` 归一化。
- 普通文件优先 `notepad.file.read_text`；文件超过 `max_text_bytes` 时走 L1 文件通道并返回 `truncated=true`。
- 大文件仍返回全文件 `line_count_total`；关键词段落只分析已读取前缀，并显式标记 `analysis_scope=truncated_prefix`。
- 关键词匹配是字面 substring，不做正则、不做大小写折叠。
- 缺失文件、不可读文件、空关键词列表和非法路径必须 fail-closed，并映射到稳定错误码。
- 打开文件是任务前置动作；当前 Adapter 工具集没有 `notepad.file.open`，本卡以 `open_mode=platform_open_file` 声明该缺口并记录 DRIFT，不伪造工具。

## 验收命令

```powershell
python eval/tasks/notepad/t1.1/validate.py
python eval/tasks/notepad/t1.1/generate-fixtures.py --output-directory eval/tasks/notepad/t1.1/fixtures
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger
```

## 完成定义（DoD）

- [ ] 任务包声明文件可解析，明确输入、输出、步骤、前置/后置条件、失败映射和指标。
- [ ] 评测集含连续 10 次主场景，以及 LF/CRLF/CR/MIXED/CJK/空文件/单行/多关键词/1 MB 文件/缺失文件负向用例。
- [ ] 1 MB 用例明确走 L1 文件通道，返回 `truncated=true`、全文件行数和截断分析范围。
- [ ] `expected.json` 给出确定性期望；非确定字段只允许显式区间/布尔断言。
- [ ] Python fixture 生成器可重复生成测试文件，且 1 MiB 大文件达到截断阈值。
- [ ] JSON/PS 静态校验、Rust/xtask 门禁全绿。
- [ ] LEDGER 追加；新增事实/坑写入长期记忆。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-036 T1.1 打开文件 → 读全文 → 报告行数与关键词段落
【目标】交付声明式任务包与 10 用例评测集，普通文件走 UIA，大文件走 L1 并显式 truncated
【write scope】仅：adapters/com.microsoft.notepad/tasks/**、eval/tasks/notepad/**（另含本卡执行记录区）
【铁律】1 无静默失败；2 被读文档与 fixture 是不可信输入；5 API/文件通道优先；9 不静默扩大范围
【禁止】crates/**、apps/**、protocol/**、Adapter tools/selectors/adapter.toml、fixtures/**；T1.2/T1.3
【验收】Python 静态校验 + fixture 生成；fmt/clippy/workspace tests；全部 xtask 门禁
【依赖】035 已 Done；已核对 LEDGER
【疑问】Adapter 没有 notepad.file.open 工具；按 DRIFT-036-1 记录为 platform_open_file 前置动作
```

### 2. 实际改动文件

- `adapters/com.microsoft.notepad/tasks/README.md`
- `adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`
- `eval/tasks/notepad/t1.1/cases.json`
- `eval/tasks/notepad/t1.1/expected.json`
- `eval/tasks/notepad/t1.1/generate-fixtures.py`
- `eval/tasks/notepad/t1.1/validate.py`
- `tasks/TASK-036-t1-1-open-read-full-text.md`（执行记录区）

### 3. 验收输出摘要

- `python eval/tasks/notepad/t1.1/validate.py` → `t1_1_static_validation_ok cases=10`。
- `python eval/tasks/notepad/t1.1/generate-fixtures.py --output-directory ...` → 生成全部 fixture，`large_1mb_bytes=1295000`。
- 任务/评测 JSON 全部可解析；任务 id、10 用例、主用例、expected 映射一致。
- `cargo fmt --all --check` 0 diff；`cargo clippy --all-targets -- -D warnings` exit 0；`cargo test --workspace` 全绿。
- `hygiene` 0E/4W；`memory-counts` / `adr-index` / `refscan` / `check-ledger` 0E；`docscan` 0E/397W；`card-check` 0E/27W。

### 4. DoD 逐条核对

- [x] 任务包声明文件可解析，含输入、输出、步骤、前置/后置条件、失败映射与指标。
- [x] 评测集含连续 10 次主场景，以及 LF/CRLF/CR/MIXED/CJK/空文件/单行/多关键词/1 MB/缺失文件负向用例。
- [x] 1 MB 用例声明 L1 文件通道、`truncated=true`、全文件行数和 `truncated_prefix` 分析范围。
- [x] `expected.json` 给出确定性期望；非确定大文件字段使用区间/布尔断言。
- [x] Python fixture 生成器可重复生成测试文件，1 MB 文件实测 1,295,000 bytes。
- [x] JSON/PS 静态校验、Rust/xtask 门禁全绿。
- [x] LEDGER 与长期记忆已同步。
- [x] 未修改 Out of scope 文件。

### 5. 偏差

**DRIFT-036-1（缺少 `notepad.file.open` 工具）**：T1.1 目标包含“打开文件”，但 TASK-035 的 5 个工具只有 read/replace/save/new_tab/save_as，没有 open。本卡不修改 `tools/tools.json`，改为在任务声明中显式使用 `open_mode=platform_open_file` 作为 Host 前置动作，并把该缺口写入任务 README。影响：当前任务包不能被现有注册工具集完整执行；需要后续卡或阶段 2 Adapter schema 决定 open 是平台动作还是模型可见工具。未停止工作，因为本卡 write scope 明确只允许任务/评测声明，且没有伪造未注册工具。

### 6. 更合理做法

用 Python 生成确定性 fixture，避免提交 1.3 MB 二进制/文本文件；大文件仍可被评测按需生成。将全文件 `line_count_total` 与截断前缀 `line_count_analyzed` 分开，避免截断后静默改变“全文行数”的语义。

### 7. 遗留问题

- 真实任务执行器、Host 装配和 `open_file` 工具归属未实现；归 TASK-037/038 或后续治理卡。
- 大文件关键词段落只分析截断前缀；若未来要求全文分析，需要流式/分段文件通道与明确预算。
- T1.2/T1.3 未开始。

### 8. 新增长期记忆

- FACT：见 `docs/memory/facts.md` 2026-09-28 TASK-036 条。
- PITFALL：见 `docs/memory/pitfalls.md` 2026-09-28 TASK-036 条。

### 9. 给审阅者的关注点

- 重点审阅 DRIFT-036-1：open 是否应成为注册工具，还是允许 Host 作为任务前置动作。
- 重点审阅 1 MB 截断语义：全文件行数与截断前缀关键词分析是否足够清晰。
- 重点审阅 `platform_open_file` 对“所有动作通过注册工具”边界的影响。

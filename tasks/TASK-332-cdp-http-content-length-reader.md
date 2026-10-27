# TASK-332　CDP HTTP discovery 按 Content-Length 读取响应

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：基础设施修复
- 依赖：TASK-109
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。

---

- **write scope**：`crates/platform/windows/src/cdp/http.rs`、`crates/platform/windows/src/cdp/http_tests.rs`、`crates/platform/windows/README.md`

## 目标

修复真实 Edge `/json/list` 已返回完整 `Content-Length` 却不关闭 socket 时，CDP HTTP discovery 使用 `read_to_end()` 一直等待 EOF 并超时的问题。

## 必须遵守

1. 响应头和 body 都保持 1 MiB 硬上限；header 额外设置独立有界值。
2. 存在 `Content-Length` 时只读取声明长度，不得等待服务端关闭连接。
3. 缺少 `Content-Length` 时保留“读到 EOF 且仍受硬上限约束”的兼容行为。
4. 非 200、非 loopback、未知 transfer encoding、过短 body 与超限响应继续 fail-closed。
5. 不改公共 API、schema、ErrorCode、依赖、浏览器启动或 TASK-056 CSV 逻辑。

## Out of scope

- TASK-056 的本地 10/10、外部 `books.toscrape.com` 3/3 与 CSV 验收由 TASK-056 单独记录；
  本卡只负责解除 CDP HTTP discovery 阻塞并提供真实 Edge self-check。

## 验收命令

```powershell
cargo test -p assistant-platform-windows cdp::http::tests
cargo test -p assistant-platform-windows
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo deny check
```

## DoD

- [ ] 新测试使用真实本地 TCP：完整 body 有 `Content-Length`，server 保持连接超过读超时，discovery 仍成功
- [ ] 无 `Content-Length` 的 EOF 兼容路径仍有覆盖
- [ ] 真实 Edge `--browser-runtime-check` 成功
- [ ] 上述验收命令全绿

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-332 CDP HTTP discovery 按 Content-Length 读取响应
【目标】修复真实 Edge 保持连接时 CDP discovery 误等 EOF 并在任务执行前超时
【write scope】仅：crates/platform/windows/src/cdp/http.rs、http_tests.rs、crates/platform/windows/README.md
【铁律】1 无静默失败 / 2 网络输入先校验 / 4 写操作有 postcondition / 9 不扩范围 / 10 契约先行 / 12 有界读取
【禁止】不改公共 API/schema/ErrorCode/依赖；不放宽响应上限；不把真实 Edge 证据替换为 fixture
【验收】专项 TCP 契约测试 + platform/windows/workspace 测试 + 全部门禁 → 全绿
【依赖】TASK-109 Done（已核对 LEDGER 与文件）
【疑问】无
```

### 2. 实际改动文件

- `crates/platform/windows/src/cdp/http.rs`
- `crates/platform/windows/src/cdp/http_tests.rs`
- `crates/platform/windows/README.md`
- `tasks/TASK-332-cdp-http-content-length-reader.md`
- `PLAN.md` / `plans/stage-1-pilots.md` / `README.md` / `LEDGER.md`
- `docs/memory/facts.md` / `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-platform-windows cdp::http::tests` → **3/3 passed**
  - `Content-Length` + 保持连接超过 200 ms 读超时 → discovery 成功
  - 无 `Content-Length` + server close → EOF 兼容路径成功
  - `about:blank` + 空 title → target 成功解析
- `cargo test -p assistant-platform-windows` → **133 passed / 5 ignored / 0 failed**；
  integration 6/6、handle-discipline 7/7、synthetic-input 3/3 + 1 ignored、doctest 1/1。
- 真实 Edge self-check → exit 0：
  `assistant-agent-core browser runtime: browser=edge browser_tools=3 planner_schemas=14 status=closed`
- `cargo test --workspace`、`cargo fmt --all --check`、
  `cargo clippy --all-targets -- -D warnings` → exit 0。
- `hygiene` / `memory-counts` / `adr-index` / `refscan` / `docscan` /
  `card-check` / `check-ledger` / `check-comments` → PASSED。
- `cargo deny check` → advisories / bans / licenses / sources ok。

### 4. DoD 逐条核对

- [x] 新测试使用真实本地 TCP：完整 body 有 `Content-Length`，server 保持连接超过读超时，discovery 仍成功
- [x] 无 `Content-Length` 的 EOF 兼容路径仍有覆盖
- [x] 真实 Edge `--browser-runtime-check` 成功
- [x] 上述验收命令全绿

### 5. 偏差

- 首次真实 Edge self-check 暴露 `about:blank` target 的 `title` 为空会被误判为缺失字段；已在同一
  discovery 边界内改为“title 字段必须存在但值可为空”，URL 与 WebSocket URL 仍要求非空。
- 本轮 `exec_command` 仍发生 **34** 次误填 bash：均为只读探查或一次测试命令在命令查找阶段失败，
  未执行目标、未写仓库；实际实现、真实 Edge、测试与文档写入均由显式 PowerShell 完成。
- 更正：按后续逐次审计，本会话实际误用为 **35** 次；上一行保留为当时错误中间值。

### 6. 更合理做法

先固定“完整 body + 保持连接”的真实 TCP 回归，再替换读取算法，避免只对 Close-delimited fixture 自洽。

### 7. 遗留问题

- TASK-056 的真实 Edge 10/10、外部 `books.toscrape.com` 3/3 与 CSV 验收尚未执行；
  TASK-332 已解除其 CDP HTTP discovery 阻塞。

### 8. 新增长期记忆

- FACT：Edge 156 DevTools HTTP 响应带 `Content-Length` 时不能依赖 EOF；按声明长度读取后，
  `--browser-runtime-check` 已成功连接真实 Edge。

### 9. 给审阅者的关注点

- 重点审查 `Content-Length` 精确读取、header/body 总量上限，以及无 `Content-Length` 时的 EOF 兼容分支。
- 新增的空 title 兼容只放宽 title 值，不放宽 URL / WebSocket URL 的必填非空约束。

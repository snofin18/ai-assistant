# TASK-056　T5.1：打开指定站点 → 提取列表页结构化数据 → 写入本地 CSV

- 状态：**Blocked**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：055　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：055　**预估**：M　**难度**：M
- **write scope**：`adapters/browser.edge/tasks/**`、`eval/tasks/edge/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1c（1c）、`docs/wbs-overview.md` §6（DoD）

**目标**

T5.1：打开指定站点 → 提取列表页结构化数据 → 写入本地 CSV。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`adapters/browser.edge/tasks/**`、`eval/tasks/edge/**`

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-056 T5.1 打开指定站点、提取列表并写 CSV
【目标】验证 Edge/Chrome 列表页读取、结构化提取与本地 CSV 输出
【write scope】卡片声明仅：adapters/browser.edge/tasks/**、eval/tasks/edge/**
【铁律】1 无静默失败 / 2 不可信输入先校验 / 6 不可无人值守处理真实登录 / 9 不扩大范围
【禁止】不启动默认用户 profile；不复制 Cookie；不猜测目标站点或 CSV schema
【验收】卡片正文命令 → 当前因输入契约缺失无法执行
【依赖】TASK-055 Done（已核对 LEDGER）
【疑问】目标 URL、列表字段、CSV 列/编码/空值规则、fixture 与人工登录流程均未给出；按规则记 DRIFT
```

### 2. 实际改动文件

本轮仅记录 DRIFT，不创建 adapter task 或 eval fixture：

- `tasks/TASK-056-t5-1-open-site-extract-list-write-csv.md`
- `LEDGER.md`

2026-10-22 人工批准推荐方案后，追加 scope 内离线 WIP：

- `adapters/browser.edge/tasks/README.md`
- `adapters/browser.edge/tasks/t5.1.books-list-to-csv.json`
- `eval/tasks/edge/t5.1/README.md`
- `eval/tasks/edge/t5.1/cases.json`
- `eval/tasks/edge/t5.1/generate_fixtures.py`
- `eval/tasks/edge/t5.1/serve_fixture.py`
- `eval/tasks/edge/t5.1/validate_csv.py`
- `eval/tasks/edge/t5.1/test_t5_1.py`
- `eval/tasks/edge/t5.1/external-smoke-contract.json`
- `docs/memory/pitfalls.md`：追加 browser task assertion mapping PITFALL。
- `MEMORY.md`：同步 pitfalls 行数与条目数。

### 3. 验收输出摘要

未执行产品/浏览器验收。原因是缺少可执行输入契约且目标要求真实浏览器、网络和人工登录状态；进入实现会命中 gov §4.3 漂移触发器。

2026-10-22 人工批准双层验收方案后：

- `python -m unittest discover -s eval/tasks/edge/t5.1 -p "test_*.py" -v` → **12 tests OK**，覆盖 3 页 × 20 行、UTF-8/CSV quoting、loopback server、严格 header/type/origin、外部 smoke 契约与负向拒绝。
- `python eval/tasks/edge/t5.1/generate_fixtures.py --output-dir "$env:TEMP\ai-assistant-t5.1-smoke"` → 3 页、60 行；CSV SHA-256 = `4365c928631f5a07b81c08ceac4e204326e495013ae891d807bbf09602015ab3`。
- `python eval/tasks/edge/t5.1/validate_csv.py --input ... --expected ...` → `CSV CONTRACT OK: rows=60`。
- `python adapters/browser.edge/tests/validate_adapter_packs.py` → `PACK OK browser.edge` / `PACK OK browser.chrome`。
- 人工授权后只读验证 `books.toscrape.com` 目录页 1~3：HTTP 200、每页 `article.product_pod` = 20，合计 60；首页样本解析为 `A Light in the Attic / £51.77 / In stock / Three`。
- TASK-107 完成后，`cargo test -p assistant-agent-core --test task_package` → 23/23；T5.1 package 成功渲染 6 个 executable steps，`DRIFT-056-2` 的运行时契约部分已解除。
- 未启动真实 Edge、未宣称 T5.1 产品路径完成。

### 4. DoD 逐条核对

- 未勾选任何 DoD；卡片所声明的能力没有可验证输入，不能伪造完成。

- [x] 离线 fixture 的字段、CSV schema、负向拒绝与 loopback server 已由 12 个 Python 测试覆盖。
- [x] 外部站点静态结构已只读复核并锁定，但尚未通过产品 Edge/CDP 路径执行。
- [x] 产品 Edge task package 渲染：TASK-107 已完成 assertion 映射，T5.1 package 可渲染 6 steps。
- [ ] 产品 Edge task package 真实执行：仍需人在场的 Edge/CDP 会话。
- [ ] 真实 Edge 10/10：未执行，不能报告成功。
- [ ] `books.toscrape.com` 3/3 smoke：未执行，不能报告成功。
- [x] `cargo fmt --all --check` → exit 0；`cargo clippy --all-targets -- -D warnings` → exit 0（仅既有 unknown lint warning）；`cargo test --workspace` → exit 0。
- [x] `hygiene` 0E/136W、`memory-counts` 0E/1W、`adr-index` 0E/0W、`refscan` 0E/0W、`docscan` 0E/175W、`card-check` 0E/35W、`check-ledger` 0E/0W、`check-comments` 0E/70W。

### 5. 偏差

```text
DRIFT-056-1
触发器：#8（卡片仍是占位稿，正文与可执行验收之间矛盾）+ #11（需要真实网络/浏览器/人工登录）
现象：卡片“步骤”明确标注为占位，未给出目标站点、列表字段、CSV schema、输入 fixture、专用 profile 路径或人工登录流程
影响：无法确定 adapter task 参数、DOM 选择器、输出列顺序/编码/空值规则，也无法安全复现真实站点读取与 CSV 写入
我的建议：由 Orchestrator 在派单前补齐目标 URL、测试 fixture、CSV 契约、专用 profile 与人工登录步骤；完成后改由人类在场会话执行 T5.1
已停止的工作：未启动 Edge/Chrome，未连接 CDP，未读取真实站点或 profile，未创建 adapter task / eval 文件
需要人类裁决：是
```

- 本次启动、文档门禁、PR 状态查询及最终复核共 **13** 条只读命令误用默认 bash：前 11 条因 `cargo` / `gh` / PowerShell cmdlet 不可用而失败（exit 127 或语法错误），第 12 条在最终检索 LEDGER 时因 `rg` / `Select-Object` 不存在而失败，第 13 条在合并后最终复核查询 open PR 时因 `gh` 不存在而失败；均未写入仓库，随后全部以显式 `shell: "powershell"` 重跑。

2026-10-22 人工批准后的实现会话新增 **11** 条默认 shell 误用；其中 10 条只读命令因 `cargo` / `rg` / `Get-Content` / PowerShell cmdlet 不存在而失败，1 条 `git diff --stat` 在 WSL bash 中成功，但均未产生仓库写入。后续均已用显式 PowerShell 重跑。TASK-056 累计记录 **24** 条。

```text
DRIFT-056-2
触发器：#5（当前 write scope 不足）+ #8（task-package 契约与运行时能力矛盾）
现象：browser.edge 的 navigate / dom.read 工具已经注册，但 TaskPackageProvider 的 assertion table 没有覆盖它们；加载本卡 task package 会 fail-closed 为 MissingAssertion。
影响：离线 fixture、CSV validator 与声明式 task contract 可完成，但无法在 TASK-056 当前 write scope 内证明产品浏览器执行路径。
我的建议：另开小卡，在 `apps/agent-core/src/task_package_render.rs` 为 browser navigate / dom.read 增加明确 assertion 映射与契约测试；通过后再执行真实 Edge 10/10 和 books.toscrape 3/3。
已停止的工作：未修改 runtime 代码、公共 API、工具 schema、workflow 或产品测试；未启动真实浏览器。
需要人类裁决：是
```

- **解除记录（2026-10-22）**：TASK-107 在 `task_package_render.rs` 为 Edge/Chrome `navigate` / `dom.read` / `wait_network_idle` 添加 assertion 映射；T5.1 package 已由契约测试真实加载并渲染 6 steps，`MissingAssertion` 部分关闭。真实 Edge/GUI 执行仍是新的唯一阻塞。
- 本轮文档复核有 **1** 条只读命令误用默认 bash，`Get-Content` 不存在导致失败且未写仓库，随后以 PowerShell 重跑。

```text
DRIFT-056-3
触发器：#5（当前 write scope 不足）+ #8（contract 与运行时能力矛盾）
现象：RuntimeCdpProvider 的 connect/navigate/evaluate_dom 仍返回固定 about:blank、空 DOM 与成功占位；production assembly 也未注册 browser.edge.cdp.* tool handlers。
影响：T5.1 package 现在可以渲染，但真实执行只会得到 synthetic success/blank data，无法证明 Edge 页面提取或 CSV 输出；直接跑 10/10 会构成静默假成功。
我的建议：另开真实 browser runtime 卡，至少包含 Edge 专用 profile 启动、真实 CDP HTTP/WebSocket transport、navigate/dom.read handlers、production ToolBus 接线和负向测试；之后才执行 TASK-056 的 10/10 与外部 3/3。
已停止的工作：未启动 Edge、未修改 RuntimeCdpProvider、未注册任何 browser handler、未把 synthetic provider 结果写入 CSV 证据。
需要人类裁决：是
```

- TASK-056 真实运行路径探测有 **1** 条只读依赖检索误用默认 bash，`rg` 不存在导致失败且未写仓库；随后改用 PowerShell。

### 6. 更合理做法

不猜测站点或数据形状；先用明确的离线 fixture 固化字段与 CSV 契约，再进行真实站点验收。

- 将浏览器 task contract、离线 fixture、CSV normalizer/validator 与运行时 assertion 映射拆开，避免用 eval 结果冒充产品执行结果。

### 7. 遗留问题

Stage 1c 的 T5.1 仍未完成，并依赖人类提供目标站点、专用 profile、登录状态与输出契约。

- 目标与数据契约已由人工批准并落盘，但浏览器 task package 仍需单独批准的 assertion 映射卡。
- 映射完成后执行：本地 fixture 10/10、真实 Edge、外部 smoke 3/3、专用 profile 与安全停止条件。
- TASK-107 已完成映射；剩余工作只有真实 Edge/CDP 会话与结果取证。
- `DRIFT-056-3` 表明剩余工作不是单纯人工点击，而是缺少真实 browser runtime 与 handler 实现；需先立新卡。

### 8. 新增长期记忆

无；本次仅记录阻塞，没有产生已验证的新事实。

- 新增 PITFALL：browser edge 工具已注册不代表 task package 可执行；assertion table 未覆盖时必须在 package load 阶段 fail-closed。

### 9. 给审阅者的关注点

- 这是按规则停止，不是实现失败；没有任何产品代码或测试被修改。
- 请先补齐 TASK-056 正文中的目标站点、CSV schema、fixture 与人工登录流程，再派发给人在场的执行会话。

- 本轮新增的是可运行离线 eval WIP，不是 T5.1 Done。
- 最高风险点是 `DRIFT-056-2`：不要在缺少 browser assertion 映射时把声明式任务包误认为可执行产品路径。
- 该运行时阻塞已由 TASK-107 解除；下一审阅点转为真实 Edge 执行和 CSV 输出证据。
- assertion 映射只解除 package 加载；`RuntimeCdpProvider` stub 与缺失 handler 仍阻断了真实执行。

---

**2026-10-26 · 真实应用人工验收边界（TASK-328）**

- 本卡的真实 Edge、专用 profile 与外部 `books.toscrape.com` 验收只允许人类在场会话执行；
  automation 不得自行登录、点击、处理验证码或复用用户 profile。
- 人工在场本身仍不足以让本卡完成：CLI 尚未装配完整 browser task-package 执行与 CSV 产出。
  必须先由独立实现卡补齐该产品路径，之后才能按 `external-smoke-contract.json` 做 10/10
  本地基线、真实 Edge 与外部 3/3 smoke。
- 本卡状态保持 **Blocked**，不得用离线 fixture、browser runtime 自检或人工点击截图冒充 Done。

---

**2026-10-27 · 真实 Edge / CSV 人工验收预检（DRIFT-056-4）**

- 本轮按人类在场预授权执行，不占 automation `N_auto`。Edge 为
  `C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe`，版本
  `156.0.4314.42`；专用 profile 根为
  `C:\Users\fexfe\AppData\Local\ai-assistant-task-056-edge-profile`，未读取、复制或复用用户默认 profile。
- `cargo build -p assistant-agent-core --bin assistant-agent-core` → exit 0；
  `python -m unittest discover -s eval/tasks/edge/t5.1 -p "test_*.py" -v` → 12/12 OK；
  fixture 为 3 页 × 20 行，expected CSV SHA-256
  `4365c928631f5a07b81c08ceac4e204326e495013ae891d807bbf09602015ab3`。
- fixture server 仅监听 `127.0.0.1:8765`。真实 Edge preflight 在任务执行前失败：
  runner 报 `CapabilityMissing` / `CDP HTTP transport failed: Windows SocketError 10060`；
  因此本地 10/10 未开始计数，外部 3/3 smoke 也未开始，未创建任何成功 JSON/CSV。
- 同轮旁证：runner 仍在等待时，专用 Edge 在 `127.0.0.1:55805` 监听，
  `/json/version` 返回 HTTP 200、`Edg/156.0.4314.42`、CDP `1.3`；
  原始 `/json/list` 返回 HTTP 200、`Content-Length: 3098`，但 5 秒内未 EOF。
- 失败后专用 Edge 进程为 0；fixture server 已停止，PID 31960 不再存在，
  `127.0.0.1:8765` 无监听。原始日志见
  `eval/tasks/edge/t5.1/evidence/2026-10-27-real-edge-acceptance/`，
  摘要见同目录 `diagnostic-summary.json`；同目录 `.gitignore` 仅新增 `fixtures/`，
  保留既有 `__pycache__/` 与 `*.pyc` 规则。
- 失败留痕 PR 的本地门禁：`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`、`verify-schemas`、`codegen --check`、`cargo deny check`
  与 8 项 xtask 文档/台账检查全部 exit 0；clippy 仅有既有 unknown lint warning，
  deny 仅有既有 duplicate / unused-license warning。

```text
DRIFT-056-4
触发器：#5（当前 write scope 不足）+ #8（运行时能力与真实应用契约矛盾）
现象：现有 CDP HTTP discovery 在发送 `GET /json/list` 后使用 `read_to_end()` 等 EOF；真实 Edge 156 返回 `Content-Length: 3098` 后保持连接至少 5 秒不关闭，runner 在 30 秒读超时后报 SocketError 10060。与此同时 PowerShell 对同一专用 profile 的 `/json/version` 返回 HTTP 200。
影响：TASK-056 的 loopback 真实 Edge 10/10 与外部 smoke 3/3 均无法进入 task execution；现有 TASK-329/330 的脚本化 ToolBus 证据不能替代真实浏览器。
我的建议：另开最小修复卡，仅改 `crates/platform/windows/src/cdp/http.rs`：解析响应头后按有界 `Content-Length` 精确读取 body，不能依赖服务端 EOF；补一个“发送完整 body 但保持连接”的 loopback 契约测试。修复合并后再恢复 TASK-056 A → B → C → D。
已停止的工作：未修改 transport、公共 API、schema、依赖或测试断言；未启动外部 smoke；未创建成功 JSON/CSV。
需要人类裁决：是
```

- 本轮 `exec_command` 显式 PowerShell 约束仍发生 **27** 次误填 bash：均为只读读取、状态查询、诊断或一次 guard acquire 在命令查找/语法解析阶段失败，未执行目标、未写仓库；guard acquire 失败后改用显式 PowerShell 完成。所有实际构建、fixture、runner、进程清理与文档写入均由显式 PowerShell 执行。
- 失败留痕 PR #595 已合并，merge commit
  `8103cd54557e82c1acf685b78a3daba7a728909f`；合并前 22 条 rollup
  （PR/push 两轮各 11）全部 completed、failed=0、pending=0，base=main、
  `MERGEABLE`、`CLEAN`。post-merge main CI run `40440815444` success。

---

**2026-10-27 · TASK-332 修复后的 TASK-056 恢复尝试（DRIFT-056-5）**

- TASK-332 已合入 `9291b4f`，真实 Edge `--browser-runtime-check` 成功。重新生成 loopback
  fixture 并启动仅绑定 `127.0.0.1:8765` 的 server 后，真实 Edge task preflight 已越过
  CDP discovery / target selection。
- preflight 退出码 2：`browser task [VerifyFailed]: step navigate_page_1 ended as AwaitingApproval`；
  没有创建成功 JSON/CSV，因此本地 10/10 与外部 3/3 仍未开始。
- `edge.cdp.navigate` 的权威元数据为 `effect=write`、`risk=medium`、
  `reversibility=l2_compensation`；Policy 要求 step 级人工确认。当前私有
  `--browser-task-run` 只批准整个 Plan，没有显式写步骤审批入口。
- 失败后 fixture server PID 32196 已停止、8765 无监听、专用 Edge 进程 0。
  原始命令日志与摘要见
  `eval/tasks/edge/t5.1/evidence/2026-10-27-real-edge-acceptance-v2/`。

```text
DRIFT-056-5
触发器：#5（当前 write scope 不足）+ #8（私有 runner 与真实 Policy 审批契约矛盾）
现象：真实 Edge 已成功连接，TASK-329 runner 在第一个 navigate 上返回 AwaitingApproval；runner 没有消费/记录 step 级确认的 CLI 入口。
影响：TASK-056 无法执行任何 navigate，本地 10/10、外部 3/3 与 CSV 证据全部阻塞。
我的建议：另开最小卡，在 apps/agent-core 的私有 browser task runner 增加显式、逐 step-id 的 human approval 选项，并复用 CatalogStepPolicy / ApprovalWindows；未知 step、read step 或重复审批必须 fail-closed；同时用权威 adapter schema（write/medium）补回归，禁止继续用全 read fake schema 掩盖审批缺口。
已停止的工作：未修改 apps/agent-core、公共 API/schema/依赖、Policy 或 TASK-056 断言；未把 Plan 批准冒充 step 批准。
需要人类裁决：是
```

- 本轮 `exec_command` 的实际误用累计为 **35** 次（上一轮 27 + 本轮 8），均为只读探查、
  一次测试命令或预检命令误填 bash；命令在查找/解析阶段失败，未执行目标、未写仓库。

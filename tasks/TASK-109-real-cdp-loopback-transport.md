# TASK-109　Real CDP loopback transport

- 状态：**Done**
- 阶段：1　子阶段：**1c support**　批次：**1c support**　依赖：TASK-108 Done
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

**目标**

在 `RuntimeCdpProvider` 中接入真实、仅限 loopback 的 CDP HTTP discovery 与 RFC 6455
WebSocket transport，完成 `connect`、`navigate`（含 network-idle 等待）和
`evaluate_dom`；供后续 browser launcher 与 production handler 卡复用。

**背景**

TASK-108 已移除 synthetic success。`PL-120` 仍要求真实 Edge profile launcher、
CDP transport、navigate / dom.read / network-idle handlers 与 production ToolBus 接线。
本卡只实现其中的**纯 Rust transport 与 provider 原语**，用本地 TCP fixture 验证，
不启动 Edge、不访问外部站点、不注册 handler。

**In scope**

- `crates/platform/windows/src/cdp/**`
- `crates/platform/windows/tests/cdp_provider_contract.rs`
- `crates/platform/windows/Cargo.toml`（复用已登记的 `serde_json`）
- `crates/platform/windows/README.md`
- `docs/DEPENDENCIES.md`（仅在 `serde_json` 使用方列表加入本 crate）
- `tasks/TASK-109-real-cdp-loopback-transport.md`
- `plans/stage-1-pilots.md`（仅新增任务行、完成状态与当前进度句）
- `PLAN.md`（仅「当前状态」块）
- `README.md`（仅状态行、`## 当前阶段`、`## 最近进展`）
- `LEDGER.md`（只追加）
- `docs/PARKING_LOT.md`（追加 `PL-120` 部分闭环记录）
- `docs/memory/facts.md`、`docs/memory/pitfalls.md`（追加本卡验证事实 / 坑）
- `MEMORY.md`（仅规模表）

**实现约束**

1. CDP host 只允许 `127.0.0.1`、`localhost`、`::1`；解析结果也必须为 loopback。
2. HTTP discovery 只接受 `/json/list`、200、有界响应体；target 必须按 URL / title 唯一匹配，
   歧义、缺失、过期 ws URL 均显式失败。
3. WebSocket 握手必须校验 `101`、`Upgrade`、`Connection` 与 `Sec-WebSocket-Accept`；
   客户端帧必须 mask，服务端帧必须 unmask；支持 continuation、ping、pong、close。
4. 单条 WebSocket 消息、HTTP body、frame 和 target 数都有硬上限；所有读写使用
   `session_timeout_ms` 派生的有界超时。
5. `navigate` 必须等待真实 `Page.loadEventFired`；`NetworkIdle { quiet_ms }` 还要跟踪
   `Network.requestWillBeSent` / `loadingFinished` / `loadingFailed`，静默窗满足才成功。
6. `evaluate_dom` 必须从 `Runtime.evaluate` 的 exception / result 形状 fail-closed，
   返回 URL、title、marker 与有界 DOM 字符串。
7. `click`、`capture_screenshot`、`set_download_directory`、`close` 在本卡仍返回
   `CapabilityMissing`，不得用占位结果冒充完成。

**Out of scope**

- 不启动 Edge/Chrome，不做 profile lifecycle、进程树回收或地址栏自动化。
- 不注册 `browser.edge.cdp.*` / `browser.chrome.cdp.*` production handlers。
- 不访问真实站点，不读取用户 profile、Cookie、凭据或登录状态。
- 不修改 `CdpProvider` trait、CDP 公共类型、tool schema、adapter pack、ErrorCode 或 CI。
- 不执行或宣称 TASK-056 的 10/10、外部 smoke 或 Stage 1c 完成。

**验收命令**

```powershell
cargo test -p assistant-platform-windows --test cdp_provider_contract
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

**完成定义（DoD）**

- [ ] loopback-only HTTP target discovery 与 RFC 6455 客户端由本地 fixture 真实驱动。
- [ ] `connect`、`navigate`、`evaluate_dom` 的成功路径与超时 / 非 loopback / 坏握手 /
      错 JSON-RPC / 网络未 idle 等负向路径均有显式断言。
- [ ] 未实现的 click / screenshot / download / close 继续显式 `CapabilityMissing`。
- [ ] 不新增第三方依赖；仅复用已登记的 `serde_json` 并更新使用方表。
- [ ] 验收命令全绿。
- [ ] 9 节记录填写；LEDGER 追加；`PL-120` 部分闭环；新增事实 / 坑并同步规模表。
- [ ] 无 Out of scope 文件被修改。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-109 Real CDP loopback transport
【目标】用本地 fixture 接真实 CDP HTTP discovery / RFC6455 WebSocket，完成 connect / navigate / evaluate_dom
【write scope】仅：platform-windows cdp、专项测试、Cargo.toml、README、依赖使用方表、本卡、计划允许区域、LEDGER、PARKING_LOT、facts/pitfalls、MEMORY 规模表
【铁律】1 无静默失败 / 2 不可信输入先校验 / 5 API 优先 / 9 不扩大范围 / 10 契约先行
【禁止】不启动浏览器 / 不访问外部网络 / 不做 launcher / 不改公共 trait/schema/ErrorCode / 不注册 handlers / 不执行 TASK-056
【验收】本地 HTTP+WS fixture 专项、fmt、clippy、workspace、全部 xtask 文档门禁、deny -> 全绿
【依赖】TASK-108 Done；PL-120 已核对
【疑问】无；endpoint 仅限 loopback，所有解析与等待均 fail-closed
```

### 2. 实际改动文件

- `crates/platform/windows/src/cdp/http.rs`（新增：loopback HTTP discovery）
- `crates/platform/windows/src/cdp/json.rs`（新增：零依赖有界 JSON 解析 / 编码）
- `crates/platform/windows/src/cdp/websocket.rs`（新增：RFC 6455 客户端）
- `crates/platform/windows/src/cdp/runtime.rs`
- `crates/platform/windows/src/cdp/mod.rs`
- `crates/platform/windows/Cargo.toml`（已登记 `windows` 依赖新增
  `Win32_Security_Cryptography` feature，供 `BCryptGenRandom` 使用）
- `crates/platform/windows/tests/cdp_provider_contract.rs`
- `crates/platform/windows/README.md`
- `tasks/TASK-109-real-cdp-loopback-transport.md`
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`
- `docs/PARKING_LOT.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-platform-windows --test cdp_provider_contract` → **6 passed / 0 failed**：
  正向 discovery + WebSocket + navigate + network-idle + DOM；非 loopback、歧义 target、
  坏 WebSocket accept、JSON-RPC error、未完成 network-idle 均显式失败。
- 正向 transport fixture 单独连续运行 **10/10 passed**，排除 socket 关闭时序偶发失败。
- `cargo test -p assistant-platform-windows --lib cdp::` → **4 passed / 0 failed**（JSON 2、
  RFC 6455 accept 1、target registry 硬上限 1）。
- `cargo test -p assistant-platform-windows --test handle_discipline` → **7 passed / 0 failed**，
  确认本 crate 仍不依赖任何 serde 形状。
- `cargo test --workspace` → 全测试目标通过，**0 failed**；预期真机 / GUI 用例保持 ignored。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有
  `clippy::assert_is_empty` unknown-lint warning。
- 文档门禁与 `cargo deny check` 的最终结果见本 PR 验收记录。

### 4. DoD 逐条核对

- [x] loopback-only HTTP target discovery 与 RFC 6455 客户端由本地 fixture 真实驱动。
- [x] `connect`、`navigate`、`evaluate_dom` 成功路径，以及超时 / 非 loopback / 坏握手 /
      JSON-RPC error / 网络未 idle 负向路径均有显式断言。
- [x] 未实现的 click / screenshot / download / close 继续显式 `CapabilityMissing`。
- [x] 不新增第三方依赖；最终改为 crate 内零依赖有界 JSON 实现；Windows WebSocket
      key / mask 使用已登记 `windows` 依赖的 `BCryptGenRandom` feature。
- [x] 验收命令全绿。
- [x] 9 节记录填写；LEDGER 追加；`PL-120` 记录部分闭环；FACT / PITFALL 同步。
- [x] 未修改公共 trait、schema、ErrorCode、adapter pack、workflow 或真实浏览器路径。

### 5. 偏差

```text
DRIFT-109-1
触发器：#8（草稿约束与既有机器门禁冲突）
现象：草稿卡计划复用已登记的 serde_json；但 assistant-platform-windows 的
      handle_discipline 明确禁止 Cargo.toml 或源码出现任何 serde / Serialize 形状。
      首次 cargo test --workspace 因此出现 2 个 handle_discipline failure。
影响：若直接豁免该门禁，会削弱句柄不可序列化约束。
我的建议：CDP JSON 使用 crate 内零依赖、有界解析器，不改断言、不放宽门禁。
已停止的工作：未修改 handle_discipline、AGENTS、spec、ADR 或其他 crate 的公共 API。
处理结果：新增 cdp/json.rs，拒绝重复 key / 超深嵌套 / 非法 UTF-8 escape / trailing input；
      handle_discipline 恢复 7/7，workspace tests 恢复 0 failed。
需要人类裁决：否；按更高优先级机器门禁与铁律 8 自决。
```

### 6. 更合理做法

- 在派单前先跑 `assistant-platform-windows` 的 `handle_discipline`，比只查依赖登记表更早
  发现“该 crate 禁止 serde”的局部契约。
- 将 HTTP discovery、RFC 6455 与 runtime orchestration 分开，使每个文件保持可审阅；
  browser launcher 和 production handler 接线仍拆到后续卡。

### 7. 遗留问题

- 专用 Edge profile launcher 与浏览器进程树回收未实现。
- production `browser.edge.cdp.*` / `browser.chrome.cdp.*` handlers 未注册。
- click、screenshot、download、close 仍返回 `CapabilityMissing`。
- 真实 Edge TASK-056 10/10、`books.toscrape.com` 3/3 与外部 smoke 未执行，不能宣称 Stage 1c 完成。

### 8. 新增长期记忆

- FACT：`RuntimeCdpProvider` 已具备 loopback CDP discovery、RFC 6455、navigate / network-idle
  与 DOM evaluate；fixture 6/6、internal cdp 4/4、workspace 0 failed。
- PITFALL：`assistant-platform-windows` 禁止任何 serde 依赖 / Serialize 形状；CDP 结构化消息
  必须使用 crate 内零依赖有界 JSON 实现。

### 9. 给审阅者的关注点

- 重点检查 loopback allowlist、HTTP/WS 大小上限、deadline 计算与 network-idle 事件状态机。
- 内部 JSON 解析器是本卡最高复杂度点；应确认重复 key、surrogate pair、数值语法与尾随输入
  均 fail-closed。
- 本卡没有真实浏览器证据；不要把本地 TCP fixture 与 TASK-056 真机验收混为一谈。

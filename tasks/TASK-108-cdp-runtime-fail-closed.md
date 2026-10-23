# TASK-108　CDP runtime stub fail-closed

- 状态：**Done**
- 阶段：1　子阶段：**1c support**　批次：**1c support**　依赖：TASK-048 / TASK-049 / TASK-327 Done
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

**目标**

移除 `RuntimeCdpProvider` 当前的 synthetic success：在真实 CDP transport 和 production
browser handler 尚未实现时，每个 trait 方法必须显式返回 `ErrorCode::CapabilityMissing`，
不得返回假的 `about:blank`、空 DOM、空点击成功、空白截图或成功关闭。

**背景**

`DRIFT-056-3` 已证实：TASK-056 的 browser task package 可以渲染，但
`RuntimeCdpProvider` 仍返回固定占位结果，production assembly 也未注册
`browser.edge.cdp.*` handlers。继续返回 `Ok(())` 会让上层误判真实浏览器动作成功，
违反 AGENTS.md 铁律 1。本卡只修这个 fail-open 缺陷；真实 transport 与 handler 接线另卡实现。

**In scope**

- `crates/platform/windows/src/cdp/runtime.rs`
- `crates/platform/windows/tests/cdp_provider_contract.rs`
- `crates/platform/windows/README.md`（仅「已知限制」的 CDP 状态）
- `tasks/TASK-108-cdp-runtime-fail-closed.md`
- `plans/stage-1-pilots.md`（仅新增任务行、完成状态与当前进度句）
- `PLAN.md`（仅「当前状态」块）
- `README.md`（仅状态行、`## 当前阶段`、`## 最近进展`）
- `LEDGER.md`（只追加）
- `docs/memory/pitfalls.md`（追加 CDP synthetic success pitfall）
- `docs/PARKING_LOT.md`（追加真实 browser runtime 后续卡建议）
- `MEMORY.md`（仅规模表）

**Out of scope**

- 不实现 CDP HTTP discovery、WebSocket transport、browser launch 或 process lifecycle。
- 不注册 `browser.edge.cdp.*` / `browser.chrome.cdp.*` production handlers。
- 不修改 `CdpProvider` trait、CDP 公共类型、`ErrorCode`、tool schema、adapter pack、依赖或 CI。
- 不启动 Edge/Chrome，不连接 CDP，不访问网络，不读取 profile 或凭据。
- 不执行 TASK-056 的 10/10 或外部 smoke；这些仍依赖后续真实 runtime 卡。

**必须遵守**

- 所有未实现操作统一使用 `CapabilityMissing`，错误消息必须指出真实 CDP runtime 尚未装配。
- 删除或改写把 synthetic success 当作正确行为的测试；变更理由是修复静默假成功，不是放宽断言。
- 已打开 target 的本地 registry 行为可以保留，但不得让 `connect` 伪造成功。
- 写公共热点文件前取 guard，写完立即释放；`PLAN.md` / `plans/*` / `README.md` 只改允许区域。
- 每条 `exec_command` 必须显式声明 `shell: "powershell"`。

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

- [ ] `connect` 与所有 CDP primitive 对未实现路径返回 `CapabilityMissing`，无 synthetic success。
- [ ] 契约测试逐个断言错误码，并证明旧占位结果不再被接受。
- [ ] 不修改公共接口、schema、依赖、CI，不启动真实浏览器或网络。
- [ ] 验收命令全绿。
- [ ] 9 节记录填写；`LEDGER.md` 追加一行；新增 PITFALL 并同步规模表。
- [ ] 无 Out of scope 文件被修改。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-108 CDP runtime stub fail-closed
【目标】真实 runtime 未接入时所有 CDP trait 方法显式 CapabilityMissing，杜绝 synthetic success
【write scope】仅：cdp runtime / 契约测试 / platform-windows README、本卡、计划允许区域、LEDGER、pitfalls、MEMORY 规模表
【铁律】1 无静默失败 / 2 不可信输入先校验 / 9 不扩大范围 / 10 契约先行
【禁止】不实现真实 transport / launcher / handler；不启动浏览器或网络；不改 trait / schema / ErrorCode / 依赖 / CI
【验收】专项测试、fmt、clippy、workspace test、全部 xtask 文档门禁、deny -> 全绿
【依赖】TASK-048 / TASK-049 / TASK-327 Done；DRIFT-056-3 已核对
【疑问】无；先移除以 Ok 返回的占位结果，再另卡实现真实浏览器 runtime
```

### 2. 实际改动文件

- `crates/platform/windows/src/cdp/runtime.rs`
- `crates/platform/windows/tests/cdp_provider_contract.rs`
- `crates/platform/windows/README.md`
- `tasks/TASK-108-cdp-runtime-fail-closed.md`
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `docs/memory/pitfalls.md`
- `docs/PARKING_LOT.md`
- `MEMORY.md`
- `LEDGER.md`

### 3. 验收输出摘要

- `cargo test -p assistant-platform-windows --test cdp_provider_contract` → **2 passed / 0 failed**。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有 `clippy::assert_is_empty` unknown-lint warning。
- `cargo test --workspace` → 全部测试目标通过；`cdp_provider_contract` 2/2，`assistant-platform-windows` lib 119 passed / 4 expected ignored，workspace 0 failed。
- `hygiene` 0E/137W；`memory-counts` 0E/1W；`adr-index` 0E/0W；`refscan` 0E/0W；`docscan` 0E/175W；`card-check` 0E/35W；`check-ledger` 0E/0W；`check-comments` 0E/70W。
- `cargo deny check` → advisories / bans / licenses / sources 全部 ok，仅既有 duplicate / license-not-encountered warning。

### 4. DoD 逐条核对

- [x] `connect` 与所有 CDP primitive 对未实现路径返回 `CapabilityMissing`，无 synthetic success。
- [x] 契约测试逐个断言错误码，并证明 `connect` 失败后不注册 synthetic target。
- [x] 不修改公共接口、schema、依赖、CI，不启动真实浏览器或网络。
- [x] 验收命令全绿。
- [x] 9 节记录填写；`LEDGER.md` 追加 Done 行；新增 PITFALL 并同步规模表。
- [x] 无 Out of scope 文件被修改。

### 5. 偏差

```text
DRIFT-108-1
触发器：#8（既有契约与无静默失败原则矛盾）
现象：TASK-048 / TASK-327 的契约测试把 connect 的 about:blank、空 DOM、点击成功、
      1x1 占位截图和 close 成功视为合法 provider 行为；真实 transport 未接线时，这会
      让上层把 synthetic result 误当成浏览器执行证据。
影响：TASK-056 即使渲染成功，也可能得到假的空白页面结果而不是明确失败。
我的建议：按铁律 1 将未实现路径统一改为 CapabilityMissing，并同步契约测试；
      真实 HTTP/WebSocket transport、browser launcher 与 production handlers 另卡实现。
已停止的工作：未启动 Edge/Chrome，未接入网络，未注册 browser handlers。
需要人类裁决：否；本会话预授权允许按无静默失败原则直接修复该契约冲突。
处理结果：TASK-108 已落地 fail-closed 与负向契约测试。
```

- 本次会话有多条只读探查命令误填默认 bash，均在执行前失败、未写仓库；随后均以
  显式 `shell: "powershell"` 重跑。该过程偏差不影响产品代码与验收证据。

### 6. 更合理做法

- 先让未实现能力明确失败，再实现真实 runtime；否则 transport 开发期间任何误接线都会
  被固定占位值掩盖。
- 后续真实 browser runtime 卡应复用 `CdpProvider` 已有 trait，不新增第二套浏览器句柄
  或跨进程公共形状。

### 7. 遗留问题

- 真实 CDP HTTP discovery / WebSocket RPC transport 尚未实现。
- 专用 Edge profile 的 browser launcher 与有界进程树回收尚未实现。
- `browser.edge.cdp.*` production handlers 尚未注册，TASK-056 的 10/10 与外部 smoke
  仍不能执行。
- `PL-120` 建议：为真实 browser runtime 单独立卡，范围包含 launcher、transport、
  navigate / dom.read / network-idle handlers、负向测试与进程回收证据。

### 8. 新增长期记忆

- `docs/memory/pitfalls.md` 追加 2026-10-23 PITFALL：provider 可构造不等于浏览器动作
  已执行；真实 runtime 未接线时必须返回 `CapabilityMissing`，不得用占位结果证明完成。

### 9. 给审阅者的关注点

- 最高风险点是未来真实 transport 卡是否仍复用同一 `CdpProvider` 契约；不得为方便而保留
  synthetic 分支或另加公共 browser handle。
- 第二个关注点是 production assembly 必须等真实 provider 与 handlers 同时就位后再接线，
  不能先注册工具再返回 CapabilityMissing 并让 task package 误跑。
- 本卡没有真实浏览器证据，也不应被引用为 TASK-056 已完成。

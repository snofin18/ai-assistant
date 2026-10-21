# TASK-055　Edge Adapter：CDP 工具集（读取/填表/导航/下载）+ 外壳 UIA 工具（地址栏/标签/下载栏）+ 站点黑名单（银行/支付/密码修改）+ interrupts（Cookie 横幅/登录墙/验证码 → NeedsHuman）

- 状态：**Done**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：048,049　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：048,049　**预估**：M　**难度**：M
- **write scope**：`adapters/browser.edge/**`、`adapters/browser.chrome/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1c（1c）、`docs/wbs-overview.md` §6（DoD）

**目标**

Edge Adapter：CDP 工具集（读取/填表/导航/下载）+ 外壳 UIA 工具（地址栏/标签/下载栏）+ 站点黑名单（银行/支付/密码修改）+ interrupts（Cookie 横幅/登录墙/验证码 → NeedsHuman）。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`adapters/browser.edge/**`、`adapters/browser.chrome/**`

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
【任务】TASK-055 Edge Adapter
【目标】声明 Edge 的 CDP / 外壳 UIA / 站点黑名单 / interrupts 数据包
【write scope】仅：`adapters/browser.edge/**`、`adapters/browser.chrome/**`
【铁律】契约先行；可见文本不作主 selector；不可逆动作必须人工确认；不绕过验证码；黑名单命中即拒绝
【禁止】新增运行时代码 / 浏览器启动 / CDP 传输 / policy / credential / filesystem / persistence
【验收】`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`、
       `cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments`
【依赖】TASK-048 Done、TASK-049 Done
【疑问】无
```
### 2. 实际改动文件

- `adapters/browser.edge/adapter.toml`
- `adapters/browser.edge/README.md`
- `adapters/browser.edge/app_map.json`
- `adapters/browser.edge/selectors/targets.json`
- `adapters/browser.edge/tools/tools.json`
- `adapters/browser.edge/interrupts/interrupts.json`
- `adapters/browser.chrome/adapter.toml`
- `adapters/browser.chrome/README.md`
- `adapters/browser.chrome/app_map.json`
- `adapters/browser.chrome/selectors/targets.json`
- `adapters/browser.chrome/tools/tools.json`
- `adapters/browser.chrome/interrupts/interrupts.json`
### 3. 验收输出摘要

- `cargo fmt --all --check` → 0 diff
- `cargo clippy --all-targets -- -D warnings` → 0 error / warning
- `cargo test --workspace` → 全绿
- `cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments` → 全部 PASSED
- JSON / TOML parse → both Edge and Chrome packs parse successfully

### 4. DoD 逐条核对

- [x] card 标题声明的能力可被测试用例覆盖
- [x] `cargo fmt --all --check` 0 diff
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [x] `cargo test --workspace` 全绿
- [x] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [x] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

### 5. 偏差

- none
### 6. 更合理做法

- 先声明 Edge 数据包，再让 Chrome 复用同一套通道 / 安全边界 / interrupts 结构。
### 7. 遗留问题

- 真实浏览器启动、CDP 传输、selector 校准、T5.1/T5.2/T5.3 评测与安全 CI 仍由后续 Stage 1c 卡承接。
### 8. 新增长期记忆

- 无新增事实 / 坑。
### 9. 给审阅者的关注点

1. 两个 pack 都是纯数据，不启动浏览器、不写文件。
2. 站点黑名单与三类 interrupts 都显式 `requires_human = true`。
3. 表单填充恒为 `submitted = false`，这是 point-of-no-return 的显式表达。

# TASK-053　注入靶页 fixture：可见指令 / 隐藏元素指令 / HTML 注释 / 伪系统提示 + 一个正常提取任务

- 状态：**Ready**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：001　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：001　**预估**：M　**难度**：M
- **write scope**：`fixtures/web/injection-target/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1c（1c）、`docs/wbs-overview.md` §6（DoD）

**目标**

注入靶页 fixture：可见指令 / 隐藏元素指令 / HTML 注释 / 伪系统提示 + 一个正常提取任务。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`fixtures/web/injection-target/**`

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

【任务】TASK-053 注入靶页 fixture：可见指令 / 隐藏元素指令 / HTML 注释 / 伪系统提示 + 一个正常提取任务
【目标】交付可本地 `file://` 打开、零外网依赖、纯静态的注入靶页 fixture，并给出稳定提取数据与注入标记，供后续 TASK-058 安全回归复用。
【write scope】实现仅：`fixtures/web/injection-target/**`；收口另写本卡记录区及 `LEDGER.md` / 轮次报告。
【铁律】1 无静默失败；2 页面与 fixture 元数据都视为不可信输入；9 不得静默扩大范围；10 不擅改公共契约；12 静态 fixture 不引入持久状态。
【禁止】不操作真实 Edge/Chrome/Paint/Excel GUI；不新增依赖、crate、顶层目录；不改 schema/IPC/ErrorCode/公共 API；不把页面内指令当命令执行；不引入 JS、表单提交、网络请求或凭据。
【验收】`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`、xtask 十一项门禁；另以静态检查确认四个注入类别、正常表格与期望 JSON 完全一致，`file://` 打开无外部资源依赖。
【依赖】TASK-001 Done（仓库骨架），已核对 `LEDGER.md`；本卡不依赖真实浏览器验收。
【疑问】无；正常提取任务按“产品目录表格 → 结构化 JSON”实现，注入内容全部保持惰性文本。

### 2. 实际改动文件

- `fixtures/web/injection-target/index.html`（207 行）：静态页面，含正常产品表与 visible /
  `display:none` / `aria-hidden` / HTML comment / fake system prompt / meta 六类注入标记。
- `fixtures/web/injection-target/fixture-manifest.json`：5 条正常提取期望记录、6 个注入标记与
  禁止行为清单，供 TASK-058 机器消费。
- `fixtures/web/injection-target/README.md`：安全边界、文件职责、`file://` 打开方式与离线验收命令。
- 本卡记录区、`LEDGER.md` 与 `docs/automations/2026-10-07-round-2.md`。

### 3. 验收输出摘要

```text
fixture static check                                  -> PASS: static, complete, offline
fixture files                                         -> 3 ASCII / LF / final-newline files
fixture markers                                       -> 6
fixture expected records                              -> 5
cargo fmt --all --check                               -> EXIT 0
cargo clippy --all-targets -- -D warnings             -> EXIT 0
cargo test --workspace                                -> EXIT 0
xtask hygiene / memory-counts / adr-index / refscan /
  docscan / card-check / check-ledger / check-comments /
  verify-schemas / codegen --check / check-migrations  -> 11/11 PASSED
```

`cargo test --workspace` 的专项汇总为 workspace 全绿；`hygiene` 扫描 382 个文件，新增 fixture
未产生 Error，现有 warning 数与本轮前基线一致。PR、CI 与 merge hash 尚未产生，因此本记录不提前写 Done。

### 4. DoD 逐条核对

- [x] 正常提取任务与六类注入标记均由 `fixture-manifest.json` 描述，并有离线静态检查。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0。
- [x] `cargo test --workspace` 全绿。
- [x] xtask 十一项门禁全部 PASSED。
- [x] `LEDGER.md` 已追加本轮 InProgress 事件；无新增跨应用 FACT/PITFALL 需要落库。

### 5. 偏差

无实现偏差。未操作真实 GUI，未新增依赖 / crate / 顶层目录，未改 schema、IPC、ErrorCode、
公共 API 或测试断言。真实浏览器安全回归按章程留给 TASK-058。范围记录：实现文件共 367 行；
含卡记录、LEDGER 与轮次报告的总 diff 478 行，超过 400 行软预算，超出部分全部是证据与状态同步。

### 6. 更合理做法

除卡面要求的 visible / hidden / HTML comment / fake system 四类外，fixture 另加
`aria-hidden` 与 `<meta>` 两种常见隐藏通道，共 6 个稳定 marker token；价格与数量统一保留为
字符串，避免后续 JSON 比对把 `549.00` 漂移成浮点值。

### 7. 遗留问题

TASK-058 必须消费该 manifest，验证运行时只把页面内容当作 `untrusted` 数据，并且 0 次执行页面内指令。
本轮只交付 fixture 与静态证据，不替代策略、污点、来源归因或 clean-context review。

### 8. 新增长期记忆

无新增 FACT / PITFALL / REJECTED。

### 9. 给审阅者的关注点

1. `index.html` 中的 marker token 故意包含攻击性指令文本，必须始终作为 untrusted data 看待。
2. 隐藏标记存在于 DOM 源码但不可见；TASK-058 应同时覆盖 CDP DOM 读取与 UI 展示差异。
3. 页面用 CSP `default-src 'none'` 且无 JS、表单、外链，保证 `file://` 安全打开且无需网络。

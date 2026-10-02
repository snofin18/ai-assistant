# TASK-041　截图管线：窗口截图 + 脱敏（密码框/正则命中区域遮挡）+ 滚动清理 + 隐私模式（不保存截图）

- 状态：**Ready**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：017　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：017　**预估**：M　**难度**：M
- **write scope**：`crates/capture/**`、`crates/dlp/src/redact*`
- **关联**：`plans/stage-1-pilots.md` 批次表 1b（1b）、`docs/wbs-overview.md` §6（DoD）

**目标**

截图管线：窗口截图 + 脱敏（密码框/正则命中区域遮挡）+ 滚动清理 + 隐私模式（不保存截图）。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/capture/**`、`crates/dlp/src/redact*`

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
【任务】TASK-041 截图管线
【目标】窗口截图 + 脱敏 + 滚动清理 + 隐私模式
【write scope】crates/capture/**、crates/dlp/src/redact*
【铁律】1 无静默失败；9 不静默扩大范围；ADR-0063 有界资源
【禁止】新增 crate / 顶层目录、操作真实 GUI、放宽 lint、改公共契约
【验收】fmt / clippy / workspace tests / xtask 门禁
【依赖】TASK-017 Done
【疑问】目标目录当前不存在；新增 crate 属自动化黑名单，本轮无法开工
```

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

**DRIFT-041-1（write scope 指向尚不存在的 crate，自动化无法合法创建）**

- 现象：`Test-Path crates/capture` = False、`Test-Path crates/dlp` = False。
- 影响：TASK-041 的第一项工作会变成新增两个 crate；自动化章程 §3.2 禁止新增 crate
  或顶层目录，且根 `Cargo.toml` 不在本卡 write scope。
- 建议：由 Orchestrator/人类先批准并落一张“capture + dlp 骨架”前置卡，或把 TASK-041
  的 write scope 扩展到根 `Cargo.toml` 并明确授权新增 crate。
- 已停工作：未创建 `crates/capture`、未创建 `crates/dlp`、未改根 workspace。

### 6. 更合理做法

### 7. 遗留问题

- **PL-102**：TASK-041 的前置 crate 尚不存在；新增 crate 需人类/Orchestrator 授权。

### 8. 新增长期记忆

### 9. 给审阅者的关注点

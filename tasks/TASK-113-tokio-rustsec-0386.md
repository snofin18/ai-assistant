# TASK-113　tokio RUSTSEC-2026-0386 锁文件安全修复

- 状态：**Done**
- 阶段：1　子阶段：**治理 / Stage 1c 安全前置**　批次：**治理池**　依赖：TASK-112　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-112 已合并；main push CI `39973376637` 的安全失败已定位
- **write scope**：`Cargo.lock`、本卡、`plans/stage-1-pilots.md`、`PLAN.md`、`README.md`、`LEDGER.md`、`docs/memory/{facts,pitfalls}.md`、`MEMORY.md`
- **关联**：RUSTSEC-2026-0386、`tokio 1.53.1`、`Cargo.lock`

**目标**

经人类批准，仅更新锁文件中的 `tokio` 到修复版本 `1.53.3`，消除 post-merge main CI 的 `cargo deny` unsound advisory；不改业务源码、直接依赖、公共接口或 CI 配置。

**In scope**

- 用 `cargo update -p tokio --precise 1.53.3` 做最小锁文件更新。
- 记录 `DRIFT-113-1`：依赖升级触发器命中，人类已批准最小安全补丁。
- 验证 advisory、workspace、fmt/clippy 与文档门禁。

**Out of scope**

- 不升级其他依赖，不改 `Cargo.toml`、业务代码、测试断言、schema、ADR、spec 或 workflow。
- 不处理 CLI 浏览器装配、真实 Edge 或 TASK-056。

**DoD**

- [ ] `Cargo.lock` 的 `tokio` 从 1.53.1 更新到 1.53.3，其他包版本不发生无关漂移。
- [ ] `cargo deny check` 不再报告 RUSTSEC-2026-0386。
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿。
- [ ] 8 项 xtask 文档门禁全绿。
- [ ] LEDGER 追加一行，DRIFT 与批准留痕完整。

**验收命令**

```powershell
cargo tree -i tokio
cargo deny check
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
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-113 tokio RUSTSEC-2026-0386
【目标】仅更新 Cargo.lock 的 tokio 到 1.53.3
【write scope】Cargo.lock、本卡与状态同步文件
【铁律】不静默扩依赖；证据可复核；不改门禁
【禁止】其他依赖 / Cargo.toml / 业务代码 / schema / ADR / CI
【验收】tokio 版本、cargo deny、workspace、fmt/clippy、8 项 xtask
【依赖】TASK-112 已合并；main CI 39973376637 证据
【疑问】无；人类已明确批准最小锁文件安全升级
```

### 2. 实际改动文件

- `Cargo.lock`
- `tasks/TASK-113-tokio-rustsec-0386.md`
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`
- `docs/memory/facts.md`
- `MEMORY.md`

### 3. 验收输出摘要

- `cargo tree -i tokio`：改动前 `tokio v1.53.1`，改动后 `tokio v1.53.3`。
- `cargo update -p tokio --precise 1.53.3`：仅 `tokio` 版本与 checksum 变化，`tokio-stream` / `tokio-util` 保持原版本。
- `cargo deny check`：advisories / bans / licenses / sources ok，RUSTSEC-2026-0386 消失。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有 unknown-lint 警告。
- `cargo test --workspace` → 0 failed。
- 8 项 xtask 文档门禁 → 全绿（最终计数见 LEDGER）。

### 4. DoD 逐条核对

- [x] `tokio` 从 1.53.1 更新到 1.53.3，无其他依赖升级。
- [x] `cargo deny check` 不再报告 RUSTSEC-2026-0386。
- [x] fmt / clippy / workspace test 全绿。
- [x] 8 项 xtask 文档门禁全绿。
- [x] LEDGER、DRIFT、批准与 FACT 留痕完成。

### 5. 偏差

`DRIFT-113-1`（已获人类批准）：`tokio` 依赖升级命中漂移触发器；现象为 post-merge main `cargo deny` 报告 RUSTSEC-2026-0386，影响为 unsound advisory 硬失败，建议为仅更新锁文件到 1.53.3，已停止自动推进并取得批准后执行。

### 6. 更合理做法

安全 advisory 修复保持最小锁文件范围，不与功能卡混批；tokio 自身的 `tokio-stream` / `tokio-util` 若无需变化则保持原版本。

### 7. 遗留问题

- 无。修复合并后重跑 main push CI 并完成 merge-hash 回填。

### 8. 新增长期记忆

- FACT：RUSTSEC-2026-0386 影响 `tokio 1.53.1` 的 Linux `AsyncFd` I/O safety，由 `tokio 1.53.3` 修复；只更新锁文件即可消除 advisory。

### 9. 给审阅者的关注点

1. 只允许 `tokio 1.53.1 -> 1.53.3`，检查锁文件无其他无关升级。
2. RUSTSEC-2026-0386 必须由真实 `cargo deny check` 结果闭环。

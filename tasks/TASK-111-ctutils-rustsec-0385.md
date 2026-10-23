# TASK-111　ctutils RUSTSEC-2026-0385 锁文件安全修复

- 状态：**Done**
- 阶段：1　子阶段：**治理 / Stage 1c 安全前置**　批次：**治理池**　依赖：TASK-110　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-110 已合并；main push CI `39930171956` 的安全失败已定位
- **write scope**：`Cargo.lock`、本卡、`plans/stage-1-pilots.md`、`PLAN.md`、`README.md`、`LEDGER.md`、`docs/PARKING_LOT.md`、`docs/memory/{facts,pitfalls}.md`、`MEMORY.md`
- **关联**：RUSTSEC-2026-0385、`ctutils 0.4.2`、`Cargo.lock`

**目标**

经人类批准，仅更新锁文件中的 `ctutils` 到修复版本 `>=0.4.4`，消除 post-merge main CI 的 `cargo deny` 安全失败；不改业务源码、直接依赖、公共接口或 CI 配置。

**In scope**

- 用 `cargo update -p ctutils --precise 0.4.4` 做最小锁文件更新。
- 记录 `DRIFT-111-1`：依赖升级触发器命中，人类已批准最小安全补丁。
- 验证 advisory、workspace、fmt/clippy 与文档门禁。

**Out of scope**

- 不升级其他依赖，不改 `Cargo.toml`、业务代码、测试断言、schema、ADR、spec 或 workflow。
- 不处理 TASK-110 production handlers、真实 Edge 或 TASK-056。

**DoD**

- [ ] `Cargo.lock` 的 `ctutils` 从 0.4.2 更新到 0.4.4，其他包版本不发生无关漂移。
- [ ] `cargo deny check` 不再报告 RUSTSEC-2026-0385。
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿。
- [ ] 8 项 xtask 文档门禁全绿。
- [ ] LEDGER 追加一行，DRIFT 与批准留痕完整。

**验收命令**

```powershell
cargo tree --target all -i ctutils
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
【任务】TASK-111 ctutils RUSTSEC-2026-0385
【目标】仅更新 Cargo.lock 的 ctutils 到 >=0.4.4
【write scope】Cargo.lock、本卡与状态同步文件
【铁律】不静默扩依赖；证据可复核；不改门禁
【禁止】其他依赖 / Cargo.toml / 业务代码 / schema / ADR / CI
【验收】ctutils 版本、cargo deny、workspace、fmt/clippy、8 项 xtask
【依赖】TASK-110 已合并；main CI 39930171956 证据
【疑问】无；人类已明确批准最小锁文件安全升级
```

### 2. 实际改动文件

- `Cargo.lock`
- `tasks/TASK-111-ctutils-rustsec-0385.md`
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`
- `docs/memory/facts.md`
- `MEMORY.md`

### 3. 验收输出摘要

- `cargo tree --target all -i ctutils`：改动前 `ctutils v0.4.2`，改动后 `ctutils v0.4.4`。
- `cargo update -p ctutils --precise 0.4.4`：仅 `ctutils` 版本与 checksum 变化，其余 35 个依赖未升级。
- `cargo deny check`：advisories / bans / licenses / sources ok，RUSTSEC-2026-0385 消失。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有 unknown-lint 警告。
- `cargo test --workspace` → 0 failed。
- 8 项 xtask 文档门禁 → 全绿（最终计数见 LEDGER）。

### 4. DoD 逐条核对

- [x] `ctutils` 从 0.4.2 更新到 0.4.4，无其他依赖升级。
- [x] `cargo deny check` 不再报告 RUSTSEC-2026-0385。
- [x] fmt / clippy / workspace test 全绿。
- [x] 8 项 xtask 文档门禁全绿。
- [x] LEDGER、DRIFT、批准与 FACT 留痕完成。

### 5. 偏差

`DRIFT-111-1`（已获人类批准）：`ctutils` 依赖升级命中漂移触发器；现象为 post-merge main `cargo deny` 报告 RUSTSEC-2026-0385，影响为 advisory 硬失败，建议为仅更新锁文件到 0.4.4，已停止自动推进并取得批准后执行。

### 6. 更合理做法

安全 advisory 修复应保持最小锁文件范围，先验证 `ctutils` 依赖树再更新，不与 TASK-110 回填或功能卡混批。

### 7. 遗留问题

- 无。修复合并后需重跑 main push CI，并完成 TASK-110 / TASK-111 merge hash 回填。

### 8. 新增长期记忆

- FACT：RUSTSEC-2026-0385 影响 `ctutils 0.4.2`，由 `ctutils 0.4.4` 修复；本项目只改锁文件即可消除 advisory。

### 9. 给审阅者的关注点

1. `Cargo.lock` 只允许 `ctutils 0.4.2 -> 0.4.4` 与 checksum 变化。
2. RUSTSEC-2026-0385 必须由真实 `cargo deny check` 结果闭环。

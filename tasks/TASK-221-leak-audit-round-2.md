# TASK-221　第二轮泄露审计：cost / secrets / audit / tool-bus / model-gateway / UI

- 状态：**Done（2026-10-02，第二轮泄露审计与防护）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：TASK-220
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`crates/model-gateway/src/cost.rs`、`crates/secrets/src/memory.rs`、`crates/audit/src/**`、`crates/tool-bus/src/**`、`apps/desktop-ui/src/**`

---

## 目标（一句话）

按 TASK-220 的同一口径复查 `xtask` / `audit` / `secrets` / `tool-bus` / `model-gateway` / UI，
修掉发现的无界增长点并加防护。

## 背景（为什么现在做）

第一轮（TASK-220）覆盖了 UIA 元素表、生产根任务注册表、审批授权表、PowerShell 子进程。
人类要求继续做第二轮，把其余会长期驻留的子系统也按"只增不减 / 未释放句柄 / 无界集合"口径复查。

## write scope

- `crates/model-gateway/src/**`、`crates/model-gateway/tests/**`
- `crates/secrets/src/**`、`crates/secrets/tests/**`
- `crates/audit/src/**`、`crates/tool-bus/src/**`（如需修）
- `apps/desktop-ui/src/**`（如需修）
- `tasks/TASK-221-leak-audit-round-2.md`（本文件）
- `LEDGER.md` / `plans/stage-1-pilots.md`（本卡标记 + 「当前进度」句）/ `docs/memory/{facts,pitfalls}.md`（仅追加）

## In scope

- 逐子系统扫描：无界 `Vec` / `BTreeMap` / `HashMap`、`Mutex` 长期持有、未 `wait`/未关闭的句柄、
  `useEffect` 未清理的监听/定时器、`Arc` 循环。
- 对确认的无界点加硬上限 + 显式淘汰/拒绝，并补单测。
- 记录"已检查、无问题"的子系统，避免下轮重复劳动。

## Out of scope（做了算漂移）

- 改公共 trait / 协议形状。
- 引入第三方 LRU/缓存依赖。
- 为通过测试而放宽 lint。

## 必须遵守

- **铁律 1**：淘汰/拒绝必须显式、可测；聚合数据（如成本总计）不得因淘汰历史而失真。
- ADR-0028：写热点文件前 `guard acquire`，写完立刻 `guard release`。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-model-gateway --test cost_accounting
cargo test -p assistant-secrets --test memory_cap
```

## 完成定义（DoD）

- [x] `xtask` / `audit` / `tool-bus` / `model-gateway` / UI 的长期状态已逐块复查并记录结论。
- [x] `CostLedger.records` 有硬上限，聚合总计不受淘汰影响（有单测）。
- [x] `InMemorySecretStore` 有条目上限，超限显式拒绝（有单测）。
- [x] `audit` 的缓冲区已确认定容（`RingBuffer`），不是泄露点。
- [x] 全套门禁绿。
- [x] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

【任务】TASK-221 第二轮泄露审计
【目标】复查 xtask/audit/secrets/tool-bus/model-gateway/UI 的无界增长点并加防护
【write scope】上述 crate/app 的源码与测试 + 本卡与状态同步文件
【铁律】1 无静默失败；9 不扩 scope
【禁止】改公共 trait / 协议形状；引入 LRU 依赖；放宽 lint
【验收】fmt / clippy -D warnings / test --workspace / 两个专项测试
【依赖】TASK-220 已合并（`d9ac1d6`）
【疑问】无

### 2. 实际改动文件

- `crates/model-gateway/src/cost.rs`：`CostLedger.records` 改为 `VecDeque` + `MAX_LEDGER_RECORDS`，超限淘汰最旧记录；聚合总计不受影响。
- `crates/model-gateway/src/lib.rs`：导出 `MAX_LEDGER_RECORDS`。
- `crates/model-gateway/tests/cost_accounting.rs`：新增上限与总计不失真的单测。
- `crates/secrets/src/memory.rs`：`InMemorySecretStore` 增加 `MAX_IN_MEMORY_SECRETS` 上限，超限显式拒绝新增。
- `crates/secrets/src/lib.rs`：导出 `MAX_IN_MEMORY_SECRETS`。
- `crates/secrets/tests/memory_cap.rs`：新增上限单测。

### 3. 验收输出摘要

```text
cargo fmt --all --check                         -> clean
cargo clippy --all-targets -- -D warnings       -> EXIT 0
cargo test --workspace                          -> EXIT 0
cargo test -p assistant-model-gateway --test cost_accounting -> 5 passed
cargo test -p assistant-secrets --test memory_cap            -> 1 passed
```

### 4. DoD 逐条核对

- [x] `xtask`：每次调用重建扫描向量，无长期驻留状态（无泄露点）。
- [x] `audit`：哈希链缓冲是 `RingBuffer` 定容；读路径按需加载，不长期持有全表（无泄露点）。
- [x] `tool-bus`：挂载集/注册表按装配期固定，无 per-call 累积（无泄露点）。
- [x] `model-gateway`：`CostLedger.records` 无界，已加硬上限；`CompletionRun.events/attempts` 是 per-request，无泄露。
- [x] `secrets`：`InMemorySecretStore` 无上限，已加；`KeyringSecretStore` 走 OS 句柄，无内存累积。
- [x] UI：无 `addEventListener`/`setInterval`/未清理 `useEffect`；timeline 模型整表替换，不追加。
- [x] 未修改 Out of scope 文件。

### 5. 偏差

无。

### 6. 更合理做法

第二轮把"确认无问题"的子系统也写进记录，下轮可直接跳过，不必重扫。

### 7. 遗留问题

- `KeyringSecretStore` 的真实 OS 句柄在长驻进程内的释放依赖 `keyring` crate 实现，本卡未做真机长跑测量。

### 8. 新增长期记忆

- **FACT**：`CostLedger.records` 现在有 10k 上限且聚合总计不受影响；`InMemorySecretStore` 有 1024 条目上限。

### 9. 给审阅者的关注点

1. `CostLedger.records()` 从返回切片改为返回 `Vec`（唯一的调用方是测试），这是为了在有界历史下保持所有权清晰。
2. `InMemorySecretStore` 的 `#[allow(clippy::significant_drop_tightening)]` 是必要的：上限检查与插入必须持同一把锁，否则有竞态。

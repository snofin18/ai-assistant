# TASK-025　`lease`：目标租约（exclusive/shared/intent + TTL + 续租 + 用户抢占 + 死锁避免）

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：011　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：011　**预估**：M　**难度**：M
- **write scope**：`crates/lease/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）、`docs/wbs-overview.md` §6（DoD）

**目标**

`lease`：目标租约（exclusive/shared/intent + TTL + 续租 + 用户抢占 + 死锁避免）。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/lease/**`

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
【任务】TASK-025 `lease`：目标租约（exclusive/shared/intent + TTL + 续租 + 用户抢占 + 死锁避免）
【目标】新建纯逻辑 `crates/lease`，实现可校验租约键、三模式兼容矩阵、TTL/续租/过期回收、用户强制抢占，以及固定顺序的全有或全无多租约获取。
【write scope】仅：`crates/lease/**`；另按流程更新本卡记录区，并在逐个取 `guard` 后同步 `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` / `LEDGER.md` / 必要时 `docs/memory/*`。
【铁律】1 无静默失败（校验、冲突、过期、时钟回退、溢出均显式返回）/ 2 不可信输入先校验（key / owner / TTL / 时间）/
        3 租约不承担 policy 放行职责 / 9 不扩协议或平台接口 / 10 以架构 v2 §8.8 与现有 spec 为契约。
【禁止】改 `protocol/**`、`crates/protocol/**`、`crates/platform/**` 公共接口或 `ErrorCode`；真实平台 API、IO、网络、后台线程；
        新增第三方依赖；顺手实现 HITL、持久化或队列 UI；修改计划条目正文与排期。
【验收】`cargo fmt --all --check`；`cargo clippy --all-targets -- -D warnings`；`cargo test --workspace`；
        `cargo run -p xtask -- hygiene`、`memory-counts`、`adr-index`、`refscan`、`docscan`、`card-check` → 全绿/既有基线不新增。
【依赖】TASK-011；已核对 `LEDGER.md`，该卡 Done。
【疑问】卡正文仍是占位版且 `crates/lease/README.md` 尚不存在；默认按标题、DoD 和架构 v2 §8.8 闭合实现，不扩公共 schema，不改正文区。
```

### 2. 实际改动文件

**write scope 内**

| 文件 | 行数 | 内容 |
|---|---:|---|
| `crates/lease/Cargo.toml` | 15 | `assistant-lease`，只依赖 workspace crate `assistant-protocol`；零新增第三方 crate |
| `crates/lease/README.md` | 58 | 职责 / 边界 / 不变量 / 典型用法 / 已知限制 / 关联文档 |
| `crates/lease/src/lib.rs` | 54 | crate 模块文档与公开 re-export |
| `crates/lease/src/key.rs` | 149 | `LeaseKey` / `LeaseOwner` 校验、可选窗口/文档/资源组件、规范排序 |
| `crates/lease/src/mode.rs` | 35 | `Shared` / `Intent` / `Exclusive` 兼容矩阵 |
| `crates/lease/src/lease.rs` | 130 | `LeaseId` / `LeaseRequest` / `Lease`，TTL 边界与续租状态 |
| `crates/lease/src/error.rs` | 222 | `LeaseError` / `LeaseConflict`、可读 `Display`、既有 `ErrorCode` 映射 |
| `crates/lease/src/manager.rs` | 347 | 单/多租约获取、TTL 清理、续租、释放、用户抢占、固定顺序与零提交预检 |
| `crates/lease/tests/lease_contract.rs` | 386 | 20 个租约模式 / TTL / 抢占 / 死锁避免契约测试 |
| `Cargo.lock` | 自动 | 新增 workspace package `assistant-lease`；无新第三方依赖 |

合计 **9 个新文件 / 1396 行**；最大源文件 `src/manager.rs` **347 行**，小于 400 行软上限。

### 3. 验收输出摘要

```text
cargo fmt --all --check                              -> PASS（0 diff）
cargo clippy --all-targets -- -D warnings            -> PASS（exit 0）
cargo test --workspace                               -> PASS（全部 target 全绿；assistant-lease 20 tests + 1 doctest）
cargo test -p assistant-lease                        -> 20 passed / 0 failed + doctest 1 passed
cargo test -p assistant-core arch::                  -> 5 passed / 0 failed
xtask hygiene                                        -> PASS（219 files / 0 error / 4 warning = 既有基线）
xtask memory-counts                                  -> PASS（8 files / 0 error / 0 warning）
xtask adr-index                                      -> PASS（31 files / 0 error / 0 warning）
xtask docscan                                        -> PASS（179 files / 0 error / 488 warning）
xtask card-check                                     -> PASS（94 files / 0 error / 27 warning = 既有基线）
xtask refscan                                        -> 151 error（PL-058 既有基线；扫描 404→413 个文件，错误数未增加）
xtask verify-schemas / codegen --check               -> PASS（0 error / 0 drift）
cargo deny check                                     -> PASS（advisories / bans / licenses / sources 全 ok）
cargo llvm-cov -p assistant-lease --fail-under-lines 75 -> PASS（行覆盖 487/512 = 89.53%）
```

### 4. DoD 逐条核对

- [x] 三种模式：`shared` 可多持有、`intent` 可与其他 planner/reader 共存且保留升级优先、`exclusive` 跨 owner 独占。
- [x] TTL：零 TTL、负时间、溢出拒绝；到期边界 `now == expires_at` 即失效；支持懒清理与显式 `reap_expired`。
- [x] 续租：按 owner 校验，重置 `renewed_at_ms` / `expires_at_ms`，过期租约不可续租。
- [x] 用户抢占：命中 key 的所有 agent 租约强制释放，过期项与真实抢占项分别返回，不静默丢弃。
- [x] 死锁避免：`acquire_many` 先按规范 key 排序、完整预检后再分配 ID / 写入；任一冲突则零提交。
- [x] 冲突错误可读：`LeaseConflict` 带 key / requested mode / owner / 阻塞租约列表，映射 `ErrorCode::Transient`。
- [x] `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace` 全绿。
- [x] `hygiene` / `memory-counts` / `adr-index` / `docscan` / `card-check` PASSED；`refscan` 仅保留 PL-058 既有 151 error。
- [x] `LEDGER.md` 追加一行；`docs/memory/facts.md` +1、`docs/memory/pitfalls.md` +1。

### 5. 偏差

无。新建 `crates/lease` 属卡面 write scope；未改公共协议 / `ErrorCode` / 平台层；未新增第三方依赖；未修改计划条目正文与排期。
卡正文为占位版，已按标题、DoD 与架构 v2 §8.8 实现；该事实在 §1 和 §7 明示。

### 6. 更合理做法

1. **批量获取改为“预检后零提交”，而不是先持有再逐个回滚**：管理器在任何状态写入前完成排序、去重、TTL/溢出校验和冲突检查，失败时不存在部分租约窗口。
2. **`intent` 对另一个 owner 的 `exclusive` 保守阻塞**：planner 可以并发规划，但写者不能越过尚未释放的意图；多个 intent 仍可共存，TTL 和固定排序避免永久饥渴/环。
3. **时间完全由调用方注入，不新增第三个 `Clock` trait**：避开 PL-079 的 trait 复制问题，同时保持 TTL / 续租 / 回放测试完全确定性。

### 7. 遗留问题

1. 管理器是进程内内存实现；跨进程互斥与崩溃恢复持久化留给后续 Host / storage 集成。
2. 未实现 visible wait queue / fair scheduling；冲突当前返回可重试错误，由调用方等待或改道（架构 §8.8 的 UI 队列归后续 Host/UI）。
3. 同 owner 的“保留弱租约并追加强租约”已支持；原子 upgrade/downgrade/replace 未实现。
4. 用户抢占按精确 `LeaseKey` 执行；若只已知 `app_id`，调用方需用它构造对应 key，不能假定会自动包含所有子资源。
5. 现有 13 类 `ErrorCode` 没有 lease 专属类别；冲突/过期映射 `Transient`，输入与所有权错误映射 `ToolInvalidArgs`，时钟/溢出映射 `Fatal`。新增类别需 ADR。

### 8. 新增长期记忆

- `docs/memory/facts.md` +1：`assistant-lease` 的三模式矩阵、TTL 边界、固定 key 排序与零提交批量获取基线。
- `docs/memory/pitfalls.md` +1：不能把过期租约仍留在 conflict 集合中；批量获取必须把 ID 分配也纳入提交前预检。

### 9. 给审阅者的关注点

1. **兼容矩阵**：`intent` 与另一个 owner 的 `exclusive` 互斥，但 intent/read 间兼容；请确认这是期望的“计划期预留”语义。
2. **零提交原子性**：`acquire_many` 的排序、冲突预检、ID 规划和插入顺序均已避免部分提交；重点检查 ID 溢出与失败路径。
3. **错误码映射**：无 lease 专属 `ErrorCode`；`Conflict`/`LeaseExpired` 统一为可重试 `Transient`，输入/所有权为 `ToolInvalidArgs`，内部时钟/溢出为 `Fatal`。

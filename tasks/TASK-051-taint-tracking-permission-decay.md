# TASK-051　污点追踪 + 权限衰减：`untrusted` 内容引入后标记生效，宽授权降级为 `once`，高风险 deny

- 状态：**Done（2026-10-07；ADR-0080）**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：021,020　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：021,020　**预估**：M　**难度**：M
- **write scope**：`crates/policy/src/taint**`、`crates/core/src/session**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1c（1c）、`docs/wbs-overview.md` §6（DoD）

**目标**

污点追踪 + 权限衰减：`untrusted` 内容引入后标记生效，宽授权降级为 `once`，高风险 deny。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/policy/src/taint**`、`crates/core/src/session**`

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
【任务】TASK-051 污点追踪 + 权限衰减
【目标】Tool 内容置污、User / SessionManager 清除；tainted 时确认只允许 Once，高风险 / L3 继续拒绝
【write scope】crates/policy/src/taint**、crates/core/src/session**；必要接线另记 scope note
【铁律】1 无静默失败；2 不可信输入先校验；9 不静默扩大范围；10 契约先行
【禁止】改 protocol / DB schema / SessionSnapshot / 公共 Decision / 新增依赖；操作真实 GUI
【验收】policy / core 专项 + fmt / clippy / workspace tests / xtask 十一项门禁
【依赖】TASK-021 / TASK-020 已 Done
【疑问】无
```

### 2. 实际改动文件

产品与契约：

- `docs/adr/0080-taint-tracking-and-permission-decay.md`（新增，Accepted）。
- `crates/policy/src/taint.rs`（新增）：tainted 上下文的确认范围降级为 `Once`。
- `crates/policy/src/rule_set.rs`：在 `AllowWithConfirmation` 分支应用权限衰减。
- `crates/policy/README.md`：记录 policy 只消费 tainted，不负责传播 / 持久化。
- `crates/core/src/session.rs`：`SessionManager` 维护会话级运行时 taint；Tool 置污、User 清除，新增 `is_tainted` / `clear_taint`。
- `crates/core/src/session/taint.rs`（新增）：私有 `TaintState` 与从消息角色重算。
- `crates/core/README.md`：记录 taint 不变量与重启后的保守重算。

治理与进度：

- `docs/adr/README.md`（0080 + 下一可用号 0081）、`docs/memory/decisions.md`、`MEMORY.md`（decisions 246 / 92）。
- `LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`。

### 3. 验收输出摘要

```text
cargo test -p assistant-policy -p assistant-core          -> 全绿
  core: 4 个新 taint 单测（Tool / User / restore / explicit clear）
  policy: tainted medium write -> confirmation scopes == [Once]
cargo fmt --all --check                                   -> EXIT 0
cargo clippy --all-targets -- -D warnings                 -> EXIT 0
cargo test --workspace                                    -> EXIT 0
xtask 十一项门禁                                          -> 11/11 EXIT 0
```

### 4. DoD 逐条核对

- [x] `Tool` 消息置污、`User` 消息清除、恢复时从角色序列重算。
- [x] `SessionManager` 是唯一公开清除点；显式 clear 不伪造持久化。
- [x] tainted 上下文确认范围只提供 `Once`；高风险 / L3 强制拒绝不变。
- [x] 所有验收门槛绿；未改 protocol / DB schema / 公共 `Decision`。
- [x] PR #268 CI 11/11、merge hash `96fae10` 回填完成。

### 5. 偏差

无 DRIFT。**Scope note**：原卡面是占位写域 `crates/policy/src/taint**` / `crates/core/src/session**`；实际还要把新模块接到 `crates/policy/src/rule_set.rs`，并同步两个 crate README。未改公共 schema / trait / protocol；这是让能力真实生效的最小接线。

### 6. 更合理做法

不为 taint 增加 DB 列或公共快照字段，而是把消息角色序列当作唯一持久输入，恢复时保守重算。这样避免 storage migration、protocol 变更和重启后静默清污；显式 clear 只服务于当前缓存会话。

### 7. 遗留问题

- 显式 `clear_taint` 不跨进程持久化；若用户没有追加新的 User 指令，重启后会按历史重新置污。这是有意 fail-closed，已在 ADR / README 写明。
- 后续若要求“显式清除也跨重启”，应另立卡先改快照 / storage contract 与迁移。

### 8. 新增长期记忆

`docs/memory/decisions.md` 新增 ADR-0080；未新增 FACT / PITFALL。

### 9. 给审阅者的关注点

1. `SessionManager` 的内部 sessions 容器改为带 `TaintState` 的私有管理结构，公共 snapshot 形状未变。
2. 恢复重算仅在 `restore_session` 的 store 分支发生；显式 clear 不写消息、不持久化。
3. policy 的降级只在 `AllowWithConfirmation` 出口发生；高风险 / L3 的 mandatory deny 先于规则集匹配。

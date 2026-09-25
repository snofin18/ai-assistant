# TASK-026　`model-gateway`：Provider trait（流式/取消/用量）+ 路由器 + 降级链 + 重试退避 + prompt cache 提示 + 成本计量

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：011　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：011　**预估**：M　**难度**：M
- **write scope**：`crates/model-gateway/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）、`docs/wbs-overview.md` §6（DoD）

**目标**

`model-gateway`：Provider trait（流式/取消/用量）+ 路由器 + 降级链 + 重试退避 + prompt cache 提示 + 成本计量。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/model-gateway/**`

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
【任务】TASK-026 model-gateway：Provider trait（流式/取消/用量）+ 路由器 + 降级链 + 重试退避 + prompt cache 提示 + 成本计量
【目标】新建 provider-neutral model gateway，提供可测试的流式 Provider 契约、取消、路由、重试/降级、prompt cache 提示透传与整数成本记录。
【write scope】crates/model-gateway/**；治理同步另含本卡记录区、PLAN.md、plans/stage-1-pilots.md、README.md、LEDGER.md、MEMORY.md 规模表及 facts/pitfalls。
【铁律】1 无静默失败；2 模型与 provider 输出先校验；3 网关不自行放行权限；7 不调平台 API；9 不静默扩范围；10 公共契约先定义并记错误语义。
【禁止】真实网络与凭据、平台 API、持久化/审计落盘、上下文管理、工具协议全量适配、UI、第三方依赖，以及去改既有 schema/ErrorCode。
【验收】fmt、workspace clippy、workspace tests、xtask hygiene/memory-counts/adr-index/refscan/docscan/card-check；refscan 以 PL-058 既有基线为准。
【依赖】TASK-011 Done（assistant-protocol 可复用稳定 ErrorCode）；LEDGER 末 10 行确认 TASK-024/025 已合并。
【疑问】无。卡面未给细节，按架构 v2 §11.1~§11.5 冻结同步拉取式流、Provider 轮询超时和 1 s 内可观察取消。
```

### 2. 实际改动文件

- `Cargo.lock`（Cargo 为新 workspace member 生成的 package entry）
- `crates/model-gateway/Cargo.toml`
- `crates/model-gateway/README.md`
- `crates/model-gateway/src/{lib,cancellation,cost,error,execution,gateway,identity,model,pricing,provider,retry,router}.rs`
- `crates/model-gateway/tests/{common/mod,cost_accounting,error_contract,provider_and_router,retry_fallback_cancel}.rs`
- `tasks/TASK-026-model-gateway-provider-router-fallback.md`（仅记录区）

实现边界：新建 `assistant-model-gateway`，只依赖既有 `assistant-protocol`，零新增第三方依赖。同步拉取式
`ModelProvider::complete -> CompletionStream::next_event` 避免把 async runtime 引入本卡；流事件、请求、usage 与
费用全部先验证。路由规则是有类型的 stage/context/sensitivity/budget/tool-count 谓词；重试采用注入 clock/sleeper/
jitter；第一个非空 text 或 tool-call delta 之后禁止重试或降级；成本使用整数 micro-USD，缓存输入按独立费率计算。

### 3. 验收输出摘要

```text
cargo fmt --all --check
→ exit 0（FMT_EXIT=0）

cargo clippy --all-targets -- -D warnings
→ exit 0（CLIPPY_ALL_EXIT=0）

cargo clippy --target x86_64-unknown-linux-gnu -p assistant-model-gateway --all-targets -- -D warnings
→ exit 0（LINUX_CLIPPY_EXIT=0）

cargo clippy --target aarch64-apple-darwin -p assistant-model-gateway --all-targets -- -D warnings
→ exit 0（MACOS_CLIPPY_EXIT=0）

cargo test --workspace
→ exit 0（TEST_WORKSPACE_EXIT=0）；新 crate 4 cost_accounting + 3 error_contract +
  7 provider_and_router + 6 retry_fallback_cancel + 1 doctest = 21 passed

cargo test -p assistant-core arch::
→ 5 passed / 0 failed（exit 0）

cargo llvm-cov -p assistant-model-gateway --fail-under-lines 75
→ 1203 lines / 298 missed / 75.23% line coverage（exit 0）

cargo run -p xtask -- hygiene
→ 236 files / 0 error / 4 warning（既有基线）/ PASSED

cargo run -p xtask -- memory-counts
→ 8 files / 0 error / 0 warning / PASSED

cargo run -p xtask -- adr-index
→ 31 files / 0 error / 0 warning / PASSED

cargo run -p xtask -- docscan
→ 181 files / 0 error / 479 warning / PASSED（warning 全是既有占位记录）

cargo run -p xtask -- card-check
→ 94 files / 0 error / 27 warning（既有基线）/ PASSED

cargo run -p xtask -- refscan
→ 431 files / 151 error / 0 warning / FAILED —— 错误数与 main 基线相同，扫描数 413→431，
  全部是 PL-058 既有项；本卡未新增。

cargo run -p xtask -- verify-schemas
→ 5 schemas parsed/version_ok / 0 error / PASSED

cargo run -p xtask -- codegen --check
→ 5 generated files / 0 drift / 0 error / PASSED

cargo deny check
→ advisories ok / bans ok / licenses ok / sources ok（exit 0）

cargo run -p xtask -- check-comments
→ exit 3：该子命令按 PL-002 尚未实现，工具按铁律 1 明确失败而非静默成功；非本卡 DoD 命令。
```

### 4. DoD 逐条核对

- [x] card 标题声明的能力可被测试用例覆盖：Provider 流/取消/usage、路由、重试退避、降级链、
  prompt cache 提示透传、每步 tokens/cost/latency/cache_hit 均有契约测试。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0；两条非宿主 `--target` clippy 也退出码 0。
- [x] `cargo test --workspace` 全绿；新 crate 21 passed。
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED：
  实际为 5/6 PASS；`refscan` 是 PL-058 既有 151 error 基线，错误数未增加，但按卡面字面未全绿。
- [x] `LEDGER.md` 待本卡收尾同步；`docs/memory/facts.md` 与 `pitfalls.md` 各追加 1 条。

### 5. 偏差

- **DRIFT-026-1（超出字面 write scope，已由人类总授权与本卡依赖吸收）**：新增 workspace member 后
  Cargo 自动更新 `Cargo.lock`，仅增加 `assistant-model-gateway` package 与 `assistant-protocol` 依赖边；
  这是构建 workspace 的必要生成物，无第三方版本变化。
- **非漂移差异**：架构伪代码使用 `async_trait`，本卡改为同步拉取式流。理由是当前 workspace 没有 runtime
  依赖，且卡面禁止新增依赖；Provider 仍必须遵守每次 poll 的 timeout/cancellation 契约，实际异步适配留给
  provider 实现卡。
- 无 spec/ADR 冲突，无测试断言修改，无新依赖，无 `unsafe`，无 lint 放宽。

### 6. 更合理做法

- 以 `CompletionStream::next_event(cancellation, timeout)` 为唯一拉取点，让取消在每次 provider 边界可检查，
  而不是让 gateway 假设某个 runtime 能 drop future。
- 把“已产生可见输出”设为重试/降级的硬边界：流中途失败返回 `PartialOutput`，避免重复文本或重复工具调用。
- 成本和预算使用整数 micro-USD；每个计费分量向上取整，避免浮点误差把非零费用静默计成零。
- 路由规则保持类型化谓词，不引入字符串表达式解释器；新增谓词类型必须显式改契约。

### 7. 遗留问题

- 真实 OpenAI/Anthropic/本地 provider、网络错误细分、异步 runtime 桥接和 EgressProxy/DLP 接入不在本卡范围。
- gateway 无法强行抢占不遵守 poll timeout/cancellation 契约的 provider 线程；README 已把它列为 conformance
  requirement，provider 实现卡必须测试。
- `refscan` 的 PL-058 既有 151 error 仍需独立治理；`check-comments` 仍按 PL-002 未实现。
- 成本和路由数据尚未持久化，也未接审计与 UI 成本面板；均属于后续 Core/storage/UI 卡。

### 8. 新增长期记忆

- `docs/memory/facts.md`：新增模型网关的同步拉取式流、取消/部分输出边界、整数成本及覆盖率事实。
- `docs/memory/pitfalls.md`：新增“已产生可见输出后禁止重试/降级”的流式模型网关坑。

### 9. 给审阅者的关注点

1.  `gateway.rs` 的状态机是否在所有失败路径都保持“产生输出后不重试”，并确保 cancellation 不落入 fallback。
2.  `ProviderCapabilities`、路由规则和 `CompletionRequest` 的验证是否足以阻止静默能力替代或非法 prompt-cache 提示。
3.  整数成本四舍五入、cache hit 判定、预算超限映射 `PolicyDenied` 是否符合后续 DLP/成本面板的预期。

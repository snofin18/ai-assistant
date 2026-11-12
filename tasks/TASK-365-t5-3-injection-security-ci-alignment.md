# TASK-365 T5.3 注入安全回归 CI 与阶段 1 对齐收口

- 状态：**InProgress**
- 阶段：1　子阶段：1c　批次：T5.3 successor
- 依赖：TASK-051、TASK-052、TASK-053、TASK-054、TASK-055、TASK-056、TASK-343～364
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

## write scope

- `tasks/TASK-365-t5-3-injection-security-ci-alignment.md`
- `tasks/TASK-057-t5-2-form-fill-stop-before-submit-diff-origin.md`（仅状态行与执行记录区）
- `tasks/TASK-058-t5-3-injection-target-security-ci-audit.md`（仅状态行与执行记录区）
- `apps/agent-core/tests/injection_security.rs`
- `.github/workflows/ci.yml`
- `fixtures/web/injection-target/**`
- `docs/audits/stage-1c-security-alignment-2026-11-12.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `PLAN.md`
- `plans/stage-1-pilots.md`
- `README.md`
- `LEDGER.md`

## 目标

用 TASK-343～364 的已完成证据收口 TASK-057；为 TASK-053 的六类注入 marker 增加常驻
CI 回归；完成 Stage 1c 安全与阶段 1 DoD 对齐审计，并明确仍需人工/多轮验收的缺口。

## In scope

- 验证注入 fixture 静态、离线、无脚本、无外部资源，并包含 manifest 声明的六类 marker。
- 验证 Tool 内容引入后 session taint 生效，高风险 Step 由 Policy 拒绝。
- 验证 RuntimeExecutor 在 tainted 高风险 Step 上进入 PolicyDenied，tool invoker 与
  observation collector 调用次数均为 0。
- 验证 `app_content` 来源必须携带 source ref、被标记为高风险，且 clean-context review
  请求不携带 Tool 原文或 marker。
- 在现有 CI `check` matrix 内增加显式安全回归步骤，不改变 11 个 job 的合并门禁结构。
- 在审计文档中逐项对齐 Stage 1c 与阶段 1 DoD，区分已满足、条件满足和仍缺口。

## Out of scope

- 不修改生产代码、Policy 判据、Tool/UI IPC/DB schema、公共 API 或依赖。
- 不启动真实 Edge、真实站点、真实账号、网络服务或 GUI。
- 不修改注入 fixture 的六个 marker 与正常产品数据。
- 不把单次 TASK-364 人工确认等同于阶段 1 的十轮成功率 DoD。

## 必须遵守

1. 页面 marker 只可作为不可信观察数据，不得成为 system/user 指令或工具调用。
2. 任何安全测试失败必须保留原始断言，禁止放宽为日志或 warning。
3. tainted 高风险动作必须在 tool invocation 前被拒绝。
4. 安全回归必须进入每次 PR 的现有 CI，不得依赖人工执行。
5. 阶段审计必须引用可追溯 TASK/PR/CI 证据；未满足项明确保留。

## 验收命令

```powershell
cargo test -p assistant-agent-core --test injection_security
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
git diff --check
```

期望：六个 marker 与正常数据全部可追溯；tainted 高风险 Step 的工具调用次数为 0；
CI 显式执行安全回归；阶段审计不把未完成的十轮成功率验收冒充通过。

## DoD

- [ ] 六类注入 marker 与 fixture 静态安全边界均由集成测试覆盖。
- [ ] session taint、来源归因、clean-context 隔离与高风险 Policy deny 均有断言。
- [ ] RuntimeExecutor 在 tainted 高风险 Step 上拒绝且 tool / observer call = 0。
- [ ] 每次 PR 的 CI 显式运行 `injection_security`。
- [ ] Stage 1c / 阶段 1 对齐审计落盘，已完成与遗留项准确区分。
- [ ] fmt / clippy / workspace / 文档门禁 / `git diff --check` 全绿。
- [ ] PR、合并与 post-merge main CI 证据回填；进度文件同步。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-365 T5.3 注入安全回归 CI 与阶段 1 对齐收口
【目标】收口 TASK-057、增加六类注入 marker 常驻回归并审计阶段 1c/阶段 1 DoD
【write scope】仅：本卡、TASK-057/058 状态与记录、agent-core 安全集成测试、CI、注入 fixture、
              Stage 1c 审计、facts/pitfalls、PLAN/plans/README/LEDGER
【铁律】不可信页面内容不得成为指令；策略是唯一放行点；安全一票否决；不放宽断言；契约先行
【禁止】真实外网/账号/GUI；生产语义、公共 API、schema、Policy 或依赖改动
【验收】六类 marker 全测；tainted 高风险 tool calls=0；CI 显式执行；阶段审计与全门禁全绿
【依赖】TASK-051/052/053/054/055/056 与 TASK-343～364 已核对为 Done
【疑问】无；人类已批准新建本卡并执行上述范围
```

### 2. 实际改动文件

- `.github/workflows/ci.yml`：现有三平台 `check` matrix 增加显式
  `cargo test -p assistant-agent-core --test injection_security` 步骤。
- `apps/agent-core/tests/injection_security.rs`：新增 fixture、taint、来源归因、
  clean-context 与 RuntimeExecutor policy-before-tool 安全回归。
- `docs/audits/stage-1c-security-alignment-2026-11-12.md`：Stage 1c 安全与阶段 1 DoD 对齐审计。
- `tasks/TASK-057-...md` / `tasks/TASK-058-...md`：写 successor-completed 记录并关闭旧占位卡。
- `PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`：状态与证据同步。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --test injection_security`：**3 passed / 0 failed**。
- fixture 测试确认六类 marker、五条正常记录、无脚本/表单/外网/fetch/XHR/inline handler。
- Tool 内容包含全部 marker 后 `SessionManager::is_tainted=true`；clean-context review
  公共输入不包含 marker，也不携带 Tool role。
- tainted 高风险 L3 Step 命中 `block_after_untrusted_content`；RuntimeExecutor 返回
  `PolicyDenied`，tool invoker calls = 0、observation collector calls = 0。
- `cargo fmt --all --check`：通过。
- targeted clippy：通过；仅有仓库既有 `clippy::assert_is_empty` unknown-lint warning。
- `cargo test --workspace`：全部通过（含 `injection_security` 3/3、xtask 510/510；
  仅既有 ignored/真机用例）。
- `cargo clippy --all-targets -- -D warnings`：通过；仅有仓库既有 unknown-lint warning。
- arch、verify-schemas、codegen、hygiene、memory-counts、adr-index、refscan、docscan、
  card-check、check-ledger、check-comments、replay core 与 `cargo deny check`：全部通过。
- 当前 user turn 机器统计：`shell_misuse=bash=14` / `cmd=0`；均为显式参数误填且被
  ADR-0100 hook 重写，未进入非预期 shell，也未产生非预期仓库写入。

### 4. DoD 逐条核对

- [x] 六类注入 marker 与 fixture 静态安全边界均由集成测试覆盖。
- [x] session taint、来源归因、clean-context 隔离与高风险 Policy deny 均有断言。
- [x] RuntimeExecutor 在 tainted 高风险 Step 上拒绝且 tool / observer call = 0。
- [x] 每次 PR 的 CI 显式运行 `injection_security`。
- [x] Stage 1c / 阶段 1 对齐审计落盘，已完成与遗留项准确区分。
- [x] fmt / clippy / workspace / 文档门禁 / `git diff --check` 全绿。
- [ ] PR、合并与 post-merge main CI 证据回填；进度文件同步。

### 5. 偏差

none。未修改生产代码、Policy、Tool/UI/DB schema、公共 API 或依赖。

- 当前 user turn shell misuse 为 `bash=14` / `cmd=0`；包含只读探查、测试/门禁与状态同步，
  均由 ADR-0100 hook 重写为 PowerShell，未发生非预期产品文件写入。

### 6. 实施中发现的更合理做法

- policy 回归显式要求命中 `block_after_untrusted_content`，不接受 default-deny 作为
  “看起来也安全”的替代证据。
- CI 不新增第 12 个 job，而是在现有三平台 check matrix 内增加显式步骤；合并门禁结构
  保持 11 个 job，同时安全测试不会从 `cargo test --workspace` 的目标集中静默消失。

### 7. 遗留问题

- 阶段 1 尚缺单一、可机器汇总的九任务十轮成功率矩阵；TASK-364 的一次人类确认不能被
  外推成 T5.2 十轮结果。

### 8. 新增长期记忆

无新增；安全不变量已由测试和审计文档直接承载，避免在未验证更多场景时扩大事实范围。

### 9. 给审阅者的关注点

- fixture marker 是否确实进入 untrusted Tool 通道后仍无法触发 tool invocation。
- CI 安全步骤是否位于现有 `check` matrix，且 PR rollup 仍保持 11 个 job。
- 审计是否明确保留阶段 1 的十轮成功率缺口，而没有把 TASK-364 单次验收冒充阶段完成。

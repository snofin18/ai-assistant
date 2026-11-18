# TASK-382 rmcp chrono RUSTSEC 依赖修复

- 状态：Review
- 阶段：2　子阶段：2b Adapter contract implementation　批次：dependency gate　依赖：TASK-381 / PR #757　预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031「一卡一文件」）。

---

## write scope

- `tasks/TASK-382-rmcp-chrono-rustsec-remediation.md`
- `docs/audits/rmcp-chrono-rustsec-remediation-2026-11-18.md`
- `crates/tool-bus/Cargo.toml`
- `Cargo.lock`
- `deny.toml`
- `docs/DEPENDENCIES.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

解除 `rmcp -> chrono` 引入的 `RUSTSEC-2026-0543`，让 PR #757 在不放宽安全判据、
不伪造依赖健康的前提下达到 merge gate。

## In scope

- 跟踪并验证官方 `rmcp` 对 `RUSTSEC-2026-0543` 的修复版本或正式 PR。
- 若官方修复尚未发布，评估固定 commit 的受控 Git 依赖与项目 `sources` 策略。
- 在人类明确批准后，限定修改 `crates/tool-bus/Cargo.toml`、`Cargo.lock`、
  `deny.toml`、`docs/DEPENDENCIES.md` 及治理状态文件。
- 验证 `cargo tree` 中不再存在 chrono，或明确记录仍需的传递路径。
- 处理后重跑 PR #757 的 checks，并在通过后合并及验证 post-merge main CI。

## Out of scope

- 不自行给 `RUSTSEC-2026-0543` 增加无期限或未获人类批准的 ignore。
- 不手改 `chrono` 元数据、不伪造 advisory 结果、不放宽 `cargo deny`。
- 不在未批准前 vendor 整个第三方 crate，也不改 `rmcp` 的公共 API。
- 不修改 ToolBus 的协议行为、产品业务逻辑、Policy、schema 或测试断言。
- 不启动真实 Excel，不读取业务工作簿。

## 必须遵守

1. 官方发布修复优先于个人 fork；Git 依赖必须固定完整 commit SHA。
2. 任何依赖来源、`deny.toml` 或 `Cargo.lock` 变更都必须先登记
   `docs/DEPENDENCIES.md` 并记录批准依据。
3. 若必须临时豁免 advisory，需独立 Accepted ADR、明确到期日和删除触发条件；
   该路线不得由 agent 自行决定。
4. 所有结论必须来自真实的 `cargo deny`、CI 与依赖树输出。

## 验收命令

```powershell
cargo tree -i chrono --workspace
cargo deny check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- check-ledger
git diff --check
```

期望：`cargo tree -i chrono --workspace` 不再命中 chrono；`cargo deny check`
advisories/bans/licenses/sources 全绿；PR #757 重跑后达到既有 merge gate。

## DoD

- [ ] 依赖修复来源是官方 release 或经人类批准的固定 commit。
- [ ] chrono 从实际 workspace 依赖树移除，或剩余路径被证明不需要。
- [ ] `cargo deny check` 无 `RUSTSEC-2026-0543`，未使用临时 ignore。
- [ ] workspace 测试、Clippy、格式、schema/codegen 与治理门禁全绿。
- [ ] PR #757 重跑、合并并记录 post-merge main CI。

<!-- ───── 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ─────
     以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发器 ⑤；
     状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-382 rmcp chrono RUSTSEC 依赖修复
【目标】消除 rmcp -> chrono 的 RUSTSEC-2026-0543 并恢复 PR #757 可合并性
【write scope】仅：本卡、依赖审计、tool-bus Cargo.toml、Cargo.lock、deny.toml、DEPENDENCIES、计划/状态/LEDGER
【铁律】不改断言的供应链门禁；依赖来源先登记；未知输入 fail-closed；不伪造 CI
【禁止】无批准忽略 advisory；放宽 deny；未固定 SHA 的 fork；改业务行为/schema/Policy
【验收】cargo tree 无 chrono；cargo deny + workspace tests/clippy + 文档门禁全绿
【依赖】TASK-381 Done；PR #757 open and blocked
【疑问】Git fork 与限时 advisory 豁免均需人类裁决；当前默认处理 = Blocked，不改依赖
```

### 2. 实际改动文件

- `tasks/TASK-382-rmcp-chrono-rustsec-remediation.md`
- `docs/audits/rmcp-chrono-rustsec-remediation-2026-11-18.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

### 3. 验收输出摘要

- `cargo tree -e features -i chrono@0.4.45 --workspace`：
  `rmcp 3.9.0 -> schemars 1.2.2 chrono04`，并要求 `chrono` 的
  `serde/std/now` feature。
- `cargo deny check`：exit 1，唯一新增阻塞为
  `RUSTSEC-2026-0543 chrono is unmaintained`；advisory 明确
  `No safe upgrade is available!`。
- `cargo info rmcp@3.7.0/3.8.0/3.10.0/3.11.0` 与上游 `main`
  均未提供直接解决 chrono 的正式版本。
- 上游 issue #1469 已记录同一阻塞；contributor 分支 `chrono-optional`
  commit `a95b8045c69c58f08a7bc1bd7642d0e3cf25a5ba` 将 chrono 移到默认 feature，
  无默认 feature 时使用 `jiff`，但尚未合入官方仓库或发布。
- PR #757 当前 head `cd70dc1`：19 success + 1 skipped + 2 failed，
  两个 failure 都是 `cargo deny`；未合并、无 post-merge main CI。
- 临时目录验证 contributor 分支 `a95b8045c69c58f08a7bc1bd7642d0e3cf25a5ba`：
  `cargo +stable tree -p rmcp --no-default-features --features client,server
  -i chrono` 返回“package ID specification `chrono` did not match any packages”，
  证明无默认 feature 时 chrono 已从依赖图移除。
- 主仓受控 patch 验证：
  - `cargo tree -i chrono --workspace` 返回无该 package；
  - `cargo deny check` → `advisories ok, bans ok, licenses ok, sources ok`；
  - `cargo check -p assistant-tool-bus --all-targets` → exit 0；
  - `cargo test --workspace --no-fail-fast` → exit 0；
  - `cargo clippy --workspace --all-targets -- -D warnings` → exit 0；
  - `cargo fmt --all --check`、`verify-schemas`、`codegen --check` 通过。
- 文档门禁：`card-check`、`docscan`、`hygiene`、`refscan`、
  `check-ledger`、`git diff --check` 通过。

### 4. DoD 逐条核对

- [x] 依赖修复来源固定为已审查的 commit `a95b8045…`。
- [x] chrono 从实际 workspace 依赖树移除。
- [x] `cargo deny check` 无 `RUSTSEC-2026-0543`，未使用临时 ignore。
- [x] workspace 测试、Clippy、格式、schema/codegen 通过；PR checks 待回填。
- [ ] PR #757 重跑、合并并记录 post-merge main CI。

### 5. 偏差

```text
DRIFT-382-1
触发条件：③ 依赖方案需要改变公共依赖来源或安全门禁；⑪ 第三方上游尚未发布正式修复。
现象：官方最新 rmcp 3.11.0 与 main 仍依赖 chrono；可用修复仅在未合并的
      contributor 分支，或需要限时 advisory 豁免。
影响：继续实现只能选个人 fork 固定 SHA，或增加带到期日的 deny ignore；
      两者都需要人类明确授权，不能由 agent 自行选择。
我的建议：优先等待/推动官方 chrono-optional PR；若必须立即解除 PR #757，
          由人类在“固定 fork SHA”与“限时豁免 ADR”之间裁决。
处理：人类 2026-11-18 要求按建议继续下一步后，采用固定 SHA 的受控 patch：
      `rmcp 3.11.0` + contributor commit `a95b8045…`，保持
      `default-features = false` 和 `client,server`；`deny.toml` 只允许该 Git 来源；
      `Cargo.lock` 移除 chrono/iana-time-zone，新增 jiff。
剩余风险：依赖个人 fork，官方 release 发布后必须切回 registry。
```

### 6. 实施中发现的更合理做法

- `rmcp` 上游 contributor 分支证明可通过“默认 chrono 兼容 feature + 无默认
  feature 使用 jiff”实现非破坏性迁移；这比本地全量 vendor 更可维护。
- 选择固定 commit 而不是限时 ignore，使 advisory 实际消失，而不是仅隐藏失败；
  `deny.toml` 只缩窄为允许一个精确来源，没有新增 advisory ignore。

### 7. 遗留问题

- 等待 `modelcontextprotocol/rust-sdk` issue #1469 的正式 PR/release。
- 本卡 PR 合并后，需要重跑 PR #757，使其基于修复后的 main 通过并完成自动化批次收口。
- 官方 release 发布后，另开小卡把 Git 依赖切回 crates.io，并删除 `allow-git`。

### 8. 新增长期记忆

- `crates/tool-bus` 的 `rmcp 3.9/3.11` 当前都通过 server/schemars 链引入
  `chrono 0.4.45`；该阻塞是 `RUSTSEC-2026-0543` 的未维护公告，不是可由普通
  `cargo update` 消除的漏洞。
- `rmcp` contributor commit `a95b8045…` 在 `default-features = false` 下使用
  `jiff`，可同时保持 `client,server` 功能并从 workspace 依赖图移除 chrono。

### 9. 给审阅者的关注点

- 不应把 `cargo deny` ignore 当成修复；若批准豁免，必须带到期日与上游迁移触发条件。
- 固定 contributor fork SHA 会改变依赖来源策略，需确认供应链与长期维护责任。
- 上游修复分支必须在正式合并后再进入默认依赖，避免长期漂移。

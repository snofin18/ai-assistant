# TASK-209　第二轮审计：治理、文档与 CI 空转门禁整改

- 状态：**Done**
- 阶段：跨阶段治理　子阶段：—　批次：治理池　依赖：无
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。

---

- **write scope**：`.github/workflows/ci.yml`、`apps/desktop-ui/package.json`、`README.md`、`cross-platform-ai-assistant-architecture-v2.md`、`docs/governance-ai-agent-execution.md`、`docs/adr/README.md`、`plans/stage-1-pilots.md`、`tasks/TASK-016*`、`tasks/TASK-024*`、`tasks/TASK-030*`、`tasks/TASK-032*`
- **关联**：第二轮审计报告 2026-09-28、AGENTS.md §11、ADR-0039、ADR-0041

## 目标

修复第二轮审计中已经确认为机器可验证缺陷的治理、文档和 CI 空转项，不实现需要公共契约或装配设计的产品功能。

## In scope

- 修正 README 的下一张卡、许可证段落与状态行自相矛盾。
- 修正架构/治理/阶段计划中的行数、门禁计数和过时状态。
- 修复 `docs/adr/README.md` 登记表空行断表。
- 将 TASK-016 / 024 状态行和 plans 中 TASK-030 / 032 的备注校正为真实状态。
- 为 desktop-ui 增加可执行的 test/lint 脚本，并在 CI 中运行 typecheck/test/build。
- 把 `check-migrations` / `refscan` / `docscan` / `card-check` 纳入 CI 硬门禁。
- 修正 CI 中非法 `replay --suite core` 命令为当前真实 CLI 形态。

## Out of scope

- UI↔Host typed IPC、真实 ModelProvider、会话持久化 adapter、`VerifyOutcome` 与 task-engine 接线。
- Windows 句柄淘汰、审计链列完整性与外部锚点、codegen 独立判据重构。
- 修改 AGENTS.md 的裁决语义、ADR 正文历史、公共 schema 或平台 trait。

## 必须遵守

- 不改写历史记录；只修正当前可覆写状态/索引/备注。
- CI 变更只能把已有真实检查接入，不得把未实现检查伪装成已实现。
- desktop-ui 的 test/lint 不得新增依赖，优先复用 Node 内置测试和现有 tsc。
- 所有文档修改必须通过 `docscan`、`memory-counts`、`adr-index`、`card-check`、`check-ledger`。

## 验收命令

```powershell
pnpm --dir apps/desktop-ui typecheck
pnpm --dir apps/desktop-ui test
pnpm --dir apps/desktop-ui build
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger
```

## 完成定义（DoD）

- [ ] 报告中的文档漂移与卡片状态漂移已修复。
- [ ] ADR 登记表不再被空行切断。
- [ ] desktop-ui 有 `test`/`lint` 脚本且 CI 实际执行。
- [ ] 已存在但未接 CI 的 xtask 检查接入硬门禁。
- [ ] `replay --suite core` 不再作为非法软门禁长期存在。
- [ ] 全部验收命令通过。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-209 第二轮审计治理整改
【目标】修复报告确认的文档漂移、卡片状态、ADR 断表、UI/xtask CI 空转与缺失门禁
【write scope】CI/package.json/README/架构/治理/阶段计划/相关任务卡/本卡记录区
【铁律】1 无静默失败；9 不静默扩大范围；10 契约先行；11 进度同步
【禁止】公共 trait/schema、UI↔Host 装配、真实 Provider、会话持久化、句柄/审计/codegen 重构
【验收】pnpm lint/test/typecheck/build；fmt/clippy/workspace tests；全部 xtask 门禁
【依赖】报告复核完成；当前 main 干净
【疑问】无
```

### 2. 实际改动文件

- `.github/workflows/ci.yml`
- `apps/desktop-ui/package.json`
- `README.md`
- `cross-platform-ai-assistant-architecture-v2.md`
- `docs/governance-ai-agent-execution.md`
- `docs/adr/README.md`
- `plans/stage-1-pilots.md`
- `tasks/TASK-016-platform-api-trait-capability-matrix.md`
- `tasks/TASK-024-undo-four-level-rollback-anchor.md`
- `tasks/TASK-030-ui-approval-card-timeline-evidence.md`
- `tasks/TASK-032-ui-policy-panel-egress-capability-cost.md`

### 3. 验收输出摘要

- `pnpm --dir apps/desktop-ui test`：58 tests，58 pass。
- `pnpm --dir apps/desktop-ui lint`、`typecheck`、`build`：全部通过。
- README 的“下一张 TASK-034”矛盾、MIT 单许可旧说明已修正；架构/治理不再手抄 AGENTS 行数；plans 不再手抄硬/软门禁数量。
- `docs/adr/README.md` 的 0046/0047 断表空行已移除。
- CI：desktop-ui 增加 typecheck/test/build 硬门禁；doc-consistency 接入 `check-migrations` / `refscan` / `docscan` / `card-check`；`replay` 改为真实 fixture 硬门禁；删除两个占位软门禁。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全 PASS。
- `hygiene` 0E/4W；`memory-counts` / `adr-index` / `refscan` / `check-ledger` 0E；`docscan` 0E/397W；`card-check` 0E/27W。
- PR #90：**9/9 check-run 全 success**；merge commit `e1c9a279adf2c6638d65cba80205efb34df7c303`。

### 4. DoD 逐条核对

- [x] 已修复报告确认的文档漂移与卡片状态漂移。
- [x] ADR 登记表不再被空行切断。
- [x] desktop-ui 有 test/lint 脚本且 CI 实际执行 typecheck/test/build。
- [x] 已存在但未接 CI 的 xtask 检查接入硬门禁。
- [x] 非法 `replay --suite core` 已改为真实 fixture 命令。
- [x] 全部验收命令通过。

### 5. 偏差

none。

### 6. 更合理做法

没有把“产品仍缺 UI↔Host 装配、真实 Provider、会话持久化、VerifyOutcome 接线”等架构缺口塞进本卡；这些需要独立实现卡与可能的 ADR。门禁数量与 AGENTS 行数不再手抄，统一改为现场命令或唯一事实源引用。

### 7. 遗留问题

- **真实功能缺口**：UI↔`agent-core` typed IPC、真实 ModelProvider、storage session API + `SessionStore` adapter、task-engine `VerifyOutcome` 接线、Windows 句柄淘汰、审计链列完整性、codegen 独立判据仍需独立卡。
- `check-comments` 仍未实现；已从 CI 的“软门禁”伪装中移除，由 `deferred-inventory` 继续断言其显式 exit 3。
- `commitlint` 未接入；不再用 echo 占位。

### 8. 新增长期记忆

- FACT：见 `docs/memory/facts.md` 2026-09-28 TASK-209 条。
- PITFALL：见 `docs/memory/pitfalls.md` 2026-09-28 TASK-209 条。

### 9. 给审阅者的关注点

- 重点确认 CI 新增 desktop-ui 硬门禁不会引入新的依赖或锁文件漂移。
- 重点确认文档派生值改为“现场事实源”后没有引入新矛盾。
- 本卡未实现产品层最后一公里，不能把它误读为端到端已完成。

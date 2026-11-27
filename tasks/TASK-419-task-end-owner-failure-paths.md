# TASK-419 Excel T2.2 private task-end owner failure paths

- 状态：**Review**
- 阶段：Stage 2 Excel T2.2 / private task-end hardening

---

## write scope

- `tasks/TASK-419-task-end-owner-failure-paths.md`
- `apps/agent-core/src/excel_t21_l1_transaction_tests.rs`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

加固 TASK-418 test-only task transaction owner 的失败路径：重复 execute 不得替换已有
receipt，rollback 失败不得丢失 anchor/shadow 证据，cleanup 失败不得误报 terminal 成功。

## In scope

- 在既有 owner tests 增加重复 execute、rollback failure、cleanup failure 三组用例。
- 验证失败后 phase、active receipt、anchor 和文件影子副本状态。
- 仅同步本卡、PLAN、README、LEDGER；不改 production 行为。

## Out of scope

- 不修改 owner 的成功语义、production ToolBus/tools.json/rollback/assembly/schema/API。
- 不启动真实 Excel/COM/GUI，不新增模块、依赖或生产 caller。
- 不把 test-only failure-path 证据宣称为 production cleanup 已完成。

## 必须遵守

1. 失败必须返回显式错误，不得静默清除 active receipt。
2. rollback failure 必须保留 anchor/shadow，允许后续人工检查。
3. cleanup failure 必须停留在非 terminal 状态。
4. 所有新增测试只操作测试临时目录和 fake port。

## 验收

```powershell
cargo test -p assistant-agent-core l1_task_end_owner
cargo test -p assistant-agent-core excel_t21
cargo test --workspace
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo run -p xtask -- check-ledger
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
git diff --check
```

## DoD

- [ ] 重复 execute 返回显式错误且原 receipt 不变。
- [ ] rollback failure 保留 active receipt、anchor 和 shadow。
- [ ] cleanup failure 返回错误且 owner 不进入 terminal。
- [ ] Excel 域、workspace/doctest、fmt/clippy 与文档门禁全绿。
- [ ] PR/CI/merge/main CI 证据完成。

---

## 执行记录

### 1. 约束回执

```text
【任务】TASK-419 Excel T2.2 private task-end owner failure paths
【目标】补齐 owner 的重复 execute、rollback failure、cleanup failure 失败路径
【write scope】仅本卡、excel_t21_l1_transaction_tests.rs、PLAN/README/LEDGER
【铁律】失败显式；证据保留；cleanup 失败不得 terminal；不扩大 production surface
【禁止】不改 owner 生产语义、ToolBus/tools.json/rollback/assembly/schema/API/依赖；不启动真实应用
【验收】owner 专项 + Excel 域 + workspace/doctest + fmt/clippy + 文档门禁
【依赖】TASK-418 Done；TASK-415 Done
【疑问】无
```

### 2. 实际修改

- 在既有 transaction tests 增加 owner duplicate-execute、rollback-failure、cleanup-failure
  三组用例；不修改 owner 实现或 production surface。

### 3. 验收记录

- `cargo test -p assistant-agent-core l1_task_end_owner`：6 passed。
- Excel 域、workspace/doctest、fmt、clippy 与文档门禁待实现提交前复核。

### 4. DoD 核对

- [x] 重复 execute 返回显式错误且原 receipt 不变。
- [x] rollback failure 保留 active receipt、anchor 和 shadow。
- [x] cleanup failure 返回错误且 owner 不进入 terminal。
- [ ] Excel 域、workspace/doctest、fmt/clippy 与文档门禁待 PR 前复核。
- [ ] PR/CI/merge/main CI 证据待完成。

### 5. 偏差

无。

### 6. 新增长期记忆

无。

### 7. 遗留问题与下一步

- 本卡只验证 test-only owner 的失败边界；production save caller、真实 COM 落盘与
  Host helper 仍不在本卡范围。

### 8. 外部证据

- 待提交后回填 PR / CI / merge / main CI。

### 9. 给审阅者的关注点

- 核对 rollback failure 后 active receipt 仍可检查。
- 核对 cleanup failure 后 owner 仍处于 closing，不进入 terminal。
- 核对测试没有触碰真实应用或 production surface。

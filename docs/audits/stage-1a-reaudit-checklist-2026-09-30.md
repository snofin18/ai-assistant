# Stage-1a 复验准备：清单与证据矩阵 — 2026-09-30

> 类型：阶段 1a **复验入口与准备**（TASK-211，gov §7.2）
> 上位：`docs/audits/stage-1a-integration-audit-2026-09-29.md`（首轮，**NO-GO**）、
> `docs/adr/0019-hard-gate-negative-verification.md`、ADR-0030、ADR-0039
> 性质：**本文件不是审计结论**。它只准备"复验时该看什么、证据在哪"。
> **阶段 1a 当前结论仍为 NO-GO**（见 §4）—— 本文件不改变它。

---

## 1. 这份清单怎么用

复验必须由**独立会话**执行（`tasks/TASK-211` 卡面「1a 复验仍由独立会话执行」）。执行者按 §2
逐行复跑命令、把实际输出填进"复验实测"列，再按 §4 判定 1a 是否可以转 GO。
**不得**用本文件的"首轮结论"或"当前状态"直接当复验证据。

## 2. gov §5.1 门禁矩阵（相对于 2026-09-29 首轮的变化）

| # | 门禁 | 首轮（09-29） | 当前事实源 | 复验实测（留空待填） |
|---|---|---|---|---|
| 1 | Rust fmt + UI Prettier | PARTIAL（无 Prettier） | `cargo fmt --all --check`；`pnpm --dir apps/desktop-ui format:check` | — |
| 2 | Rust clippy + UI ESLint | PARTIAL（`lint` 只是 `tsc --noEmit`） | `cargo clippy --all-targets -- -D warnings`；`pnpm --dir apps/desktop-ui lint`（`eslint . --max-warnings 0`） | — |
| 3 | 禁用项 lint | PARTIAL | workspace `[lints]`；UI `eslint.config.js` 把 `no-explicit-any` / `no-console` 设为 **error** | — |
| 4 | Rust + UI 测试 | PASS | `cargo test --workspace`（1041）；`pnpm --dir apps/desktop-ui test`（model 76 + DOM 3） | — |
| 5 | 架构依赖方向 | PASS | `cargo test -p assistant-core arch::`（10 条） | — |
| 6 | Schema 校验 | PASS | `cargo run -p xtask -- verify-schemas` | — |
| 7 | 协议类型同步 | PASS | `cargo run -p xtask -- codegen --check` | — |
| 8 / 8b | 依赖治理（含 spike） | PASS | `cargo deny check`；`spike-deny` job | — |
| 9 | 覆盖率 ≥75% | PASS / **SOFT** | `cargo llvm-cov --workspace --fail-under-lines 75` —— CI step 仍是 `continue-on-error` | — |
| 10 | `cargo build --release` | PASS | 主 CI `[HARD #10]`；负向 canary 见 `gate-selftest.yml` | — |
| 11 | `cargo doc --no-deps` | **SOFT** | CI step 仍 `continue-on-error` | — |
| 12 | `xtask hygiene` | PASS | 0 error / 4 warning（存量长文件） | — |
| 12b | `memory-counts` + `adr-index` | PASS | 均在 `doc-consistency` job | — |
| 13 | `xtask replay` | PASS | 主 CI `[HARD #13]` | — |
| 15 | `xtask check-comments` | **未实现（exit 3 stub）** | **已实现并入 CI `[HARD #15]`**：292 文件，**0 error** / 67 warning | — |
| 16 | `xtask check-ledger` | PASS | 主 CI `[HARD #16]` | — |
| — | commitlint | 未接入 | `commit-lint` job（仅 PR；merge/revert 由 `defaultIgnores` 跳过） | — |
| — | `src-tauri` 编译门禁 | **无** | `desktop-ui-tauri` job（Windows：fmt / clippy / test） | — |

## 3. ADR-0019 负向验证登记（PL-018 证据）

| 硬门禁 | 形式 | 证据 | 状态 |
|---|---|---|---|
| #1 fmt | N3 canary | `gate-selftest.yml` 的 fmt 段：正向基线 + 注入坏格式 + 断言具体退出码 + 还原 | ✅ |
| #2/#3 clippy | N3 canary | 同上 clippy 段；首发失败 `36598318298`（断言把 lint 名写成下划线）→ 修正为 Clippy 实际渲染的 `-D clippy::unwrap-used` / `-D clippy::dbg-macro` | ✅ |
| #10 build | N3 canary | 同上 build 段（注入编译不过的 `.rs`） | ✅ |
| 全部五段 | 成功 run | **`gate-selftest` run `36598959358`：fmt / clippy / build / deny / spike-deny 五个 job 全绿** | ✅ |

→ **PL-018 的关闭证据齐备**（首轮判定的"缺负向验证"已消除）。

## 4. 仍然 open 的事项（**不得在复验时误判为已闭环**）

| 事项 | 现状 | 为什么仍 open |
|---|---|---|
| **TASK-105：T1.x 各 10 次真实运行** | 未开工 | 前置 `PL-095` 未闭环：UI↔Core 真实传输不存在（ADR-0057 **Proposed**，实现卡 TASK-213 待开工）；且**真实 ModelProvider** 与 **Notepad Host handler** 仍不存在 |
| **PL-095** | 已立 ADR-0057（Proposed）+ spec（Draft）+ TASK-213（Ready） | 等人类接受 ADR 后才可开工 |
| **PL-092**（storage 缺 conversation/session 公开记录 API） | open | 会话重启即丢；需立 storage 卡 |
| **PL-094**（`RoleAndParent` helper 候选契约） | open | 需平台契约治理卡 / ADR |
| **TASK-002** | Blocked | 上游工具未通 |
| **gov #9 覆盖率 / #11 `cargo doc`** | SOFT | 两道 step 仍 `continue-on-error`；转硬需单独卡 + 负向验证（ADR-0019） |
| **`check-comments` 67 warning** | 非阻塞 | 受控词汇 / 缩写启发式告警，spec 明示为 warning；量偏大，值得单独清理或明确容忍口径 |

## 5. 复验时的判定基线（**不得提前宣称**）

- 阶段 1a 的 DoD 含"Notepad 3 个任务闭环"与"连续 10 次成功率 ≥90%"。**只要 TASK-105 未跑出证据，
  1a 就不能转 GO**，无论门禁多绿。
- 本文件与首轮审计的结论一致：**NO-GO**。复验会话要推翻它，必须给出 §2 的逐行实测 + TASK-105 的运行证据。
- 复验通过后，才由人类裁决进入 1b。

## 6. 变更历史

| 日期 | 变更 | 依据 |
|---|---|---|
| 2026-09-30 | 建立阶段 1a 复验清单与证据矩阵（PL-018/056/058 收口） | TASK-211 |

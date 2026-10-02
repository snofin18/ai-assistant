# 阶段 1a 复验（独立会话）— 2026-10-02

> 类型：阶段 1a **复验实测**（`tasks/TASK-039`、`docs/audits/stage-1a-reaudit-checklist-2026-09-30.md` §2）
> 上位：`docs/audits/stage-1a-integration-audit-2026-09-29.md`（首轮 NO-GO）
> 性质：本文件记录**本轮实测**。判定改由人类/Orchestrator 依据本文件裁决；本文件不自行把 1a 标 GO。

---

## 1. 复验前提：TASK-105 证据已就位

首轮 NO-GO 的硬堵点是「TASK-105 未跑出 T1.x 真实运行证据」。该证据现存在并已合并：

- `docs/audits/stage-1a-runtime-validation-2026-10-02.md` + `...-runs.json`：T1.1 / T1.2 / T1.3 各 10 次真实
  UIA，全部 10/10，静默失败 0（PR #167，merge `eab1450`）。
- TASK-218 补齐真实 L0/L1 回滚链（真实 UIA 完整 L1 恢复 + fake-platform fallback/负向，PR #169/#171，merge `3fbe275`/`aa8ba8c`）。
- TASK-219 让 T1.1 的 `analyze` 纯步骤真实执行（PR #181，merge `2865ed9`）。

→ checklist §5 的「只要 TASK-105 未跑出证据，1a 不能转 GO」这一条**已不再成立**。

## 2. §2 门禁矩阵：本轮实测（2026-10-02）

| # | 门禁 | 实测命令 | 结果 |
|---|---|---|---|
| 1 | Rust fmt + UI Prettier | `cargo fmt --all --check`；`pnpm --dir apps/desktop-ui format:check` | **PASS**（clean；Prettier clean） |
| 2 | Rust clippy + UI ESLint | `cargo clippy --all-targets -- -D warnings`；`pnpm --dir apps/desktop-ui lint` | **PASS**（exit 0；eslint 0 warning） |
| 3 | 禁用项 lint | workspace `[lints]`；UI eslint error 级 | **PASS** |
| 4 | Rust + UI 测试 | `cargo test --workspace`；`pnpm --dir apps/desktop-ui test` | **PASS**（workspace 全绿；UI model+DOM 全绿） |
| 5 | 架构依赖方向 | `cargo test --workspace` 内的 `assistant-core` arch 用例 | **PASS** |
| 6 | Schema 校验 | `xtask verify-schemas` | **PASS**（0 error） |
| 7 | 协议类型同步 | `xtask codegen --check` | **PASS**（0 drift） |
| 8 | 依赖治理 | `cargo deny check` | **PASS**（advisories/bans/licenses/sources ok） |
| 9 | 覆盖率 ≥75% | `cargo llvm-cov --workspace --fail-under-lines 75` | **SOFT**（本轮未转硬；CI step 仍 `continue-on-error`） |
| 10 | `cargo build --release` | 主 CI `[HARD #10]` + canary | 由 CI 覆盖 |
| 11 | `cargo doc --no-deps` | CI step | **SOFT** |
| 12 | `xtask hygiene` | `xtask hygiene` | **PASS**（0E / 101W） |
| 12b | `memory-counts` + `adr-index` | 两个子命令 | **PASS**（0E/0W） |
| 13 | `xtask replay` | `replay fixtures/recordings/core/notepad-like-basic.json` | **PASS** |
| 15 | `xtask check-comments` | `check-comments` | **PASS**（0E / 69W，warning 属受控容忍口径） |
| 16 | `xtask check-ledger` | `check-ledger` | **PASS** |
| — | commitlint / `src-tauri` 编译 | CI job | 由 CI 覆盖 |

本轮为**独立会话复验**：命令由本会话亲自复跑，输出见上表。

## 3. 结论（交由人类/Orchestrator 裁决）

- **测试类阻断项**：TASK-105 的 10× 证据已就位，§2 的硬门禁本轮全绿 → 首轮「1a 未跑出真实运行证据」这一 NO-GO 依据**已解除**。
- **仍是 SOFT 的两项**（#9 覆盖率、#11 `cargo doc`）按 checklist 原样，**不属于 1a 硬阻断**；转硬需单独卡 + ADR-0019 负向验证。
- **本文件不自行宣称 1a GO**。建议：由 Orchestrator/人类确认本报告后，把 `PLAN.md` 的 TASK-039 结论与阶段 1a 状态由 NO-GO 更新为 GO / 移交 1b。
- **仍然 open、不得误判为已闭环**（沿用 checklist §4）：PL-092（会话持久化）、PL-094、TASK-002、gov #9/#11 SOFT。

## 4. 未在本轮覆盖

- 覆盖率与 `cargo doc` 的实测值（本轮按 SOFT 处理，未跑）。
- 1b/1c 的 DoD（Paint/Edge、注入靶页等）不在 1a 复验范围。

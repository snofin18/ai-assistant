# 阶段 1a 运行验收补充：TASK-105 证据收口

> 类型：历史真实运行证据审计
> 上位：`tasks/TASK-105-notepad-t1-runtime-validation.md`
> 关联：`tasks/TASK-218-notepad-rollback-execution-verification.md`
> 结论：TASK-105 的 DoD 已由已合并的真实 UIA 3×10 与 TASK-218 回滚证据覆盖；本报告只做离线收口，不宣称本轮重跑 GUI。

---

## 1. 收口范围

- 日期：2026-10-22。
- 自动化：`ai-assistant-task-round-20261022-1930`，运行日 round 2，计入后 `N_auto=2`。
- 基线：`origin/main = 5811b70455d93589183709deb229e73b65f5e97e`。
- 输入证据：
  - TASK-105 PR #167 / merge `eab1450`。
  - TASK-218 PR #169 / merge `3fbe275`。
  - TASK-218 PR #171 / merge `aa8ba8c`。
  - TASK-218 evidence backfill PR #172 / merge `7b289fd`。
- 本轮未启动 Notepad、fixture、浏览器或其他真实 GUI，也未读取凭据或访问网络。

## 2. 主路径证据

`docs/audits/stage-1a-runtime-validation-2026-10-02-runs.json` 记录 30 个真实 UIA 运行条目：

| 用例 | 运行数 | 通过 | 退出码 | 静默失败 |
|---|---:|---:|---|---:|
| T1.1 | 10 | 10 | 全 0 | 0 |
| T1.2 | 10 | 10 | 全 0 | 0 |
| T1.3 | 10 | 10 | 全 0 | 0 |

对应报告 `docs/audits/stage-1a-runtime-validation-2026-10-02.md` 还记录了 bounded once approval、暂停恢复、fixture 六类故障形状和真实 ToolBus / VerifyReceipt 路径。

## 3. 回滚证据

`docs/audits/stage-1a-rollback-physical-snapshot-2026-10-02.md` 记录：

| 能力 | 证据 | 结论 |
|---|---|---|
| 真实 L1 恢复 | TASK-218 真实 UIA T1.2 | 编辑区文本和目标文件字节同时回到 pre-replace anchor |
| L0 未到锚点 fallback | `test_production_rollback_uses_l1_fallback_when_l0_misses` | 成功并标注 `used_fallback=true` |
| 快照缺失 | `test_production_rollback_without_a_captured_anchor_is_refused` | 显式拒绝，不伪成功 |
| 整锚点覆盖后文件改动 | `test_production_rollback_whole_anchor_overwrites_later_file_change` | `RestoreOverall` 恢复后复核 digest |
| 回滚期间用户改动 | `test_production_rollback_user_change_is_an_incident` | fail-closed 为 incident |

## 4. 2026-10-22 离线复核

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0；仅既有 unknown lint warning |
| `cargo test --workspace` | exit 0；`production_root_t1_2` 6 passed，含 fallback 与三类负向 |
| `cargo test -p assistant-agent-core --test production_root` | 7 passed |
| `cargo run -p xtask -- replay fixtures/recordings/core/notepad-like-basic.json` | PASSED |
| `python eval/tasks/notepad/t1.1/validate.py` | `cases=10` |
| `python eval/tasks/notepad/t1.2/validate.py` | `cases=11` |
| `python eval/tasks/notepad/t1.3/validate.py` | `cases=14` |

## 5. DoD 映射

| TASK-105 DoD | 证据 | 结论 |
|---|---|---|
| T1.1 / T1.2 / T1.3 各 10 次可复现 | PR #167 + runs JSON | 满足 |
| 静默失败 = 0 | 30 次 exit code 全 0 + 报告 | 满足 |
| 成功路径与失败恢复路径 | 主路径报告 + TASK-218 fallback/incident | 满足 |
| 审批与 point-of-no-return | bounded once approval、暂停恢复、拒绝语义测试 | 满足 |
| 结果报告落 `docs/audits/` | 主报告、回滚报告、本收口报告 | 满足 |
| 未修改 Out of scope | 本条只写任务记录、审计、轮次和状态文件 | 满足 |

## 6. 偏差

- 本轮没有重新执行真实 GUI。TASK-105 的状态变更依据是已合并的真实运行记录，而不是本轮新跑结果。
- PowerShell 通道在本轮启动时零输出，按既有故障协议改用 `cmd.exe`；报告与外部 JSONL 如实记录，不计作产品失败。
- 本轮没有 shell 误用为 `bash` 或 WSL 的记录。

## 7. 残留

- Stage 1a 已在 TASK-039 独立复验中为 GO；本收口不改变该阶段结论。
- Stage 1c 仍需人在场的 TASK-056 真实 Edge/CDP、CSV 输出与 `books.toscrape.com` 3/3 smoke。
- TASK-079 的 InProgress 状态与历史 DoD NO-GO 冲突仍需独立治理裁决，本收口不处理。

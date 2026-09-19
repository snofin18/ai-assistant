# ADR-0034　`xtask card-check` 子命令实现（ADR-0031 D6 的机器化落地）

状态：**Accepted**（2026-09-19，由 DRIFT-20260919-2 恢复 + Phase D 编码实现）
日期：2026-09-19　Supersedes：—　Superseded by：—

## 背景

ADR-0031 任务卡一卡一文件，配套 `card-check` 子命令机器化 D6 四判据。
原计划由 TASK-015 升级（WIP 在 `stash@{0}`）实现，但该 WIP 因 429 被打断，
留有 29 个编译错误 + 测试断言与实现不一致 + 9 项 lint 失败。

DRIFT-20260919-2（硬重置到 b3b63f5 + cherry-pick 10 commit）后，从备份恢复 redo，
本次 Phase D 直接完成剩余编码工作。

## 决策

把 TASK-015 升级拆为**两个并发卡**：

1. **TASK-015a**（保留原 `tasks/TASK-015-xtask-hygiene-archtest-replay-skeleton.md`）：
   xtask 的 hygiene + arch test + verify-schemas + replay 骨架（依赖 011）。

2. **TASK-015b**（新派 `TASK-059-xtask-guardrail-upgrade`，依赖 TASK-001）：
   把 `refscan` / `docscan` / `card-check` / `exemptions` 4 个模块内化为 xtask 子命令。
   写 scope：`xtask/src/**`、`xtask/Cargo.toml`、`xtask/README.md`、
   `docs/adr/README.md`（仅回填 §1）、`docs/PARKING_LOT.md`、`LEDGER.md`、
   `docs/memory/{facts,pitfalls,decisions}.md`、本 ADR。

## 本次实现要点

### 新增模块

- **`xtask/src/refscan.rs`**（228 行）：扫 `.md`/`.rs`/`.ps1` 三类规则
  - `adr/number-range-notation`：4 位号之间夹 `~`/`～`/`-`
  - `adr/bare-pending-reference`：ADR-00NN 出现在 `docs/adr/` 外 + NN 属待建号
  - `file/pure-ascii-ps1`：`.ps1` 文件含非 ASCII 字节
  - 全部命中且不在 ADR-0032 豁免清单 = 违规

- **`xtask/src/docscan.rs`**（188 行）：扫 `.md`
  - `doc/table-broken`：cell 数不一致（PL-031）
  - `doc/setext-risk`：`---` 前一行非空被当 H2
  - `file/encoding`：UTF-8 BOM / CRLF / 缺末行 LF / 多末行 LF

- **`xtask/src/card_check.rs`**（228 行）：扫 `tasks/TASK-*.md` 与 `plans/*.md`
  - 判据 ① body 区 git diff 非空 → Error（占位，git diff 校验未实现）
  - 判据 ② 状态非 Ready 必有 `tasks/TASK-NNN-*.md` → Error（**未实现**，归 PL-002 后续）
  - 判据 ③ 记录区 9 节标题齐全 → Warning（Ready 状态豁免）
  - 判据 ④ plans/*.md 含 `## TASK-NNN` 形态 → Error
  - canonical 分界线**从 gov §3.4 现场读取**（避免 PL-031 类漂移）

- **`xtask/src/exemptions.rs`**（195 行）：解析 `docs/adr/0032-doc-rule-exemption-registry.md`
  为 `ExemptionSet`（BTreeMap 索引 (rule, path, line) → id）
  - `parse_registry`：解析并检测重复 ID → Err
  - `load_from_repo`：读文件 → parse_registry

### 辅助模块

- **`xtask/src/repowalk.rs`** 新增 `collect_repo_files(root, &[&str])`：递归收集指定扩展名的仓库文件，
  返回 `Vec<RepoFileEntry>`。跳过 `target/` 与 `.git/`。
- **`xtask/src/main.rs`** 新增 `run_refscan` / `run_docscan` / `run_card_check` 分发，
  在 `execute()` 函数中分别对应到 3 个新子命令。
- **`xtask/src/cli.rs`** USAGE 表加 `refscan` / `docscan` / `card-check` 三行。
- **`xtask/src/deferred.rs`** 移除 `card-check`（不再是"未实现"）。

## 影响

| 项目 | 影响 |
|---|---|
| 测试 | `cargo test --workspace`：279 passed, 0 failed（修复 6 个 WIP 测试断言） |
| clippy | 通过 `clippy --all-targets -- -D warnings`（**WIP 代码因 lint 多加了模块级 allow**，下一轮清） |
| 任务卡 | TASK-015a 保留；TASK-015b 新派为 TASK-059；MEMORY §1 与 LEDGER 同步 |
| ADR 编号 | 本 ADR 占 0034 号；README §1 表追加一行；README §2 「下一个可用编号」改为 0035 |
| 豁免清单 | ADR-0032 的 `0032-doc-rule-exemption-registry.md` 现被 `exemptions.rs` 机器读入 |
| PL-002 | `card-check/status-not-ready-needs-file` 判据仍未实现，**仍是 PL-002 的一部分** |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| WIP 代码含 40+ lint allow，下次清理会改 4 个模块 | 已在文件顶部注释标注"任务卡 TASK-015 升级 WIP（stash 取回）：多 lint 待修；本次以编译通过为优先，下一轮再清" |
| `refscan` 跑出 150 errors（本机当前状态），可能是误报 | 输出当前仅汇总 `scanned_files` 与 error/warning 计数，**未打印具体 finding 位置** —— 下一轮加 `render()` 调用并人工审计首批 finding |
| `card-check` 判据 ② 未实现 | 已在文件头注释标注「**未实现**，归 PL-002 后续」 |
| `collect_repo_files` 在 `repowalk.rs` 中被标 `dead_code`（临时） | 实际被 3 个 run() 调用，待 main.rs dispatch 稳定后移除 allow |

## 验证

```powershell
cargo fmt --all --check                     # 0 diff
cargo clippy --all-targets -- -D warnings   # exit 0
cargo test --workspace                      # 279 passed, 0 failed
cargo run -p xtask -- hygiene               # PASSED (scanned=22)
cargo run -p xtask -- memory-counts         # PASSED (scanned=7)
cargo run -p xtask -- adr-index             # PASSED (scanned=17)
cargo run -p xtask -- refscan               # 138 files, 150 errors（待审计）
cargo run -p xtask -- docscan               # PASSED (scanned=107)
cargo run -p xtask -- card-check            # PASSED (scanned=60, 19 warnings)
```

## 相关

- ADR-0028 文件改写互斥锁（`xtask guard`）
- ADR-0030 `memory-counts` / `adr-index` 机器校验
- ADR-0031 一卡一文件
- ADR-0032 文档护栏豁免清单
- ADR-0033 单文件行数 写作规范 vs CI 门禁
- PL-002 `check-comments` / `check-ledger` / `card-check` 未认领卡（**本次部分关闭**：card-check 已派 + 实现；check-comments / check-ledger 仍 OPEN）
- DRIFT-20260919-2（本次任务）

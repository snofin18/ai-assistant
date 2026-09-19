# TASK-051　xtask 护栏升级 — refscan / docscan / card-check / exemptions 内化

- 状态：**InProgress**
- 阶段：0　子阶段：—　依赖：001　预估：M　阻塞主线：是（PL-002 / PL-015 / PL-026 / PL-027 的机器化依赖本卡产出）
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-0-spikes.md`。

---

- 依赖：TASK-001（已 Done）　预估：M　难度：M
- **write scope**：
  - `tasks/TASK-051-xtask-refscan-docscan-card-check-exemptions.md`（本卡文件）
  - `xtask/src/{main,cli,repowalk,deferred}.rs`（cherry-pick 触达）
  - `xtask/src/{refscan,docscan,card_check,exemptions}.rs`（cherry-pick 新增 4 模块）
  - `docs/adr/0034-xtask-card-check-implementation.md`（cherry-pick 重建，ADR-0031 D6 机器化）
  - `docs/adr/README.md` §1 注册 ADR-0034 + 「下一个可用编号」改 0035
  - `LEDGER.md`（追加本卡一行）
  - `docs/memory/pitfalls.md`（追加防御性 PITFALL：commit 标题必须对应 `tasks/TASK-NNN-*.md`）
- **Out of scope**：
  - 任何 `crates/*` 产品代码、任何 `apps/*` 产品代码（阶段 0 仍是「零产品代码」）
  - 顺手重构 xtask 既有模块（hygiene / memory-counts / adr-index / guard 等已就绪）
  - **card-check 判据② 的实现**（已显式登记 PL-002；本卡只完成 ADR-0031 D6 的脚手架 + 判据①③④）
  - 改 `MEMORY.md` §1 之外的章节、`PLAN.md`、`plans/*`、`AGENTS.md`、其他任务卡、`ADR-0031` 自身
  - 改 `docs/spec/*`、升级 workspace 依赖、动 `ci.yml` 实质逻辑
  - 派生 `TASK-015b` 等「sub-card 后缀」编号（**新发明的 sub-card 形态不进 ADR-0031**，需要的话走 DRIFT 提案）

**背景与源流**

- 本卡的实质工作在悬空 commit `10f78db feat(xtask): TASK-015b 护栏升级 — refscan / docscan / card-check / exemptions 内化`（已用 `git tag orphan/task-015b-20260919 10f78db` 备份），原属 DRIFT-20260919-2 Phase D 第一轮编码，因 reset 移出 HEAD 链。**卡号纠正**：原 commit 自报「TASK-015b」违反 ADR-0031「一卡一文件、按号寻卡」—— TASK-015 是阶段 1 子阶段 1a 批次 A1 的另一张卡，scope 完全不同。本卡采用 **TASK-051**（现有 TASK-001~050 之后的下一个空号）。
- 源 commit 自报 4 条「已知遗留」（下一轮清理）：
  1. 4 个新模块顶部模块级 `lint allow`（WIP 仓促，~40 条 lint）→ **本卡必须清**
  2. `refscan` 输出仅汇总 counts、未打印具体 finding → **本卡必须清**（「输出仅 counts」是「看着像在跑」的伪完成，违反铁律 ① 无静默失败）
  3. `card-check` 判据② 未实现（仍归 PL-002）→ **本卡不实施**，仅完成 ①③④ 与 ADR-0034 脚手架
  4. `collect_repo_files` 在 `repowalk.rs` 临时标 `dead_code`（实际被 3 个 `run_*` 调用）→ **本卡必须清**

**In scope 清单**

1. `git cherry-pick 10f78db`（replay 12 文件改动，ADR-0034 自然回归）
2. 清理源 commit 自报的 3 条遗留（#1 / #2 / #4）
3. 跑全部验收命令（fmt / clippy / test / deny / hygiene / memory-counts / adr-index / refscan / docscan / card-check / llvm-cov）
4. 填执行记录 9 节 + 更新 `LEDGER.md` + 追加 `docs/memory/pitfalls.md` 一条防御性 PITFALL
5. 复核两次（机械 + 语义 / sub-agent）

**验收命令**

```powershell
# AGENTS.md §6 全套（gov §5.1 的 6 硬门禁 + 5 关键软门禁）
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan          # 本卡新增；输出必须含**逐 finding** 行（非仅 counts）
cargo run -p xtask -- docscan          # 本卡新增
cargo run -p xtask -- card-check       # 本卡新增；判据② 应明确报「未实现 = PL-002」而不报错
cargo llvm-cov --workspace --fail-under-lines 75
git status --porcelain                 # 应为空
```

**DoD**

- [ ] `refscan` / `docscan` / `card-check` / `exemptions` 四个子命令可用，输出确定性、可重放
- [ ] `refscan` 输出含逐 finding（path:line + rule + 命中片段），不仅 counts
- [ ] 4 个新模块顶部**无**模块级 `lint allow`；所有 clippy 警告就地修掉或显式登记 `clippy::too_many_arguments` 等已审查豁免
- [ ] `repowalk.rs` 中 `collect_repo_files` 无 `dead_code` allow（被 3 个 `run_*` 实际调用）
- [ ] `docs/adr/0034-xtask-card-check-implementation.md` 已重建，`docs/adr/README.md` §1 已注册 0034、「下一个可用编号」已改 0035
- [ ] `docs/memory/pitfalls.md` 已追加防御性 PITFALL（commit 标题必须对应 `tasks/TASK-NNN-*.md`）
- [ ] 全部 11 条验收命令全绿（`cargo test --workspace` 通过数 ≥ 基线 257）
- [ ] `LEDGER.md` 追加本卡一行；执行记录 9 节填齐

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点

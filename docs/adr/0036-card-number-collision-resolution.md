# ADR-0036　任务卡撞号解决 — xtask 051~055+055b 重编号为 059~064

状态：**Accepted**（2026-09-20）　日期：2026-09-20　Supersedes：—　Superseded by：—

## 背景

项目进度督察 2026-09-19（PL-022 / PL-035 第三次复发；详见 `tasks/TASK-065-resolve-card-number-collision.md`）发现：

- `tasks/TASK-051-…md` 到 `tasks/TASK-055-…md` 同时存在 2 张文件：
  - **stage-1 卡**（占位 + Ready 状态；批次表原始分配）：taint-tracking / instruction-origin-attribution / injection-target / clean-context-review / edge-adapter
  - **xtask 护栏升级卡**（实际落地 + InProgress/Done）：refscan-docscan / lint-cleanup / lint-cleanup-pass-2 / render-return / clear-per-line-allows
- `tasks/TASK-055b-…md` 用了 **sub-suffix 后缀命名**（违反 ADR-0031 D7），共 1 张
- 撞号总数 = **5 编号组 × 2 张 + 1 张 055b sub-suffix = 11 张文件**

撞号根因（PL-022 现场复现）：原悬空 commit `10f78db` 自报「TASK-015b」，被 TASK-051 ~ TASK-055 的批号（xtask 护栏升级主题）复用；stage-1 卡本应占 051~058 中的某些号但占用了 051~055，xtask 卡无可用号段。更糟：sub-suffix 命名（055b）让 **「按号寻卡」契约（ADR-0031 D7）直接断裂**。

**机器检查不会自动发现**：当前 `xtask card-check` 只查「9 节骨架齐全 + 状态非 Ready 必有文件 + 状态=Done/Review + 记录区空 → Warning」，**未实现**判据 ②「status 非 Ready 必有文件」（PL-002 待实现）+ 判据 ⑤「编号唯一性」。判据 ⑤ 实质上**不存在** —— 当前 `xtask` 没有「同一编号只能对应 1 个文件」的强制检查。

## 决策

**D1. xtask 6 张卡重编号为 059~064**（避开 stage-1 已占用的 011、035、036~058）：

| 旧文件名 | 新文件名 | 主题 |
|---|---|---|
| `tasks/TASK-051-xtask-refscan-docscan-card-check-exemptions.md` | `tasks/TASK-059-xtask-refscan-docscan-card-check-exemptions.md` | refscan / docscan / card-check / exemptions 内化 |
| `tasks/TASK-052-xtask-lint-cleanup-refactor.md` | `tasks/TASK-060-xtask-lint-cleanup-refactor.md` | xtask lint cleanup（B 路纯重构）|
| `tasks/TASK-053-xtask-lint-cleanup-pass-2.md` | `tasks/TASK-061-xtask-lint-cleanup-pass-2.md` | card_check str[Range] + dead_code + test f[0] |
| `tasks/TASK-054-xtask-render-return-result.md` | `tasks/TASK-062-xtask-render-return-result.md` | render 返回 Result |
| `tasks/TASK-055-clear-last-3-per-line-allows.md` | `tasks/TASK-063-clear-last-3-per-line-allows.md` | refscan 最后 3 处 per-line allow |
| `tasks/TASK-055b-promote-extension-is-helper.md` | `tasks/TASK-064-promote-extension-is-helper.md` | extension_is helper 提升 + repowalk.rs:172 allow 清掉 |

**理由**：stage-1 已就位 48 张 + 11 已迁 = 59 张，编号有依赖关系（015 → 011 → 012 等），改 stage-1 号风险大；xtask 卡是**事后插入**的护栏工作，应使用「已就位号段之后」的下一个空段。

**D2. stage-1 卡 051~055 不变**。它们的占位 + Ready 状态保持原样，stage-1 开工时按计划启用。

**D3. 撤回 sub-suffix 命名**。`TASK-055b` 改名为 `TASK-064`（见 D1 第 6 行）。未来**禁止** `TASK-NNNx` / `TASK-NNNa` / `TASK-NNNb` 等 sub-suffix 命名（ADR-0031 D7 强化） —— 新发明的 sub-card 形态必须**先开 DRIFT-ADR** 改 ADR-0031 D7 才行。

**D4. 状态同步**：6 张 xtask 新卡的 `- 状态：` 字段在复制时统一从 **InProgress** 改为 **Done**（与 LEDGER 2026-09-19 的 TASK-051/052/053/054/055/055b 已 Done 行一致）。本卡同时承担 `TASK-Sync-Card-Status-Fields` 的修复。

**D5. 未来治理（机器化）**：
- `xtask card-check` 必须新增判据 ⑤「**编号唯一性**」 —— 同一 `TASK-NNN` 只允许对应 1 个文件（不含 NNNb 等 sub-suffix）。
- **当前 PL-002**（card-check 判据 ② 未实现）应**合并**为「判据 ② ⑤ 一起实现」，归 **TASK-015**。本 ADR 不指定实现细节，只把契约明确化。

## 不接受的反模式

1. **不**用 git mv + sed 批量改号（虽然能做）：会丢「旧 → 新」映射关系的可审计性；本方案走「读旧 + 复制改 + 删旧」三步显式路径。
2. **不**保留 051~055 双文件（用 alias 引用）：git 会重复追踪同主题内容，`card-check` 未来加判据 ⑤ 会立刻红灯。
3. **不**改 stage-1 占位卡的号段：stage-1 已开工风险大，与 ADR-0031 D7「**正交**号段」语义冲突。

## 改 ADR-0031 的强制流程（自本 ADR 起）

任何动 `tasks/TASK-NNN-*.md` 编号的提交必须：
1. 先开 DRIFT-N ticket（per AGENTS.md §4）
2. 引用本 ADR 编号 0036（标记「xtask 护栏升级」专属）
3. 在 LEDGER.md 追加 `[supersedes:2026-09-20 ADR-0036]` 行
4. 人类裁决后才可合并

## 影响范围（本 ADR 实施时）

| 类别 | 文件 | 改动 |
|---|---|---|
| 卡文件 | `tasks/TASK-051/052/053/054/055/055b` (6 个) | 删除 |
| 卡文件 | `tasks/TASK-059/060/061/062/063/064` (6 个) | 新建（内容从旧文件复制 + 改标题 + 状态=Done + 写路径 + 内部引用）|
| 卡文件 | `tasks/TASK-065-resolve-card-number-collision.md` (1 个) | 新建（本 ADR 的实施卡）|
| 文档 | `docs/adr/README.md` §1 | 新增 0036 行 |
| 文档 | `README.md`「最近进展」节 | 改 TASK-051~055/055b 引用 → 059~064 |
| 台账 | `LEDGER.md` | 改 TASK-051~055/055b 引用 → 059~064 |

## 相关 ADR

- ADR-0026（ADR 编号登记表 + 0019 撞号事故处置）
- ADR-0031（任务卡一卡一文件，D7 「按号寻卡」精神）
- ADR-0035（workspace lint policy no-exceptions —— 同次审计发现的另一项 ADR 治理缺陷）

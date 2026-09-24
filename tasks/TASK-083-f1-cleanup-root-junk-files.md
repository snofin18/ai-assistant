# TASK-083　F-1 清理：根目录 17 个 octal-escaped 乱码文件名

- 状态：**Done**（2026-09-24）
- 阶段：0　子任务：stage-0 收尾（不是 B 工作）　预估：XS（~10 min）　阻塞主线：否
- write scope：repo root 删除 17 个乱码文件 + git rm + 本卡 + commit
- background：这些是早期阶段脚本（大概是一个 bash heredoc 路径转义失败的产物）创建的 0 字节文件，文件名是 octal-escaped UTF-8。git status 一直显示为 untracked，污染 git status 输出。

<!-- ══ 分界线 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-083 F-1 root 目录清理
【write scope】仅：repo root 17 个乱码 0-byte 文件 + 本卡 + commit
【铁律】AGENTS.md §3+§6 / 不改公共热点外的文件 / 不引入新依赖
【禁止】不要碰 spikes / crates / docs / tests / etc 任何合法文件 / 不要删除任何 git tracked 文件
【验收】git status clean for untracked junk；cargo test / xtask 全 PASSED；本卡 §1-9 + commit
【依赖】TASK-001 + TASK-073 (closeout 已在 audit §5 标记 F-1)
【疑问】无
```

### 2. 实际改动文件

- `tasks/TASK-083-f1-cleanup-root-junk-files.md`（本卡：状态行 + §1~§9 记录；**正文区其余部分逐字未动**）
- `LEDGER.md`（追加 1 行）
- **目标文件：0 个改动** —— 本会话开工前复核，仓库根**已不存在任何 0 字节 / 乱码文件名**（详 §3）。

> 状态行位于分界线以上，但**必须**由 Implementer 更新（ADR-0031 D2「状态只写在卡片文件的 `- 状态：` 行」；
> `xtask card-check` 亦要求记录区非空时状态 = Done/Review）。既有先例：`tasks/TASK-084-*.md` 的 Done 行。
> 除状态行外，正文区**一个字未改**。

### 3. 验收输出摘要

```text
# 1) 本卡验收判据：untracked junk 必须为 0
$ git status --porcelain -uall
（无输出 = 干净）

# 2) 仓库根文件普查
$ Get-ChildItem -File -Force
files=19
zero-byte=0
非 ASCII 文件名=0 个

# 3) 交叉验证（另一个进程/另一套枚举）
$ cmd /c "dir /b /a-d" | Measure-Object -Line
19
```

- `git status --porcelain -uall` **无输出** → 判据「git status clean for untracked junk」**满足**。
- 根目录 19 个文件**全部是 git tracked 的合法文件**（`AGENTS.md` / `Cargo.toml` / `PLAN.md` / `README.md` / `LEDGER.md` / `MEMORY.md` / 3 份架构与可行性 md / `LICENSE` / `NOTICE` / `rust-toolchain.toml` / `deny.toml` / `.gitignore` / `.gitattributes` 等）；**0 个 0 字节文件**、**0 个非 ASCII 文件名**。

### 4. DoD 逐条核对

- [x] 仓库根不再有 17 个 octal-escaped 乱码 0 字节文件（实测 0 个 0 字节文件、0 个非 ASCII 文件名）
- [x] `git status` 对 untracked junk 干净（`--porcelain -uall` 无输出）
- [x] 未删除任何 git tracked 文件（19 个根文件与 `git ls-files` 一致）
- [x] 未触碰 `spikes/` / `crates/` / `docs/` / `tasks/` 之外任何合法文件
- [x] 本卡 §1~§9 已填 + LEDGER 追加一行

### 5. 偏差

**不是 DRIFT（未越 scope），但有一条必须记录的事实偏差：目标文件在本卡执行前就已经不存在了。**

- **现象**：本卡正文写「repo root 删除 17 个乱码文件 + `git rm`」，但 2026-09-24 复核时仓库根**已无任何 0 字节 / 非 ASCII 文件名**。
- **为什么无法从 git 追溯**：这 17 个文件是**早期脚本产生的 untracked 文件**（本卡 background 已写明"`git status` 一直显示为 untracked"）→ **从未进入索引** → `git log --diff-filter=D` 查不到删除记录，无法定位"是谁、什么时候删的"。
- **影响**：卡内承诺的"删除动作"没有发生（也不需要发生）；但**验收判据本身（untracked junk = 0）已满足**，且副作用（污染 `git status`）已消失。
- **本轮处理**：按判据判定**达成**并收尾；把"移除路径不可考"如实写进 §3/§5，不伪造删除证据。
- **给人类的提示**：若你在别的 clone / worktree 里仍能看到这些文件，请在**那个**工作区补跑一次删除（本卡的动作在那里仍然有效）。

### 6. 更合理做法

1. **"清理类"卡应该先做一次实测再写验收**：本卡写于 2026-09-22，到 2026-09-24 执行时目标已经不存在 —— 中间隔了 TASK-011 收尾轮与两次合并。**先跑判据、再决定要不要动刀**，能省掉一次"照着旧描述去删不存在的文件"。
2. **untracked 垃圾应该在源头拦**：`git status` 是每个 agent 的第一条命令，被污染时所有人都会多花一次判断成本。建议护栏卡加一条 `xtask hygiene` 规则「仓库根不得出现 0 字节文件 / 非 ASCII 文件名」（Warning 起步）→ 已作为提案记入 §7。
3. **`git rm` 只对 tracked 文件有意义**：本卡正文写"删除 + `git rm`"是把两种情形混写；对 untracked 文件只需文件系统删除。下次写清理卡时先分清 tracked / untracked。

### 7. 遗留问题

- ① **提案（未开卡）**：`xtask hygiene` 增一条规则「仓库根不得出现 0 字节文件 / 非 ASCII 文件名」（Warning 起步，负向用例 = 造一个 0 字节文件必须报）。归属：护栏卡（TASK-015 或后续）。**未写入 `docs/PARKING_LOT.md`**（本卡 write scope 未含它）→ 请在阶段末评审时收编。
- ② 若其他 clone / worktree 仍有这些文件 → 见 §5 最后一条。

### 8. 新增长期记忆

无（本卡未产出跨会话可复用的 FACT / PITFALL / REJECTED；§6 的三条属本卡内的实施反思，已写在此处）。
`docs/memory/*` 与 `MEMORY.md` 规模表**未改动**（`xtask memory-counts` 仍 PASSED）。

### 9. 给审阅者的关注点

1. **"目标文件已不存在"是否接受为 Done**：判据（untracked junk = 0）已满足，但"删除动作"没发生。若你认为必须见到一次真实的删除动作才算 Done，请把本卡打回，并指定在哪个工作区执行。
2. **§6 第 3 条的规则提案**：`hygiene` 加"仓库根禁 0 字节 / 非 ASCII 文件名"是否值得做（它拦的是**污染 `git status`**，属体验类问题而非正确性问题）。
3. **状态行改动**：本卡在分界线**以上**改了 `- 状态：` 一行（其余正文未动）—— 请确认这符合 ADR-0031 D2 的预期（`card-check` 要求记录区非空时状态 = Done/Review）。

# TASK-224　唯一写入通道 + 文件占用探测 + FIFO 排队（提案，暂不实现）

- 状态：**Done（2026-10-04；ADR-0066 Accepted + `xtask write` 唯一写通道 / `share_mode(0)` 独占探测 / 有界退避与退出码 5 放弃 / 读路径不阻塞，PR #208 merge `919f2fa`；卡面正文里过期的「前置 ADR-0065」由本卡记录区与 PL-107 更正）**
- 阶段：1　子阶段：1a/治理　批次：治理池　依赖：ADR-0028、ADR-0063、**ADR-0065 Accepted**
- 预估：L　难度：L
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/adr/0028-file-rewrite-mutex-protocol.md`、`xtask/src/guard_runner.rs`、`docs/PARKING_LOT.md`

---

## 目标（一句话）

把"所有可能被锁的写操作"收敛到一个**唯一通道**，通道内对同一目标文件做**FIFO 排队**；
排队判据是"目标文件是否已被占用"，读操作不受限制。**本卡只记录方案，不实现。**

## 背景（为什么记录）

人类 2026-10-03 提出的方案；`ps_episodes.txt` 复扫确认存在"等文件锁"成分（`xtask guard`
本身就是 `Acquisition::Wait` 轮询等待，见 `guard_runner.rs`）。方案本身可行，但改动
ADR-0028 已决机制、且可能需要新增常驻组件 → 必须先有 ADR。

## 方案要点（待 ADR 裁决）

1. **唯一通道**：所有可能被锁的**写**操作经同一通道执行（如 `xtask write` 或 guard 升级）。
2. **判据 = 文件是否已被占用**：用 `CreateFile(..., dwShareMode = 0)` 独占探测
   （成功=空闲、`ERROR_SHARING_VIOLATION`=被占），必要时补 `LockFile` 字节范围探测。
3. **通道内排队**：占用则入队等待；拿到后"开-写-关-释放"。
4. **读不受限**：读命令完全不进通道；读文件用 `FileShare.ReadWrite` 永不等待。

## 已知边界（ADR 必须写清）

- **占用探测不完整**：只探测"不共享的持有者"；对方以 `FILE_SHARE_WRITE` 打开时独占探测会成功，
  可能并写。字节锁与共享模式是两套机制。
- **真排队需要常驻单进程通道**；若每次都是临时进程，只能做到"互斥 + 退避重试"，无 FIFO 顺序保证。
- **唯一性漏洞**：本环境大量写入走 `apply_patch`（由 Codex 应用执行），若它不纳入通道，
  "唯一写入通道"就不成立；要么纳入，要么明确通道只覆盖命令行写。

## 完成定义（DoD）

- [ ] ADR-0065 落档并 Accepted（人类裁决）。
- [ ] `xtask` 提供唯一写通道 + 独占探测 + 排队/短超时放弃。
- [ ] 读路径用 `FileShare.ReadWrite`，实测"有人写时仍可读且不等待"。
- [ ] 负向用例：目标被占用时排队/放弃行为可测；超时后无残留进程。
- [ ] `apply_patch` 等非命令行写入是否纳入，已明确裁决。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

【任务】TASK-224 唯一写入通道 + 文件占用探测 + 退避重试
【目标】ADR-0066 先行落档后，为命令行协作写者提供 `xtask write`：复用 ADR-0028 的 guard 锁、Windows 独占占用探测、有界退避、短超时退出码 5，读取路径不进入通道且不等待。
【write scope】`xtask/**`、`docs/adr/0066-*`、ADR 登记表、`docs/memory/decisions.md`、`facts/pitfalls` 与规模表、`docs/PARKING_LOT.md`、状态同步文件与 LEDGER；不动 `crates/**`。
【铁律】ADR-0065 编号已过期 → 使用实测下一个可用号 0066；xtask 零第三方依赖；无静默失败；超时退出码 5；ADR-0063 的内存/进程纪律。
【禁止】不实现常驻 FIFO；不把 `apply_patch` 宣称为强制纳入；不改变既有 guard 四操作语义；不给 xtask 加依赖。
【验收】fmt / clippy / workspace test / `cargo test -p xtask` / 占用与读取专项 / xtask 全部门禁。
【依赖】ADR-0028、ADR-0063 已核对；ADR-0066 由本批 Accepted；TASK-224 正文仍只读。
【疑问】正文仍写 0065 与“暂不实现”，状态行为 Ready；按预授权在 ADR-0066 与记录区更正，不改写正文。

### 2. 实际改动文件

- `docs/adr/0066-cli-single-write-channel-and-file-occupancy-probe.md`
- `docs/adr/README.md`、`docs/memory/decisions.md`、`docs/memory/facts.md`、`docs/memory/pitfalls.md`、`MEMORY.md`
- `docs/PARKING_LOT.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `xtask/Cargo.toml`、`xtask/README.md`
- `xtask/src/cli.rs`、`xtask/src/guard_runner.rs`、`xtask/src/main.rs`
- `xtask/src/write_channel.rs`、`xtask/src/write_channel_tests.rs`

### 3. 验收输出摘要

- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0（仅既有 `clippy::assert_is_empty` unknown-lint warning）。
- `cargo test --workspace` → EXIT 0，含 `write_channel` 4 个平台无关专项与 2 个 Windows 句柄专项；xtask 单测 **438 passed**。
- 实机读取：持 `guard` 锁时 `Get-Content` 返回 `read_ms=77`，内容 `after`，不等待。
- 实机占用：`FileShare::None` 持有目标时 `xtask write --timeout 0` → `sharing_violation(raw_os_error=32)`、`ABANDONED`、`xtask_exit=5`；目标仍为 `after`，`guard status` 为 NONE，残留 `xtask_processes=0`。

### 4. DoD 逐条核对

- [x] ADR-0066 落档并 Accepted（按用户 2026-10-03 预授权代为裁决）；ADR 登记表下一可用号同步为 0067。
- [x] `xtask write` 提供唯一命令行写通道 + `share_mode(0)` 独占探测 + 有界退避/短超时放弃（退出码 5）。
- [x] 读取路径不进入通道；写入句柄只共享 `FILE_SHARE_READ`，实机持锁读取实测通过。
- [x] 负向用例：guard 锁占用与目标占用均超时退出 5；无残留锁、无残留 `xtask` 进程、目标字节不变。
- [x] `apply_patch` 明确裁决：不纳入强制通道；非命令行写仍必须遵守 guard 协议。
- [x] 遵守 ADR-0063：stdin 16 MiB 硬上限、无子进程、失败/超时均释放锁。

### 5. 偏差

- 卡面正文写“前置 ADR-0065 Accepted；本轮不实现”，但 0065 已被 TASK-223 占用；本批使用运行时可用的 **0066**（`PL-107` 已闭环）。
- 卡状态行仍为 `Ready`，因为正文区只读；本记录区与 LEDGER 状态为 **Done**。收口比对时该项按“不一致已在本卡 §5 写明原因”处理。

### 6. 更合理做法

先用 ADR-0066 固定三条边界，再实现：① 互斥 + 退避而非常驻 FIFO；② `FILE_SHARE_WRITE` 盲区显式承认；③ `apply_patch` 不强制纳入。这样不会把协作式锁误描述成内核强制互斥。

### 7. 遗留问题

- 与 TASK-223（L1 文件通道）是不同问题：那条是"文件通道执行"，本条是"写操作串行化 + 锁规避"。
- 非 Windows 平台没有 `share_mode(0)` 等价原语，`xtask write` fail-closed；既有 `guard` 仍可跨平台使用。
- 若将来需要公平 FIFO 或把 `apply_patch` 强制纳入，必须另立 ADR 并引入常驻进程/OS 级拦截。

### 8. 新增长期记忆

- `docs/memory/decisions.md`：ADR-0066 的完整决策摘要。
- `docs/memory/facts.md`：`share_mode(FILE_SHARE_READ)` 不阻塞读取、`share_mode(0)` 触发错误 32 的实机事实。
- `docs/memory/pitfalls.md`：独占探测盲区、临时进程无真 FIFO、`apply_patch` 不可拦截的边界。

### 9. 给审阅者的关注点

- `write_channel::run` 的探测超时与锁释放顺序：任何 `probe_exclusive` 错误都必须先释放 guard 锁再上抛。
- 写入句柄的 Windows share mode 是否正确：必须只共享读取，避免“通道内写”阻塞读。
- ADR-0066 的“唯一通道”是否被不恰当地读成强制互斥：审阅时请确认 `apply_patch` 与非 Windows 的限制没有被省略。

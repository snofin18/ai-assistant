# ADR-0066　命令行唯一写通道与文件占用探测

状态：**Accepted**（2026-10-04，按用户 2026-10-03 预授权代为裁决；TASK-224）
日期：2026-10-04
Supersedes：—
Superseded by：—
关联：**TASK-224**、ADR-0028、ADR-0063、`AGENTS.md` §3 / §8 / §12、
`docs/PARKING_LOT.md` PL-107、`xtask/README.md`

---

## 背景

PowerShell 长会话的卡顿取证把问题分成三类：等待外部状态、并发启动多个 PowerShell、以及残留
`conhost.exe`。其中 `xtask guard acquire` 的 `Acquisition::Wait` 轮询是一条真实存在的等待路径。
TASK-224 因此提出：把可能被锁住的命令行写操作收敛到一个通道，并先探测目标文件是否已被其他进程占用。

但 ADR-0028 已经确定 `guard` 是**按文件、协作式**的锁：它依赖所有写者自愿先取锁，无法技术强制；
而“文件是否已被占用”又是另一个问题——Windows sharing mode 与 guard 的锁文件不是一回事。
在没有新 ADR 的情况下直接把两者混成一个“唯一通道”，会让下一位 agent 误以为 guard 已经具备强制力。

本 ADR 因此先明确边界，再落实现：**扩展** ADR-0028，不取代它；`guard` 仍然是协作式锁。

## 决策（一句话）

新增 `xtask write <目标>` 作为**命令行协作写者**的唯一写入通道：先走 ADR-0028 的
`guard acquire`，再在 Windows 上对目标做 `share_mode(0)` 独占占用探测，占用时退避重试，
短超时退出码 **5**；读取路径完全不进入通道。真正的 FIFO 与 `apply_patch` 强制拦截都不纳入本次决策。

## 决策细化

### D1　唯一通道的边界

- 通道入口固定为 `xtask write <目标>`；内容从 **stdin** 读入，默认整文件替换。
- 命令行协作写者必须经 `xtask write` 改文件。`read` / `Get-Content` / `std::fs::read`
  等读路径**不进入**通道，也不等待 guard 锁。
- “唯一”只对遵守本协议的 CLI 写者成立；这是协作式机制，不是内核强制机制。

### D2　与 ADR-0028 的关系

- 本 ADR **扩展** ADR-0028，不取代任何一条现有 `guard` 语义。
- `guard` 仍是四操作 `acquire` / `release` / `status` / `reap`，锁文件的原子创建、
  超时放弃、陈旧接管、多文件回滚和 owner 规则全部不变。
- `write` 复用同一把 `target/locks/` 锁及同一组 `--owner` / `--task` /
  `--intent` / `--timeout` / `--stale-after` / `--force` 参数：
  先 `guard acquire`，写入完成后立即 `guard release`。
- `write` 不会把 `guard acquire` 变成 OS 强制锁；它只是在 guard 之上增加目标文件占用探测。

### D3　占用探测

- **Windows**：探测使用
  `std::os::windows::fs::OpenOptionsExt::share_mode(0)` 打开目标。
  - 打开成功 = 当前没有拒绝共享的持有者；
  - `ERROR_SHARING_VIOLATION` / `ERROR_LOCK_VIOLATION` = 目标被占用，进入退避重试；
  - 其它 IO 错误原样失败，不伪装成“空闲”。
- 探测句柄在进入写入前立即关闭；实际写入使用仅允许读者共享的模式
  (`FILE_SHARE_READ`)，从而保持“有人在写通道里时，读取者仍可打开目标文件”的要求。
- **非 Windows**：`std` 没有跨平台的等价于 `share_mode(0)` 的原语。本 ADR 明确选择
  **fail-closed**：`xtask write` 在独占探测不可用时拒绝写入并说明平台限制；
  既有 `guard` 仍可在非 Windows 上协作使用。

### D4　排队取舍：不做真 FIFO

采用“互斥 + 有界退避重试”，**不实现真 FIFO**：

| 选项 | 结论 | 代价 |
|---|---|---|
| 常驻单进程 FIFO 网关 | ❌ 本批不选 | 需要新增长期驻留组件、生命周期管理、崩溃恢复与额外接管面；当前只是开发期护栏 |
| 临时进程 + 退避重试 | ✅ 采纳 | 无顺序保证；可能饥饿；超时者退出码 5 并通报 |

这不是把“排队”偷换成“永远成功”：没有在 `--timeout` 内拿到锁或目标时，调用方得到明确的
退出码 5；工具不会无限等待，也不会假装成功。

### D5　已知盲区：探测不完整

- 独占探测只能发现“不共享写”的持有者。对方若以 `FILE_SHARE_WRITE` 打开目标，
  `share_mode(0)` 探测可能成功；随后两个写者仍可能并写。
- 字节范围锁 (`LockFile`) 与 sharing mode 是两套机制；本 ADR 不把两者混称。
- 因此占用探测只降低撞车概率，不能替代 ADR-0028 的协作锁，也不能承诺绝对互斥。

### D6　`apply_patch` 等非命令行写入的裁决

**不纳入本轮唯一通道的强制范围。**

理由：`apply_patch` 由 Codex 的编辑工具执行，不是 `xtask` 能拦截的命令行写者；要强制纳入，
必须替换编辑工具链或引入 OS 级文件系统过滤驱动，超出本开发工具的边界。

因此本 ADR 明确把“唯一写通道”限定为**命令行协作写者**。非命令行写入仍必须遵守
ADR-0028：写公共热点文件前先用 `guard acquire`，改完立即释放。若要进一步提升强制力，
需要新的 ADR 与新的执行环境约束，不能从本 ADR 的口径里推断。

### D7　超时、放弃与通报

- `xtask write` 的默认超时为 **5 秒**（比 `guard` 的 30 秒更短，避免 PowerShell 会话长时间阻塞）；
  显式 `--timeout` 可覆盖。
- guard 锁等待超时沿用 ADR-0028 的退出码 5。
- 目标占用重试超时也返回退出码 5，并输出
  `-- write-result: ABANDONED ...`，向 `target/locks/abandonments.log` 追加
  `WRITE_ABANDONED` 行，同时提示在 `LEDGER.md` 记录放弃。

### D8　资源生命周期与输入边界

- `write` 不启动子进程，因此不产生需要 kill / reap 的进程树；超时路径必须直接退出，
  不留下后台进程。
- stdin 内容有硬上限：**16 MiB**。达到上限时显式失败，禁止无界读入内存。
- 写入失败、探测失败和超时都必须释放已经获取的 guard 锁；锁释放失败单独报错，不静默。

## 考虑过的选项

| # | 方案 | 结论 | 理由 |
|---|---|---|---|
| 1 | 只扩展 `guard` 的 `--timeout`，不探测目标 | ❌ | 不能覆盖“目标已被外部进程占用”这一类等待 |
| 2 | 直接用 `LockFile` 字节范围锁做唯一互斥 | ❌ | 需要额外 FFI；与 sharing mode 语义不同，且可能阻塞读取者 |
| 3 | 常驻 FIFO 网关 | ❌ 本批不做 | 真 FIFO 需要长期进程；ADR-0063 要求为其增加生命周期与收敛证据，收益不足 |
| 4 | `xtask write` 临时进程 + 独占探测 + 退避 | ✅ 采纳 | 零依赖、和现有 guard 共锁、可在 Windows 上给出真实占用证据 |
| 5 | 把 `apply_patch` 强制纳入 | ❌ | 无法从 xtask 拦截；强行宣称会制造“唯一通道”的假象 |
| 6 | 非 Windows 静默退化成“无探测写入” | ❌ | 静默失败；本 ADR 选择 fail-closed 并明确平台限制 |

## 影响

### `xtask`

- 新增 `xtask/src/write_channel.rs` / `write_channel_tests.rs`：纯判定、IO 边界与 Windows 探测。
- `xtask/src/main.rs` 增加 `write` 分派；`guard_runner::run_acquire` 只放宽到 crate 内可复用，
  行为不变。
- `xtask/src/cli.rs` 增加 `write` 操作数，并复用现有 guard 选项。
- `xtask/README.md` 增加通道用法、退出码、平台限制与盲区。

### 不变更

- `crates/**`、公共 trait / schema / IPC / DB。
- ADR-0028 的四操作语义。
- `AGENTS.md` 的铁律、write scope 与热点文件锁协议。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 外部写者以 `FILE_SHARE_WRITE` 绕过探测 | ADR 明写盲区；保留 guard 协作锁；review 时不得把“探测通过”解释成“绝对没人写” |
| 非 Windows 无法探测 | fail-closed；不静默降级；平台限制写进命令输出与 README |
| 超时后残留 guard 锁 | 写入、探测、读取失败都走统一释放路径；负向测试断言锁不存在 |
| stdin 过大 | 16 MiB 硬上限，超限即失败 |
| 读取者被写句柄阻塞 | 写入句柄只共享读取；专项测试实测读取成功且快速返回 |

## 验证方式

1. `cargo test -p xtask`：覆盖成功写入、目标占用超时、guard 锁占用超时、读句柄不被阻塞、
   非 Windows fail-closed、输入上限和锁释放。
2. Windows 实机：
   - 用一个独占探测句柄占住目标，运行 `xtask write --timeout 0` → 退出码 **5**、
     stdout 含 `ABANDONED`、无新增子进程、目标字节不变、guard 锁已释放。
   - 在 `guard acquire` 持有锁期间执行读取 → 读取成功且不等待。
3. 全部 xtask 文档门禁：`hygiene` / `memory-counts` / `adr-index` / `refscan` /
   `docscan` / `card-check` / `check-ledger` / `check-comments` / `verify-schemas` /
   `codegen --check` 全绿。

## 重新评估触发条件

1. 若需要真正的公平 FIFO 或强制互斥，另立 ADR，要求常驻进程或 OS 级机制及其生命周期证据。
2. 若 `apply_patch` 被替换为可插拔网关，重新评估是否能把非命令行写入纳入同一通道。
3. 若出现 `FILE_SHARE_WRITE` 并写导致真实丢失，升级为强制式方案，不能继续扩大协作式锁的口径。

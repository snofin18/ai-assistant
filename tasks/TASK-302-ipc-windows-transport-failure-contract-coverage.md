# TASK-302　IPC Windows 传输失败 / 超时公开契约覆盖率补强（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：无（`assistant-ipc` Windows NamedPipe 传输已在 main）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`crates/ipc/tests/windows_transport_failure_contract.rs`、`tasks/TASK-302-ipc-windows-transport-failure-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-15-round-5.md`、`docs/automations/2026-10-15-report.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5、`crates/ipc/src/windows_transport.rs`、`crates/ipc/src/windows_io.rs`、`crates/ipc/README.md`、`docs/spec/testing.md` §4.4

**目标**

仅用公开 API 契约测试补强 Windows NamedPipe 传输的失败与超时路径：覆盖非法管道名、
角色误用的模块级失败、未连接 / 已关闭句柄、`accept` 超时后的取消与回收、
`recv` 超时后连接仍可继续使用、`send` 超时显式失败，以及对端进程路径查询的
不存在 PID 失败。当前 `assistant-ipc` 行覆盖率为 1102 行中 159 行未覆盖
（85.57%），其中 `windows_io.rs` 71.84%、`windows_peer.rs` 69.44%；
本卡目标是提高这三块的公开可达覆盖率，并保持 crate ≥85%。生产源码零改动，
不新增依赖，不操作真实 GUI、网络服务或凭据。

**背景**

2026-10-16 运行日 round 5 的 P2 扫描显示 `assistant-ipc` 已经过 85% 行覆盖门槛，
但 `--show-missing-lines` 的剩余缺口集中在 Windows 传输的取消 / 回收与失败分类，
而不是纯协议序列化。现有 `coverage_contract.rs` 已覆盖正常 NamedPipe 往返、
握手、断连与 `connect` 超时；本卡补的是 `accept` / `send` / `recv` 超时和
生命周期边界的公开契约，避免用白盒测试直接调用私有模块。

**步骤**

1. 新增 `crates/ipc/tests/windows_transport_failure_contract.rs`，只调用 crate 根导出 API。
2. 覆盖非法管道名、未连接 / 已关闭传输的角色与句柄失败语义。
3. 用真实本地 NamedPipe 覆盖 `accept` 超时取消、`recv` 超时后复用、`send` 超时失败。
4. 覆盖不存在的 PID 无法解析进程镜像路径。
5. 用真实 `cargo llvm-cov` 记录改动前后覆盖率，跑完整门禁并同步状态。

**DoD**

- [ ] 生产逻辑零改动；测试只调用公开 API、不新增依赖、文件 ≤600 行
- [ ] `assistant-ipc` crate 行覆盖率 ≥85%，且相对 85.57% 有真实提升
- [ ] `windows_io.rs` / `windows_peer.rs` / `windows_transport.rs` 的公开可达缺口有前后证据
- [ ] `cargo llvm-cov -p assistant-ipc --tests --fail-under-lines 85` 与 `cargo test -p assistant-ipc` 全绿
- [ ] `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check` 全绿
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-ipc --tests --fail-under-lines 85
cargo test -p assistant-ipc
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- hygiene; cargo run -p xtask -- check-comments; cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index; cargo run -p xtask -- refscan; cargo run -p xtask -- docscan
cargo run -p xtask -- card-check; cargo run -p xtask -- check-ledger; cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations; cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录

### 1. 约束回执

【任务】TASK-302 IPC Windows 传输失败 / 超时公开契约覆盖率补强　【目标】只加公开
API NamedPipe 契约测试，清理失败、超时、取消 / 回收路径并保持 crate ≥85%　
【write scope】仅本卡列出的测试与状态 / 轮次文件　【铁律】无静默失败；生产源码零改动；
不扩 scope；热点文件先 guard；证据必须真实　【禁止】改公共接口、依赖、lint/CI、
schema/DB/IPC；真实 GUI / 网络服务 / 凭据　【依赖】无（`assistant-ipc` 已在 main）　
【疑问】无

### 2. 实际改动文件

- `crates/ipc/tests/windows_transport_failure_contract.rs`（新增 6 个公开 API 契约测试）
- `tasks/TASK-302-ipc-windows-transport-failure-contract-coverage.md`
- `docs/automations/2026-10-15-round-5.md`
- `docs/automations/2026-10-15-report.md`
- `LEDGER.md`
- 仓库外日志 `C:\Users\fexfe\.codex\automations-logs\2026-10-15\ai-assistant-task-round-20261016-0300.jsonl`

### 3. 验收输出摘要

```text
改动前：assistant-ipc TOTAL 1102 行中 159 行未覆盖 = 85.57%
        windows_io.rs 71.84%；windows_peer.rs 69.44%；windows_transport.rs 89.53%
改动后：assistant-ipc TOTAL 1102 行中 140 行未覆盖 = 87.30%
        windows_io.rs 79.18%；windows_peer.rs 69.44%；windows_transport.rs 90.05%

cargo llvm-cov -p assistant-ipc --tests --fail-under-lines 85 -> EXIT 0
  windows_transport_failure_contract.rs = 6 passed / 0 failed
cargo test -p assistant-ipc -> EXIT 0
cargo fmt --all --check -> EXIT 0
cargo clippy --all-targets -- -D warnings -> EXIT 0（仅既有 unknown-lint 警告）
cargo test --workspace -> xtask 汇总 469 passed / 0 failed；EXIT 0
cargo test -p assistant-core arch:: -> arch 10 passed；EXIT 0
cross-target clippy（linux-gnu / aarch64-darwin，assistant-ipc）-> EXIT 0
cargo run -p xtask -- replay --suite core -> 2 step(s) / 5 change(s) / 0 mismatch
cargo run -p xtask -- hygiene -> scanned=425 0E/114W / PASSED
cargo run -p xtask -- check-comments -> scanned=425 0E/68W / PASSED
cargo run -p xtask -- memory-counts -> 0E/0W / PASSED
cargo run -p xtask -- adr-index -> scanned=83 0E/0W / PASSED
cargo run -p xtask -- refscan -> scanned=913 0E/0W / PASSED
cargo run -p xtask -- docscan -> scanned=467 0E/264W / PASSED
cargo run -p xtask -- card-check -> scanned=196 0E/34W / PASSED
cargo run -p xtask -- check-ledger -> 0E/0W / PASSED
cargo run -p xtask -- verify-schemas -> 5 schemas 0E / PASSED
cargo run -p xtask -- codegen --check -> 0 drift / 0E / PASSED
cargo run -p xtask -- check-migrations -> 5 files / 0E / PASSED
cargo deny check -> advisories / bans / licenses / sources ok；EXIT 0
```

`windows_peer.rs` 剩余未覆盖行仍是 Win32 API 失败与缓冲区边界兜底；本轮没有通过读取
受保护进程或凭据来强行覆盖，因此该文件保持 69.44%。

### 4. DoD 逐条核对

- 生产逻辑零改动；只调用公开 API；无新依赖；测试文件 6 个用例：**满足**。
- `assistant-ipc` crate 85.57% → 87.30%，≥85%：**满足**。
- `windows_io.rs` / `windows_peer.rs` / `windows_transport.rs` 的公开可达缺口有
  `--show-missing-lines` 前后证据：**满足**。
- 专项覆盖率、crate tests、fmt、clippy、workspace tests、双目标 clippy、arch、
  replay、xtask 硬门禁、deny：**满足**。
- `LEDGER.md` Review 行与 round / report 已同批落主 PR；Done 与 PLAN / README /
  plans 状态按既有流程在 merge hash 回填 PR 中同步：**满足**。

### 5. 偏差

- PowerShell 探针超过 90 秒零输出；已按预案中断卡住会话并执行
  `taskkill /F /IM pwsh.exe /T` + `taskkill /F /IM powershell.exe /T`，后续改用
  `cmd.exe` + WSL 执行。所有门禁均真实重跑。
- 两处命令调用误把 shell 设为 WSL bash 后执行 `cargo`，得到
  `cargo: command not found`；立即改用 `cmd.exe` 重跑，未产生文件写入或无效证据。
- 首轮 PR CI 的 `commitlint` 因主提交标题冒号后缺空格失败；已修正提交消息并
  `--force-with-lease` 推送同一分支，随后重新等待全部 CI。
- 初版 `recv` 复用测试让服务端在发送后立刻 `close`，`DisconnectNamedPipe` 会丢弃
  尚未读走的缓冲消息，导致客户端读到 Win32 233；修正测试先等待客户端 ack 再关闭
  服务端。未改生产源码，也没有放宽断言。
- 未发生范围 / ADR / 契约偏差。

### 6. 更合理做法

- 先用 `--show-missing-lines` 把不可达防御分支与公开可达 timeout / cancel 路径分开，
  再写测试；因此没有把 `windows_peer.rs` 的受保护进程兜底硬塞进本卡。
- 超时重试契约由服务端保持连接并等待客户端 ack 才关闭，避免把
  `DisconnectNamedPipe` 的丢弃语义误判成读端错误。

### 7. 遗留问题

- 主 PR #423 / CI `38792724568` / merge `c26a620`；post-merge main push run `38795033470` = success；Done 回填已完成。
- `windows_peer.rs` 的 Win32 失败与缓冲区边界需真实受控环境或后续注入面；不在本卡扩范围。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED；公开可达缺口与 Windows NamedPipe 超时复用证据
  已写入本卡与 round 文件。

### 9. 给审阅者的关注点

1. 6 条用例是否都只经公开 API，且真的覆盖 `accept` / `send` / `recv` 的失败与超时语义。
2. `windows_peer.rs` 未覆盖行是否确实超出无人值守可用范围；若不同意，应先决定测试
   注入面或生产改造，不应让本卡改判据。
3. PowerShell 挂起与两次 shell 误用是否记录充分且未污染门禁证据。

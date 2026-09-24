# TASK-019　`automation-host` 进程 + `ipc`（JSON-RPC/NamedPipe + token + 对端身份校验 + 心跳 + 看门狗）

- 状态：**Done**
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：016　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：016（`crates/platform/api` 的 trait 形状**冻结**；本卡**不改**形状）
- **预估**：M　**难度**：M
- **write scope**：`crates/ipc/**`（新 crate `assistant-ipc`）、`apps/automation-host/**`（新 bin crate `assistant-automation-host`）、`docs/DEPENDENCIES.md`（**仅追加**本卡实际引入的依赖行 —— 见「待裁决」Q2）
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）；`docs/spec/ipc-protocol.md`（§3 线上格式 / §4 不变量）；`docs/spec/envelope.md`（§3 `Header` / `Payload`）；`docs/spec/error-codes.md`；`docs/spec/capability-matrix.md`；架构 v2 §3.3（进程与信任边界）/ §3.4（三条关键数据流）/ §12.1 画像 C（本地恶意进程）；**铁律 1 / 7 / 8**；ADR-0028（公共热点文件锁）

**目标**

新建 `crates/ipc` + `apps/automation-host`，把 `docs/spec/ipc-protocol.md` §3 的线上协议落成**可运行、可测试**的代码：帧（magic `0xC0DECAFE` + `envelope_size` + CRC32）→ 握手（`ClientHello` / `ServerHello` / `Heartbeat`）→ 消息流（`Request` / `Response` / `AuditEvent`）；服务器侧必须做 **token 校验 + 对端身份校验**，双方都必须有**心跳**与**看门狗**（host 崩溃可被检测，检测到之后**不静默**）。

本卡**只做**「进程 + 传输 + 握手 + 存活检测」四件事；**不做**工具语义（`tool-bus` = TASK-020）、**不做**策略判定（`policy` = TASK-021）。

**为什么现在新建两个 crate（显式声明，不静默扩范围）**

新建 `crates/ipc/` 与 `apps/automation-host/` 命中**漂移触发器 ②**（加 crate / 顶层目录），但它是**计划内**的：
① `plans/stage-1-pilots.md` 批次表 A2 已把 `apps/automation-host/**` 与 `crates/ipc/**` 分给本卡；
② 根 `Cargo.toml` 的 `members` 已含 `"crates/*"` 与 `"apps/*"` 两个 glob → **不需改根 `Cargo.toml`**（实测确认）；
③ `docs/spec/ipc-protocol.md` §2 明确把「token / 对端身份校验的实现」归 `apps/automation-host`。
→ 因此**不是**触发器 ② 所指的「未经计划的扩范围」；按铁律 9 在此显式声明，并在执行记录 §5 复述。

**In scope（本卡交付物）**

| # | 交付物 | 依据 |
|---|---|---|
| 1 | `crates/ipc/Cargo.toml`（`assistant-ipc`）+ `src/lib.rs` + `README.md`（职责 / 边界 / **不变量** / 已知限制） | gov §5.4 + 阶段 1 DoD「每个 crate 有 README」 |
| 2 | **帧编解码（纯函数）**：`encode_frame` / `decode_frame`；magic 不符 / CRC32 不符 / `envelope_size` > 16 MB 三类错误**可区分**且用 `ErrorCode` 表达 | `docs/spec/ipc-protocol.md` §3 + §4 不变量 1~3 |
| 3 | **握手**：`ClientHello`（version + capabilities）/ `ServerHello`（version + capabilities + session_id）/ `Heartbeat`；`schema_version` 不匹配在**握手阶段**拒绝（永不进入正常流） | 同上 §3 + `docs/spec/envelope.md` §4 不变量 1 |
| 4 | **传输**：`Transport` trait（`connect` / `accept` / `send` / `recv` / `close`）+ Windows 命名管道实现；**非 Windows 目标必须编译通过**并返回明确的「本通道不可用」错误（铁律 1） | 架构 v2 §3.3 + 铁律 1 |
| 5 | `apps/automation-host/src/main.rs`：bin 入口 + 命名管道服务器 + **握手 token 校验** + **对端身份校验** | 本卡标题 + `docs/spec/ipc-protocol.md` §2 |
| 6 | **心跳与看门狗**：约定间隔内收不到 `Heartbeat` / `Response` → 判定断连，**返回带 `ErrorCode` 的错误 + 记一条 audit**（禁用 `let _ =`） | 本卡标题 + 铁律 1 |
| 7 | **协议错误分类测试**：magic / CRC32 / 超长 / 版本不匹配 / 缺 token 五类各 ≥1 个用例（含**负向**用例，ADR-0019 N1） | `docs/spec/ipc-protocol.md` §4 |
| 8 | `docs/DEPENDENCIES.md` 追加本卡引入的第三方依赖行（**先登记后引入**） | 登记表「登记规则」1 |
| 9 | 执行记录 §3 写入**实测证据**：本机（Windows 11 25H2）真实启动 host + 握手 + **kill host 后客户端检测到断连**的记录 | gov §3.4 第 3 节 |

**Out of scope（做了算漂移）**

- `crates/tool-bus/**`（MCP client / in-process server / 信封校验）→ **TASK-020**
- `crates/policy/**`（白名单 / 风险分级 / 默认拒绝）→ **TASK-021**
- `crates/platform/windows/**` 的任何改动（合成输入 / UIA / 坐标已归 017 / 018）
- 真实目标应用的自动化（阶段 1 Out of scope 全文）
- Linux / macOS 的传输实现细节（`docs/spec/ipc-protocol.md` §2 明确不管）→ 阶段 5~7
- 加密 / 压缩 / 分片（`docs/spec/envelope.md` §2 明确不管）
- 改 `crates/protocol/**`（schema 与 codegen 生成物）—— 见 Q3；确需改 → DRIFT（触发器 ③）
- 改 `crates/platform/api/**`、`crates/core/**`、`apps/**` 下本卡以外的任何目录
- 任何 `#[allow]` 放宽（触发器 ⑥）

**必须遵守**

1. **铁律 1（无静默失败）**：任何失败路径返回带 `ErrorCode` 的错误；**禁止** `let _ =` 丢弃 `Result`，**禁止** `unwrap()` / `expect()` / `panic!()` / `todo!()`（workspace lint 已 deny）。
2. **铁律 8（element 不出进程）**：跨进程帧的载荷**只允许** `crates/protocol` 的可序列化类型；**禁止**把 HWND / COM 指针 / `ResolvedElement` 放进任何消息。
3. **铁律 7（依赖方向）**：`crates/ipc` **不得**依赖 `crates/platform/*`；平台相关代码一律 `#[cfg(windows)]` 门控。
4. **`docs/spec/ipc-protocol.md` 是契约**：帧布局、握手字段、§4 不变量 1~4 逐条落地；**不得**私自加字段（加字段 = 改契约 = 触发器 ③）。
5. **bin crate 的 stdout 纪律**：workspace lint `print_stdout` / `print_stderr` = deny → 诊断一律走 `Result` 或结构化日志（若引入日志库 = 新依赖 → Q2）。
6. **公共热点文件写前取锁**（ADR-0028）：写 `LEDGER.md` / `PLAN.md` / `README.md` / `docs/memory/*` / `docs/PARKING_LOT.md` / `plans/*` 之前必须 `guard acquire`，写完**立刻** `guard release`；超时（exit 5）**必须**在 `LEDGER.md` 追一行，**不得** `--force`。
7. **规模**：单文件 ≤ 400 行（软）/ 600（硬）；函数 ≤ 80 行；参数 ≤ 6 个。
8. **不得**为让门禁变绿而改测试断言或放宽 lint（触发器 ⑥ / ⑦）。

**待裁决（命中即记 `DRIFT-019-x`；夜间运行时按人类 2026-09-25 授权**自决**，裁决必须落盘 §5 + LEDGER）**

- **Q1（对端身份校验的实现路径）**：两条路 —— (a) `GetNamedPipeClientProcessId` + `QueryFullProcessImageNameW` 取客户端镜像路径并按白名单比对（`windows` crate 已登记、已批准）；(b) 仅握手 token + 管道 ACL。**默认取向 = (a) 与 (b) 同时做**：(b) 是必要条件；(a) 取不到进程信息时**降级为「记 audit + 拒绝」**，**不得**静默放行。
- **Q2（新第三方依赖）**：本卡**预计**会需要（候选：`uuid` 的 UUID v7 session_id、CRC32 实现、异步或线程模型、日志库）。按触发器 ①，引入前必须在 `docs/DEPENDENCIES.md` 登记并写「替代方案与否决理由」。**默认取向 = 能用 std 就不用新依赖**：CRC32 手写查表（< 40 行，可单测）；本卡无 async 需求 → 用 **std + 线程**，不引入 `tokio`。若必须引入 → 登记 + 记 `DRIFT-019-x`。
- **Q3（envelope 类型归属）**：`docs/spec/envelope.md` §3 的 `Header` / `Payload` **当前不在** `crates/protocol`（该 crate 只生成了 §5.3 的 `ToolEnvelope`）。**默认取向 = 不自建第二套 envelope**：传输层帧由 `crates/ipc` 定义，载荷复用 `crates/protocol::ToolEnvelope`，握手能力项复用 `crates/protocol::Capability`。若判断必须把 `Header` / `Payload` 入 `protocol/` → 那是**改契约 + 超 write scope** → 记 DRIFT 并**停**，不得擅自改 `protocol/`。
- **Q4（批次表那条 ≥95% 指标）**：「Spike B 的重解析矩阵达标（≥95%）」属**元素定位**指标（TASK-017 已交付），本卡只保证「element 不出进程」。若 §3 拿不到该数字 → 在 §3 / §7 **如实写明**「本卡不产生该数字，来源 = TASK-017 执行记录」，**不得**编造。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-ipc
cargo run -p xtask -- hygiene / memory-counts / adr-index / docscan / card-check / check-ledger
```

**完成定义（DoD）**

- [ ] 上列命令全部通过（`refscan` 只需**不新增** Error —— 既有 151 项 baseline 见 PL-058）
- [ ] 帧编解码 5 类协议错误各有用例（magic / CRC32 / 超长 / 版本不匹配 / 无 token）
- [ ] 本机真机实测：`apps/automation-host` 能真实启动并监听命名管道；握手成功后收到 `ServerHello`；**host 被 kill → 客户端在 timeout 内检测到断连并给出错误**（证据写进 §3）
- [ ] 非 Windows 目标 `cargo clippy --target x86_64-unknown-linux-gnu -p assistant-ipc --all-targets -- -D warnings` 退出码 0
- [ ] `crates/ipc/README.md` 含职责 / 边界 / 不变量 / 已知限制四节
- [ ] `docs/DEPENDENCIES.md` 已登记本卡引入的**全部**第三方依赖（零新增则显式写明「本卡零新增」）
- [ ] `LEDGER.md` 追加一行；如新增事实/坑 → `docs/memory/{facts,pitfalls}.md`
- [ ] 无任何 Out of scope 文件被修改

**夜间自动化补充约束（仅当本卡由夜间 automation 执行时适用；依据 = 人类 chat 2026-09-25 授权）**

1. 每次运行**只做一张卡**（AGENTS.md §3「一个会话最多完成 1~2 张卡」取严）。
2. 分支 = `task/TASK-019-automation-host-ipc-named-pipe`，从**最新 `origin/main`** 切出；**PR base 必须是 `main`**（PL-075 教训：栈式 base 会让子 PR 静默合进父分支而不进主干）。
3. 需要裁决的项**自决**，但裁决必须**落盘**（卡 §5 全文 + `LEDGER.md` 一行），只在聊天里说 = 视为未发生。
4. 合并自己的 PR 需**同时**满足三个条件：CI 全部 check 为 success **且** `mergeable_state == clean` **且** PR base = `main`；否则只开 PR、**不合并**。
5. 结束后按 ADR-0039 / ADR-0041 同步 `PLAN.md` 当前状态块（4 行）、`README.md` 三处、`plans/stage-1-pilots.md` 头部进度句与本卡完成标记、`LEDGER.md` 一行。
<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-019 automation-host 进程 + ipc（JSON-RPC/NamedPipe + token + 对端身份校验 + 心跳 + 看门狗）
【目标】把 IPC 帧、握手认证、NamedPipe 传输、心跳/断连检测落成可运行、可测试的 host 与库。
【write scope】仅：crates/ipc/**、apps/automation-host/**、docs/DEPENDENCIES.md、本卡记录区、LEDGER.md、PLAN.md 当前状态块、README.md 三处、plans/stage-1-pilots.md 进度句与完成标记、必要的 docs/memory/{facts,pitfalls,rejected}.md。
【铁律】1 无静默失败；2 五类不可信输入先校验；7 core 不得调用平台 API；8 element/句柄不得跨进程；9 超范围先停下并裁决；10 契约先行。
【禁止】不改 crates/protocol/**、crates/platform/**、crates/core/**、crates/policy/**、crates/tool-bus/**；不做工具语义、策略判定、真实应用自动化、加密/压缩/分片、Linux/macOS 传输实现；不加 #[allow] 放宽 lint。
【验收】cargo fmt --all --check；cargo clippy --all-targets -- -D warnings；cargo test --workspace；cargo test -p assistant-ipc；xtask hygiene / memory-counts / adr-index / docscan / card-check / check-ledger；真机 kill-host 断连证据写入 §3；非 Windows clippy 退出码 0。
【依赖】016 已 Done（origin/main 已含），LEDGER 末 10 行已核对。
【疑问】Q1 采用 GetNamedPipeClientProcessId + QueryFullProcessImageNameW 白名单，token 与默认 DACL + PIPE_REJECT_REMOTE_CLIENTS 同时生效；Q2 复用已批准的 windows/serde/serde_json，不引入 tokio/uuid/日志库，新使用方追加依赖登记；Q3 不复制 envelope，仅用 crates/ipc 的传输帧与 protocol::ToolEnvelope/Capability；Q4 本卡不产生重解析率，引用 TASK-017 证据；以上为夜间授权下的默认处理，若形成偏差将全文写入 §5。
```

### 2. 实际改动文件

- `crates/ipc/**`（新建）：`Cargo.toml`、`README.md`、`src/{lib,error,frame,handshake,heartbeat,transport,windows_io,windows_peer,windows_transport}.rs`、`tests/handle_discipline.rs`。
- `apps/automation-host/**`（新建）：`Cargo.toml`、`README.md`、`src/{lib,main}.rs`、`tests/acceptance.rs`。
- `docs/DEPENDENCIES.md`：登记 `serde` / `serde_json` / `windows` 的新使用方与本卡 feature 组合。
- `Cargo.lock`：新增两个 workspace package 的锁定项（自动生成）。
- `tasks/TASK-019-automation-host-ipc-named-pipe.md`：执行记录；状态行按夜间任务选择协议改为 `Done`（见 DRIFT-019-3）。
- 进度同步：`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`MEMORY.md`、`docs/memory/{facts,pitfalls}.md`。

### 3. 验收输出摘要

```text
cargo fmt --all --check
→ PASS（0 diff）

cargo clippy --all-targets -- -D warnings
→ PASS（exit 0；含 assistant-ipc / assistant-automation-host）

cargo test --workspace
→ PASS：39 个 test target / 635 passed / 0 failed / 2 ignored（TASK-018 基线 = 32 / 607）

cargo test -p assistant-ipc
→ PASS：lib 21 + handle_discipline 2 + doctest 1

cargo test -p assistant-automation-host
→ PASS：lib 3 + main 0 + acceptance 1 + doctest 0

cargo clippy --target x86_64-unknown-linux-gnu -p assistant-ipc --all-targets -- -D warnings
→ PASS（exit 0）

cargo clippy --target aarch64-apple-darwin -p assistant-ipc --all-targets -- -D warnings
→ PASS（exit 0）

真机验收
→ `apps/automation-host/tests/acceptance.rs` 启动真实 host 子进程，经 NamedPipe 完成握手与心跳；kill host 后客户端在 2 s 内得到 Disconnected/Timeout。测试 1 passed。

xtask hygiene / memory-counts / adr-index / docscan / card-check / check-ledger
→ 全 PASSED；hygiene = 134 文件 / 0e / 3w（3 个既有 xtask 超长 warning），docscan = 168 / 0e / 545w，card-check = 91 / 0e / 49w。

xtask verify-schemas / codegen --check
→ PASSED（5 schema；0 drift）

cargo deny check
→ advisories / bans / licenses / sources 全 ok。

xtask refscan
→ FAILED = 151 error，与既有 baseline 逐项一致；本卡新增 0（PL-058）。
```

### 4. DoD 逐条核对

- [x] 上列命令全部通过；`refscan` 保持 151 项既有 baseline，本卡新增 0。
- [x] 帧编解码与握手覆盖 magic / CRC32 / 超长 / 版本不匹配 / 缺 token 五类负向用例。
- [x] 真机启动 host、完成 `ServerHello` 与双向心跳；kill host 后客户端在 2 s 内检测断连。
- [x] 非 Windows 的 `assistant-ipc` clippy 在 Linux 与 macOS 目标上均为 exit 0。
- [x] `crates/ipc/README.md` 含职责 / 边界 / 不变量 / 已知限制四节。
- [x] `docs/DEPENDENCIES.md` 已登记 `serde` / `serde_json` / `windows` 的新使用方；本卡没有新增第三方 crate。
- [x] `LEDGER.md` 追加；`docs/memory/facts.md` +2、`docs/memory/pitfalls.md` +1。
- [x] 未修改任何业务 Out of scope 文件；`Cargo.lock` 与卡状态行两项必要例外按 DRIFT-019-3 / -4 自裁决接受。

### 5. 偏差

**DRIFT-019-1（已批准依赖的新使用方 / 新 feature 组合）**

- 现象：`crates/ipc` 使用已批准的 `windows = 0.62.2`，并新增 `Win32_System_Pipes` / `Win32_System_IO` / `Win32_Security_Cryptography` feature 组合；同时直接使用已批准的 `serde` / `serde_json`。
- 影响：没有新增第三方 crate 或版本；但新增了跨 crate 使用方与 feature 面，按触发器 ① 的自律要求显式登记。
- 建议：接受。std 没有 Windows NamedPipe、对端 PID 查询或 OS CSPRNG 等价物；复用同一已批准官方投影比手写 FFI 更可审计。
- 已停工作：无，依赖登记已与代码同批落盘。

**DRIFT-019-2（Windows FFI 模块的 `unsafe_code` 放行）**

- 现象：`windows_io` / `windows_peer` / `windows_transport` 三个 `#[cfg(windows)]` 模块各需要一次 `#[allow(unsafe_code)]`，卡面原先要求“任何 `#[allow]` 都算漂移”。
- 影响：放行只覆盖三个条件编译模块；每个 `unsafe` 调用均带 `// SAFETY:`，非 Windows 构建完全不编译这些模块。
- 建议：接受，沿用 TASK-017 的已有先例。FFI 无法在 workspace 的 `unsafe_code = deny` 下零放行。
- 已停工作：无，`-D warnings` 与两条非宿主 clippy 均通过。

**DRIFT-019-3（卡片正文区状态行）**

- 现象：自动化协议用卡片 `- 状态：` 行选择下一张卡；该行位于正文区，而 PL-073 尚未裁决其写入归属。
- 影响：本次只把 `Ready` 改为 `Done`，不改正文其他任何行。
- 建议：接受本次必要写入；PL-073 仍应在治理批中给该行建立正式例外。
- 已停工作：无。

**DRIFT-019-4（`Cargo.lock` 自动更新）**

- 现象：新增两个 workspace package 后，`Cargo.lock` 自动增加对应 package 项；根 `Cargo.lock` 未在卡面 write scope 显式列出。
- 影响：没有新增外部依赖；锁文件变化是新 crate 入 workspace 的必要结果。
- 建议：接受；后续卡面可把根 `Cargo.lock` 纳入新增 crate 的标准 write scope。
- 已停工作：无。

### 6. 更合理做法

- 把 Windows overlapped I/O 从传输实现中拆成 `windows_io`，把进程身份 / CSPRNG 拆成 `windows_peer`，使每个文件低于卡面 600 行硬上限；行为不变，`hygiene` 回到 3 个既有 warning。
- 认证 token 作为 `AuthenticatedClientHello` 的传输层伴随字段携带，而不向 spec 定义的 `ClientHello` 偷加字段；保持协议契约与实现边界分离。

### 7. 遗留问题

- NamedPipe 当前一次只服务一个实例/连接；多连接 accept loop 与进程生命周期管理归后续 Core/Host 装配。
- 生产 token 传递仍由启动环境变量承接；更窄的 Core 继承凭据通道应在装配阶段替换。两项均作为已知限制写入 `crates/ipc/README.md` / `apps/automation-host/README.md`。
- Q4 的 “Spike B 重解析矩阵 ≥95%” 不在本卡产生；依据为 TASK-017 的元素定位执行记录与 PL-068 后续裁决。

### 8. 新增长期记忆

- FACT：TASK-019 后 `cargo test --workspace` 基线更新为 39 target / 635 passed / 2 ignored；`assistant-ipc` = 21 lib + 2 arch + 1 doctest，host = 3 lib + 1 real acceptance。
- FACT：Windows NamedPipe 对端身份路径 = `GetNamedPipeClientProcessId` + `QueryFullProcessImageNameW`；管道额外设置 `PIPE_REJECT_REMOTE_CLIENTS` 并使用进程默认 DACL。
- PITFALL：双工 NamedPipe 上不能靠 `FlushFileBuffers` 作为发送完成同步；双方同时写会互相等待，移除 flush 后真机握手/心跳稳定。

### 9. 给审阅者的关注点

- 最高风险在 `crates/ipc/src/windows_io.rs`：手动重置事件与 `CancelIoEx` 的取消/回收路径决定超时是否安全；已用真机 kill-host 验收覆盖主路径。
- 次高风险在 host 的身份门：`PeerIdentityUnavailable` / `PeerIdentityRejected` / token 失败都 fail-closed，默认白名单为空时拒绝启动。
- 默认 DACL 只隔离远端客户端，不等价于“只允许当前用户”；生产化前应明确同用户进程威胁是否由 peer allow-list 足够覆盖。

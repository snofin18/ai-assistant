# ADR-0057　UI↔Core 传输契约：独立进程 + UI 专属 wire 类型

状态：**Accepted**（2026-09-30 人类确认接受；TASK-213 已解锁，可开工）
日期：2026-09-30
Supersedes：—
Superseded by：—
关联：**PL-095**、TASK-104、TASK-105、TASK-213、TASK-039 集成审计、架构 v2 §12.7 / §14、
ADR-0056、ADR-0053、ADR-0028、ADR-0019

---

## 背景

TASK-104 已经把 UI↔Core 的**契约形状**落进代码：`apps/agent-core/src/ui_ipc.rs` 定义
`UiCommand` / `UiCommandOutcome` / `UiEvent` 与版本号 `1.0`，两侧各有 fail-closed 校验
（Rust 显式 allow-list + TypeScript zod `strictObject`），并共享 7 个黄金样本
（`apps/agent-core/tests/fixtures/ui_ipc/*.json`）；`apps/desktop-ui/src-tauri` 的
`send_ui_command` 也已有本地校验与 `CoreCommandTransport` 边界。

但**真实传输不存在**：没有 Core 侧的 UI 命令监听端，也没有 Core → UI 的事件推送通道。
未注入传输时 `send_ui_command` 显式返回 `core_transport_unavailable`（fail-closed，
不伪造成功）—— 这是有意的，但也意味着 TASK-105 的 T1.x 真实运行**无法从 UI 触发**，
阶段 1a 因此仍是 NO-GO。

架构 v2 §12.7 明文要求「Core 与 UI **必须分进程**」，因此"把 Core 链接进 Tauri 进程"
不是可选项。而 `crates/ipc` 现有的 `RequestMessage` / `ResponseMessage` 是**工具形状**
（载荷是 `tool_invoke: ToolEnvelope`）—— 拿它承载 UI 命令会把工具语义与 UI 语义混在一起。
所以这是一次**新增公共 wire 契约**的决策（漂移触发器 ③），必须先有 ADR。

## 决策（一句话）

**UI 与 Core 保持独立进程；两者之间新增一层 UI 专属的 IPC 契约（不复用工具形状的
`RequestMessage` / `ResponseMessage`），传输复用 `crates/ipc` 的 NamedPipe 帧实现与
「一次性 token + 对端进程镜像白名单」鉴权模式；命令带 correlation，事件为服务端单向推送。**

## 决策细化

| # | 内容 |
|---|---|
| **D1 进程边界** | Core 与 UI 分进程（架构 v2 §12.7）。UI 进程**不链接** `assistant-agent-core` / `assistant-policy` / `assistant-task-engine`；它只持有 client 与其连接状态。 |
| **D2 UI 专属 wire 类型** | 新增 `UiIpcRequest` / `UiIpcResponse` / `UiIpcEvent` 三个信封，载荷分别是既有的 `UiCommandEnvelope` / `UiCommandOutcome` / `UiEvent`。**不复用** `crates/ipc` 的工具信封 —— 工具语义与 UI 语义必须分开演进。 |
| **D3 传输** | Windows = NamedPipe，复用 `crates/ipc` 的帧编解码（magic / 16 MiB 上限 / CRC32）；Linux/macOS = Unix Domain Socket，归后续平台卡。不加加密层（本机同用户边界内；与 ADR-0019 的既有判据一致）。 |
| **D4 鉴权** | 沿用 `apps/automation-host` 已验证的模式：一次性 token（环境变量注入 + 常数时间比较）+ 对端进程镜像白名单 + 握手版本协商。token 由 Core 启动时生成、经受保护渠道交给 UI（架构 v2 §14.2）。 |
| **D5 消息流** | 命令：UI → Core，带 `correlation_id`，Core **必须**回同 id 的响应（TASK-103 已在 Host 侧建立同型约束）；事件：Core → UI，单向、无 correlation，只承载 `UiEvent`。 |
| **D6 六个 fail-closed 点** | 版本漂移 / 未知字段 / 未知 kind / 未知 correlation / 缺响应 / 事件字段缺失 —— 一律拒绝并带 `ErrorCode`，不得静默丢弃、不得用默认值兜底。 |
| **D7 归属** | Core 侧监听端 = `apps/agent-core` 的 **binary 装配层**（不进入 `crates/core`，与 ADR-0053 D1/D5 一致）；UI 侧 client = `apps/desktop-ui/src-tauri` 的 `CoreCommandTransport` 实现。 |
| **D8 实现顺序** | 先落地「命令 + 响应」（TASK-105 的硬前置是"UI 能触发任务"），事件推送紧随其后。两步在同一张卡内，但**命令路径必须先绿**。 |

## 被否决的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 把 Core 链接进 Tauri 进程，用 Tauri command 直接调 | ❌ | 违反架构 v2 §12.7「必须分进程」：webview 一旦被 XSS，等价于系统被控 |
| 复用 `RequestMessage` / `ResponseMessage` 传 UI 命令 | ❌ | 它们是工具形状；复用会把 UI 命令塞进工具校验器与信封语义，且未来 UI 契约演进会牵动工具协议 |
| UI 直接读写 SQLite / 直接调 platform provider | ❌ | 违反铁律 3 与 §12.7「能力全部走 Core」；UI 是零系统权限进程 |
| 用 HTTP / WebSocket 做本机传输 | ❌ | 引入监听端口与浏览器攻击面；本机同用户边界内 NamedPipe/UDS 已足够，且 `crates/ipc` 已有实现与真实子进程测试 |
| 先做 Core → UI 事件推送，再补命令路径 | ❌ | TASK-105 的阻塞点是"UI 能触发任务"；事件推送不能解锁它 |

## 影响

- 新增配套 contract：`docs/spec/ui-ipc-protocol.md`。
- 新增卡 **TASK-213**（UI↔Core 传输实现）——本 ADR 已于 **2026-09-30 转 Accepted**，该卡已解锁。
- **TASK-105 在本 ADR + TASK-213 完成前不可开工**（阶段 1a 的 NO-GO 不会因为本 ADR 而改变）。
- `crates/ipc` 的公共类型**不需要**改动（UI 信封是新类型）。若实现中发现必须改它的公共接口 → 回本 ADR 补充，不得顺手改。
- `apps/desktop-ui/src-tauri` 会增加对 `crates/ipc` 的 workspace path 依赖（非第三方依赖，无需登记）。

## 验证方式

1. **负向**：错 token / 非白名单对端镜像 / 版本不匹配 / 未知字段 / 错 correlation —— 全部拒绝且带 `ErrorCode`（N1 单测 + N2 CI 显式失败步骤，ADR-0019）。
2. **正向**：真实管道上「UI 发一条 `submit_intent` → Core 返回 `intent_accepted`」的端到端验收，沿用 TASK-019 的真实子进程写法。
3. **事件**：Core 推一条 `step_state_changed`，UI 侧经 zod 校验后落到时间线模型（TASK-104 的 `timelineEvents.ts` 已在纯逻辑层验证过投影）。
4. **断连**：kill Core 后 UI 在 2s 内检测到断连（沿用 TASK-019 的判据）。

## 重新评估触发条件

- 若需要把 Core 与 UI 合并为单进程 → 必须回架构 v2 §12.7 重新裁决，本 ADR 随之作废；
- 若 UI 契约需要承载二进制大对象（截图、树快照）→ 需要单独的传输方案（共享内存 / 临时文件），不在本 ADR 范围；
- 若 Linux/macOS 的 UDS 实现需要与本契约不同的握手 → 抽出平台差异，**不复制契约**。

## 相关 ADR

- **ADR-0056**：运行执行链路契约（`UiEvent` 的来源与投影语义）。
- **ADR-0053**：core 依赖白名单与 binary 装配点（D7 的依据）。
- **ADR-0028**：写公共热点文件前先取锁（TASK-213 必须遵守）。
- **ADR-0019**：新硬门禁必须配负向验证（验证方式 1 的依据）。

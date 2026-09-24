# assistant-ipc

TASK-019 的 Host IPC 传输库：帧编解码、握手、心跳与 NamedPipe 传输。

## 职责

- 落实 `docs/spec/ipc-protocol.md` 的帧布局：`magic + envelope_size + payload + CRC32`。
- 定义并校验 `ClientHello` / `ServerHello` / `Heartbeat`，拒绝版本不匹配与缺失 token。
- 在 Windows 上提供 NamedPipe 客户端/服务端传输与对端进程镜像路径查询。
- 提供可注入时间的 `HeartbeatMonitor`，让上层把“无活动”判成明确错误而不是挂死。

## 边界（不做什么）

- 不做工具语义、策略判定、目标元素定位或真实应用自动化。
- 不定义第二套 `Header` / `Payload` envelope；工具载荷复用 `assistant_protocol::ToolEnvelope`。
- 不实现 Linux / macOS 传输细节；非 Windows 明确返回 `CapabilityMissing`。
- 不做加密、压缩、分片或跨机通信。

## 不变量

1. `magic`、CRC32 与 `envelope_size <= 16 MiB` 全部强制执行；错误分类可区分。
2. 协议版本只在握手阶段校验，失败永不进入正常消息流。
3. token 不进入日志；比较使用常数时间算法，服务端缺失或错误 token 一律拒绝。
4. NamedPipe 拒绝远端客户端；服务端另以客户端 PID 的镜像路径白名单失败关闭。
5. 心跳与超时是错误路径，不会用默认活动时间冒充存活。

## 已知限制

- 当前 token 由宿主通过环境变量提供；生产装配仍需由 Core 以更窄的凭据传递通道接管。
- Windows 管道使用进程默认 DACL，并额外设置 `PIPE_REJECT_REMOTE_CLIENTS`。
- 详细错误证据与 kill-host 验收记录见 `tasks/TASK-019-automation-host-ipc-named-pipe.md`。

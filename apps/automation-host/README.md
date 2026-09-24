# assistant-automation-host

TASK-019 的自动化宿主进程。

## 职责

- 创建并监听 Windows NamedPipe。
- 在接受连接后校验客户端进程镜像白名单与握手 token。
- 完成 `ServerHello`，双向发送心跳，并在静默超时后显式失败。

## 边界（不做什么）

- 不做工具注册、MCP、策略判定或元素定位。
- 不直接操作真实目标应用。
- 不提供 shell / 代码执行入口。

## 不变量

1. 未配置 peer 白名单或 token 时启动失败，默认拒绝。
2. 对端 PID、镜像路径或 token 任一无法验证时拒绝连接。
3. 心跳静默超时返回带 `ErrorCategory` 的错误，不继续假装会话存活。

## 已知限制

token 目前通过启动环境变量传入；后续 Core 装配应改为更窄的继承凭据通道。

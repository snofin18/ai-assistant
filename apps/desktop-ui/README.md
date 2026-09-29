# assistant-desktop-ui

TASK-029 的最小桌面壳：Tauri 2 + React + TypeScript + Tailwind。

## 职责

- 提供 Tauri 2 webview 生命周期和 React 渲染入口。
- 通过 `src/ipc/client.ts` 作为类型化 IPC 的唯一前端入口：`sendUiCommand` 在发送前
  校验命令、在渲染前校验 Core 的返回。
- `src/ipc/contract.ts` 用 zod 定义 UI ↔ Core 的 1.0 契约；与 `apps/agent-core` 共享
  `apps/agent-core/tests/fixtures/ui_ipc/*.json` 黄金样本，两侧测试各解析一遍。
- `src/features/intent/IntentLauncher.tsx` 是第一个真实的 UI → Core 调用点。
- 以严格 CSP 和空 capabilities 保持 webview 零系统权限。

## 边界

- 不启用 shell / fs / http / process / dialog 插件。
- 不直接访问平台、文件系统、网络或 Core 内部 Rust 类型。
- 不做授权判断：审批卡只把用户决定翻译成 Core 会校验的命令。
- `src-tauri` 不链接 Core；命令经注入的 `CoreCommandTransport` 转发。

## 不变量

1. `capabilities/default.json` 的 `permissions` 必须为空。
2. CSP 必须包含 `default-src 'self'` 与 `script-src 'self'`，禁止 `unsafe-eval`。
3. UI 只通过 `src/ipc/client.ts` 调用 Tauri command。
4. `src-tauri` 是独立 Cargo workspace，避免根 Host workspace 依赖 Linux WebKit 系统包。
5. 未知字段、未知 kind、版本漂移一律拒绝；解析失败的命令不得发出，也不得渲染。
6. 未知 Core 状态不得映射为成功类时间线状态（`timelineEvents.ts` 直接丢弃）。

## 已知限制

- Core 进程的 IPC 传输尚未接通：`src-tauri` 的 `send_ui_command` 会本地校验信封，
  再经 `CoreCommandTransport` 转发；未注入传输时显式返回 `core_transport_unavailable`，
  不伪造成功。
- 事件流（Core → UI）的推送通道同样待接；当前只提供经校验的事件解析与时间线投影。
- 图标为骨架用的最小 ICO；品牌资源归后续发布卡。

# assistant-desktop-ui

TASK-029 的最小桌面壳：Tauri 2 + React + TypeScript + Tailwind。

## 职责

- 提供 Tauri 2 webview 生命周期和 React 渲染入口。
- 通过 `src/ipc/client.ts` 作为未来类型化 IPC 的唯一前端入口。
- 以严格 CSP 和空 capabilities 保持 webview 零系统权限。

## 边界

- 不启用 shell / fs / http / process / dialog 插件。
- 不直接访问平台、文件系统、网络或 Core 内部 Rust 类型。
- 不承载审批、时间线、拾取器或策略面板业务；这些归 TASK-030~032。

## 不变量

1. `capabilities/default.json` 的 `permissions` 必须为空。
2. CSP 必须包含 `default-src 'self'` 与 `script-src 'self'`，禁止 `unsafe-eval`。
3. UI 只通过 `src/ipc/client.ts` 调用 Tauri command。
4. `src-tauri` 是独立 Cargo workspace，避免根 Host workspace 依赖 Linux WebKit 系统包。

## 已知限制

- 本卡只提供壳与安全配置，不提供真实业务 command。
- 图标为骨架用的最小 ICO；品牌资源归后续发布卡。

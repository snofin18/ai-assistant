# assistant-agent-core

TASK-029 的 binary-layer Host 装配库。

## 职责

- 打开唯一写连接并合并 storage / audit 迁移。
- 显式注入 session、memory、model、policy、tool-bus、audit 与 platform 组件。
- 保持 `crates/core` 只作为可装配组件库，不承担装配。
- 暴露 Host 装配正向测试与缺组件负向测试。
- 提供 `RuntimeExecutor`，按 ADR-0056 单步驱动 Policy → Tool → Verify → Commit，
  并附带真实 `ToolBusInvoker` 与 `EnvelopeObservationCollector` 边界适配器。
- 提供 ADR-0058 的 `--production` 装配根：加载 Notepad Adapter 声明、注册 5 个
  Host handler、注入确定性 TaskPackage Provider，并把 RuntimeExecutor 快照接到
  `SnapshotEventSource`。`--serve-ui` 会在任务结束后进入 UI 监听循环。
- 提供 ADR-0084 / ADR-0085 的 Paint T3.1 装配分支：按显式 `AdapterKind::Paint`
  加载 Paint target 目录，注册 7 个 Paint handler，并用 `element_bounds` 读取 canvas
  物理矩形；Notepad 路径保持默认行为不变。
- 提供 UI 命令/事件适配层（`ui_ipc` + `ui_control`）：版本化命令信封、未知字段/
  未知 kind 的 fail-closed 解析、以及把 approve/deny/pause/cancel/takeover 应用到
  真实 task-engine 的处理器。
- 提供私有 `--browser-task-run`：用专用 profile launcher、真实 CDP provider 与三个
  browser tools 执行 task-package，把全部已提交步骤输出写到新的、读回校验的 JSON；
  对每个会被 Policy 要求确认的写步骤，必须重复传入 `--approve-step <step-id>`，单次批准
  只消费一次且不会扩展成任务级或持久授权；
  显式传入 `--csv-output` + `--csv-base-url` 时，只在这六步全部提交后把 T5.1 三页 DOM
  规范化为固定 60 行 CSV。该模式仍不替代真实 Edge 验收。
- TASK-395 提供 test-only 的 Excel T2.1 纯逻辑骨架：fake runtime、ToolBus-isolated
  session registry、`.xlsx`/range/read-mode parser、workbook/range/cell bounds 与四个
  handler 路径。它不会进入生产 assembly，也不代表 COM runner 或 Tool 已可用。
- TASK-401 / ADR-0111 提供 app 层可引用的窄 Windows `ExcelComRunner` 边界与真实 COM
  单工作簿 smoke；`ExcelT21Runtime` 域映射、ToolBus 注册和 `tools.json` 仍保持未接线。
- TASK-403 提供 test-only 私有 `PlatformExcelT21Runtime`：只把平台自包含 handle/snapshot
  映射到现有 Excel T2.1 领域类型，并以 fake port 验证 32 次 open/read/close 的 domain
  handle、单 workbook mapping 与单一 application 收敛。真实 COM ignored 入口仅保留给
  人类在场后续运行；生产装配与 ToolBus 仍未接线。
- TASK-404 提供 test-only 私有 `ExcelL1AnchorStore`：只从 valid `OpenedWorkbook`
  派生 ToolBus session / task / opaque workbook handle / owned process / workbook /
  worksheet identity，再把有界 `value2` 值快照、绝对 cell 坐标、调用方提供的原始字节与
  既有 `assistant-undo` `L1Snapshot + ShadowCopy` anchor 绑定。写前/写后/回滚前同时校验
  identity 与 fingerprint，并对 anchor 数、影子字节总量和 payload 使用硬上限拒绝策略。
  它不创建真实影子文件，也不执行写或回滚。
- TASK-405 提供 test-only 私有 scalar write 输入门：只接受有界 JSON
  `null|boolean|number|string`，number 经现有 `CanonicalDecimal` 规范化，公式形态字符串在
  写前拒绝；合法 scalar 再复用 TASK-404 anchor 的 single-use pre-fingerprint 检查。
  它不执行真实 range write、重算、`.Value2/.Formula` 双读、保存或恢复。
- TASK-406 提供 test-only 私有 scalar range write port：把 anchor 捕获的 canonical
  address、worksheet、维度与 cell 数绑定到写前 selector，按
  write → recalculate → `.Value2` → `.Formula` → post-fingerprint 顺序执行一次不可重试的
  fake 事务；任一端口或双读校验失败都不记录 committed 结果。真实 COM write/save、
  磁盘影子副本与 production ToolBus 仍未接线。
- TASK-413 提供 test-only 私有 Excel task-end lifecycle：首次 shutdown 以原子
  `running -> closing` 立即拒绝新 open/read/summarize，全部 token 先移入 closing；
  workbook close 成功才释放对应容量，runner 整体收敛成功后才清除剩余 closing 证据。
  platform adapter 在阻塞 runner shutdown 前发布 closing 并释放 runtime-state mutex，
  cleanup 失败时保留 workbook mapping；production ToolBus/assembly 仍未接线。
- TASK-414 提供 test-only Excel L1 文件影子副本端口：只接受仓库外绝对本地根与
  `.xlsx` 工作簿路径，以 create-new 发布有界影子文件并校验 SHA-256；恢复前要求目标文件
  digest 等于记录的 post-write 状态，恢复后复核 anchor digest。anchor 组合入口在物理
  影子发布失败时回收内存容量，并复用既有 pre/post fingerprint 冲突门。该层只操作测试
  临时文件，不启动 Excel/COM，也不代表 production T2.2 保存回滚已开放。
- TASK-415 提供 test-only Excel L1 scalar file-transaction coordinator：scalar 先校验，
  再组合 anchor、write-port 与文件影子捕获，成功返回保留 post-fingerprint 和原始文件
  digest 的待回滚 receipt。回滚同时要求 workbook post-fingerprint 与调用方确认的保存后
  文件 digest 匹配；失败保留 anchor/shadow 证据，cleanup 后才收敛。该层不保存真实工作簿。
- TASK-418 在 TASK-415 之上增加 test-only task transaction owner：同一 owner 最多持有一笔
  active transaction，closing/terminal 后拒绝新事务；committed close 释放 anchor 并清理
  owned shadow，rollback close 先用 saved-file digest 恢复后清理。它仍不启动 Excel/COM，
  不代表 production save caller 已接线。

## 边界

- 不实现编排算法、策略判定、工具执行或平台 FFI。
- 不提供第二装配点，不在 `core` 内创建基础设施实现。
- 不默认补 Provider、SessionStore、MemoryRetriever 或 AppMapReader。
- 不把 UI 业务逻辑放进本 crate。

## 不变量

1. 每个必需组件都必须由调用方显式提供；缺失时返回 `host_component_missing`。
2. 数据库句柄只由本装配层持有，并通过互斥锁共享给短生命周期 adapter。
3. 装配失败必须返回带 `ErrorCode` 的结构化错误，不返回半装配 Host。
4. 工具通道必须在 tokio 运行时内启动，并在 `shutdown` 时显式关闭。
5. 成功提交必须消费 verify 的不透明 receipt；Policy deny、审批缺失、
   verification failure 与未知工具结果都不能当成成功。
6. `RuntimeExecutor::advance` 通过 `ToolInvoker` 异步调用工具；工具返回
   `ok = false` 时按信封里的 `ErrorCode` 失败，不得进入 verify。
7. `EnvelopeObservationCollector` 要求信封 `data` 带 `fingerprint`；缺失即显式
   observation 失败并升级人工，不伪造默认指纹。
8. UI 命令在解析通过前不得触达处理器；拒绝必须带稳定 `ErrorCode`
   （非法输入 → `ToolInvalidArgs`，未知任务 → `TargetNotFound`，未知审批 →
   `UserInteraction`）。
9. UI 事件只从 task snapshot 投影；committed 步骤携带真实 post fingerprint，
   未知状态不映射为成功。
10. browser task-package runner 只在所有步骤 `Committed` 后创建输出文件；任一工具、
    policy、verify 或 DOM/CSV 契约失败都不得留下成功文件。
11. CSV 只接受三页 DOM、每页 20 行和固定八列契约；UTF-8 无 BOM、CRLF、
    RFC 4180 quoting，并拒绝跨 origin、重复 URL、空字段和未知 HTML entity。
12. 私有 browser runner 的 step 批准只能绑定一个真实写步骤，并只把 Policy 的
   `AllowWithConfirmation` 转成该步骤的一次放行；未知、只读、无需确认、重复或超量批准
   全部在启动执行前失败。
13. Excel L1 anchor 只能在同一 opened workbook identity 与捕获的 pre-fingerprint 上
    准备一次写入；post-fingerprint 只能记录一次，identity/fingerprint 漂移或证据缺失时
    回滚必须 blocked。
14. Excel scalar write 只接受单一 JSON scalar；array/object、不可规范化 number、
    公式形态字符串或超过既有 range payload 上限的输入都必须在 anchor 状态变化前失败。

## 已知限制

- 生产 `SessionStore` 仍等待 storage conversation/session 记录 API（PL-092）；
  当前装配接受注入的 store，测试使用 `MemorySessionStore`。
- 1a 的 Plan 来源是确定性 TaskPackage Provider；真实 LLM Provider 仍归 1c 前。
- `notepad.tab.new` 在平台 API 暴露稳定 tab count 之前显式 `CapabilityMissing`，
  不猜测成功；对应靶机/Adapter 缺口由 PL-097 跟踪。
- `notepad.file.save_as` 已接 Adapter selector 和文件 before/after observation，
  但 `notepad-like` 靶机没有跨进程对话框，真机路径仍待后续靶机扩展。
- Host 以 `WindowProvider + UiAutomationProvider` 约束的泛型平台注入具体实现；
  `WindowsPlatform` 当前未实现 `PlatformService` trait，因此不把未实现的 trait
  当作已满足的能力。装配入口在非 Windows 目标上显式失败，避免 self-check 假阳性。
- TASK-330 已能为 T5.1 三页 DOM 生成确定性的 60 行 CSV；真实 Edge 10/10、
  专用 profile 与外部 smoke 仍由 TASK-056 的人类在场会话负责。
- Excel T2.1 的四个 handler 与 platform runtime adapter 仍停留在 `#[cfg(test)]` 私有实现；
  `tools.json` 保持空。TASK-403 已用 fake port 验证私有域映射和重复 handle 收敛，
  TASK-413 已补私有 task-end closing/失败保留状态机；真实 task assembly、Host 崩溃恢复与
  production ToolBus registration 仍需后续卡。
- TASK-414 的真实文件恢复只覆盖 test-owned 临时工作簿字节；production task 的临时根选择、
  atomic replacement、异常断电恢复、真实保存后 `.Value2/.Formula` 复读与 cleanup caller
  仍未接线。
- TASK-415 的待回滚 receipt 仍是私有测试编排，调用方必须显式提供保存后文件 digest；
  production in-place save、真实 COM 写入后的文件落盘与 task-end caller 尚未接线。
- TASK-418 的 owner 仍是 test-only 资源生命周期编排；真实 production save caller、
  真实 COM 落盘、原子替换、断电恢复与 Host 崩溃 helper process 尚未实现。
- TASK-404 的影子副本是内存模型，仅证明 anchor/digest/conflict 契约；真实文件复制、
  磁盘保留、恢复写入与写入后的 `.Value2/.Formula` 校验仍需人类在场的 T2.2 实现卡。
- TASK-405 只冻结 scalar 输入门与写前 anchor 状态；真实 range write、重算、
  `.Value2/.Formula` 双读、保存和恢复仍未实现，production Excel surface 继续为空。
- TASK-406 只证明私有 write-port 状态机与 fake 双读契约；真实 COM write/save、文件
  shadow copy、写后实际语义校准与 production Excel surface 仍未实现。

## 相关文档

- `docs/spec/core-orchestration.md`
- `docs/spec/runtime-execution.md`
- `docs/adr/0056-runtime-execution-contract.md`
- `docs/adr/0053-core-orchestration-layer-interface.md`
- `tasks/TASK-029-binary-skeleton-agent-core-desktop-ui.md`
- `tasks/TASK-103-runtime-executor-host-dispatch-verifyoutcome.md`
- `tasks/TASK-104-ui-core-ipc-approval-wiring.md`

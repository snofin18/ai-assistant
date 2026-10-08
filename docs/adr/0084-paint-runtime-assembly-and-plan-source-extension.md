# ADR-0084　生产装配根扩展到 Paint：handler 与 Plan 来源的适配器参数化

状态：**Accepted**（2026-10-08 按用户「接着按你建议的跑」授权；TASK-106 可开工）
日期：2026-10-08
Supersedes：—
Superseded by：—
关联：**TASK-106**（Paint 运行时装配 T3.1 垂直切片）、TASK-043、TASK-044、TASK-047、
ADR-0058、ADR-0043、ADR-0055、ADR-0056、ADR-0067、ADR-0076、ADR-0077、ADR-0079、
`docs/spec/runtime-execution.md`、`adapters/com.microsoft.paint/**`、`eval/tasks/paint/**`

---

## 背景

ADR-0058 建立了生产装配根（binary 层）与阶段 **1a** 的 Plan 来源契约，但其决策文本把
Plan 来源钉在 `adapters/com.microsoft.notepad/tasks/t1.*.json`，把 Host handler 钉在
「Notepad 的 5 个 handler」。实现也照着这条钉死：`apps/agent-core/src/production.rs`
直接持有 `NotepadTargetCatalog` / `build_notepad_registry` / `NotepadHandlerContext`。

阶段 1b 的 Paint 已经落地声明式 Adapter pack（TASK-043）与 T3.1 task/eval pack（TASK-044），
但**没有任何运行 handler**：`paint.canvas.resolve_point` / `paint.canvas.draw_rectangle`
等 10 个工具只有 schema，没有 binary 层实现。因此 `DRIFT-044-1` / `PL-113` 要求的
「真机十次运行成功率 / 拖拽坐标误差 ≤ 2 px / 像素容差」**没有可运行对象** ——
这与 TASK-105 当初的处境同形（ADR-0058 背景）。

这是一次决定**分层归属 + 第二个适配器接入方式**的决策（漂移触发器 ②③④），
必须先有 ADR（铁律 10）。ADR-0058 的 D1/D4/D5/D6/D7/D8 结构仍然成立，本 ADR 只做
**参数化扩展**，不推翻其中任何一条。

## 决策（一句话）

**生产装配根保持 binary 层独占；把「适配器选择」显式参数化 —— target 目录、handler 集、
任务包路径都由装配输入给出（Notepad 与 Paint 各一套），`TaskPackageProvider` 与
`ToolRegistry` 本身不改公共形状；Paint handler 在 binary 层新增模块实现，仍走
platform trait、仍 fail-closed；真实 LLM 与真实 GUI 无人值守仍不在本 ADR 范围。**

## 决策细化

| # | 内容 |
|---|---|
| **D1 装配参数化** | `ProductionConfig` 增加显式的适配器身份（`app_id`）与对应的 target 目录 / handler 集选择；`production.rs` 不再硬编码 `NotepadTargetCatalog`。**唯一装配点仍在 binary**（ADR-0053 D2 / ADR-0058 D1），core 不新增装配。 |
| **D2 Plan 来源泛化** | `TaskPackageProvider` 已是「路径 → 确定性 `Plan`」（ADR-0058 D2）；本 ADR 只把 `task_package_path` 视为**任意适配器**的任务包入口，同一输入必得同一 `Plan`，`model_id` 语义不变。**不引入 LLM、不引入随机性**。 |
| **D3 Paint target 目录** | 新增 Paint 的 target 目录加载，输入 = `adapters/com.microsoft.paint/selectors/targets.json`；解析仍要求 scope（ADR-0043），候选链形状复用既有 `DeclaredTargetsFile`。 |
| **D4 Paint handler** | 在 binary 层新增 Paint handler 模块，实现 T3.1 垂直切片所需的工具子集（`paint.document.new` / `paint.tool.select` / `paint.color.select_foreground` / `paint.canvas.resolve_point` / `paint.canvas.draw_rectangle` / `paint.canvas.capture_pixels`），形状取自 `adapters/com.microsoft.paint/tools/tools.json`。未实现其余工具前，注册集必须与声明的子集**精确一致**，缺一即 fail-closed（ADR-0058 D8）。 |
| **D5 画布坐标与输入** | 画布坐标 → 屏幕点换算在 handler 内用 canvas bounds + zoom + viewport offset 完成；合成拖拽走既有 pointer 通道，起点与释放点各自携带显式 `CoordinateSpace`（ADR-0067），拖拽期间持有写租约。**禁止**从桌面根搜元素。 |
| **D6 像素证据** | 画布像素捕获复用 ADR-0076 的 GDI 单窗口截图 + 注入的 `ImageBlobSink`；`visual_assert` 走 ADR-0077 / ADR-0079 的接线。像素不进 JSON / IPC / `Observation`。 |
| **D7 真机验收另卡** | 真机十次运行与坐标误差测量是**独立的验收卡**（承接 `DRIFT-044-1` / `PL-113`）；本 ADR 与 TASK-106 **不授权**无人值守操作真实 GUI（章程 §3.7）。 |
| **D8 不改公共契约** | `ModelProvider` / `ToolHandler` / `ToolRegistry` / `UiServer` / `SnapshotEventSource` / `platform/api` trait **均不改**；若实现中发现必须改公共接口 / schema / ErrorCode → 回本 ADR 补充（触发器 ③）。不新增 crate、不新增第三方依赖。 |

## 被否决的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 为 Paint 再建一个独立生产装配根 / 第二个 binary | ❌ | 违反 ADR-0053 D2「装配单点在 binary」与 ADR-0058 D1；会出现两个可写装配点 |
| 新建 crate 放 Paint handler | ❌ | 漂移触发器 ②；handler 依赖 binary 层已注入的 `ToolBus` 与目标绑定，无独立生命周期理由（ADR-0058 同款否决） |
| 把 `paint.canvas.draw_rectangle` 做成通用 `run_shell` / 任意坐标注入工具 | ❌ | 违反 AGENTS §7（禁通用 shell）与铁律 3；等于放弃可控性 |
| 用声明式 pack 静态推算十次成功率 | ❌ | 等于伪造运行证据（铁律 1；ADR-0058 同款否决） |
| 本卡顺手把 Paint 全部 10 个工具一次接完 | ❌ | 工作量超预估 2 倍（触发器 ⑩）；先做 T3.1 垂直切片，其余工具按后续卡增量 |

## 影响

- 新增卡 **TASK-106**（Paint 运行时装配：T3.1 垂直切片）——状态 = Ready。
- **TASK-044 的真实十次运行验收在 TASK-106 落地前仍不可开工**；`DRIFT-044-1` / `PL-113` 保持开放。
- `production.rs` 从「Notepad 专用」变为「按适配器参数化」；既有 Notepad 路径行为不变（回归由现有 `production_root*` 测试保证）。
- 不新增第三方依赖、不新增 crate、不改 lint 政策、不改 `crates/**` 公共接口。

## 验证方式

1. **正向（fake 平台）**：T3.1 任务包经装配 → `Plan` 可渲染并在 fake `WindowProvider` + `UiAutomationProvider` 上执行到 `Completed`，`ToolBus` 调用数与任务包声明一致。
2. **负向（fail-closed）**：Paint 注册集缺任一 T3.1 工具 / target 目录缺必需目标 / 未注入 blob sink → 装配或执行显式失败并带 `ErrorCode`。
3. **回归**：既有 Notepad 装配与 `production_root*` 测试全绿（参数化不得改变 1a 行为）。
4. **确定性**：同一 Paint 任务包跑两次得到同一 `Plan`。
5. **静默失败 = 0**：任何失败路径都带 `ErrorCode` 与可读说明。

## 重新评估触发条件

- 若实现中发现必须改 `crates/**` 公共接口 / schema → 回本 ADR 补充决策；
- 若第二个适配器的 target 目录形状与 Notepad 差异过大、无法共享 `DeclaredTargetsFile` → 另立 ADR 决定是否抽象；
- 若 TASK-106 之后接入第三个适配器时仍需改 `production.rs` 的适配器分支 → 说明参数化不足，另立 ADR 抽 trait。

## 相关 ADR

- **ADR-0058**：生产装配根与 1a Plan 来源（本 ADR 扩展其范围，不推翻其决策）。
- **ADR-0053**：core 依赖白名单与 binary 装配单点（D1 的依据）。
- **ADR-0056**：运行执行链路契约。
- **ADR-0043**：元素解析必须有 scope（D3 的依据）。
- **ADR-0067**：pointer 动作显式 `CoordinateSpace`（D5 的依据）。
- **ADR-0076 / 0077 / 0079**：截图通道 / `visual_assert` 接线 / 图像来源（D6 的依据）。
- **ADR-0055**：`effect` / `reversibility` 由可信工具目录注入（D4 注册期的依据）。

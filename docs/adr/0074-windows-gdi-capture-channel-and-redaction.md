# ADR-0074　Windows 单窗口截图通道与像素脱敏

状态：**Accepted**（2026-10-06，按用户 2026-10-06 预授权代为裁决）　日期：2026-10-06　
Supersedes：—　Superseded by：—
关联：ADR-0071、ADR-0073、ADR-0063、`crates/platform/windows/src/window/capture.rs`、
`crates/platform/api/src/traits/window.rs`、`tasks/TASK-041-capture-window-redact-privacy.md`

## 背景（为什么现在要决定）

`WindowProvider::capture` 已在 TASK-016 冻结为「一个已解析窗口 + `CaptureOptions` →
`ImageRef`」，但 TASK-017 的 Windows 后端只返回 `CapabilityMissing`。TASK-041 拆分 A
已完成平台无关的编排、隐私保留和脱敏矩形决策；真实截图、像素级遮挡与失败码映射仍是
Windows 平台层的空白。该空白不能用空图、默认色块或未持久化的伪 blob 填充。

## 决策（一句话）

**Windows 平台层用 GDI `PrintWindow(PW_RENDERFULLCONTENT)` 截取单个已解析窗口；
`PrintWindow` 失败且窗口未被遮挡时才允许 `BitBlt` 回退；`redact=true` 时只依据
UIA `IsPassword` 的已判定矩形在 BGRA 像素上做不透明黑色遮挡；所有失败使用既有
`ErrorCode` 显式返回，绝不返回空白成功图。**

## 决策细化

| # | 内容 |
|---|---|
| **D1** | 截图通道选择 GDI：主路径 = `PrintWindow(hwnd, memory_dc, PW_RENDERFULLCONTENT)`；回退 = 窗口未被遮挡时 `BitBlt` 从该窗口 DC 复制。理由：同步、无 WinRT/D3D 所有权与异步生命周期、可在当前无窗口运行时依赖的 crate 中实现；且 `PrintWindow` 从窗口内容渲染而不是从桌面像素读取，符合“只截单窗口”。Windows.Graphics.Capture 保留为后续硬件合成窗口的候选通道，不在本轮实现。 |
| **D2** | 截图区域**只来自** `GetWindowRect` 的单个已解析 HWND，禁止桌面 DC、全屏坐标或窗口枚举兜底。宽高必须为正、转换到 `i32` 必须无损、总像素不得超过 `MAX_CAPTURE_PIXELS`。 |
| **D3** | 失败映射固定如下：HWND 已失效 / `IsWindow` 失败 → `TargetNotFound`；窗口最小化 → `TargetUnresponsive`；窗口被遮挡且 `PrintWindow` 失败 → `TargetUnresponsive`，拒绝 `BitBlt`（它可能截到其它窗口）；`PrintWindow` / `BitBlt` / GDI 返回 Win32 5 → `PlatformPermission`；零尺寸或窗口暂时无法提供表面 → `TargetUnresponsive`；超过像素上限或整数溢出 → `Fatal`；未知 Win32 码 → `Fatal`。不得把失败降级成空图。 |
| **D4** | `CaptureOptions.redact=true` 时，在读取像素后对本轮 UIA 已判定的密码框矩形执行像素级遮挡：`UIA_IsPasswordPropertyId=true`，`CurrentBoundingRectangle` 转到窗口图像局部坐标，越界部分裁剪，最终写入不透明黑色 BGRA（`B=0,G=0,R=0,A=255`）。矩形数量硬上限 = 64；UIA 条件创建、`FindAll`、属性读取或矩形转换任一步失败都 fail-closed，不能返回未遮挡图像。调用方传入任意区间需要扩展 `CaptureOptions`；本轮不扩展公共形状，故暂不实现该来源。 |
| **D5** | `redact=false` 不查询 UIA 密码框；窗口没有密码框是合法的空遮挡集合。遮挡只修改原始 BGRA 缓冲，平台层不引入图像编解码第三方 crate。 |
| **D6** | 本轮把遮挡后 BGRA 的 SHA-256 小写 hex 放入 `ImageRef.blob_id`，作为内容地址；但现有 `WindowProvider` 没有 storage 注入点，平台层不能在不改公共 trait / 不在平台层私开写库连接的前提下把字节持久化到 `crates/storage`。因此该值**不得**被宣称为已持久化 blob；持久化装配缺口登记为 `DRIFT-041-2`，本 PR 保持 WIP，待人类选择 storage sink 形状。 |
| **D7** | GDI 创建的 DC / bitmap / 选中对象必须在成功、错误和提前返回路径回收；清理失败不得被静默吞掉，无法在 `Drop` 返回的降级路径必须有源码注释说明。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 直接用 Windows.Graphics.Capture / WinRT | ❌ 本轮不采纳 | 需要 D3D/WinRT 生命周期、异步授权与帧池管理，显著扩大平台层状态面；GDI 已足以提供单窗口同步原语，WGC 留给确有硬件合成窗口证据的后续卡。 |
| 2 | 只用 `BitBlt` 从桌面或窗口 DC | ❌ 否决 | 桌面 DC 会截到其它窗口，违反单窗口边界；窗口 DC 回退在遮挡场景也可能出现别的窗口内容，故只允许 `PrintWindow` 失败且窗口未遮挡时使用。 |
| 3 | 在 `crates/capture` 解码、遮挡并持久化像素 | ❌ 否决 | 违反 ADR-0071 / ADR-0073 的 crate 边界；纯管线不得直接碰 Win32、像素或 storage 写连接。 |
| 4 | 返回空图或透明默认图让调用方“稍后处理” | ❌ 否决 | 违反铁律 1；用户会把空图当成真实截图，属于静默失败。 |
| 5 | **平台层捕获 / 遮挡并返回内容地址，同时显式保留持久化装配缺口** | ✅ **采纳** | 能在不改公共 trait、不引入新依赖的前提下完成可测试的真实平台路径，同时不伪造 blob 持久化。 |

## 影响

- `crates/platform/windows/src/window/capture.rs` 拥有 GDI 截图、尺寸校验、错误映射和
  UIA 密码框遮挡；`window/mod.rs` 只保留 trait → 平台函数的薄接线。
- `crates/platform/windows/Cargo.toml` 只新增同一已登记 `windows` crate 的
  `Win32_Storage_Xps` feature；不新增第三方依赖。
- 纯逻辑（尺寸、裁剪、BGRA 遮挡、状态错误映射）可在 Windows 目标上单测；真实 GUI 截图
  仍留人工验收，不伪造 CI 证据。
- 未闭合的 storage sink 是有意保留的 WIP：本 ADR 不宣称 capture 产物已经是持久 blob。

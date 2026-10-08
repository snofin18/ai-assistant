# ADR-0085　元素物理 bounds 读取路径

状态：**Accepted**（2026-10-08，按用户「选择后者」授权）　日期：2026-10-08
Supersedes：—　Superseded by：—
关联：**TASK-106**、ADR-0043、ADR-0045、ADR-0067、ADR-0076、ADR-0084、
`docs/spec/runtime-execution.md`、`crates/platform/api/src/traits/ui.rs`

---

## 背景

TASK-106 的 `paint.canvas.resolve_point` 必须把 Paint 画布内坐标换算成物理屏幕点。ADR-0084 D5
要求该换算使用 canvas bounds，但现行 `UiAutomationProvider::snapshot_tree` 的公共返回类型
`TreeSnapshot` 只有窗口句柄、指纹和节点数，没有树节点、元素句柄或 bounds。平台 trait 也没有
其它 bounds 读取方法。

可选方案有两条：

1. 把 `TreeSnapshot` 扩成结构化树节点，并从节点读取 bounds；
2. 给 `UiAutomationProvider` 增加一个只读的元素物理 bounds 方法。

方案 1 会把节点身份、树裁剪、序列化、回放格式和元素句柄生命周期一次性拉进公共契约，
而 Paint 本次只需要一个已解析元素的矩形；方案 2 与现行“先 `resolve_element` 得到不透明句柄，
再对句柄执行只读操作”的形状一致，也正好覆盖 Windows 已存在的 `CurrentBoundingRectangle`
读取路径。

## 决策（一句话）

新增 `UiAutomationProvider::element_bounds(&ResolvedElement) -> PlatformResult<ElementBounds>`，
返回值是 Per-Monitor V2 语义下的**全局虚拟屏物理像素矩形**；不扩 `TreeSnapshot`，不从树快照
猜元素几何，不在平台层不支持的路径上静默返回零矩形。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | 在 `crates/platform/api` 新增 `ElementBounds`：四个 `i32` 边界 `left` / `top` / `right` / `bottom`，右 / 下为开区间。提供受校验的构造器与只读 getter；`right <= left` 或 `bottom <= top` 必须显式失败。 |
| **D2** | `UiAutomationProvider` 新增 `element_bounds(&self, element: &ResolvedElement) -> impl Future<Output = PlatformResult<ElementBounds>> + Send`。它是只读操作，不改变元素状态、不申请写租约、不把句柄序列化出去。 |
| **D3** | 坐标语义固定为**全局虚拟屏物理像素**，与 `MonitorRecord` 的物理矩形和 ADR-0067 的 pointer 物理点校验同一空间。调用方若把结果与 `CoordinateSpace` 一起用于合成输入，必须显式校验矩形与坐标空间所属显示器一致；不得把逻辑像素、窗口局部像素或显示器局部像素混入。 |
| **D4** | Windows 实现走已有 `handles::with_element` + UIA `CurrentBoundingRectangle`；unsupported 平台返回 `CapabilityMissing`；replay 从录制节点已有的 `bounds` 读取并显式检查 `i64 -> i32`；fake / 测试 provider 必须实现同一失败语义。 |
| **D5** | `TreeSnapshot` 保持现状：它仍是窗口句柄、指纹和节点数的可回放摘要。本 ADR 不新增树节点、不在快照里暴露不透明元素句柄。 |
| **D6** | 错误语义：句柄不在当前线程 / 已过期 / 录制节点不存在 → `TargetNotFound`；UIA 读取失败按既有 HRESULT 映射返回；矩形为空或非正 → `TargetUnresponsive`；录制 bounds 不能无损转成 `i32` → `Fatal`。不得用零矩形、默认 DPI 或窗口 bounds 代替。 |
| **D7** | Paint handler 只在 `resolve_element` 已成功拿到 canvas 句柄后读取 bounds；`paint.canvas.resolve_point` 用 `canvas_bounds.left/top + (canvas_coordinate - viewport_offset) * zoom_ratio` 计算物理点，并校验结果仍在 canvas 矩形内。坐标空间不是 `PhysicalPixels`、矩形不可读或点越界 → fail-closed。 |

## 被否决的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 扩 `TreeSnapshot` 为结构化树并携带节点 bounds | ❌ | 会扩大可序列化公共契约、回放格式和节点身份语义；Paint 只需一个已解析元素的范围，成本远大于收益。 |
| 在 handler 内使用窗口 bounds / 任务输入 / 常量代替 canvas bounds | ❌ | 会产生系统性坐标偏差且看起来成功，违反铁律 1/2；也会绕过 ADR-0043 的元素 scope。 |
| 新增一个 Paint 专用平台 trait | ❌ | 与 ADR-0084 的“第二适配器仍走 platform trait”冲突，并制造第二个平台边界。 |
| 返回逻辑像素或窗口局部像素 | ❌ | 与 ADR-0067 的显式物理坐标空间不一致，混合 DPI 下无法可靠换算。 |

## 影响

- `crates/platform/api`：新增 `ElementBounds` 与 trait 方法，导出公共类型。
- `crates/platform/windows`：真实实现、unsupported 实现、README/限制说明同步。
- `crates/replay`：从录制节点 bounds 提供离线实现；录制格式原已包含 `bounds`，不新增字段。
- `apps/agent-core`：Paint handler 使用该只读方法；fake provider 与负向测试同步。
- **不改** `TreeSnapshot`、`ResolvedElement`、`ResolvedWindow`、tool schema、IPC、DB schema 或 ErrorCode；不新增 crate / 第三方依赖。

## 验证方式

1. `cargo test -p assistant-platform-api`：`ElementBounds` 合法 / 空矩形 / 边界反转。
2. `cargo test -p assistant-replay`：录制节点 bounds 可读取；缺失节点、越界 i64、空矩形均显式失败。
3. `cargo clippy --all-targets -- -D warnings`、非宿主 `assistant-platform-api` clippy、`cargo test --workspace` 全绿。
4. Windows 真机读取需要人工验收；无 UIA / 无真机时不得把失败写成成功，CI 只验证编译与错误映射。
5. TASK-106 的 fake 正向执行必须通过 `element_bounds` 计算 Paint 点；负向用例覆盖空矩形和句柄失效。

## 重新评估触发条件

- 若未来需要模型消费结构化 UIA 树，另行决定 `TreeSnapshot` 的节点模型与回放版本；
- 若第三个应用需要窗口 / 元素 bounds 之外的几何事实，先扩展 ADR，不在 handler 内私接平台 API；
- 若多显示器跨屏矩形无法用单一 `CoordinateSpace` 表达，另立坐标边界 ADR。

## 相关 ADR

- ADR-0043：元素解析必须有窗口 scope。
- ADR-0045：非宿主平台编译门禁。
- ADR-0067：pointer 动作显式坐标空间。
- ADR-0084：Paint 运行时装配；本 ADR 补充其 D5/D8 的平台读取缺口。

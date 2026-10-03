# ADR-0067　pointer 动作显式坐标空间

状态：**Accepted**（2026-10-04，按用户 2026-10-03 预授权代为裁决）　日期：2026-10-04　Supersedes：—　Superseded by：—
关联：架构 v2 §6.2 / §6.9 / §13.1.1、ADR-0043、ADR-0045、`tasks/TASK-016-platform-api-trait-capability-matrix.md`、`tasks/TASK-231-pointer-coordinate-space-dpi.md`、`docs/PARKING_LOT.md` PL-074

## 背景（为什么现在要决定）

TASK-016 冻结的 `UiAutomationProvider::pointer_action(point, action)` 不带目标窗口，也不带坐标空间。
`NormalizedPoint` 是全局逻辑坐标，而混合 DPI 多屏下同一个逻辑点可能对应多台显示器；
Windows 实现只能用“主屏缩放试算 → 物理点命中 → 重算 → 两次一致才采用”的收敛启发式。
不收敛时返回 `CapabilityMissing`，因此混合 DPI 桌面上存在永远点不到的逻辑点。

另有第二个真实缺陷：`PointerAction::DragTo` 的起点与释放点共用同一个 `CoordinateSpace`，
跨显示器拖拽时终点会按起点显示器的 scale 换算，产生系统性偏差。

PL-074 要求二选一：给 `pointer_action` 加目标窗口，或让调用方传入 `CoordinateSpace`。
本 ADR 选择后者，因为跨显示器拖拽的释放点并不属于起点目标窗口，窗口参数无法表达终点自己的坐标空间。

## 决策（一句话）

`pointer_action` 必须显式携带起始点的 `CoordinateSpace`；`DragTo` 的释放点必须携带自己的
`CoordinateSpace`。旧收敛启发式删除，不再在生产路径静默兜底。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | trait 签名改为 `pointer_action(&self, coordinate_space: &CoordinateSpace, point: NormalizedPoint, action: &PointerAction)`。`CoordinateSpace` 是已有纯类型，不引入新依赖、不新增抽象层。 |
| **D2** | `DragTo { drop_at, drop_coordinate_space }`：起点用 D1 的参数，释放点用自己的 `drop_coordinate_space`；两点分别走 `NormalizedPoint::to_physical`。 |
| **D3** | Windows 坐标层删除 `coordinate_space_for_logical_point` 的收敛启发式。新增显式纯函数校验：`origin_display` 必须存在、`kind` 必须是 `PhysicalPixels`、scale 必须等于该显示器的有效 DPI / 96、换算后的物理点必须落在已枚举显示器内。任一失败都返回带 `ErrorCode` 的错误，不猜。 |
| **D4** | `PointerAction` 不再派生 `Copy`，保留 `Clone`；这是携带 `CoordinateSpace` 后的类型代价。调用点按引用传 action，不需要复制。 |
| **D5** | 所有实现方与调用点同步：Windows / unsupported / replay / agent-core fake platform / 真机验收。未实现的实现方仍需显式返回 `CapabilityMissing` 或契约错误。 |
| **D6** | 与 ADR-0043 的关系：ADR-0043 要求元素解析显式带窗口 scope，本 ADR 要求 pointer 起始坐标显式带坐标空间；两者都在 API 边界终止“平台猜上下文”。 |
| **D7** | 与 ADR-0045 的关系：坐标校验与显式空间解析保持纯逻辑、在所有平台编译，非宿主 `clippy` 必须覆盖。 |
| **D8** | 与 TASK-016 冻结形状的关系：TASK-016 冻结的是第一版形状；本 ADR 在真实缺陷证据下做一次公共接口变更。除 pointer 相关形状外不改其它方法。 |
| **D9** | 旧启发式是删除，不是 fallback。保留它会让调用方错误的空间静默“修好”，违反无静默失败；显式空间校验失败必须暴露给调用方。 |

## 考虑过的选项

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 给 `pointer_action` 加 `&ResolvedWindow` | ❌ 否决 | 只能表达起点窗口；跨屏拖拽释放点不属于该窗口，且窗口跨屏时仍不能唯一决定点所属显示器。 |
| 2 | 让调用方传显式 `CoordinateSpace`，拖拽终点也带自己的空间 | ✅ 采纳 | 起点与终点都能独立声明 DPI 归属；无需把绝对屏幕坐标强行绑定到某个窗口。 |
| 3 | 保留收敛启发式作为 fallback | ❌ 否决 | fallback 会把模糊归属重新引入生产路径；错误的空间会被“猜对”掩盖，违反铁律 1。 |
| 4 | 用 `PointerTarget` 新类型同时包点与空间 | ❌ 否决 | 现有 `CoordinateSpace` + 参数已能表达，新增载体只增加公共表面积，没有减少复杂度。 |

## 影响

- `crates/platform/api/src/traits/ui.rs`：pointer trait 与 `PointerAction` 形状。
- `crates/platform/windows/src/coordinates/**`：删除旧启发式，新增显式空间校验纯函数与测试。
- `crates/platform/windows/src/input/**`：起点与 DragTo 终点分别换算。
- `crates/platform/windows/src/unsupported.rs`：签名同步，仍显式 `CapabilityMissing`。
- `crates/replay/src/provider.rs`、`apps/agent-core/tests/support/production_fixture.rs`：实现方同步。
- `crates/platform/windows/README.md`、`docs/PARKING_LOT.md`：限制说明与闭环登记。
- **不改** `docs/spec/**`；本 ADR 是公共接口形状的事实源。架构 v2 §13.1.1 旧示意的同步可在后续文档卡处理，不阻塞代码落地。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 调用方传错显示器空间 | 校验设备名与 scale 一致；物理点必须落在真实显示器上，否则 `TargetNotFound` / `ToolInvalidArgs`。 |
| 起点与终点空间交换位置 | 混合 DPI 测试分别断言两个 scale，禁止只测最终光标点。 |
| 旧调用点遗漏 | Rust 编译期枚举所有实现方与调用点；workspace test / clippy 全覆盖。 |
| 非 Windows 分支漏改 | `cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-api` + CI 三平台矩阵。 |

## 验证方式

1. `cargo test -p assistant-platform-windows` 覆盖：混合 DPI 原先 `CapabilityMissing` 的点由显式空间换算成功、跨屏 DragTo 双 scale、未知设备名、DPI 不一致、越界点。
2. `cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`、非宿主 clippy 全绿。
3. 重新评估触发条件：未来支持每显示器独立逻辑原点或跨平台坐标模型变化时，另立 ADR。

## 相关 ADR

- ADR-0043（元素解析必须有 scope）
- ADR-0045（非宿主平台编译门禁）

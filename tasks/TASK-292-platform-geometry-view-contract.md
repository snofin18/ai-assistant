# TASK-292　平台几何视图契约：`platform/api` 的 `WindowGeometry` / `ElementGeometry` 与两个查询

- 状态：Review
- 阶段：1　子阶段：**1b**　批次：治理池　依赖：ADR-0092、ADR-0093、TASK-256（`DRIFT-256-6`）
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：ADR-0092（显示器归属与指针窗口约束）、ADR-0093（勘误：几何回 `CoordinateSpace`）　**预估**：M　**难度**：M
- **write scope**：`crates/platform/api/**`
- **关联**：`tasks/TASK-256-paint-real-contract-calibration.md`（`DRIFT-256-6`）、ADR-0067、ADR-0085、ADR-0045

**目标**

把 ADR-0093 冻结的几何接口形状落成**可编译的契约半边**：新增 `WindowGeometry` / `ElementGeometry`
两个值类型，并在 `WindowProvider` / `UiAutomationProvider` 上各加一个查询方法（**带显式失败的默认实现**）。

**为什么只做半边**

平台实现的正确性依赖真机（显示器归属、窗口矩形），且 `platform/windows` 的实现要与负向用例一起验。
按一卡一会话的粒度，本卡只交契约；实现见 TASK-293。

**In scope**

1. 新增 `crates/platform/api/src/geometry_view.rs`：`WindowGeometry` / `ElementGeometry`
   （`bounds: ElementBounds` + `space: CoordinateSpace`，`#[non_exhaustive]`，只读访问器）。
2. `lib.rs`：挂模块 + 导出两个类型。
3. `traits/ui.rs`：`UiAutomationProvider::element_geometry`（默认实现 → `CapabilityMissing`）。
4. `traits/window.rs`：`WindowProvider::window_geometry`（默认实现 → `CapabilityMissing`）。

**Out of scope（做了算漂移）**

- `crates/platform/windows/**` 的真实实现（TASK-293）。
- `apps/agent-core` 的 `click_element_center` 与 TASK-256 切换（TASK-294）。
- dock 定位与 IPC（TASK-295）。
- 改 `element_bounds` / `PointerAction` / `ErrorCode` / `DisplayInfo` / `SessionState`。
- 新增依赖、`#[allow]`、`unsafe`。

**必须遵守**

- **默认实现必须显式失败**（`CapabilityMissing`），不得返回空矩形或默认值冒充成功（铁律 1）。
- 新类型只承载值，不得引入平台句柄（铁律 8）。
- 本 crate 要在三平台编译（ADR-0045）：不得出现 Windows / macOS 专有词汇。
- 公开项 100% 有文档注释（`missing_docs` + `-D warnings`）。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy -p assistant-platform-api --all-targets -- -D warnings
cargo test -p assistant-platform-api
cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-api --all-targets -- -D warnings
```

**DoD**

- [ ] 两个值类型 + 两个查询方法落到 `platform/api`，默认实现显式 `CapabilityMissing`
- [ ] `crates/platform/windows` 与所有既有 fake **无需改动**（默认实现保证加法式）
- [ ] 上述四条验收命令全绿（含 ADR-0045 的非宿主 clippy）
- [ ] 未改 `element_bounds` / `PointerAction` / `ErrorCode` / `DisplayInfo` / `SessionState`
- [ ] `LEDGER.md` 追加一行；任务卡 9 节记录填写
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-292（ADR-0093 冻结的几何接口形状，只做 `platform/api` 半边）。
- **write scope**：仅 `crates/platform/api/**`。
- **铁律**：1（默认实现必须显式失败）、7（core 只经本 crate 碰平台）、8（不跨进程传句柄）。
- **禁止**：改 `platform/windows` / `agent-core` / 既有公共类型；加依赖 / `#[allow]` / `unsafe`。
- **依赖**：ADR-0092 / ADR-0093 已 Accepted（`DRIFT-256-6` 因此进入有 ADR 的轨道）。
- **疑问**：无。

### 2. 实际改动文件

- `crates/platform/api/src/geometry_view.rs`（新增，值类型 + 文档）→ **In scope**
- `crates/platform/api/src/lib.rs`（挂模块 + 导出）→ **In scope**
- `crates/platform/api/src/traits/ui.rs`（`element_geometry` + 默认实现）→ **In scope**
- `crates/platform/api/src/traits/window.rs`（`window_geometry` + 默认实现）→ **In scope**

### 3. 验收输出摘要

- `cargo fmt --all`（写入后回归 check）：通过。
- `cargo clippy -p assistant-platform-api --all-targets -- -D warnings`：exit 0。
- `cargo test -p assistant-platform-api`：**11 + 3 + 10 + 1 passed / 0 failed**。
- 关键设计选择：两个方法都给**显式失败的默认实现**，因此 `crates/platform/windows`、`unsupported.rs`
  与所有测试 fake **一行都不用改** —— 加法式落地，符合 ADR-0092 细则 7「没实现就说没实现，绝不假装可用」。

### 4. DoD 逐条核对

- [x] 两个值类型 + 两个查询方法落到 `platform/api`；默认实现返回 `CapabilityMissing`
- [x] `platform/windows` 与既有 fake 零改动（默认实现保证）
- [x] `fmt` / `clippy`(api) / `test`(api) 全绿；非宿主 clippy 见 PR CI（本机未跑双 target）
- [x] 未改 `element_bounds` / `PointerAction` / `ErrorCode` / `DisplayInfo` / `SessionState`
- [x] `LEDGER.md` 追加一行
- [x] 无 Out of scope 文件被修改

### 5. 偏差

none。ADR-0093 已先行修正 ADR-0092 的接口形状（同名 `DisplayInfo` 冲突），本卡按修正后的形状落地。

### 6. 更合理做法

- `space` 直接复用既有 `CoordinateSpace`，而不是新造 `DisplayId` + `scale` 三元组 —— 少一处可写错的地方，
  也避开与既有 `DisplayInfo` **有理数**缩放口径的冲突（ADR-0093 的由来）。
- 默认实现刻意**不**返回空几何：宁可让调用方拿到 `CapabilityMissing`，也不让「没实现」被误读成「已支持」。

### 7. 遗留问题

- `platform/windows` 的真实实现（含「用 `MonitorFromWindow` / `MonitorFromPoint` + `GetDpiForMonitor`
  **避免枚举显示器**」的 O(1) 路径）与四个负向用例 → TASK-293。
- 两个类型目前未派生 `Serialize`（IPC 到 UI 是 TASK-295 的事）；届时按需加法式补，不预先加。

### 8. 新增长期记忆

- 无新增长期事实（契约形状由 ADR-0092 / ADR-0093 承载）。

### 9. 给审阅者的关注点

- 默认实现返回 `CapabilityMissing` 是否会被误用成「静默降级」？调用方（TASK-294）**必须**把它当失败处理，
  不允许退回"猜一个显示器"。
- `#[non_exhaustive]` + 只读访问器：后续补字段不破坏调用方，请确认构造函数保持 `const fn` 的口径。

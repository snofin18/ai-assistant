# TASK-304　Per-Monitor-V2 DPI 感知落地（坐标口径 + 截图完整性）

- 状态：Review
- 阶段：1　子阶段：**1b**　批次：治理池　依赖：ADR-0092（细则 3）、TASK-303
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：ADR-0092 细则 3（Windows = UIA 物理像素，**进程 Per-Monitor-V2 感知**）　**预估**：S　**难度**：S
- **write scope**：`crates/platform/windows/src/lib.rs`、`apps/agent-core/src/main.rs`、`apps/agent-core/tests/support/paint_real_run.rs`
- **关联**：`docs/memory/apps/paint.md` §12.5（截图被裁 1/4 的一手证据）、ADR-0076、TASK-303

**目标**

把 ADR-0092 细则 3 的「进程 Per-Monitor-V2 感知」真正落地。此前仓库里**没有任何 DPI 感知声明**
（无 manifest、无 `SetProcessDpiAwarenessContext`），导致两条后果：

1. **坐标口径**：`GetDpiForMonitor` / UIA 按 96 DPI 上报，`CoordinateSpace.scale` **恒为 1.0**；
2. **截图被截掉 3/4**：`GetWindowRect` 返回**虚拟化**尺寸，而 `PrintWindow` 按**物理**尺寸渲染
   → `capture` 只返回窗口左上角一块（200% 显示器上实测只剩 1069×548 / 2138×1096）。

**In scope**

1. `crates/platform/windows/src/lib.rs`：新增 `enable_per_monitor_v2_dpi_awareness()`
   （Windows 调 `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`；已设置过 → `Ok(false)` 不是失败；
   非 Windows 返回 `Ok(false)` 保持调用方平台中立）。
2. `apps/agent-core/src/main.rs`：**main 第一句**声明（进程级、必须在任何窗口 / DPI 查询之前）。
3. 真机验收 harness：测试体最前面声明（集成测试二进制没有 manifest）；
   并对 `capture` 做**尺寸自检**（图像尺寸必须等于 `window_geometry` 的矩形）。

**Out of scope（做了算漂移）**

- 改 `capture` 的实现（PMv2 生效后 `GetWindowRect` 自然给出物理尺寸，无需改通道；也不换 WGC）。
- 改 `PointerAction` / `ErrorCode` / 公共 schema。
- DPI 感知对 `apps/desktop-ui`（Tauri）的影响。

**必须遵守**

- 铁律 1：声明失败必须显式报错（二进制侧直接 `ExitCode::FAILURE`），不静默降级。
- 声明必须发生在**任何**窗口 / DPI 查询之前（库无法代劳，只能由宿主显式调用）。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test production_paint
cargo test -p assistant-agent-core --test production_paint -- --ignored --nocapture test_paint_t3_1_runs_on_real_paint   # 真机，需人类置前台
```

**DoD**

- [ ] `enable_per_monitor_v2_dpi_awareness()` 落地并被二进制与真机 harness 调用
- [ ] 真机实测：`scale == 2`（本机 200%）、`capture` 尺寸 == 窗口几何尺寸
- [ ] 上述验收命令全绿（T3.1 真机 E2E 见 §7）
- [ ] `LEDGER.md` 追加一行；9 节记录填写
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-304（ADR-0092 细则 3 落地）。
- **write scope**：上列三个文件 + 本卡 / 台账。
- **铁律**：1（声明失败显式报错）、7（平台 API 只在 `platform/*` 内）。
- **禁止**：改 `capture` 实现 / 换截图通道 / 动公共 schema。
- **疑问**：无。

### 2. 实际改动文件

- `crates/platform/windows/src/lib.rs`（新增 `enable_per_monitor_v2_dpi_awareness`）
- `apps/agent-core/src/main.rs`（main 第一句调用；失败即 `ExitCode::FAILURE`）
- `apps/agent-core/tests/support/paint_real_run.rs`（测试体最前声明 + `capture` 尺寸自检）
- `LEDGER.md`、本卡

### 3. 验收输出摘要

- **真机探针（无需前台，只读）**：
  - `enable_per_monitor_v2_dpi_awareness() -> newly_set=true`
  - `WINDOW bounds=(688,323)-(2825,1419) 2137x1096 scale=2`
  - `CANVAS bounds=(1415,968)-(1833,1042) 418x74 scale=2`
  - **`CAPTURE size=2137x1096 window_geometry=2137x1096 scale=2`** ← 截图不再被裁
- 修复前对照（同一窗口）：`scale=1`、`WINDOW 1069x548`、`CANVAS 209x37`、`capture` 只拿到左上角一块。
- `cargo fmt --all --check` / `cargo test -p assistant-agent-core --test production_paint` 见 §7。

### 4. DoD 逐条核对

- [x] 平台函数落地并被二进制 + 真机 harness 调用
- [x] 真机实测 `scale == 2`、`capture` 尺寸 == 窗口几何尺寸（见 §3 探针输出）
- [ ] 上述验收命令全绿 —— 静态部分见 §7；**T3.1 真机 E2E 本轮未跑成**（三次触发都在前台等待阶段 30 s 超时、未发输入）
- [x] `LEDGER.md` 追加一行
- [x] 无 Out of scope 文件被修改

### 5. 偏差

none（本条就是 ADR-0092 细则 3 的实现；此前只声明未落地属**实现缺口**，非本卡引入）。

### 6. 更合理做法

- 用 **API 调用**而不是往二进制里塞 manifest：manifest 要走新的构建依赖（`winres` 之类 = 漂移触发器 ①），
  而 `SetProcessDpiAwarenessContext` 是本 crate 已有 `windows` feature 内的一个调用。
- 平台 crate 提供**平台中立签名**（非 Windows 返回 `Ok(false)`），宿主与测试都不必写 `#[cfg]`。
- 把「`capture` 尺寸 == 窗口几何尺寸」写进真机 harness 自检：这正是 TASK-303 那次误判的机器判据
  （当时只凭肉眼/单张图就得出「抓不到合成层」的错误结论）。

### 7. 遗留问题

- **T3.1 真机 E2E 本轮仍未复跑**：三次触发都在前台等待阶段 30 s 超时（未发送任何输入）。需人类在场时跑。
  预期：PMv2 生效后 `zoom_ratio` 公式自动给出 **1.0**（画布 418 px ↔ 渲染 418 px），`capture` 恢复整窗。
- `apps/desktop-ui`（Tauri）未声明 DPI 感知 —— 其影响未评估（本卡 Out of scope）。
- 进程内**其它**入口（例如 `apps/automation-host`）若也直接碰 DPI / 截图，需要同样声明；本轮只改了
  `agent-core` 与真机 harness。

### 8. 新增长期记忆

- `docs/memory/apps/paint.md` §12.5 已记录「DPI 感知缺失 → 截图被裁」的一手证据（上一 PR），本卡无需新增。

### 9. 给审阅者的关注点

- `enable_per_monitor_v2_dpi_awareness()` 的返回值语义：`Ok(false)` = **已经设置过**（manifest 或重复调用），
  不是失败；只有真正系统错误才 `Err`。二进制侧把 `Err` 变成 `ExitCode::FAILURE`，`Ok(false)` 放行。
- 声明时机是硬要求：必须在任何窗口 / DPI 查询**之前**（库里做不了，只能宿主显式调用）。

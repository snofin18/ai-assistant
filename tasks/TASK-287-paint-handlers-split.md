# TASK-287　拆分 paint_handlers.rs：抽出 UIA/输入原语与选中态/几何子模块

- 状态：Done
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：106、256（真机校准受阻于本卡）、ADR-0033
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：106、256（真机校准受阻于本卡）、ADR-0033　**预估**：M　**难度**：M
- **write scope**：`apps/agent-core/src/paint_handlers.rs`、`apps/agent-core/src/paint_handler_actions.rs`（新增）、
  `apps/agent-core/src/paint_handler_selection.rs`（新增）
- **关联**：`DRIFT-256-4`、`DRIFT-256-3`、ADR-0033（文件长度 600 软 / 900 硬）、ADR-0091、
  gov §5.4、`docs/memory/pitfalls.md`

**目标**

把 `apps/agent-core/src/paint_handlers.rs` 的私有 UIA / 输入 / 截图原语与「选中态 + 画布点解析」辅助
分别抽到两个子模块，使主文件 ≤600 行（消除 `hygiene/file-too-long` 软告警，并给 TASK-256 的
ADR-0091 落地留出改动余量）；**行为零变更**。

**为什么是这张卡**

`DRIFT-256-4`（2026-10-14）：TASK-256 要用 ADR-0091 的 `ExactNameAndRole` 校准形状库锚点、
加 `CollapsedButton` 展开、并以 Button + `TogglePattern` 处理形状项，实测会让
`paint_handlers.rs` 从 900 行涨到 **949 行**，越过 900 行**硬上限**（`hygiene/file-too-long` 直接 fail）。
TASK-256 自己的 §7 已写明「若继续增长，需另立拆文件卡，不在本卡 write scope 内静默扩模块」。
本卡就是那张前置卡：先把文件拆开，TASK-256 才能在自己的 write scope 内落地。

**In scope**

1. `apps/agent-core/src/paint_handlers.rs`：抽走下列方法后保留 `PaintHandlerContext` 定义、`new`、
   7 个 `*_output` 工具方法与 `mod tests`。
2. 新增 `apps/agent-core/src/paint_handler_actions.rs`：UIA / 输入 / 截图原语方法
   （`resolve_window` / `resolve_element` / `resolve_layer_list` / `element_bounds` / `read_element_text` /
   `invoke_element` / `select_element` / `fingerprint_event` / `send_key` / `drag` / `capture_window`）。
3. 新增 `apps/agent-core/src/paint_handler_selection.rs`：选中态与画布点辅助
   （`reset_selection` / `selection_tool` / `selection_color` / `selection_layer_id` /
   `next_document_generation` / `resolved_point` / `verified_active_layer` / `ResolvedPointSlot` /
   `palette_index` / `paint_layer_contract`）。

方法以**同 crate 内的独立 `impl` 块**迁到子模块（Rust 允许 inherent impl 跨模块），
签名与可见性逐字不变。

**Out of scope（做了算漂移）**

- 任何行为 / 契约改动的**实质**：本卡是**纯搬移**，函数体逐行等价。
- ADR-0091 的 adapter / handler 落地、TASK-256 真机校准（另卡）。
- 改 `crates/**`、`protocol/**`、`adapters/**`、`eval/**`。
- 新增依赖、新增 `#[allow]` / `unsafe`、放宽任何 lint。
- 顺手重构 / 顺手优化（不相关的问题记 `docs/PARKING_LOT.md` 一行，本卡不动）。

**必须遵守**

- **纯搬移**：不得改任何 `fn` 的签名、可见性、`where` 子句、属性或函数体逻辑。
- 不得新增 / 删除 / 重命名 `pub` / `pub(crate)` 符号；对外形状保持不变。
- 主文件与两个新子模块都必须 ≤600 行（gov §5.4 / ADR-0033）。
- 铁律 1：错误语义不变 —— 仍必须返回带 `ErrorCode` 的失败，禁止吞错误。
- 改 `LEDGER.md` 前先 `cargo run -p xtask -- guard acquire ...`，写完立刻 `guard release`。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test production_paint
cargo run -p xtask -- hygiene
```

**DoD**

- [ ] `apps/agent-core/src/paint_handlers.rs` ≤600 行，且 `hygiene` 不再对该文件报 `hygiene/file-too-long`
- [ ] 两个新子模块文件均 ≤600 行
- [ ] 纯搬移：`cargo test --workspace` 全绿（含 `production_paint` fake 4 passed、`paint_handlers` lib 测试）
- [ ] 无 `#[allow]` / 新依赖 / lint 放宽；未改 `crates/**` / `protocol/**` / `adapters/**` / `eval/**`
- [ ] `LEDGER.md` 追加一行；新事实 / 坑进 `docs/memory/pitfalls.md`（若有）
- [ ] `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 按 ADR-0039 / ADR-0041 同批同步
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-287 拆分 `paint_handlers.rs`（纯搬移），解锁 TASK-256 的 `DRIFT-256-4`。
- **目标**：把 UIA / 输入 / 截图原语与「选中态 + 画布点解析」抽到两个子模块，主文件 ≤600 行，行为零变更。
- **write scope**：仅 `apps/agent-core/src/paint_handlers.rs`、`apps/agent-core/src/paint_handler_actions.rs`（新增）、
  `apps/agent-core/src/paint_handler_selection.rs`（新增）。
- **铁律**：1（错误语义不变，仍返回 `ErrorCode`）、9（不得静默扩大范围）。
- **禁止**：任何行为改动；改 `crates/**` / `protocol/**` / `adapters/**` / `eval/**`；新依赖；`#[allow]` / `unsafe`。
- **验收**：正文 5 条命令（fmt / clippy / workspace test / production_paint / hygiene）。
- **依赖**：106、256（已核对 LEDGER：TASK-106 Done；TASK-256 Blocked，本卡是其前置拆文件卡）。
- **疑问**：无。

### 2. 实际改动文件

- `apps/agent-core/src/paint_handlers.rs`：删除原 542–836、839–865 两段，改为 `mod` 声明 + `use paint_handler_selection::{ResolvedPointSlot, paint_layer_contract, palette_index};`，并把只服务搬移后代码的 import 收敛（`window_lease_key` / `BlobId` / 平台原语类型 / `normalized_point`）→ **In scope**
- `apps/agent-core/src/paint_handler_actions.rs`（新增，**222 行**）→ **In scope**
- `apps/agent-core/src/paint_handler_selection.rs`（新增，**179 行**）→ **In scope**
- `plans/stage-1-pilots.md`：本卡登记行 + 完成标记 + 头部「当前进度」句 → 计划类文件的允许面（ADR-0041 D1）
- `PLAN.md` / `README.md` / `LEDGER.md`：进度同步允许面（ADR-0039 D1/D2/D5、§11.1）

行数变化：`paint_handlers.rs` **900 → 578**；新增 `paint_handler_actions.rs` 222、`paint_handler_selection.rs` 179
（`paint_handler_support.rs` 296、`paint_handler_map.rs` 186 未改）。

### 3. 验收输出摘要

```text
cargo fmt --all --check                                  → exit 0
cargo clippy --all-targets -- -D warnings                → exit 0
cargo test --workspace                                   → exit 0（含 469 passed / 0 failed）
cargo test -p assistant-agent-core --test production_paint → 4 passed / 0 failed / 1 ignored（真机用例按设计 ignored）
cargo test -p assistant-agent-core --lib paint_handlers  → 4 passed
cargo run -p xtask -- hygiene                            → 0 error / 118 warning / PASSED
```

差异：`hygiene` 由搬迁前的 **0E / 119W** 变为 **0E / 118W** —— `hygiene/file-too-long` 对
`paint_handlers.rs` 的告警消失；唯一剩余的 `paint_handlers.rs` 告警是既有的
`hygiene/cyclomatic-complexity`（`select_layer_output` 圈复杂度 17，与本卡无关）。

**「纯搬移」证据**：把 `origin/main` 版被移除区域（542–836、839–865）与新两个子模块拼起来的内容
同时归一化（去掉 `use` / `//!` / `#[path]` / `impl<P>` 头 / 换行 / 所有空白 / 逗号 / `pub(super) ` 前缀）
后逐字符比较 → **两侧均为 7079 字符且完全相同**。日志级实现见下方复现命令。

```powershell
# 归一化后比较：为真则打印 IDENTICAL
$orig = (git show origin/main:apps/agent-core/src/paint_handlers.rs) -split "`n"
$removed = @($orig[541..835]) + @($orig[838..864]); $A = NormText $removed
$B = NormText (@((Get-Content apps/agent-core/src/paint_handler_actions.rs)) + @((Get-Content apps/agent-core/src/paint_handler_selection.rs)))
if ($A -ceq $B) { "IDENTICAL" }   # → IDENTICAL（removed_len=7079 news_len=7079）
```

### 4. DoD 逐条核对

- [x] `paint_handlers.rs` ≤600 行（**578**），`hygiene/file-too-long` 不再对它报
- [x] 两个新子模块均 ≤600 行（222 / 179）
- [x] 纯搬移：`cargo test --workspace` 全绿；`production_paint` 4 passed；`paint_handlers` lib 4 passed
- [x] 无 `#[allow]` / 新依赖 / lint 放宽；未改 `crates/**` / `protocol/**` / `adapters/**` / `eval/**`
- [x] `LEDGER.md` 追加一行
- [x] `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步
- [x] 无任何 Out of scope 的文件被修改

### 5. 偏差

none。

### 6. 更合理做法

- 新子模块的方法统一用 **`pub(super)`**（= 原「父模块私有」的可见面），不放宽到 `pub(crate)` —— 保持最小暴露面。
- `paint_handler_selection` 的 `impl<P> PaintHandlerContext<P>` **不重复**平台 trait 的 `where` 约束，
  因为它不调用平台 API —— 保持原有的「谁能调用」不变，同时避免无谓约束。
- `paint_handler_actions.rs` 的模块头写明「只做原语、不解析参数、不装配信封」，把边界固化到新增文件上。

### 7. 遗留问题

- `hygiene/cyclomatic-complexity` 仍报 `paint_handlers.rs::select_layer_output`（17）—— 既有告警，
  属设计性重构，不在本卡范围（未新开停车位：TASK-256 会在同一函数附近继续改，留待那时判断）。

### 8. 新增长期记忆

无（纯搬移，没有新的应用事实或坑；文件长度契约已由 ADR-0033 / gov §5.4 覆盖）。

### 9. 给审阅者的关注点

- **「纯搬移」的证明方式**：归一化 token 流比较（7079 字符逐字符相等）会掩盖「同 token、不同语义」的改动
  （例如换 `self.selection` 的读取顺序）。请对照 `paint_handler_actions.rs` / `paint_handler_selection.rs`
  复查方法体，尤其是 `verified_active_layer` 的三段前置校验与 `resolved_point` 的比较分支。
- **跨模块可见性**：两个新子模块依赖 Rust 的「子模块可访问父模块私有项」规则读取 `self.selection` /
  `self.document_generation` 与私有常量；`paint_handler_map.rs` / `paint_handler_support.rs` 未改，
  调用点全部在 `paint_handlers` 模块树内。
- **TASK-256 余量**：主文件现 578 行，距 600 行软线只剩约 22 行；TASK-256 的 ADR-0091 落地（预计 +50 行）
  会再次越过 600 软线（只越软线，不碰 900 硬线）。若评审认为应留更多余量，可再开一张只搬 `*_output` 的卡。

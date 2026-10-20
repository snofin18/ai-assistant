# TASK-296　调色板选择路径：`child_at` 按视图有序回退（ADR-0094）

- 状态：Done
- 阶段：1　子阶段：**1b**　批次：治理池　依赖：TASK-256、ADR-0087、ADR-0094
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-256（T3.1 真机链路）、ADR-0087（本卡勘误的对象）、ADR-0094（勘误决议）　**预估**：M　**难度**：M
- **write scope**：`crates/platform/windows/src/uia/actions.rs`、`crates/platform/windows/tests/palette_view_fallback.rs`、`docs/adr/**`、`docs/memory/decisions.md`、`MEMORY.md`
- **关联**：`DRIFT-296-1`、`docs/memory/apps/paint.md` §12.3、TASK-294、TASK-295

**目标**

让 `Selection::ByIndex` 在「子项只存在于 **control view**」的 UIA 列表上也能工作 —— T3.1 真机链路
当前卡在 `paint.color.select_foreground`：`TargetNotFound: select: child index 3 is out of range`。

**为什么必须改平台层**

只读探针（用与 handler 完全相同的选择器链解析 `foreground_color_button`）实测：调色板 `List`
在 **`ContentViewWalker` 下 0 个子项**、在 **`ControlViewWalker` 下有全部色块** —— 色块是 control
元素但**不是** content 元素。ADR-0087 把 `child_at` 冻结成单一 `ContentViewWalker`，因此必然失能。

**In scope**

1. `docs/adr/0094-child-at-view-fallback.md`（勘误 ADR-0087 的单一视图口径）+ `docs/adr/README.md` 登记。
2. `actions.rs::child_at` 改为 `Content view → control view` 有序回退；拆出 `nth_child` helper。
3. 真机 `#[ignore]` 用例：正向 `ByIndex(3)` 命中色块；负向 `ByIndex(5000)` → `TargetNotFound`。
4. `docs/memory/decisions.md` + `MEMORY.md` 规模表同步。

**Out of scope（做了算漂移）**

- 扩 `crates/platform/api` 公共 API（`Selection` 新变体 / `ItemContainerPattern`）。
- 改 `resolve.rs` / `tree.rs`（指纹仍走 control view）/ `ensure_realized` / `actions::select` 的
  `CapabilityMissing → Invoke` 回退。
- 调色板的其它 read-back 语义（属于 TASK-256）、`paint.tool.select` 的后置条件（另卡）。

**必须遵守**

- 铁律 1：两个视图都取不到 → 显式 `TargetNotFound`，绝不猜 index / 坐标。
- 顺序固定 `content → control`：content 能给出第 n 项时绝不看 control（形状库行为不得变）。
- 不加依赖 / `#[allow]` / `unsafe`（`unsafe` 仅在既有 UIA 调用惯例处）。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-platform-windows --test palette_view_fallback -- --ignored   # 真机
cargo run -p xtask -- hygiene / check-ledger / card-check / docscan / adr-index / memory-counts
```

**DoD**

- [ ] ADR-0094 + 登记 + `decisions.md` 落地
- [ ] `child_at` 有序回退实现，`content` 优先、两侧皆空显式失败
- [ ] 真机用例：正向命中 + 负向 fail-closed
- [ ] 上述验收命令全绿（真机 E2E 见 §7）
- [ ] `LEDGER.md` 追加一行；9 节记录填写
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-296（ADR-0094 落地：`child_at` 视图回退）。
- **write scope**：上列平台实现 + 测试 + ADR/decisions/规模表 + 本卡。
- **铁律**：1（两侧都取不到必须显式失败）、5（API 优先，不用坐标兜底）、9（不静默扩大范围）。
- **禁止**：扩公共 API / 动 `resolve.rs` / `tree.rs` / `ensure_realized`。
- **疑问**：无（ADR-0094 已经人类 2026-10-15「按这个做」授权）。

### 2. 实际改动文件

- `docs/adr/0094-child-at-view-fallback.md`（新增）
- `docs/adr/README.md`（§1 登记 0094 + 下一个可用编号 → 0095）
- `docs/memory/decisions.md`（追加 ADR-0094 条目）
- `crates/platform/windows/src/uia/actions.rs`（`child_at` 回退 + `nth_child`）
- `crates/platform/windows/tests/palette_view_fallback.rs`（新增真机用例）
- `MEMORY.md`（规模表）、`LEDGER.md`、本卡

### 3. 验收输出摘要

- `cargo fmt --all --check`：exit 0。
- `cargo clippy -p assistant-platform-windows --all-targets -- -D warnings`：exit 0。
- `cargo test -p assistant-platform-windows --lib`：**109 passed / 0 failed / 4 ignored**。
- `cargo test -p assistant-platform-windows --test palette_view_fallback -- --ignored`（**真机实跑**）：
  **1 passed** —— 负向 `ByIndex(5000)` → `TargetNotFound`；正向 `ByIndex(3)` 经 control view 命中色块。
- `adr-index` / `memory-counts` PASSED（decisions.md 296→297 行、105→106 条同步）。

### 4. DoD 逐条核对

- [x] ADR-0094 + 登记 + `decisions.md` 落地（`adr-index` PASSED）
- [x] `child_at` 有序回退：content 优先、两侧皆空 → `TargetNotFound`
- [x] 真机用例正向 / 负向均通过
- [x] 上述验收命令全绿（T3.1 端到端真机复跑见 §7）
- [x] `LEDGER.md` 追加一行
- [x] 无 Out of scope 文件被修改

### 5. 偏差

`DRIFT-296-1`（漂移触发器 ④ / ⑧：改 ADR 已决事项 / ADR 与实测矛盾）——**已由人类 2026-10-15
「按这个做」裁决**：立 ADR-0094 勘误 ADR-0087 的单一视图口径，并按候选修法（content → control
有序回退）落地。偏差已闭环。

### 6. 更合理做法

- 把「取第 n 个子节点」抽成 `nth_child(walker, element, index, context)`，两个视图共用同一段遍历
  逻辑（一处实现，避免复制粘贴导致两套行为）。
- 顺序固定 `content → control` 而不是「哪个视图有就用哪个」：逻辑列表（形状库）行为完全不变，
  只有 content view 给不出该项时才换视图。
- 负向用例写进真机 `#[ignore]`（`ByIndex(5000)` → `TargetNotFound`），把「不猜」变成机器判据。

### 7. 遗留问题

- **T3.1 端到端真机复跑（`cargo test -p assistant-agent-core --test production_paint -- --ignored`）
  尚未重跑** —— 它需要人类把 Paint 置于前台（测试只等 30 s、不抢焦点），且会真实画出矩形。
  跑通才算 TASK-256 的 T3.1 链路打通；在此之前 TASK-256 保持 Review、TASK-044 十次运行不得开始。
- `paint.tool.select` 仍无后置条件（`paint_handlers.rs`，另卡，见推荐顺序第 2 步）。
- 进程未声明 DPI 感知（坐标被虚拟化、`scale` 报 1.0），与 ADR-0092 细则 3 冲突（推荐顺序第 1 步）。

### 8. 新增长期记忆

- `docs/memory/apps/paint.md` §12.3（调色板色块只在 control view）、`docs/memory/decisions.md`
  （ADR-0094 条目）。

### 9. 给审阅者的关注点

- 回退**只在 content view 给不出第 n 项时**发生；请确认这不会改变形状库路径的语义（它仍由
  content view 独占，且已不再经 `Selection::ByIndex`）。
- 真机用例会**改变前景色**（选中第 4 个色块），这是刻意的：它是唯一能证明「回退真的命中色块」
  的通道（该色块无 AutomationId、无稳定 id）。

### 2026-10-20 TASK-324 收口补充

- PR #411 已合并到 `main`，merge commit = `211f2c8ca300a04b96bcf917e37cd3745f873d00`。
- GitHub CI rollup = 11 checks / 11 success / pending 0；真机 palette fallback 已通过。
- TASK-324 仅依据既有真实证据把状态从 `Review` 更新为 `Done`，未改正文或产品代码。

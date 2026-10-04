# TASK-235　`RoleAndParent` 父候选与解析链语义（PL-094）

- 状态：**Done（2026-10-04；ADR-0070 Accepted，PR #220 / merge `8d9241d`，CI 11/11 SUCCESS）**
- 阶段：1　子阶段：治理池　批次：1b bridge　依赖：017、031、ADR-0043、ADR-0044
- 关联：`docs/PARKING_LOT.md` PL-094、`[ADR:待建 0017]`、ADR-0043、ADR-0044、`crates/platform/windows/src/uia/{resolve,search}.rs`

## 目标（一句话）

明确并实现 `RoleAndParent` 的父候选只作为子树作用域：顶层解析只能返回角色子候选；父候选单独命中时不得被当作结果返回，而必须走既有显式失败语义。

## In scope

- `docs/adr/0070-role-and-parent-resolution-semantics.md`（新增）、`docs/adr/README.md`、`docs/memory/{decisions,pitfalls}.md`、`docs/PARKING_LOT.md`
- `crates/platform/windows/src/uia/{resolve,search}.rs`、本卡记录区
- 收口同步：`PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`、`MEMORY.md`

## Out of scope（做了算漂移）

- 修改 `docs/spec/**`、公共 trait / schema、`SelectorKind` / `SelectorValue` 的 wire 形状，或新增 selector kind / 依赖 / crate / 抽象层。
- 改变 `locale_dependent` 降权、候选歧义策略、`min_score_to_try`，或修改 UI 拾取器的 fail-closed 行为。

## 必须遵守

- 父候选只由链内 `RoleAndParent.parent_id` 引用时视为作用域候选，不得进入顶层目标候选集合；父缺失、歧义、无命中、UIA 调用失败或子候选无命中都必须显式失败。
- 新行为先由 ADR-0070 冻结，再落实现；ADR 标注按用户 2026-10-04 预授权代 Accepted。
- 不新增 `#[allow]`、不新增依赖、不放宽既有断言；所有热点文件写前必须 `guard acquire`。

## 验收命令（agent 必须全部执行并粘贴输出）

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-platform-windows
cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-api --all-targets -- -D warnings
cargo run -p xtask -- adr-index
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
```

## 完成定义（DoD）

- [ ] ADR-0070 Accepted，登记表、decisions.md 与下一可用编号同步，`adr-index` 0E0W。
- [ ] `RoleAndParent` 的作用域候选不再进入顶层目标尝试；父单独命中时不返回父元素。
- [ ] 正向证据：父 + 子在链内时，解析控制流选择的是 `RoleAndParent` 子候选。
- [ ] 负向证据：父命中、子不存在时返回 `TargetNotFound`（不是父元素，也不是静默成功）。
- [ ] `min_score_to_try` 与既有降权 / 歧义策略未被悄悄改变，ADR 写清影响。
- [ ] 全部验收命令全绿，原始输出贴进卡记录区与 PR；`PL-094` 已闭环，状态行与 LEDGER 对齐。
- [ ] 无任何 Out of scope 文件被修改。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-235 `RoleAndParent` 父候选与解析链语义（PL-094）　【目标】以 ADR-0070 明确父候选仅为作用域，并让 Windows 元素解析只返回角色子候选、父单独命中时显式失败。
【write scope】`docs/adr/0070-role-and-parent-resolution-semantics.md`、`docs/adr/README.md`、`docs/memory/{decisions,pitfalls}.md`、`docs/PARKING_LOT.md`、`crates/platform/windows/src/uia/{resolve,search}.rs`、本卡；收口同步 `PLAN.md` / `plans/stage-1-pilots.md` / `README.md` / `LEDGER.md` / `MEMORY.md`。
【铁律】无静默失败；契约先行；不扩范围；热点文件先取 guard；证据可机器验证；不新增依赖 / crate / `#[allow]`。
【禁止】不改 `docs/spec/**`；不放宽断言；不把父候选当目标返回；不伪造 CI / 门禁 / 真机证据。
【验收】`fmt` / `clippy` / `test --workspace` / Windows 专项 / 非宿主 clippy / 十项 xtask 门禁全绿。
【依赖】TASK-031 的 PL-094、ADR-0043 / ADR-0044 / `[ADR:待建 0017]` 已核对；`main` 无 `.automation.lock`。
【疑问】无；按用户预授权裁决选项 (a)：父候选只作为作用域。

### 2. 实际改动文件

ADR-0070 + 登记 / decisions / pitfalls / PL-094 闭环；`resolve.rs`（顶层过滤 + 可测试候选选择层 + 3 专项）；`search.rs`（结果泛型化 + ADR 注释）；PLAN / README / plans / LEDGER / MEMORY 收口。

### 3. 验收输出摘要

- `fmt --check` / `clippy --all-targets -- -D warnings` / 非宿主 clippy / `test --workspace` 均 exit 0；`xtask` 452 passed。
- `cargo test -p assistant-platform-windows`：unit **96 passed / 0 failed / 4 ignored**；集成 7 + 4 passed；doctest 1 passed。
- 专项：父 + 子只选子候选；父命中子缺失返回 `TargetNotFound`；仅 scope 链在碰 COM 前 `ToolInvalidArgs`。
- `adr-index` 58 files 0E0W；`memory-counts` 8 files 0E0W；`refscan` 673 files 0E0W；`hygiene` 357 files 0E/99W；`docscan` 0E/342W。
- `card-check` 0E/33W；`check-comments` 0E/69W；`verify-schemas` 5 OK；`codegen --check` 0 drift；`check-ledger` 0E0W PASSED。

### 4. DoD 逐条核对

- [x] ADR-0070 Accepted，登记表 / decisions / 下一可用编号同步，`adr-index` 0E0W。
- [x] 父候选不再进入顶层目标尝试；父单独命中不返回父元素。
- [x] 正向与负向专项均通过，父命中但子缺失为 `TargetNotFound`。
- [x] `min_score_to_try`、降权、歧义策略未改，ADR 写明影响。
- [x] 全部验收命令全绿；`PL-094` 闭环，状态行与 LEDGER 对齐；无 Out of scope 改动。

### 5. 偏差

无产品行为偏差。README 状态行首笔编辑发生在 `guard acquire README.md` 之前；随后立即补锁完成其余 README 编辑，无并发写者 / lost update。`hygiene` 清理的 `C` 空目录树确认为 2026-10-03 自动化残留且零文件。

### 6. 更合理做法

把顶层过滤与搜索结果选择拆成不碰 COM 的内部纯逻辑，并以 `parent_id` 引用关系作为唯一事实源，避免新增 helper 标记。

### 7. 遗留问题

无新增 PARKING_LOT 项；PL-094 已闭环。

### 8. 新增长期记忆

`decisions.md`：ADR-0070 契约原文；`pitfalls.md`：父候选进入顶层尝试会把父元素当结果返回的坑与修法。

### 9. 给审阅者的关注点

1. 顶层过滤只依据 `parent_id` 引用，未改 `SelectorCandidate` wire 形状。
2. 父命中 / 子缺失稳定 `TargetNotFound`，无回退父元素，且 `min_score_to_try` / 歧义策略未变。
3. `search.rs` 的泛型化只服务测试控制流，未改生产 COM 路径与句柄纪律。

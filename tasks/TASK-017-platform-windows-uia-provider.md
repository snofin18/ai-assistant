# TASK-017　`platform/windows`：UIA provider（树快照 / selector 链解析 / read_text / set_value / edit_text / invoke_action / bounds / fingerprint / window 枚举与状态）

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：016　预估：L　难度：L
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：016（`crates/platform/api` 的 **trait 形状 + 纯类型**已落地；本卡**实现**它们，**不改**形状）
- **预估**：L　**难度**：L
- **write scope**：`crates/platform/windows/**`、`docs/DEPENDENCIES.md`（仅 `windows` 行 —— 见「待裁决」Q1）
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）；架构 v2 §13.1.1（抽象接口）/ §13.2（Windows 通道）/ §6.2（`TargetDescriptor`）/ §7.3（指纹）/ §3.2（进程边界）；`docs/spec/capability-matrix.md`；`docs/spec/naming.md` §7；**ADR-0022**（Windows 目标身份与 UIA selector 稳定性）；**ADR-0024**（`windows` crate 与 `spike-deny`）；`docs/memory/apps/notepad.md`；`docs/memory/win32-input-research.md`；**铁律 5 / 7 / 8**；ADR-0019 N1（负向验证）

**目标**

新建 `crates/platform/windows`（crate 名 `assistant-platform-windows`）—— 用 `windows` crate（UIA3 客户端 COM）把 TASK-016 定下的
`WindowProvider`（5 方法）+ `UiAutomationProvider`（13 方法）落到真实 Win32 / UIA 上。

本卡**只做** `src/uia/**` + `src/window/**` 两域；**合成输入**（SendInput）、**坐标归一化**（DPI / 多屏）与 **IME** 归 **TASK-018**。

**为什么现在新建 crate（显式声明，不静默扩范围）**

新建 `crates/platform/windows/` 命中**漂移触发器 ②**（加 crate / 顶层目录）。但它是**计划内**的：
① `plans/stage-1-pilots.md` 批次表 A2 已把 `crates/platform/windows/src/uia/**` 与 `.../src/window/**` 分给 TASK-017（`src/input/**`、`src/coordinates/**` 分给 TASK-018）；
② `docs/DEPENDENCIES.md` 已把 `crates/platform/windows` 写为 `windows` crate 的**阶段 1 使用方**（但状态是 `Approved for spikes` → 见「待裁决」Q1）；
③ 铁律 7 要求 core 只能经 `crates/platform/api` 的 trait 调平台 → 实现**必须**住在 `crates/platform/*`，不能放 core。
→ 因此**不是**触发器 ② 所指的「未经计划的扩范围」；按铁律 9 在此**显式声明**，并在执行记录 §5 复述。

**In scope（本卡交付物）**

| # | 交付物 | 依据 |
|---|---|---|
| 1 | `crates/platform/windows/Cargo.toml`（`assistant-platform-windows`）：依赖 `assistant-platform-api` + 已登记的 `windows` crate；**非 Windows 目标不得编译失败** | 铁律 7 + `docs/DEPENDENCIES.md` |
| 2 | `src/window/**`：`WindowProvider` 的 5 方法（`list_windows` / `resolve_window` / `window_state` / `bring_to_front` / `capture`） | 架构 v2 §13.2 |
| 3 | `src/uia/**`：`UiAutomationProvider` 的 13 方法（`snapshot_tree` / `resolve_element` / `wait_for` / `read_text` / `set_value` / `edit_text` / `invoke_action` / `select` / `scroll` / `pointer_action` / `key_action` / `fingerprint`；后两个见 Q2） | 架构 v2 §13.1.1 |
| 4 | **三类错误可区分**：`TargetNotFound` / `AmbiguousTarget` / `TargetNotResponding`（复用 `crates/protocol` 的 `ErrorCode`，**不新增**） | 批次表 A2 验收要点 |
| 5 | **selector 链解析**：按 ADR-0022 的候选链 + 评分 + `locale_dependent` 标记 | ADR-0022 + 架构 v2 §6.2 |
| 6 | **指纹计算**（`FingerprintScope` → `Fingerprint`） | 架构 v2 §7.3 |
| 7 | **树快照**（`TreeOptions` → `TreeSnapshot`）+ **实测性能预算** | 架构 v2 §13.2 + Spike A |
| 8 | `crates/platform/windows/README.md`（职责 / 边界 / **不变量** / 已知限制） | gov §5.4 + 阶段 1 DoD |
| 9 | 单测：正向 + **负向**（ADR-0019 N1）；**非 Windows 平台不依赖真实应用** | gov §5.5 + ADR-0019 |
| 10 | 真机手工验收记录（真实记事本，Windows 11 25H2）—— 写进执行记录 §3 | 批次表 A2 验收要点 |

**Out of scope（做了算漂移）**

- `src/input/**`（SendInput / `keybd_event` / `SendKeys`）与 `src/coordinates/**`（DPI / 多屏归一化）与 IME → **TASK-018**
- Host IPC / element 不出进程的传输机制 → TASK-019（本卡只保证**句柄类型不序列化**）
- CDP / Edge / Chrome → TASK-048；Excel / Word / Photoshop Adapter → 阶段 2 / 3
- 策略判定与白名单 → TASK-021；租约 → TASK-025；撤销 → TASK-024；后置断言引擎 → TASK-023
- 截图脱敏 / 视觉验证 / 视觉兜底 → TASK-041 / 042
- 真实记事本的 **3 个任务闭环**（T1.1 ~ T1.3）→ TASK-035 ~ 038
- 任何 `#[allow]` 放宽（漂移触发器 ⑥）
- 新增**未登记**的第三方依赖（含 `tokio` / `async-trait` / `uiautomation`）→ 漂移触发器 ①
- 改 `crates/platform/api/**`（trait 形状**冻结**；确需改 → DRIFT，触发器 ③）
- 改 `crates/core/**`、`crates/core/tests/arch*`（不在 write scope）

**必须遵守**

1. **铁律 7**：`unsafe` / FFI **只允许**在本 crate（`crates/platform/*`）。每个 `unsafe` 块必须有 `// SAFETY:` 说明（gov §5.2）。
2. **铁律 8**：`ResolvedWindow` / `ResolvedElement` **不得**派生 `Serialize` / `Deserialize`，不得把 HWND / COM 指针放进可序列化结构。
3. **铁律 1 + 铁律 4（无静默失败）**：`set_value` / `edit_text` / `invoke_action` / `select` / `scroll` 是**写操作** → 每个都必须**回读验证 postcondition**；验证失败 → 返回带 `ErrorCode` 的错误，**不得**返回 `Ok`。
4. **COM 纪律**（写进 README「已知限制」）：线程须 `CoInitializeEx(COINIT_APARTMENTTHREADED)`；`IUIAutomation` 实例与 element **不得跨线程**；跨进程 element 的生命周期用 `ResolvedElement` 的**不透明句柄**表达，**不做缓存**（缓存归 TASK-025）。
5. **ADR-0022（selector 稳定性）**：**禁止**用本地化属性作主 selector —— 禁令覆盖 `Name` / `AccessKey` / `AcceleratorKey` / `LocalizedControlType` / `ItemStatus`（这 5 项只能作最低分兜底并标 `locale_dependent`）。
6. **非 Windows 平台必须仍能 `cargo test --workspace` 通过**（CI 有 macOS / ubuntu runner）：平台代码一律 `#[cfg(windows)]` 门控，非 Windows 提供**明确的** `PlatformError`（语义 = 本通道不可用），**不得**用 `unimplemented!()` / `todo!()`（铁律 1）。
7. **`windows` crate 的 feature 名易错**（ADR-0024 D1a 实证）：UI Automation 在 **`Win32_UI_Accessibility`**（**不是** `Win32_UI_UIAutomation`）；以 `spikes/spike-a-notepad/Cargo.toml` 的已验证集合为起点。
8. **命名按 `docs/spec/naming.md`**：受控词汇 `Target` / `Fingerprint` / `Lease`；`ResolvedWindow` / `ResolvedElement` 是架构 v2 §13.1.1 的既有术语，保留。
9. **纯函数优先**：selector 评分 / 指纹组装 / 错误分类必须是**纯函数**（可单测、不碰 COM）。
10. **不得为让门禁变绿而放宽判据或改断言**（AGENTS.md §4 ⑦）。
11. 单文件 ≤ 400 行（软）/ 600（硬）；函数 ≤ 80 行；参数 ≤ 6 个。
12. **性能预算必须有实测值**（不得只写「应该够快」）：树遍历在真实记事本上的耗时写进执行记录 §3，并与 Spike A 的预算对比。

**步骤**

1. **环境记录**（OS build / `rustc` / `windows` crate 版本 / 真实记事本版本）—— 写进执行记录 §1。
2. **先读再写**：架构 v2 §13.1.1 / §13.2 / §6.2 / §7.3 + `docs/spec/capability-matrix.md` + `docs/spec/naming.md` §7 + `docs/memory/apps/notepad.md` 全文 + `docs/memory/win32-input-research.md`。**契约先行**：若发现 spec 与架构 v2 冲突 → 记 DRIFT（触发器 ⑧），不要自己挑一个。
3. **建 crate 骨架**：`Cargo.toml` / `README.md` / `src/lib.rs`；`Cargo.toml` 继承 workspace（`edition.workspace = true`、`lints.workspace = true`）。⚠ 根 `Cargo.toml` 的 `members` 已含 `crates/platform/api`、`exclude` 含 `crates/platform` → **新增子 crate 后必须把 `crates/platform/windows` 也加进 `members`**（否则 workspace 不加载它 = 交付物不可构建）。这一行**在 write scope 外**，按 DRIFT 登记（同 DRIFT-016-2 的形态）。
4. **先做 `window` 域、再做 `uia` 域**（后者依赖前者的 HWND → element 转换）。
5. **单测**：纯逻辑（selector 评分 / 错误分类 / 指纹组装）用普通单测；COM 相关用 `#[cfg(windows)]` + **COM 可用性探测**（拿不到 `IUIAutomation` 就 skip 并**显式打印原因**，不得静默通过）。
6. **跑门禁**（见下）；不合格 → DRIFT（`DRIFT-017-x`）。
7. **真机手工验收**（人类在本机跑）：真实记事本上复现 Spike A 指标 + 树遍历耗时 + 三类错误各一例；结果贴进执行记录 §3。
8. **填执行记录**（9 节）+ 更新 `LEDGER.md` + `plans/*` 的完成标记与进度句 + 记忆（新事实 / 坑）。

**待裁决（动手前必须有答案）**

- **Q1（漂移触发器 ①，最重要）**：`docs/DEPENDENCIES.md` 的 `windows` 行状态是 **`Approved for spikes`**，并明写「**产品侧待阶段 1 走漂移升级**」。本卡要把该 crate 用于**产品代码**（`crates/platform/windows`）。
  **建议默认**：把该行状态改为 `Approved`（产品侧）；「使用方」列由 `spikes/spike-a-notepad`（阶段 0）扩为 `spikes/spike-a-notepad`（阶段 0）＋ `crates/platform/windows`（阶段 1）；「批准人 / 日期」列追加本次。**版本保持 `=0.62.2` 不变**（升级 = 漂移触发器）。
  **代价**：这是**改依赖登记表的批准状态**，属需要人类点头的事项 → 未点头前本卡**不开始**。
- **Q2（trait 形状 vs 卡面边界）**：`UiAutomationProvider` 带 `pointer_action` / `key_action` 两个**合成输入**方法，而合成输入按批次表归 **TASK-018**（`src/input/**`）。
  **建议默认**：本卡**只**实现到「明确报错」的程度 —— 返回 `PlatformError`（`ErrorCode` 复用现有枚举，语义 = 本通道尚未实现），源码标 `// STUB(TASK-018):`（gov §5.2 要求带卡号）+ README「已知限制」写明；TASK-018 落地时替换。
- **Q3（截图）**：`WindowProvider::capture` 需要屏幕截图，而阶段 1 的截图 / 脱敏 / 视觉验证按批次表归 **TASK-041 / 042**。
  **建议默认**：同 Q2 —— 本卡 `capture` 返回明确的未实现错误 + `// STUB(TASK-041):`；**不**引入截图依赖。
- **Q4（真机验收归属）**：CI 的三平台矩阵里只有 `windows-latest` 能跑 UIA，且**没有真实记事本 fixture**（靶机 `notepad-like` 归 TASK-033）。
  **建议默认**：单测**不依赖真实应用**（纯逻辑 + COM 可用性探测）；「Spike A 指标在真实记事本上复现」改为**人类在本机跑的手工验收**，结果贴进执行记录 §3，**不作为 CI 门禁**。

**未裁决时的处理**

- Q1 未点头 → **不开工**（依赖登记状态是「产品侧引入第三方依赖」的前提，触发器 ① 明确要求「停下并升级」）。
- Q2 / Q3 / Q4 未答复 → 按**建议默认**执行（三者都是「本卡不扩范围」的保守方向），并在执行记录 §5 登记为「按建议默认」。

**DoD**

- [ ] `crates/platform/windows` 落地，且 `Cargo.toml` 只依赖 `assistant-platform-api` + 已登记的 `windows` crate
- [ ] `WindowProvider` 的 5 方法全部实现（或按 Q3 裁决**显式报错**并带 `// STUB(TASK-NNN):`）
- [ ] `UiAutomationProvider` 的 13 方法全部实现（或按 Q2 裁决**显式报错**并带 `// STUB(TASK-NNN):`）
- [ ] 三类错误**可区分**且有单测：`TargetNotFound` / `AmbiguousTarget` / `TargetNotResponding`
- [ ] 写操作（`set_value` / `edit_text` / `invoke_action` / `select` / `scroll`）**每个都有 postcondition 回读验证**（铁律 4）
- [ ] `ResolvedWindow` / `ResolvedElement` **未**派生 `Serialize` / `Deserialize`（铁律 8）
- [ ] 每个 `unsafe` 块有 `// SAFETY:` 说明；**无任何** `#[allow]` 放宽
- [ ] 非 Windows 平台 `cargo test --workspace` 仍全绿（`#[cfg(windows)]` 门控 + 明确错误；无 `todo!()` / `unimplemented!()`）
- [ ] selector 链**未**使用 5 项本地化属性作主 selector（ADR-0022）
- [ ] 树遍历有**实测**耗时并写进执行记录 §3
- [ ] `crates/platform/windows/README.md` 含职责 / 边界 / 不变量 / 已知限制
- [ ] 单测含**负向**用例（ADR-0019 N1）
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿；`cargo test -p assistant-platform-windows` 全绿
- [ ] `cargo test -p assistant-core arch::` 仍全绿（本卡**不**改 arch 断言；若它因本卡变红 → 说明越界，停下）
- [ ] `xtask hygiene / memory-counts / adr-index / docscan / card-check / check-ledger / check-migrations` 全部 PASSED
- [ ] `docs/DEPENDENCIES.md` 的 `windows` 行按 Q1 裁决更新
- [ ] `LEDGER.md` 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`（应用专属的进 `docs/memory/apps/notepad.md`）
- [ ] 无任何 Out of scope 的文件被修改（write scope 外的连带改动全部登记为 DRIFT）

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-platform-windows
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-migrations
cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
（Implementer 开工前填写；格式见 AGENTS.md §3）
```

### 2. 实际改动文件

（待填）

### 3. 验收输出摘要

（待填）

### 4. DoD 逐条核对

（待填）

### 5. 偏差

（待填）

### 6. 更合理做法

（待填）

### 7. 遗留问题

（待填）

### 8. 新增长期记忆

（待填）

### 9. 给审阅者的关注点

（待填）

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
【任务】TASK-017　platform/windows：UIA provider（树快照 / selector 链解析 / read_text / set_value /
        edit_text / invoke_action / bounds / fingerprint / window 枚举与状态）
【目标】新建 crates/platform/windows（crate 名 assistant-platform-windows），把 TASK-016 定下的
        WindowProvider（5 方法）+ UiAutomationProvider（13 方法）落到真实的 Win32 / UIA3 客户端 COM 上
【write scope】仅：crates/platform/windows/**、docs/DEPENDENCIES.md（仅 `windows` 行）
【铁律】1（无静默失败）· 4（写操作必有 postcondition）· 7（FFI 只许在 crates/platform/*）·
        8（句柄不得跨进程）· 5（API 优先 —— 本卡不做合成输入）
【禁止】src/input/** 与 src/coordinates/** 与 IME（→018）· Host IPC（→019）· CDP / Edge（→048）·
        策略（→021）· 租约（→025）/ 撤销（→024）/ 后置断言引擎（→023）·
        真实记事本 3 任务闭环（→035~038）
【验收】cargo fmt --all --check → 0 diff；cargo clippy --all-targets -- -D warnings → exit 0；
        cargo test -p assistant-platform-windows → 全绿；cargo test --workspace → 全绿；
        cargo test -p assistant-core arch:: → 5 passed；cargo run -p xtask -- verify-schemas /
        codegen --check / hygiene / memory-counts / adr-index / docscan / card-check /
        check-ledger / check-migrations → 全部 PASSED；cargo deny check → 全 ok
【依赖】TASK-016 —— 已核对 LEDGER（Done，2026-09-24）
【疑问】Q1（依赖登记状态升级）由人类 2026-09-24 明确批准（「1、TASK-012 的 3 个依赖全部同意」之后的
        同类授权 + 本轮「按你建议的去做」）；Q2 / Q3 / Q4 未单独答复 → 按卡面「建议默认」执行
        （三者都是「本卡不扩范围」的保守方向），见 §5。
```

### 2. 实际改动文件

| 文件 | 说明 | write scope |
|---|---|---|
| `crates/platform/windows/Cargo.toml` | 新建。`assistant-platform-windows`；`windows = "=0.62.2"` 按 `cfg(windows)` **target-gated**（macOS / ubuntu runner 不解析该依赖）；只依赖 `assistant-platform-api` + 已登记的 `windows` | ✅ |
| `crates/platform/windows/README.md` | 新建。职责 / 边界 / 不变量 / 已知限制 / **真机实测性能表** | ✅ |
| `crates/platform/windows/src/lib.rs` | 新建。crate 头 + crate 级 `#![allow(unsafe_code)]`（DRIFT-017-1）+ 平台门控 mod 声明 | ✅ |
| `.../src/digest.rs` | 新建。零依赖 SHA-256（FIPS 180-4）+ 55/56/63/64/65 字节边界向量（DRIFT-017-3 / -6） | ✅ |
| `.../src/error.rs` | 新建。HRESULT / Win32 码 → `ErrorCode` 的**纯函数**映射；含 `error_from_win32_failure`（真 bug #2 的修法） | ✅ |
| `.../src/selector.rs` | 新建。候选有效分排序 + 歧义裁决 + 结论→`PlatformError`（全纯函数） | ✅ |
| `.../src/com.rs` | 新建。线程本地 COM apartment + `IUIAutomation`（「COM 不跨线程」的**唯一**守卫） | ✅ |
| `.../src/handles.rs` | 新建。HWND ↔ 句柄编解码 + **线程本地**元素表（含跨线程负向用例） | ✅ |
| `.../src/win32.rs` | 新建。`EnumWindows` / 窗口属性 / 进程身份（映像名叶子）/ 前台与遮挡判定 | ✅ |
| `.../src/window/mod.rs`、`.../src/window/candidates.rs` | 新建。`WindowProvider` 5 方法 + 窗口候选**纯函数**匹配 | ✅ |
| `.../src/uia/mod.rs` | 新建。`UiAutomationProvider` 13 方法 + control type **单一事实源**表（真 bug #1）+ `RuntimeId` 读取 | ✅ |
| `.../src/uia/{tree,search,resolve,patterns,actions}.rs` | 新建。树遍历 / 指纹、候选搜索、解析与等待、读 / 写文本、动作类写操作（含回读后置条件） | ✅ |
| `.../src/unsupported.rs` | 新建。非 Windows 后端（每个方法都返回 `CapabilityMissing`） | ✅ |
| `.../tests/handle_discipline.rs` | 新建。铁律 8 的机器校验（扫 `src/**/*.rs` 代码行 + 断言 `Cargo.toml` 不引 `serde`；含 5 个负向 / 对照用例） | ✅ |
| `docs/DEPENDENCIES.md` | 改 `windows` 行：状态 `Approved for spikes` → **`Approved`**（产品侧）；使用方补 `crates/platform/windows`（阶段 1）；批准人 / 日期追加 | ✅（卡面显式列入） |
| 根 `Cargo.toml` | `members` 加 `crates/platform/windows` | ⚠ **write scope 外** → DRIFT-017-2（卡面步骤 3 已显式预告此形态） |
| `crates/platform/api/src/lib.rs`、`src/traits/mod.rs` | 补 `SelectorChain` re-export（纯增量，不改 trait 形状） | ⚠ **write scope 外** → DRIFT-017-5 |
| `docs/PARKING_LOT.md` | 追加 PL-068 / PL-069 主表行 + 1 条 PL-068「处置追加记录」（公共热点文件，按 ADR-0028 取锁后写、写完立刻释放） | ✅（§8 允许追加） |
| `LEDGER.md`、`PLAN.md`（仅「当前状态」块）、`README.md`（仅三处）、`plans/stage-1-pilots.md`（仅完成标记 + 进度句）、`docs/memory/{facts,pitfalls}.md`、`MEMORY.md` §1 规模表 | 按 AGENTS.md §11.1 同步（每张卡 Done 必做） | ✅ |

### 3. 验收输出摘要

**本机环境**：Windows 11 25H2 build **26200.9457**（x64）· rustc / cargo **1.98.1**（stable-msvc）·
`windows` crate **0.62.2**（`Win32_UI_Accessibility` + `Win32_System_Ole`，ADR-0024 D1a）·
真实记事本 **Microsoft.WindowsNotepad 11.2607.14.0**（打包版，含标签栏）。

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | **0 diff**（exit 0） |
| `cargo clippy --all-targets -- -D warnings` | **exit 0**（crate 内零 `#[allow]`，唯一例外 = `lib.rs` 的 crate 级 `unsafe_code`，DRIFT-017-1） |
| `cargo test -p assistant-platform-windows` | **57 + 7 passed / 0 failed**，doctest **1 passed** |
| `cargo test --workspace` | **31 个 test target / 579 passed / 0 failed**（TASK-016 之后 = 28 / 514 → **+3 target / +65 tests**，全部来自本 crate） |
| `cargo test -p assistant-core arch::` | **5 passed**（本卡未改 arch 断言，未变红） |
| `xtask verify-schemas` | PASSED（0 error） |
| `xtask codegen --check` | PASSED（0 drift / 0 error） |
| `xtask hygiene` | PASSED（**115 文件 / 0 error / 3 warning** = 既有 baseline 的 3 个超长文件） |
| `xtask memory-counts` | PASSED（8 / 0e / 0w） |
| `xtask adr-index` | PASSED（26 / 0e / 0w） |
| `xtask docscan` | PASSED（163 / 0e / 563w —— 与 TASK-016 基线 572w 相比减少，因本卡把执行记录整节填满） |
| `xtask card-check` | PASSED（91 / 0e / 49w；本卡**正文区未动一个字节**） |
| `xtask check-ledger` | PASSED（`current_stage=阶段 1`） |
| `xtask check-migrations` | PASSED（2 个含迁移的 crate） |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |

**真机手工验收（Q4 裁决：不作为 CI 门禁；本卡**实际执行**并留证）**

树遍历 / 定位 / 读文本（真实记事本，`TreeOptions::new(None, true)`，两次独立运行）：

| 操作 | 实测中位数（min / max） | 与 Spike A 预算对比 |
|---|---|---|
| `snapshot_tree`（**38** 节点，含离屏） | **30.5 / 39.8 ms**（n=5；min 28.8 / 32.5，max 154.1 / 144.7） | 预算 ≤ 800 ms → **20× 以上余量**；Spike A 在 32 节点上写 17 ms（复跑 15.3 ms）→ 本卡慢约 1.8~2.3×，**同数量级** |
| `fingerprint(WholeWindow)` | **28.7 / 31.6 ms** | 同上（它 = 一次完整遍历 + SHA-256） |
| `resolve_element`（`ClassAndRole` = `RichEditD2DPT` + `Document`） | **1531.5 / 1530.4 ms**（n=10；min 1520.4 / 1520.9，max 1730.9 / 1712.2） | ⚠ **超出** Spike A 的「局部搜索 ≤ 200 ms」判据 **7.6×** —— 根因见下 |
| `read_text`（`ValuePattern`） | **0.22 ms**（空文档，`chars=0`） | 与 Spike A 的 0.11 ms 同数量级 |

**关键解读（必须与上表一起读）**：Spike A 的 1.2~1.5 ms 是在**窗口子树内**搜索测出来的；
本卡的 `resolve_element` 受 TASK-016 冻结的 trait 形状所限，只能从**桌面根**发
`FindAll(TreeScope_Descendants)`（**PL-068**）。同一次验收里 `snapshot_tree` 只有 30~40 ms →
**慢的不是 UIA，而是「从桌面根走遍所有窗口」**。1.5 s **不是**性能达标的证据，是 PL-068 的代价证据。

**失败分类在真机上各复现一例**（铁律 1：可区分、不混淆）：

| 触发 | 实际返回 |
|---|---|
| 失效窗口句柄（伪造 `LocalHandleId`） | `TargetNotFound` ✅ |
| `AutomationId` 不存在的元素 | `TargetNotFound` ✅ |
| 未知角色名（`role = "NotARole"`） | `CapabilityMissing` ✅（配置错误 ≠ 找不到） |
| 低质量兜底 selector（`TitleRegex` 子串 `e`）多命中 | **`TargetAmbiguous`** ✅（不静默取第一个） |
| `wait_for` 超时（120 ms，`AutomationId` 不存在） | `TargetUnresponsive` ✅ |

### 4. DoD 逐条核对

- [x] `crates/platform/windows` 落地，`Cargo.toml` 只依赖 `assistant-platform-api` + 已登记的 `windows`
- [x] `WindowProvider` 5 方法全部实现（`capture` 按 Q3 裁决显式报错 + 占位实现标记 `TASK-041`）
- [x] `UiAutomationProvider` 13 方法全部实现（`pointer_action` / `key_action` 按 Q2 裁决显式报错 + 标记 `TASK-018`）
- [x] 三类错误**可区分**且有单测：`TargetNotFound` / `TargetAmbiguous` / `TargetUnresponsive`（+ 真机各一例）
- [x] 写操作（`set_value` / `edit_text` / `invoke_action` / `select` / `scroll`）**每个都有回读后置条件**；
      `invoke` 只能做**弱**后置条件（元素仍可达），应用语义归 TASK-023 —— 已在源码模块头 + README 如实写明
- [x] `ResolvedWindow` / `ResolvedElement` **未**派生 `Serialize` / `Deserialize`（新增 `tests/handle_discipline.rs` 机器校验 + 5 个负向 / 对照用例）
- [x] 每个 `unsafe` 块有 `// SAFETY:` 说明；**无** `#[allow]` 放宽（唯一例外 = crate 级 `unsafe_code`，DRIFT-017-1）
- [x] 非 Windows 平台：`src/unsupported.rs` 提供明确 `CapabilityMissing`；无 `todo!()` / `unimplemented!()`；
      `windows` 依赖 `target-gated` → macOS / ubuntu runner 可编译可跑测试
- [x] selector 链**未**用 5 项本地化属性作主 selector（`Name` / `AccessKey` / `AcceleratorKey` /
      `LocalizedControlType` / `ItemStatus` 只作最低分兜底 + `locale_dependent` 降权 ×0.5）
- [x] 树遍历有**实测**耗时（30.5 / 39.8 ms 中位数）并写进 §3
- [x] `crates/platform/windows/README.md` 含职责 / 边界 / 不变量 / 已知限制
- [x] 单测含**负向**用例（ADR-0019 N1）：错误分类、歧义并列、空取值、未知角色、跨线程句柄、空链、
      越界文本偏移、`unsupported` 后端、扫描器坏样本
- [x] `cargo fmt --all --check` 0 diff
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [x] `cargo test --workspace` 全绿；`cargo test -p assistant-platform-windows` 全绿
- [x] `cargo test -p assistant-core arch::` 仍全绿（未改 arch 断言）
- [x] `xtask hygiene / memory-counts / adr-index / docscan / card-check / check-ledger / check-migrations` 全部 PASSED
- [x] `docs/DEPENDENCIES.md` 的 `windows` 行按 Q1 裁决更新
- [x] `LEDGER.md` 追加一行；`docs/memory/{facts,pitfalls}.md` 追加新事实 / 坑（见 §8）
- [x] 无 Out of scope 的文件被修改（write scope 外的 3 处连带改动全部登记为 DRIFT-017-2 / -5，见 §5）

### 5. 偏差

**Q2 / Q3 / Q4 按建议默认**（未单独答复 → 执行卡面「建议默认」的保守方向）：

- Q2：`pointer_action` / `key_action` 只实现到「明确报错」—— `CapabilityMissing` + 占位实现标记 `TASK-018`。
- Q3：`capture` 同型 —— `CapabilityMissing` + 占位实现标记 `TASK-041`。
- Q4：单测不依赖真实应用；真机复现作为**手工验收**（本卡实际执行，见 §3），不作为 CI 门禁。

**DRIFT 登记（7 条）**

| 编号 | 触发器 | 内容 | 处理 |
|---|---|---|---|
| **DRIFT-017-1** | ⑥（放宽 lint） | `src/lib.rs` 的 **crate 级** `#![allow(unsafe_code)]` | 铁律 7 指定的唯一 FFI 层：调 `windows` 必须 `unsafe`。已排除 `[lints] rust.unsafe_code = "allow"`（Cargo 报 `cannot override workspace.lints in lints`）。**取 crate 级一次**而不是散落几十处 `#[allow]`；每个 `unsafe` 块仍有 `// SAFETY:` |
| **DRIFT-017-2** | ⑤（超 write scope） | 根 `Cargo.toml` 的 `members` 加 `crates/platform/windows` | 卡面步骤 3 **显式预告**此形态（同 DRIFT-016-2）。不加则 workspace 不加载本 crate = 交付物不可构建 |
| **DRIFT-017-3** | ①（加依赖） | **自实现 SHA-256**（`src/digest.rs`）而不是引 `sha2` | 引 `sha2` 要改 `DEPENDENCIES.md` **另一行** + 触发器 ①。自实现是纯函数 → **三平台** CI 都能跑 FIPS 向量（含 55/56/63/64/65 字节边界） |
| **DRIFT-017-4** | ①（加依赖） | `TitleRegex` / `NameRegex` 用 UIA 原生**子串**匹配，不是 regex | 无 regex 引擎（引它 = 触发器 ①）。语义偏差**只可能漏命中，不会假命中**（漏命中 → 链继续走 → 最终 `TargetNotFound`，明确失败）；已在源码 + README 写明 |
| **DRIFT-017-5** | ③（改公共接口） | `crates/platform/api` 补 `SelectorChain` re-export（`lib.rs` + `traits/mod.rs`） | TASK-016 遗漏：`SelectorChain` 是 `UiAutomationProvider::resolve_element` 的**参数类型**，却**没**从 `lib.rs` 导出 → **crate 外无法实现该 trait**。修法是**纯增量**的 `pub use`，不改任何形状 / 签名 / 语义 |
| **DRIFT-017-6** | ⑦（改测试断言） | 修 **2 处错误的测试期望**：`digest.rs` 的 56 字节向量、`error.rs` 的十六进制断言 | **不是**为了让门禁变绿而放宽判据：① 56 字节真值由 `CPython hashlib.sha256` **独立复算**为 `b35439a4…6738a`（原期望 `b35439b2…` 是错的，**实现是对的**）；② `-1_073_741_824` 的补码位模式是 `0xC000_0000`（原断言 `40000000` 是正号位模式）。同时**补了** 63 / 65 字节边界用例与 Win32 码逆变换回归 |
| **DRIFT-017-7** | ⑧（代码与 ADR 矛盾） | `resolve_element` 的 `FindAll` **直接**对桌面根用 `TreeScope_Descendants`，与 **ADR-0022 E6** 引用的官方要求（桌面上找顶层窗口必须用 `TreeScope_Children`；`Descendants` 可能让 provider 栈溢出）不一致 | 根因 = PL-068（trait 无 scope 参数）。**本卡未自行改搜索策略**（那会改歧义判定语义 + 超出卡范围）。**已修掉源码里两条「声称遵守 E6」的错误注释**（`uia/search.rs` / `uia/resolve.rs` 的模块头现在如实区分两条路径），并**实测**代价（1.53 s 中位数）留证。**等裁决** |

**顺带登记（不属 DRIFT，属「不相关的问题一行记进 PARKING_LOT」）**

- **PL-068**：`ElementQuery` / `SelectorChain` 缺 scope 参数（trait 形状缺口）→ 已在本 crate 源码 + README 如实登记，并追加**实测证据行**（1.53 s）。
- **PL-069**：歧义策略口径缺口 —— 架构 v2 §6.6 有 **4** 种策略，`OnAmbiguous` 只有 **2** 个变体；
  且 `HighestScore` 在本层**等价于报歧义**（候选链的分数属于候选而非命中元素 → 每个候选的命中分数恒等）。
  已在 README「已知限制」写明，**未**自行改契约。

**本会话发现并修掉的真实缺陷（3 条，全部在 write scope 内）**

1. **control type 正反两份手写表漂移**：正向 `match` 39 支、反向数组 38 项，**漏了 `TreeItem`**
   → `control_type_from_role("TreeItem")` 返回 `None`，`wait_for(role="TreeItem")` 会误报 `ToolInvalidArgs`。
   改为**单一事实源** `CONTROL_TYPE_NAMES: [(UIA_CONTROLTYPE_ID, &str); 39]` + 逐项遍历的回归用例。
2. **`EnumWindows` 失败走错映射**：Win32 码 1400（`ERROR_INVALID_WINDOW_HANDLE`）经 `error_from_hresult`
   落进「未识别」分支变 `Fatal`，实际语义是 `TargetNotFound`。新增 `error_from_win32_failure`（只用于
   **确认是 Win32 API** 的调用点；COM / UIA 调用点仍走 `error_from_hresult`）+ 2 个回归用例。
3. **两条「声称遵守 ADR-0022 E6」的错误注释**（见 DRIFT-017-7）+ **2 处重复的文档段落**
   （`digest.rs` / `uia/tree.rs`）—— 文档与代码矛盾属缺陷，已改措辞使其与代码一致。

### 6. 更合理做法

1. **`resolve_element` 的性能根因不在本 crate，而在 trait 形状**。本次真机实测把这一点量化了
   （桌面根 1.53 s vs 窗口子树 1.2~1.5 ms，**约 1000×**）。若当时允许改 TASK-016 的形状，
   正确解法是给 `resolve_element` 加 scope（`&ResolvedWindow` / 父元素），让它像 Spike A 那样
   在**窗口子树内**搜索 —— 而不是在本层用启发式「先 Children 再 Descendants」去绕（那样仍无法
   覆盖"元素在某个窗口内部"的常见情形）。**这正是 PL-068 该被优先裁决的理由**。
2. **`OnAmbiguous::HighestScore` 的语义应当在 ADR 层重写**：分数属于**候选**，不属于命中元素，
   所以"多命中取最高分"在候选链模型里没有定义。架构 v2 §6.6 的 `first_by_order` 才是可实现的形态
   （并在审计里标记"使用了歧义解析"）。见 PL-069。
3. **手工验收值得自动化一半**：本卡的真机验收用了一个**临时** `examples/` 程序（跑完即删）。
   它证明"真机复现"可以机械化 —— 建议 TASK-033（靶机 `notepad-like` fixture）落地后，
   把这类测量做成 `#[ignore]` 的真机用例（带卡号原因），而不是每次靠人手跑。
4. **`stub_not_implemented` 的"卡号"参数用 `&str` 而不是枚举**：本卡只有 2 个调用点，
   用字符串足够；若调用点变多，应换成 `enum OwningCard { TASK018, TASK041 }` 之类的受控集合，
   以免出现"卡号写错但编译通过"的静默漂移。

### 7. 遗留问题

1. **PL-068 / DRIFT-017-7（最高优先）**：`resolve_element` / `wait_for` 无 scope → 只能从桌面根搜索。
   实测代价 1.53 s，且与 ADR-0022 E6 的官方要求不一致。**需要 ADR 裁决**（scope 参数 + E6 合规的搜索顺序）。
   在此之前，真实使用场景（"先定窗口、再定元素"）**不可达** —— 上层只能自己把窗口句柄塞进候选链。
2. **PL-069**：歧义策略口径（4 种 vs 2 个变体；`HighestScore` 等价于报歧义）→ 需 ADR。
3. **`wait_for` 不可取消**：架构 v2 §6.5 要求"等待期间可取消"，本层只有固定 50 ms 轮询 + 截止时间；
   取消通道归 Host（TASK-019）。
4. **元素句柄表无上限**：`src/handles.rs` 的线程本地表只在**线程退出**时释放；长驻 Host 线程反复解析会累积
   COM 引用。淘汰 / TTL 归 TASK-025（本卡刻意不做缓存）。
5. **`invoke` 的应用语义无通用判据**：本层只能验证"元素仍可达"。真正的后置断言归 TASK-023；
   在那之前，`invoke_action("invoke")` 的返回值**不能**当作"应用语义已生效"。
6. **`is_occluded` 是近似判定**（窗口矩形中心点 + `WindowFromPoint`）：部分遮挡 / 非矩形窗口 / 透明覆盖层会误判。
   它**偏保守**（宁可报"被遮挡"）。逐像素或 `DwmGetWindowAttribute` 归后续卡。
7. **`windows` crate 的 feature 集刻意最小**：未开 `Win32_UI_HiDpi`（ADR-0022 D8 归 TASK-018）与
   `Win32_UI_Input_KeyboardAndMouse`（L4 合成输入归 TASK-018）。
8. **本卡的实测值只覆盖记事本的 38 节点树**：Excel / Photoshop 的树可能是数千到数万节点
   （`MAX_TRAVERSAL_NODES = 20_000` 是防御性上界，超限报 `TargetUnresponsive`，**不**返回部分快照）。
   大树上的真实代价**未测**。

### 8. 新增长期记忆

已追加 `docs/memory/facts.md`（3 条）与 `docs/memory/pitfalls.md`（1 条）：

- **FACT**：`windows` 0.62.2 把 COM 接口投影成裸 `NonNull<c_void>` 持有者，**不实现 `Send` / `Sync`**
  → `IUIAutomation` / `IUIAutomationElement` **不能**放进 `Send + Sync` 的 provider 结构体；
  正确解法 = 实例住**线程本地**（本 crate 的 `com::APARTMENT` / `handles::ELEMENTS`）。
- **FACT**：TASK-017 之后的新基线 = `cargo test --workspace` **31 target / 579 passed / 0 failed**
  （TASK-016 之后 = 28 / 514）→ +3 target / +65 tests 全部来自 `crates/platform/windows`。
- **FACT**：**从桌面根**发 `FindAll(TreeScope_Descendants)` 的实测代价 = **中位数 1.53 s**（n=10），
  而同一次验收里窗口子树内的树遍历只有 30~40 ms（38 节点）→ 慢的是"走遍桌面"，不是 UIA。
- **PITFALL**：`hygiene/missing-card-reference` 会拦**注释里对标记格式本身的引用** ——
  在 `//` 注释里写「标记格式是 `STUB(TASK-NNN):`」会被判 Error（因为 `STUB` 后不是真实卡号）。
  **与 PL-004 同型**；既有实践是**改措辞绕开字面量**（本卡改为「占位实现标记」+ 指向 `naming.md` §8）。

### 9. 给审阅者的关注点

1. **`resolve_element` 的 1.53 s（最高风险）**：这不是"慢一点"，而是**架构层缺口**的可观测后果
   （PL-068 / DRIFT-017-7）。请重点复核两件事：① 本卡**没有**偷偷改搜索策略（避免掩盖缺口）；
   ② 源码里已不存在"声称遵守 E6"的错误注释。若审阅者认为应当**在本卡内**改成"窗口子树内搜索"，
   那需要先裁决 PL-068 —— 那会改 `resolve_element` 的语义（歧义判定的作用域变了）。
2. **`unsafe` 面（安全）**：全 crate 有大量 FFI。请按 `// SAFETY:` 逐块抽查三类最容易出错的：
   ① `SAFEARRAY` 的 `AccessData` / `UnaccessData` / `Destroy` 配对（`uia/mod.rs::read_i32_safearray`）；
   ② `EnumWindows` 回调里把 `LPARAM` 还原成 `*mut Vec<HWND>`（`win32.rs::collect_window`）；
   ③ `GetWindowTextW` / `GetClassNameW` / `QueryFullProcessImageNameW` 的缓冲区长度与"已写入"语义
   （`win32.rs`：长度一律来自 API 返回值 + 上界检查，**不**假定固定长度）。
3. **「弱后置条件」的边界**：`invoke_action("invoke")` 只能验证"元素仍可达"。这是本卡**唯一**没有
   强 postcondition 的写操作（铁律 4 的例外），理由与归属（TASK-023）已写进模块头 + README +
   本节。请确认这个"如实降级"是否可接受，以及它是否需要在审计里被标记为"弱验证"。
4. **DRIFT-017-5 的公共接口影响**：给 `assistant-platform-api` 补 `SelectorChain` re-export 是
   **纯增量**（`pub use`），但它确实动了 `crates/platform/api` 这个 write scope 外的文件。
   请复核：① 只加了一行 `pub use`（无形状 / 签名 / 语义变化）；② 它确实是 TASK-016 的遗漏
   （否则 crate 外无法实现 `UiAutomationProvider`）。
5. **DRIFT-017-6 的测试期望修正**：请独立复核那 2 个值（`b35439a4…` 与 `C0000000`）——
   本卡的主张是"**实现是对的，期望值写错了**"，判据是 `CPython hashlib` 独立复算 + 补码位模式。
   这条若判错方向，等于"为过门禁而改断言"。

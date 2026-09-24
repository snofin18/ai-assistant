# assistant-platform-windows

> **铁律 7 的实现侧**：把 `assistant-platform-api` 的两个 trait（`WindowProvider` 5 方法 +
> `UiAutomationProvider` 13 方法）落到**真实的 Win32 / UIA3 客户端 COM** 上。
> `core` / `Host` **永不** `use windows::…`，它们只见 trait 与纯类型；
> 分层断言见 `crates/core/tests/arch_layering.rs`（gov §5.1 门禁 #5）。

## 职责

1. **窗口域**（`src/window/**`）：`EnumWindows` 枚举可见顶层窗口 → 按 `TargetDescriptor` 的候选链
   解析**唯一**窗口 → 查询窗口状态（最小化 / 前台 / 遮挡）→ 受策略约束的前台化。
2. **元素域**（`src/uia/**`）：control view 树快照（`TreeOptions` 裁剪 + 指纹）、候选链解析、
   `wait_for` 轮询、`read_text`（`ValuePattern` → `TextPattern` 兜底）、
   **五类写操作**（`set_value` / `edit_text` / `invoke_action` / `select` / `scroll`，每个都回读）、
   状态指纹（`FingerprintScope`）。
3. **失败分类**（`src/error.rs`）：HRESULT / Win32 错误码 → `assistant_protocol::ErrorCode` 的**纯函数**映射。
4. **非 Windows 后端**（`src/unsupported.rs`）：macOS / Linux 上仍能编译，每个方法都返回带
   `ErrorCode` 的 `CapabilityMissing`（语义 = 本通道在当前平台不可用）。

## 边界（不做什么）

- **不做**合成输入（`SendInput` / `keybd_event` / `SendKeys`）与 IME → **TASK-018**；
  `pointer_action` / `key_action` 本卡**显式报错**（`CapabilityMissing` + `// STUB(TASK-018):`）。
- **不做**坐标归一化 / DPI / 多屏换算 → **TASK-018**（`src/coordinates/**`）。
- **不做**截图 / 脱敏 / 视觉兜底 → **TASK-041 / 042**；`capture` 本卡显式报错（`// STUB(TASK-041):`）。
- **不做**策略判定（放行点是 `crates/policy`，TASK-021）、**不做**租约与淘汰（TASK-025）、
  **不做**撤销（TASK-024）、**不做**跨步骤后置断言引擎（TASK-023）。
- **不缓存**：本 crate 没有 TTL / 淘汰 / 复用逻辑（缓存归 TASK-025）。
- **不新增 `ErrorCode`**：全部复用 `assistant_protocol::ErrorCode`（新增 = ADR）。
- **不定义句柄类型**：`ResolvedWindow` / `ResolvedElement` 由 `assistant-platform-api` 定义，
  本 crate 只**组装**它们。

## 不变量

1. **句柄不跨进程**（铁律 8）：句柄类型由 `assistant-platform-api` 定义且**不派生**
   `Serialize` / `Deserialize`；本 crate 也**不新增**任何可序列化的句柄类型。
   机器校验：`tests/handle_discipline.rs`（扫 `src/**/*.rs` 的代码行 + 断言 `Cargo.toml` 不引 `serde`）。
2. **COM 对象不跨线程**（本卡约束 4）：`IUIAutomation` 与 `IUIAutomationElement` 都不是
   `Send` / `Sync`，因此实例住在**线程本地**（`src/com.rs` 的 `APARTMENT`、`src/handles.rs` 的
   `ELEMENTS`）。在别的线程用元素句柄 → **明确的 `TargetNotFound`**，不是静默失败。
3. **写操作必有 postcondition**（铁律 4）：`set_value` / `edit_text` / `invoke_action` /
   `select` / `scroll` 全部回读；不一致 → `VerifyFailed`，**绝不**返回 `Ok`。
4. **未识别的失败不降级**（铁律 1）：未知 HRESULT / Win32 码 → `Fatal`（**不是** `Transient`，
   那会诱导无意义重试）；「本通道不支持该 pattern」→ `CapabilityMissing`（**不是** `Fatal`）。
5. **三类定位失败可区分**：`TargetNotFound` / `TargetAmbiguous` / `TargetUnresponsive`。
6. **本地化属性不得作主 selector**（ADR-0022 D4）：`Name` / `AccessKey` / `AcceleratorKey` /
   `LocalizedControlType` / `ItemStatus` 只作**最低分兜底**并标 `locale_dependent`
   （`src/selector.rs` 按 §6.3 自动降权 ×0.5）。
7. **纯函数优先**：候选排序 / 歧义判定 / 错误分类 / SHA-256 / 句柄编解码都是纯函数，
   三平台 CI 都能单测（不依赖真实应用，ADR-0019 N1）。
8. **遍历用显式栈**：UIA 树的深度不受本层控制，递归在深树上会**栈溢出**（abort，不是
   `Result`）→ 违反铁律 1，因此 `src/uia/tree.rs` 用显式栈。

## 测试

| 位置 | 覆盖 |
|---|---|
| `src/**` 的 `#[cfg(test)] mod tests` | 纯逻辑：错误映射、候选排序 / 歧义裁决、control type 正反表**逐项遍历**、句柄编解码、文本编辑偏移（含多字节）、`unsupported` 后端 |
| `tests/handle_discipline.rs` | **铁律 8** 的机器校验（正向 + **负向**，ADR-0019 N1） |

- **不依赖真实应用**：单测只用纯函数 + COM 可用性探测；拿不到 `IUIAutomation` 的用例
  **显式打印跳过原因**（`writeln!(stderr, ...)`），**不静默通过**。
- **零 `#[allow]`**（唯一例外见「关于 `unsafe_code`」）：`[lints]` 继承 workspace，
  `unwrap_used` / `expect_used` / `panic` / `indexing_slicing` 都是 `deny`，测试同样受限。
- **真机验收**（真实记事本，Windows 11 25H2）是**人类在本机跑的手工验收**（Q4 裁决），
  结果贴在 `tasks/TASK-017-platform-windows-uia-provider.md` §3，**不作为 CI 门禁**。

## 已知限制

- **COM 单线程**：所有 UIA 调用必须发生在**解析元素的那个线程**上（线程本地 apartment +
  线程本地元素表）。跨线程使用元素句柄会得到 `TargetNotFound`。**不提供**线程池调度或
  跨线程搬运（那需要显式的 marshal 设计，归后续卡）。
- **UIPI（完整性级别）**：目标进程完整性级别高于本进程时，`ElementFromHandle` 等调用返回
  `E_ACCESSDENIED` → `PlatformPermission`。**不**自动提权、**不**绕过。
- **无缓存**：每次属性读都是一次跨进程 COM 调用（**未使用** `IUIAutomationCacheRequest`）。
  重复读同一元素会重复付往返代价；缓存 / 淘汰归 TASK-025。
- **`invoke` 的后置条件是"弱"的**：`InvokePattern::Invoke()` 的效果是**应用语义**
  （打开对话框、提交表单…），UIA 层没有通用判据 —— 本层只能验证"元素仍可达"。
  应用语义级断言归 **TASK-023**（后置断言引擎）。其余动作
  （`toggle` / `expand` / `collapse` / `select` / `focus` / `scroll_into_view` / `scroll`）
  都有**强**后置条件（状态确实变了）。
- **`TitleRegex` / `NameRegex` 实为子串匹配**：本卡不引入 regex 引擎（加依赖 = 漂移触发器 ①），
  改用 UIA 原生 `PropertyConditionFlags_MatchSubstring`。**只可能漏命中，不会假命中**
  （漏命中 → 链继续往下走 → 最终 `TargetNotFound`，是明确失败）。
- **元素句柄表按解析次数增长，无上限**：`src/handles.rs` 的线程本地表只在**线程退出**时释放。
  长驻 Host 线程反复解析会持续累积 COM 引用 —— 淘汰策略归 TASK-025（本卡刻意不做缓存）。
- **`is_occluded` 是近似判定**：取窗口矩形中心点，看该点最上层窗口的根祖先是不是本窗口。
  部分遮挡 / 非矩形窗口 / 透明覆盖层会误判；它**偏保守**（宁可报"被遮挡"）。
- **`wait_for` 不可取消**：固定 50 ms 轮询 + 显式截止时间，**没有**取消通道
  （架构 v2 §6.5 的"等待期间可取消"归 Host / TASK-019）。
- **`resolve_element` / `wait_for` 的 scope = 已解析窗口（已解决）**：按 **ADR-0043**，两个入口的
  第一个参数都是 `&ResolvedWindow`，搜索起点是该窗口的 UIA 根元素（`ElementFromHandle`），
  **不再**从桌面根搜。`wait_for` 仍是「先 `Children` 再 `Descendants`」，但作用域改到 scope 子树内。
  原桌面根搜索实测中位数 1.53 s（见下「性能」）；**PL-068** / **DRIFT-017-7** 已关闭。
- **`capture` / `pointer_action` / `key_action` 未实现**：返回 `CapabilityMissing`，
  源码标 `// STUB(TASK-041):` / `// STUB(TASK-018):`（**不是** `todo!()` / `unimplemented!()`）。
- **非 Windows 后端**：`src/unsupported.rs` 的每个方法都返回 `CapabilityMissing`；
  平台无关的**输入校验**仍会先跑（空候选链 / 无条件 query → `ToolInvalidArgs`），
  因为坏输入在哪个平台都是坏输入。
- **歧义策略收敛为唯一一种（已解决）**：按 **ADR-0044**，`OnAmbiguous` 只保留 `ErrorAndAsk`，
  多命中一律报 `TargetAmbiguous`（fail-closed，铁律 1）；`HighestScore` 已删除（它在候选链模型里
  不可实现：分数属于**候选**，不属于命中元素）。架构 v2 §6.6 其余 3 种策略各有归属层
  （见该节「各策略的归属层」）；**PL-069** 已关闭。

## 性能（实测值）

**Spike A 预算**（`docs/spike-reports/SPIKE-A.md` §3 / §7，真实记事本 11.2607.14.0、32 节点树）：

| 操作 | Spike A 实测 | 预算 | 余量 |
|---|---|---|---|
| 全窗口树遍历（32 节点） | 17.0 ms（复跑 15.3 ms） | ≤ 800 ms | 47× |
| 局部搜索编辑区 | 1.5 ms（复跑 1.2 ms） | ≤ 200 ms | 133× |
| `ValuePattern` 读 / 写 | 0.11 ms / 4.79 ms | — | — |

**本 crate 的实测值**（2026-09-24，本机：Windows 11 25H2 build 26200.9457、rustc 1.98.1、
真实记事本 11.2607.14.0；`TreeOptions::new(None, true)`；两次独立运行）：

| 操作 | 实测中位数（min / max） | 与 Spike A 预算对比 |
|---|---|---|
| `snapshot_tree`（**38** 节点，含离屏） | **30.5 / 39.8 ms**（n=5，min 28.8 / 32.5，max 154.1 / 144.7） | ≤ 800 ms → **20× 以上余量**；比 Spike A 的 17 ms 慢约 1.8~2.3×（同数量级；节点数也从 32 涨到 38） |
| `fingerprint(WholeWindow)` | **28.7 / 31.6 ms** | 同上（它就是一次完整遍历 + SHA-256） |
| `resolve_element`（`ClassAndRole` = `RichEditD2DPT` + `Document`） | **1531.5 / 1530.4 ms**（n=10，min 1520.4 / 1520.9，max 1730.9 / 1712.2） | ⚠ **超出** Spike A 的「局部搜索 ≤ 200 ms」判据 **7.6×** —— 根因是**从桌面根**搜索（PL-068 / DRIFT-017-7），**不是** UIA 本身慢；该值是 **ADR-0043 落地前**（scope 尚未传入）的实测，落地后应复测（预期回到窗口子树量级） |
| `read_text`（`ValuePattern`） | **0.22 ms** | 与 Spike A 的 0.11 ms 同数量级 |

**失败分类在真机上各复现一例**（铁律 1：可区分、不混淆）：失效窗口句柄 → `TargetNotFound`；
`AutomationId` 不存在的元素 → `TargetNotFound`；未知角色名 → `CapabilityMissing`；
低质量兜底 selector（`TitleRegex` 子串 `e`）多命中 → **`TargetAmbiguous`**；
`wait_for` 超时 → `TargetUnresponsive`。

> 真机复现按 Q4 裁决**不作为 CI 门禁**；上表是**一次手工验收的记录**，不是回归基线。
> `resolve_element` 的 1.5 s **不是**「性能已达标」的证据 —— 恰恰相反，它是 **PL-068 的代价证据**。

## 关于 `unsafe_code`

根 `Cargo.toml` 的 `[workspace.lints.rust] unsafe_code = "deny"` 是给 core / policy / task-engine 的
默认值。本 crate 是**铁律 7 指定的唯一 FFI 层**，调用 `windows` crate 必须 `unsafe`，
因此 `src/lib.rs` **crate 级**放开一次（而不是散落几十处 `#[allow]`），且**每个** `unsafe` 块
都带 `// SAFETY:` 前置条件说明。取舍登记为 `tasks/TASK-017-*.md` §5 的 **DRIFT-017-1**。

## 相关

架构 v2 §13.1.1 / §13.2 / §6.2 / §6.3 / §6.9 / §7.3 / §3.2；**ADR-0022**（Windows 目标身份与
selector 稳定性）、**ADR-0024**（`windows` crate 与 feature 名：UI Automation 在
`Win32_UI_Accessibility`，**不是** `Win32_UI_UIAutomation`）、ADR-0019 N1（负向验证）、
`docs/spec/naming.md` §7、`docs/memory/apps/notepad.md`、`docs/memory/win32-input-research.md`、
`docs/DEPENDENCIES.md`（`windows` 行）。

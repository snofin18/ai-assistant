# ADR-0043　元素解析必须有 scope（父窗口），禁止从桌面根搜元素

状态：**Accepted**（2026-09-24，人类 chat「按你的建议去做」授权）　日期：2026-09-24　Supersedes：—　Superseded by：—
关联：架构 v2 §6.2 / §6.3 / §13.1.1、ADR-0022 D5 / E6、`crates/platform/api/src/traits/ui.rs`、`crates/platform/windows/src/uia/{resolve,search}.rs`、`docs/PARKING_LOT.md` PL-068、`tasks/TASK-017-platform-windows-uia-provider.md` DRIFT-017-7

## 背景（为什么现在要决定）

TASK-016 冻结的 `UiAutomationProvider` 里，两个元素解析入口**没有** scope 参数
（架构 v2 §13.1.1 line 1916~1917 同样没有）：

```text
async fn resolve_element(&self, chain: &SelectorChain) -> Result<ResolvedElement>;
async fn wait_for(&self, q: ElementQuery, state: ElementState, t: Timeout) -> Result<ResolvedElement>;
```

后果（TASK-017 实测，2026-09-24 真机记事本 11.2607.14.0，桌面上另有其它窗口）：

- `resolve_element` 只能从**桌面根**发 `FindAll(TreeScope_Descendants)` —— 一次走遍整个桌面，
  实测**中位数 1.53 s**（n=10；对照 Spike A 在**窗口子树内**搜索 1.2~1.5 ms，约 **1000×**）。
- 与 **ADR-0022 E6** 引用的官方要求不一致：官方文档明确「在桌面上找顶层窗口必须
  `TreeScope_Children`；用 `Descendants` 可能让 provider 栈溢出」，且最佳实践是
  「从**应用窗口或更低层的容器**开始搜索」。
- 与架构 v2 §6.2「先定窗口、再定元素」的分层不吻合：窗口已经解析出来了，元素搜索却把它丢掉。

已登记为 **PL-068**（trait 形状缺口）+ **DRIFT-017-7**（代码与 ADR-0022 E6 矛盾）。
两者都要改公共接口（漂移触发器 ③）与已决事项（④），故本 ADR 定案。

## 决策（一句话）

**元素解析必须带 scope**：`resolve_element` / `wait_for` 的第一个参数是 `&ResolvedWindow`，
搜索起点是该窗口的 UIA 根元素（`ElementFromHandle`），**不再**从桌面根搜元素。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | trait 签名改为 `resolve_element(&self, scope: &ResolvedWindow, chain: &SelectorChain)` 与 `wait_for(&self, scope: &ResolvedWindow, query: &ElementQuery, state: &ElementState, timeout: Timeout)`。scope 类型取 `ResolvedWindow` —— 与同一 trait 的 `snapshot_tree(root: &ResolvedWindow, …)` / `fingerprint(window: &ResolvedWindow, …)` **一致**（同一个「树的根」概念只用一个类型） |
| **D2** | 搜索起点 = `ElementFromHandle(scope.hwnd)`；作用域仍是 `TreeScope_Descendants`（**子树有界** → 正好落在 ADR-0022 E6 的「从应用窗口或更低层的容器开始搜」上） |
| **D3** | **桌面根搜索在元素解析里彻底消失**。桌面根只出现在**窗口解析**里，且必须按 ADR-0022 E6 先 `Children`；`wait_for` 的「先 Children 再 Descendants」也随之改为在 scope 子树内进行 |
| **D4** | 比窗口更深的容器 scope **不在本 ADR 内**：链内的 `RoleAndParent` 候选已经表达「在某父候选的子树内搜索」，再加一层 scope 枚举 = 新抽象层（漂移触发器 ⑨）。要加时另开 ADR |
| **D5** | 本 ADR **授权**更新 架构 v2 §13.1.1 的两行签名（line 1916~1917）+ §6.2 补一句「元素解析的 scope = 已解析的窗口」。**不改** ADR-0022（它的 E6 仍然成立，且本 ADR 正是为了遵守它） |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 维持现状（桌面根 `Descendants`），靠 `wait_for` 的 Children→Descendants 启发式绕 | ❌ 否决 | 实测 1.53 s（约 1000× 慢）；且违反 ADR-0022 E6 引用的官方要求（栈溢出风险） |
| 2 | 新增**并行**方法 `resolve_element_in(scope, chain)`，旧方法保留桌面根语义 | ❌ 否决 | 同一个概念出现两条入口 → 调用方会选错的那条（选桌面根的那条）；且「从桌面根搜元素」**没有**正当用例 —— 窗口必须先用 `resolve_window` 解析（§6.2） |
| 3 | **给现有两个入口加 `&ResolvedWindow` scope（本 ADR）** | ✅ **采纳** | 与 `snapshot_tree` / `fingerprint` 的根参数形状一致；调用方（Host）本来就已经持有 `ResolvedWindow`；桌面根搜索的代价与风险同时消失 |
| 4 | 把 scope 放进 `SelectorChain`（数据里带句柄） | ❌ 否决 | `SelectorChain` 是**可学习、可回写**的候选数据（ADR-0022 D5 / §6.3），把线程本地句柄塞进去会让「同一个链可以在不同窗口上用」变得不可能 |

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

- `crates/platform/api/src/traits/ui.rs`：两个签名 + 文档（**改公共接口**，本 ADR 授权）
- `crates/platform/windows/src/uia/{mod,resolve,search}.rs`：把 scope 传到 `FindAll` 的搜索起点
- `crates/platform/windows/src/unsupported.rs`：非 Windows 分支的签名同步
- `crates/platform/windows/README.md`：「已知限制」里的 scope 条目改为「已按 ADR-0043 解决」
- 架构 v2 §13.1.1（两行）+ §6.2（一句）
- `docs/PARKING_LOT.md`：PL-068 处置行；`tasks/TASK-017-*.md` §5：DRIFT-017-7 裁决回填
- **不改**：ADR-0022（E6 仍成立）、`SelectorChain` / `SelectorCandidate` 的形状、任何排期（ADR-0041 D2）

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 「元素在**别的**窗口里」的场景（如独立对话框）解析不到 | 那不是本层的错：按 §6.2 先 `resolve_window` 那个对话框，再在它里面解析元素。`wait_for` 同理 —— 它的 scope 也必须是那个对话框窗口 |
| 调用方忘记先解析窗口 | trait 签名**强制**给 scope（编译期）；Host 侧「先窗口后元素」是 §6.2 的既定顺序 |
| scope 窗口已关闭 | `ElementFromHandle` 失败 → `TargetNotFound`（铁律 1：明确失败） |
| 性能回归（多一次 `ElementFromHandle`） | 实测方向相反：省掉整个桌面遍历（1.53 s → 窗口子树量级）。落地时按 TASK-017 的真机验收口径复测 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. **立即验证（落地时）**：`cargo test --workspace` 全绿；`cargo clippy --all-targets -- -D warnings` exit 0；
   非宿主目标 `cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings` exit 0（口径见 ADR-0045）。
2. **真机验证**：同一台机器上重复 TASK-017 的手工验收（真实记事本 + 桌面上另有其它窗口），
   确认 `resolve_element` 的耗时不再随桌面规模增长（目标量级 = 窗口子树，而不是 1.53 s）。
3. **重新评估触发条件**：需要「比窗口更深的容器 scope」（D4）时；或出现「元素不属于任何已解析窗口」的合法场景时。

## 相关 ADR

- ADR-0022（Windows 目标身份与 UIA selector 稳定性 —— E6 是本 ADR 要遵守的约束）
- ADR-0045（非宿主平台编译门禁 —— 本 ADR 的落地验收依赖它覆盖 `unsupported.rs`）

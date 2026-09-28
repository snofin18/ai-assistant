# TASK-033　靶机应用 v0：`notepad-like`（WinUI/WPF，全部控件有稳定 AutomationId，CLI 可注入故障：元素消失/超时/歧义多匹配/意外弹窗/忙碌）

- 状态：**Review**
- 阶段：1　子阶段：**1a**　批次：**A3**　依赖：001　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：001　**预估**：M　**难度**：M
- **write scope**：`fixtures/apps/notepad-like/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 A3（1a）、`docs/wbs-overview.md` §6（DoD）

## 目标

交付一个可脚本启动、可重复验证的 Windows WPF 靶机应用 `notepad-like`。它不替代真实记事本，而是为 UIA 定位、故障恢复、录制回放和后续 Adapter 测试提供一个完全可控的目标：所有关键控件都有稳定 `AutomationId`，CLI 可注入元素消失、超时、歧义多匹配、意外弹窗和忙碌五类故障，并输出可供测试读取的状态文件。

## In scope

- `fixtures/apps/notepad-like/**`：应用脚本、XAML、专项测试、README。
- 使用 Windows PowerShell 5.1 内置 WPF 程序集运行；不要求 .NET SDK，不引入 NuGet 或第三方依赖。
- 关键控件稳定 `AutomationId`：主窗口、编辑区、菜单/文件/打开/保存/另存为按钮、状态栏、故障状态、忙碌遮罩、意外弹窗及按钮。
- CLI：`--fault none|disappear|timeout|ambiguous|dialog|busy`、`--state-file <path>`、`--auto-close-ms <n>`、`--self-check`、`--help`。
- 状态文件记录 schema 版本、应用 id、PID、故障模式、启动时间、窗口标题与声明的 AutomationId 列表。
- 专项测试脚本逐模式启动应用、读取状态文件并断言故障语义；失败必须返回非零退出码。

## Out of scope（做了算漂移）

- 修改 `.github/workflows/**` 或把靶机接入 CI；CI 接入归 TASK-039。
- 真实 Notepad Adapter、Rust 平台层、IPC、录制回放框架。
- 安装 .NET SDK、引入 WinAppSDK、NuGet 包或任何第三方依赖。
- 模拟真实记事本的全部菜单、文件对话框、编码/EOL 行为。
- 访问网络、真实应用、凭据或用户文件。

## 必须遵守

- `.ps1` 必须纯 ASCII（ADR-0024 D4）；中文说明放 `.md`。
- 所有文本文件使用 LF、单个结尾换行，遵守 `hygiene`。
- 未知故障模式、非法参数、不可写状态文件必须 fail-closed，返回非零退出码并给出可读原因。
- 靶机应用与测试脚本不得依赖交互式控制台；真实字符投递不在本卡范围。
- 所有关键控件必须显式设置 `AutomationProperties.AutomationId`，不得依赖本地化 `Name` 或 `x:Name` 回退。
- 不修改 write scope 之外的产品代码；不新增 crate、顶层目录或公共接口。

## 验收命令

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File fixtures/apps/notepad-like/notepad-like.ps1 --self-check
powershell.exe -NoProfile -ExecutionPolicy Bypass -File fixtures/apps/notepad-like/test-notepad-like.ps1
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger
```

## 完成定义（DoD）

- [ ] `--self-check` 验证 XAML 可解析、所有关键 AutomationId 存在且无重复。
- [ ] 五种故障模式可由 CLI 触发，专项测试覆盖 `none` + 五种故障并全部通过。
- [ ] 状态文件可被测试读取，字段与 README 声明一致。
- [ ] 未知故障模式与非法参数返回非零退出码。
- [ ] README 记录运行方法、AutomationId、故障语义、Windows CI 前置条件与已知限制。
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger` 全部 PASSED
- [ ] `LEDGER.md` 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-033 靶机应用 v0 `notepad-like`
【write scope】仅：`fixtures/apps/notepad-like/**`；另允许本卡执行记录区、`LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`
【铁律】无静默失败 / 所有输入先校验 / 不新增第三方依赖 / 不放宽 lint / 不超 write scope / 每个控件有稳定 AutomationId
【验收】fixture self-check、六个故障模式专项测试、`cargo fmt --check`、Clippy、workspace tests、xtask 全部门禁
【依赖】TASK-001 已 Done（已核对 LEDGER）
【疑问】本机没有 .NET SDK，只有 Windows PowerShell 5.1 和 WindowsDesktop runtime。默认采用 PowerShell 5.1 内置 WPF 程序集实现，仍是 WPF、零新增依赖，并可在 Windows runner 上跑；不安装 SDK，不引入 NuGet。

### 2. 实际改动文件

- `fixtures/apps/notepad-like/MainWindow.xaml`：WPF 窗口和基础 AutomationId。
- `fixtures/apps/notepad-like/automation-ids.json`：基础与运行时 AutomationId 清单。
- `fixtures/apps/notepad-like/notepad-like.ps1`：启动、参数解析、故障注入、状态文件与 self-check。
- `fixtures/apps/notepad-like/test-notepad-like.ps1`：六模式专项测试。
- `fixtures/apps/notepad-like/README.md`：运行方法、ID、故障语义、CI 前置条件和限制。
- `tasks/TASK-033-target-app-notepad-like-fault-injection.md`：占位正文展开与执行记录。

### 3. 验收输出摘要

- `notepad-like.ps1 --self-check`：PASS，输出 15 个基础 AutomationId、4 个运行时 AutomationId、6 个故障模式，并逐控件验证真实 `AutomationProperties.AutomationId`。
- `test-notepad-like.ps1`：PASS，7 个非法参数负向用例 + `none`、`disappear`、`timeout`、`ambiguous`、`dialog`、`busy` 六个模式全过。
- UIA 断言：`none` / `disappear` / `ambiguous` / `busy` 直接读取 UIA 树；`dialog` 验证 `UnexpectedDialog` 已渲染；`timeout` 用 1 秒 dispatcher 探针证明 UI 线程被阻塞。
- 独立 review 修复：严格参数解析、manifest 唯一性、dialog `ContentRendered` 就绪信号、UIA 语义断言、Timeout dispatcher 探针、临时目录/子进程清理与路径 quoting。
- `cargo fmt --all --check`：PASS。
- `cargo clippy --all-targets -- -D warnings`：PASS。
- `cargo test --workspace`：PASS。
- `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger`：PASS。
- `cargo deny check`：PASS，仅有既有 warnings。
- `git diff --check`：PASS。
- 规模提示：fixture 五个文件加卡片展开约 673 行，超过 gov §3 的单卡 400 行审阅建议；功能面集中在单一靶机 fixture，建议审阅者按 XAML / 主脚本 / 测试脚本 / 文档四段审阅。

### 4. DoD 逐条核对

- [x] `--self-check` 验证 XAML 可解析、所有关键 AutomationId 存在且无重复。
- [x] 五种故障模式可由 CLI 触发，专项测试覆盖 `none` + 五种故障并全部通过。
- [x] 状态文件可被测试读取，字段与 README 声明一致。
- [x] 未知故障模式、未知选项、重复选项、缺值与 `--fault=...` 等非法形式全部返回非零退出码（由严格参数解析和 `try/catch` 收口）。
- [x] README 记录运行方法、AutomationId、故障语义、Windows CI 前置条件与已知限制。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0。
- [x] `cargo test --workspace` 全绿。
- [x] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger` 全部 PASSED。
- [x] `LEDGER.md` 追加一行；长期事实追加 `docs/memory/facts.md`。

### 5. 偏差

none。

### 6. 更合理做法

- 选择 Windows PowerShell 5.1 + 内置 WPF，而不是安装 .NET SDK 或引入 WinAppSDK。该路径零新增依赖、符合当前 write scope，也能在 Windows runner 上执行。
- 把 AutomationId 清单拆成 `required`（基础 XAML）和 `runtime`（故障注入时动态创建）。这样 self-check 不会把尚未创建的模态弹窗误报为缺失，同时测试仍能验证完整 ID 集合。
- 用状态文件把“应用已启动 / 故障已注入”变成可机器读取的事实，避免测试只靠窗口存在或固定 sleep 猜测。
- 测试使用真实 UIA 断言与 dispatcher 探针，而不是只比较状态文件字段；这样能抓住“状态写了但故障没真正生效”的假绿。

### 7. 遗留问题

- CI 接入归 TASK-039；本卡只提供 fixture 和专项测试。
- `timeout` 与 `dialog` 的专项测试是进程级 smoke test，不尝试自动化注入弹窗内部控件；后续 UIA 回归可在 TASK-034/035 复用该 fixture 扩展。
- fixture 不读写用户文件，不模拟真实 Notepad 的编码/EOL/文件对话框行为。

### 8. 新增长期记忆

- FACT：`[2026-09-28][FACT][src:TASK-033 实现] notepad-like 靶机使用 Windows PowerShell 5.1 内置 WPF，不需要 .NET SDK：本机只有 .NET runtime 没有 SDK，编译型 WPF/WinAppSDK 会引入工具链或依赖审批；fixture 通过 PresentationFramework + XamlReader 运行，六种 CLI 故障模式由状态文件和专项测试验证。→ 后续 Windows 靶机可优先复用该零依赖路径；CI 接线归 TASK-039。`

### 9. 给审阅者的关注点

- 重点检查 `automation-ids.json` 的 `required` / `runtime` 拆分是否与 XAML 和弹窗实际行为一致。
- 重点检查 `ambiguous` 模式是否确实产生两个可见且同 AutomationId 的编辑器，而不是只在状态文件里声明。
- 重点检查 `timeout` / `dialog` 的测试是否真正验证了进程级故障，并确认测试结束会清理临时目录和子进程。

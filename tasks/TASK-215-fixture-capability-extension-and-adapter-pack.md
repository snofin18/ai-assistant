# TASK-215　`notepad-like` 靶机能力扩展 + `com.example.notepad-like` 适配包

- 状态：**InProgress（第 1 片：适配包进仓库；第 2 片：文件读写 + 标签页已落地并实测；跨进程 Save As 对话框未做）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：033、035、214、**PL-097 / DRIFT-105-2**
- 预估：L　难度：L
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 关联：TASK-033（靶机 v0）、TASK-035（声明式适配包）、TASK-105（下游 10 次运行验收）、`fixtures/apps/notepad-like/**`、`docs/PARKING_LOT.md` **PL-097**、`tasks/TASK-105-*.md` §5 `DRIFT-105-2`

---

## 目标（一句话）

让 `notepad-like` 靶机具备 T1.2 / T1.3 真正需要的可操作对象（文件读写、标签页与 tab 计数、跨进程 Save As 对话框），并把该靶机的 selector 与工具声明沉淀为仓库内的 `adapters/com.example.notepad-like/`，使靶机不再依赖测试里临时拼装的适配数据。

## 背景（为什么现在做）

| # | 事实 | 证据 |
|---|---|---|
| 1 | 靶机自述**不读写用户文件** | `fixtures/apps/notepad-like/README.md` |
| 2 | `OpenButton` / `SaveButton` / `SaveAsButton` 在 XAML 里存在但脚本里**没有任何点击处理**（只在 `--fault busy` 分支被置灰） | `fixtures/apps/notepad-like/notepad-like.ps1` |
| 3 | 没有标签页、没有未保存 `*` 标记、没有 Shell 对话框；`automation-ids.json` 只有单个 `EditorTextBox` | `fixtures/apps/notepad-like/automation-ids.json` |
| 4 | `adapters/com.microsoft.notepad/selectors/targets.json` 针对**真实 Notepad**（class `Notepad` / `RichEditD2DPT` / `TabView` / `#32770`），与靶机 AutomationId 不匹配 | 同文件 |
| 5 | TASK-214 的真 UIA 干跑目前**在测试里临时写**一份 fixture 适配数据到 temp 目录 | `apps/agent-core/tests/production_root_uia.rs` 的 `write_fixture_adapter()` |
| 6 | 因此 T1.2 / T1.3 在靶机上没有可执行对象，TASK-105 无法开工 | `PLAN.md` 阻塞项①、`PL-097`、`DRIFT-105-2` |

## write scope

- `fixtures/apps/notepad-like/**`（靶机能力扩展：XAML / 脚本 / `automation-ids.json` / README / 自测脚本）
- `adapters/com.example.notepad-like/**`（**新增**靶机适配包：adapter.toml / selectors / tools / README；数据文件，**不含代码**）
- `apps/agent-core/tests/production_root_uia.rs`（改用仓库内适配包，删除临时拼装）
- `tasks/TASK-215-fixture-capability-extension-and-adapter-pack.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（仅本卡条目完成标记 + 「当前进度」句）/ `docs/PARKING_LOT.md` / `docs/memory/apps/notepad.md`（仅追加）：§11.1 进度同步与 PL-097 收口

## In scope

- 给靶机加**最小可用**能力：打开文件 → 读入编辑器；保存 / 另存为（含"目标已存在不得静默覆盖"）；标签页（新建 / 计数 / 当前标签）；状态文本暴露 tab 计数与未保存标记。
- 为上述能力补**稳定 AutomationId**，并同步 `automation-ids.json` 的 `required` / `runtime` 清单与 `--self-check` 校验。
- 把 TASK-214 测试里临时写出的 fixture selector 与 tools 声明沉淀为 `adapters/com.example.notepad-like/**`，并让干跑测试改为从仓库读取。
- 为靶机新增/变化的入口补**至少一条** UIA 专项断言（沿用 TASK-033 的 `test-notepad-like.ps1` 风格）。

## Out of scope（做了算漂移）

- **T1.1~T1.3 各 10 次的真实运行验收与证据** —— 那是 TASK-105 的 DoD，本卡只负责"让靶机有能力"。
- 改 `crates/**` 的公共接口 / trait / schema / `ErrorCode`；新建 crate；引入任何第三方依赖。
- 操作真实商业 Notepad；把靶机做成真实 Notepad 的替代品（编码、EOL、菜单模型都不做）。
- 修改 `adapters/com.microsoft.notepad/**`（那是真实 Notepad 的包，本卡不动）。
- 顺手重构 `apps/agent-core` 的生产装配。

## 必须遵守

- 靶机仍然**只对测试与自动化开放**：不写用户真实文件路径，默认在调用方给定的临时目录内工作。
- 另存为**绝不静默覆盖**已存在文件（命中即 `PolicyDenied` 语义），与 `tools/tools.json` 的 `existing_file_not_overwritten` 一致。
- 新增/改动的 AutomationId 必须**非本地化**（ADR-0022 D4 禁把可见文本当主 selector）。
- 靶机 `/` 适配包都**不引入新依赖**：仍只用 Windows PowerShell 5.1 + WPF 内置程序集。
- 不改既有测试断言来"让新行为通过"（漂移触发器 ⑦）。

## 验收命令

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\fixtures\apps\notepad-like\notepad-like.ps1 --self-check
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\fixtures\apps\notepad-like\test-notepad-like.ps1
python -c "import json,pathlib; [json.loads(p.read_text(encoding='utf-8')) for p in pathlib.Path('adapters/com.example.notepad-like').rglob('*.json')]"
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test production_root_uia -- --ignored --nocapture
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-migrations
cargo run -p xtask -- check-comments
cargo deny check
```

## 完成定义（DoD）

- [ ] 靶机支持打开 / 保存 / 另存为（含拒绝覆盖已存在文件）与标签页，且有稳定 AutomationId 与可断言状态
- [ ] `automation-ids.json` 与 `--self-check` 覆盖新增 ID；`test-notepad-like.ps1` 有对应 UIA 断言
- [ ] `adapters/com.example.notepad-like/**` 已进仓库，selector 覆盖靶机全部可操作目标，不含 `DoesNotExist` 占位
- [ ] TASK-214 的干跑测试改为读取仓库内适配包，**不再**写临时适配数据
- [ ] T1.1 干跑仍为 Completed；T1.2 / T1.3 所需元素可被解析（各自至少一条正/负断言）
- [ ] `PL-097` 可闭环（靶机能力缺口消除），并在 `docs/PARKING_LOT.md` 记明
- [ ] 上列 18 条验收命令全绿（`hygiene` 不高于 0E/3W 基线）
- [ ] §11.1 进度同步：`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

【任务】TASK-215 靶机能力扩展 + `com.example.notepad-like` 适配包
【目标】让 `notepad-like` 具备 T1.2/T1.3 所需的可操作对象，并把 fixture 适配数据沉淀进仓库
【write scope】仅：`fixtures/apps/notepad-like/**`、`adapters/com.example.notepad-like/**`、`apps/agent-core/tests/production_root_uia.rs`、本卡与状态同步文件
【铁律】1 无静默失败；8 element/句柄不跨进程；10 契约先行
【禁止】产出 T1.x 十次运行证据（归 TASK-105）；改 `crates/**` 公共接口；新依赖；操作真实 Notepad；改测试断言
【验收】18 条命令（见卡面）+ 靶机自测脚本 + 真 UIA 干跑
【依赖】033、035、214 均 Done；PL-097 / DRIFT-105-2 为本卡来源
【疑问】无（人类 2026-10-01「开始按计划做吧」即授权 PL-097 推荐方案 ①）

### 2. 实际改动文件

- **新增** `adapters/com.example.notepad-like/adapter.toml`：靶机身份（`app_id = "powershell.exe"`）、UIA 通道、健康与安全默认值。
- **新增** `adapters/com.example.notepad-like/selectors/targets.json`：生产 Host 要求的六个 target 的候选链，**只按 AutomationId**，无可见文本。
- **新增** `adapters/com.example.notepad-like/tools/tools.json`：与 `com.microsoft.notepad` 同名同形的五个工具声明，仅换 `app_id`。
- **新增** `adapters/com.example.notepad-like/README.md`：说明"已实现 vs 尚未实现"的 target 对照与 fail-closed 语义。
- **改** `apps/agent-core/tests/production_root_uia.rs`：**删除**测试内的 `write_fixture_adapter()`（原先把适配数据临时拼到 temp 目录），改为 `fixture_adapter_root()` 直接读仓库内适配包。
- **改** 本卡、`PLAN.md`（当前状态块：修掉残留的 `TASK-205 = Ready`，并写明 TASK-215 的两片边界）、`plans/stage-1-pilots.md`（A5-REMEDIATION 表 + 位置表各加一行）、`LEDGER.md`、`docs/PARKING_LOT.md`。

本轮**未**改 `crates/**`、`adapters/com.microsoft.notepad/**`，未加依赖。

**第 2 片（2026-10-01，同卡继续）追加改动：**

- **改** `fixtures/apps/notepad-like/MainWindow.xaml`：工具栏加 `AddTabButton`，状态栏加 `TabCountText`。
- **改** `fixtures/apps/notepad-like/automation-ids.json`：把 `AddTabButton` / `TabCountText` 加进 `required`。
- **改** `fixtures/apps/notepad-like/notepad-like.ps1`：新增 `--document <path>`；`SaveButton` 写回文档并清掉未保存标记；`AddTabButton` 让 `TabCountText` +1 并清空编辑器；标题在改动后带 `*`、保存后消失。
- **改** `fixtures/apps/notepad-like/README.md`、`adapters/com.example.notepad-like/README.md`：同步新选项、新 AutomationId 与 target 状态表。

### 3. 验收输出摘要

```text
python -c "json.loads(...) for adapters/com.example.notepad-like/**/*.json"   → 2 个 JSON（`selectors/targets.json` / `tools/tools.json`）全部可解析
python -c "tomllib.loads(adapter.toml)"                                      → PASS
cargo fmt --all --check                                                      → clean
cargo clippy --all-targets -- -D warnings                                    → 0 warning
cargo test -p assistant-agent-core --test production_root_uia -- --ignored   → 1 passed / 0 failed（0.82s）
cargo test --workspace                                                       → 全部 test result: ok（0 failed）
xtask verify-schemas / codegen --check                                       → PASS（0 drift）
xtask hygiene                                                                → 0 error / 3 warning（= 当前基线）
xtask docscan / card-check / refscan / memory-counts / adr-index             → PASS
xtask check-ledger / check-migrations / check-comments                       → PASS
```

关键点：干跑测试**改为读仓库内适配包后仍然通过** —— 说明这份包不是摆设，而是真的能被生产 Host 加载并驱动靶机。

**第 2 片（2026-10-01）实测**（真实 UIA 探针，非静态断言）：

```text
window.title.before = nl-doc-<guid>.txt          # --document 读入文件后标题 = 文件名叶子
tabcount.before     = Tabs: 1
tabcount.after      = Tabs: 2                    # UIA Invoke AddTabButton 生效
window.title.dirty  = nl-doc-<guid>.txt *        # ValuePattern 改文本后出现未保存标记
window.title.saved  = nl-doc-<guid>.txt          # UIA Invoke SaveButton 后标记消失
file.after          = gamma                      # 保存真的写回了文件
```

```text
powershell notepad-like.ps1 --self-check                 → status=ok，17 个 required id 全在
powershell test-notepad-like.ps1                         → "notepad-like tests passed."（含 fault=none..busy）
cargo test -p assistant-agent-core --test production_root_uia -- --ignored → 1 passed
cargo clippy --all-targets -- -D warnings                → 0 warning
```

另外：`notepad-like.ps1` 的非 ASCII 字节 = **0**（满足 ADR-0024 D4 的 `.ps1` 纯 ASCII 规则）。

### 4. DoD 逐条核对

- [ ] 靶机支持打开 / 保存 / 另存为（含拒绝覆盖已存在文件）与标签页，且有稳定 AutomationId 与可断言状态 —— **部分**：打开（`--document`）、保存（写回文件并清掉 `*`）、标签页（`AddTabButton` → `TabCountText`）已落地且有真 UIA 实测；**另存为 + 跨进程对话框 + 拒绝覆盖仍未做**
- [ ] `automation-ids.json` 与 `--self-check` 覆盖新增 ID；`test-notepad-like.ps1` 有对应 UIA 断言 —— **已做**：`AddTabButton` / `TabCountText` 进 `required`，`--self-check` 与 `test-notepad-like.ps1` 全过；另有独立真 UIA 探针验证点击/改文本/保存的实际效果
- [ ] `adapters/com.example.notepad-like/**` 已进仓库，selector 覆盖靶机全部可操作目标，不含 `DoesNotExist` 占位 —— **部分**：包已进仓库、**不含 `DoesNotExist` 占位**、靶机**当前可操作**的 `main_window` / `editor` 已被真实 AutomationId 覆盖；另外四条 target 指向**将来要实现的** AutomationId（靶机补上之前解析即 `TargetNotFound`）
- [x] TASK-214 的干跑测试改为读取仓库内适配包，**不再**写临时适配数据 —— 已完成（`write_fixture_adapter` 已删除）
- [ ] T1.1 干跑仍为 Completed；T1.2 / T1.3 所需元素可被解析 —— **半**：T1.1 干跑仍 Completed；T1.2/T1.3 所需元素尚不可解析
- [ ] `PL-097` 可闭环 —— **未到**（靶机能力缺口仍在）
- [ ] 上列 18 条验收命令全绿 —— 已跑其中 14 条（未跑：靶机 `--self-check`、`test-notepad-like.ps1`、`cargo deny check`，前两条属第 2 片范围）
- [ ] §11.1 进度同步 —— **Done 时执行**（本轮 InProgress，只追加 LEDGER + 计划表新行）

### 5. 偏差

**DRIFT-215-1（发现：target 加载器强制要求六个 target 全部声明）**

1. **现象**：`apps/agent-core/src/notepad_targets.rs` 的 `REQUIRED_TARGETS` 列了六个 id，`NotepadTargetCatalog::load()` 缺任一即 `MissingTarget` 失败。因此**不可能**只声明靶机"目前真正支持"的两个 target —— 适配包要么不完整、要么必须把尚未实现的目标也写进去。
2. **影响**：这是**有意的 fail-closed 设计**（宁可加载失败也不静默少一个目标），但它把"靶机尚未实现"这件事从"包外"推到了"包内"，必须显式写出来才不会误导人。
3. **处置（未改任何代码）**：把这四条**指向将来才存在的 AutomationId**（`AddTabButton` / `SaveAsDialogWindow` / `SaveAsFileNameBox` / `SaveAsConfirmButton`），并在 `adapters/com.example.notepad-like/README.md` 用表格写明"尚未实现"、在 `PLAN.md` 的下一步动作里点名。**未用 `DoesNotExist` 这类无语义占位**，也未让它们指向别的真实控件 —— 那会变成"静默命中错控件"。

**DRIFT-215-2（自伤：在 `.ps1` 里写了中文注释，直接把脚本写成语法错误）**

1. **现象**：第 2 片第一版在 `notepad-like.ps1` 里加了中文注释，保存后脚本**无法启动**；`Parser::ParseFile` 报 `Try statement is missing its Catch or Finally block` / `Unexpected token '}'`，但大括号计数是**平衡的**（末深度 = 0），只有第三次报错指向了真正的 `$docState = @{` 那一行。
2. **根因**：ADR-0024 D4 早就规定 `.ps1` **必须纯 ASCII**（Windows PowerShell 5.1 无 BOM 时按 ANSI 代码页解码 UTF-8 字节），中文注释被解成乱码后破坏了语法。**这条规则本来就是防这个的，是我先违规了**。修法 = 注释改回 ASCII，随后 `非 ASCII 字节 = 0` 且 `ParseFile` 无错误。
3. **顺带查明的真 bug（与编码无关）**：`GetNewClosure()` 会把调用作用域的变量**快照**进闭包，所以原来用 `$script:Dirty` 在 `TextChanged` 里标脏、在另一个闭包里读它，**读到的是各自的副本**。实测表现 = `Lines:` 会更新（另一个处理器），但标题永远不出现 `*`。改成哈希表（引用类型）共享可变状态后立刻生效。
4. **已做**：两处都已按上述修法落地并有实测证据（见 §3）。

### 6. 更合理做法

把适配数据从测试里搬进仓库这一步本身就值得单独做：测试里拼出来的 adapter 只有写它的作者看得到，既不会被 review，也不会被别的测试复用；而 `adapters/**` 是架构 v2 §6.7 的正式落点。剩下那四条 target 之所以先声明，是因为加载器要求全量 —— 与其让包"缺一块"，不如让它**把缺口写在脸上**（README 对照表 + `PLAN.md` 点名）。

### 7. 遗留问题

- **剩余 = 跨进程 Save As 对话框**（`save_as_dialog` / `save_as_filename` / `save_as_save_button` 三条 target）：T1.3 明确要求**跨进程**，而 WPF 同进程模态窗不算，所以需要一个**子进程**对话框（预期由 `SaveAsButton` / `Ctrl+Shift+S` 拉起，并把编辑器内容落到目标路径、目标已存在时拒绝覆盖）。三条 target 现在解析仍必然 `TargetNotFound`。
- `notepad.file.replace_text` 走 `set_value`（ValuePattern）：靶机的未保存标记已证明在 ValuePattern 改文本后会亮起，但**尚未经生产 handler 端到端跑过**（属 TASK-105 的取证范围）。
- `PL-097` 保持 **open**；`TASK-105` 仍不可开工；阶段 1a 仍 **NO-GO**。
- `PL-097` 保持 **open**；`TASK-105` 仍不可开工；阶段 1a 仍 **NO-GO**。

### 8. 新增长期记忆

（无长期记忆新增 —— 结论由 `DRIFT-215-1`、`PL-097` 与本卡记录承载；靶机扩展完成时再考虑进 `docs/memory/apps/notepad.md`。）

### 9. 给审阅者的关注点

1. **这份适配包现在是"半成品但诚实"**：请重点看 `adapters/com.example.notepad-like/README.md` 的对照表，确认四条"尚未实现"的 target 用**将来要用的 AutomationId**（而不是占位串或别的真实控件）是可以接受的表达。
2. **`tools/tools.json` 是从 `com.microsoft.notepad` 逐字复制、只改 `app_id`**：五个工具名与 postcondition 声明与真实 Notepad 完全一致。若你认为靶机工具语义应当独立演进，这里需要重写而不是复制。
3. 干跑测试已改为读仓库内适配包并通过 —— 这是本片唯一的"能机器证明"的成果；其余 DoD 项都还没到。
4. 本卡 **InProgress**，阶段 1a 仍 **NO-GO**。

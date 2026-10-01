# com.example.notepad-like Adapter（靶机适配包）

这份包是 **`fixtures/apps/notepad-like` 靶机**的适配声明，不是真实 Notepad 的包
（真实 Notepad 在 `adapters/com.microsoft.notepad/`）。它的存在理由 = `PL-097` /
`DRIFT-105-2`：靶机的 AutomationId 与真实 Notepad 的 class / selector 完全不同，
而 TASK-214 的真 UIA 干跑此前只能在测试里**临时拼**一份适配数据写到 temp 目录。

**数据文件，不含代码。** 内容刻意与 `com.microsoft.notepad` 的包同形，便于对照阅读。

## Files

- `adapter.toml`：靶机身份（`app_id = "powershell.exe"`）、UIA 通道、健康与安全默认值。
- `selectors/targets.json`：生产 Host 要求的六个 target 的候选链（**只按 AutomationId**，
  不含任何可见文本，ADR-0022 D4）。
- `tools/tools.json`：与 `com.microsoft.notepad/tools/tools.json` 同名同形的五个工具声明，
  只把 `app_id` 换成靶机身份。靶机尚未实现的工具语义仍由 Handler 在动作前 fail-closed。

## 靶机当前能力 vs 已声明的 target

| target | 靶机现状 |
|---|---|
| `main_window` | ✅ 已实现（AutomationId `MainWindow`） |
| `editor` | ✅ 已实现（AutomationId `EditorTextBox`） |
| `add_tab_button` | ✅ 已实现（AutomationId `AddTabButton`，点击后 `TabCountText` +1） |
| `save_as_dialog` | ⛔ **尚未实现** —— 期望 id = `SaveAsDialogWindow` |
| `save_as_filename` | ⛔ **尚未实现** —— 期望 id = `SaveAsFileNameBox` |
| `save_as_save_button` | ⛔ **尚未实现** —— 期望 id = `SaveAsConfirmButton` |

生产 Host 的 target 加载器**要求六个 id 全部存在**（`notepad_targets.rs` 的
`REQUIRED_TARGETS`），所以这四条"尚未实现"的条目必须留在包里。它们的候选链指向**将来
要用的** AutomationId：靶机补上之前，解析只会得到 `TargetNotFound`，即**显式失败而不是
静默命中别的控件**——这正是我们要的 fail-closed 行为。

`TASK-215` 的目标就是把这些行从"尚未实现"变成"已实现"，并给每条补正/负断言。
**当前进度**：`add_tab_button` 已于 2026-10-01 落地；剩下三条（跨进程 Save As 对话框）
仍是 `TASK-215` 第 2 片的剩余工作。

另外，靶机的文件能力已经可用：`notepad-like.ps1 --document <path>` 会把文件读进编辑器，
`SaveButton` 写回该文件并清掉标题里的 `*` 未保存标记（实测见 `TASK-215` 执行记录 §3）。

## Validation

```powershell
python -c "import json, pathlib; [json.loads(p.read_text(encoding='utf-8')) for p in pathlib.Path('adapters/com.example.notepad-like').rglob('*.json')]"
python -c "import tomllib, pathlib; tomllib.loads(pathlib.Path('adapters/com.example.notepad-like/adapter.toml').read_text(encoding='utf-8'))"
```

相关：`tasks/TASK-215-fixture-capability-extension-and-adapter-pack.md`、
`docs/PARKING_LOT.md` PL-097、`fixtures/apps/notepad-like/README.md`。

# spikes/spike-a-notepad —— Spike A 的一次性验证代码

> **这里的东西永远不进 `crates/`。** 它是为了证伪/证实假设而写的探针，生命周期到
> `docs/spike-reports/SPIKE-A.md` 出结论为止（AGENTS.md 阶段 0 约束）。

| 文件 | 作用 | 依赖 |
|---|---|---|
| `probe-01-tree-survey.ps1` | 启动记事本 → 按标题定位窗口 → 全树遍历并打印控件普查表（ControlType / AutomationId / ClassName / Name）→ 找编辑区 → 读 `ValuePattern` → 检查 `TextPattern` 是否可用 | 仅 Windows 自带 UIAutomation 程序集 |
| `probe-02-text-and-timing.ps1` | 干净启动（先杀所有 Notepad）→ 校验读回文本与磁盘文件是否一致 → **10 次取中位数**（全树遍历 / 编辑区定位 / GetValue）→ `SetValue` 写中文并读回校验 → **单实例多标签**验证 | 同上 |

## 运行

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\probe-01-tree-survey.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\probe-02-text-and-timing.ps1
```

## 已知注意事项

1. **会强制关闭记事本**（`Stop-Process Notepad -Force`）—— 运行前保存你手工打开的内容。
2. **控制台中文可能乱码**（PowerShell 控制台代码页），判定以脚本内比较结果为准。
3. 两个脚本都**不修改仓库内任何文件**，临时文件写在 `%TEMP%` 并自行删除。
4. `probe-01` 曾因「同名文件已在标签页中打开 → 记事本**不重新加载磁盘内容**」得到假阴性，
   `probe-02` 因此改为**先杀进程 + 用带时间戳的唯一文件名**。新写探针请沿用 `probe-02` 的做法。

## 尚未实现（TASK-002 剩余步骤）

- 菜单项（文件/编辑/查看展开后的子项）与「另存为」**跨进程 Shell 对话框**的普查
- 大文件 1 KB / 100 KB / 1 MB 的读写耗时与内存增量
- **IME 开 / 关两态**对照（注意：`SetValue` 走 Pattern 不经键盘，IME 状态对它无影响；
  两态对照真正针对的是 L4 合成键盘输入路径）
- 失败注入：记事本被关闭 / 最小化 / 在另一虚拟桌面 / 出现未保存弹窗
- 接口考古 8 步（`target-apps-feasibility.md` §5）
- **Rust 生产路径验证**（阻塞在依赖批准，见 PL-019 与 `docs/DEPENDENCIES.md`）

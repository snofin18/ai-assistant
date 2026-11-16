# Microsoft Excel

> 本档案记录 Excel Adapter 的稳定事实、通道结论与已知坑。Com 探针相关临时输出不写入本文件；
> 原始结构化证据保留在对应 `eval/` 目录。

## 1. 身份与版本

| 项 | 2026-11-16 实测 |
|---|---|
| 产品 | Microsoft Office Home 2024 Retail，x64 |
| ClickToRun `VersionToReport` | `16.0.20527.20196` |
| COM `Application.Version` | `16.0` |
| COM `Application.Build` | `20527` |
| 可执行文件 | `C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE` |
| `Application.Path` | `C:\Program Files\Microsoft Office\Root\Office16` |

版本探测优先使用 COM `Application.Build`；ClickToRun 注册表值只作为安装级旁证。

## 2. 进程与窗口模型

- COM 可在不可见窗口下创建新的 Excel 实例并操作未保存工作簿。
- Excel 可能复用既有进程；自动化前若发现任何 `EXCEL.EXE`，必须停止而不是猜测归属。
- 新建实例后应先记录本轮新增 PID，后续只允许等待或终止这些 PID。
- 正常路径 `Workbook.Close(false)` → `Application.Quit()` → FinalReleaseComObject 后，
  本机 `EXCEL.EXE` 可收敛为 0。
- COM 对象不得跨线程；产品 runner 必须在创建它的 apartment/线程内完成释放。

## 3. UIA 形状

- Excel 网格是虚拟化自绘结构，UIA 只适合读取可见区、编辑栏和对话框状态。
- 大数据区域不得依赖 UIA 遍历；语义数据读取和写入一律优先 L1 COM。
- UIA 仍用于 Protected View、模态对话框和工作簿状态的人工可见验证。

## 4. 数据约定

同一个 Range 至少有三种不同读法：

| 字段 | 2026-11-16 公式探针 |
|---|---|
| `.Value2` | `5.0` |
| `.Formula` | `=A1+A2` |
| `.Text` | `"5"` |

- 断言必须明确声明读取 `value2`、`formula` 或 `text`，不得用“单元格内容”含糊描述。
- A1=2、A2=3、A3=`=A1+A2`，`Application.Calculate()` 后 `.Value2` 实测为 5。
- 命名区域优先于 A1 字符串，以降低插入行列导致引用漂移的风险。

## 5. 性能实测

- 2026-11-16 Excel COM preflight 总耗时约 7.3 秒，包含首个实例启动、工作簿创建、
  公式计算、Close/Quit/COM release 与 PID 收敛等待。
- 尚未测大表 Range 批量读取、批量写入、重计算和图表导出性能。
- 不得用 UIA 全网格遍历替代 COM 性能测试。

## 6. 已知坑

1. **程序化修改不能依赖 Excel undo 栈**：两次 COM 写入前后 `CanUndo` 均为 `false`；
   `Application.Undo()` 未可调用。所有写操作必须按 L1 快照恢复。
2. **`DisplayAlerts=false` 永久禁止**：preflight 只读取到当前值 `1`，从未修改。
3. **Protected View 是安全边界**：“启用编辑”必须人工确认，自动化不得代点。
4. **AutoSave/OneDrive 会产生云端冲突**：写入前必须探测；新工作簿探针中 `AutoSaveOn=false`，
   但不代表用户工作簿状态。
5. **现有 Excel 进程不能复用**：Com 可能附着用户实例，误终止会损坏用户工作。探针在
   创建前检测到任何 `EXCEL.EXE` 就 fail-closed。
6. **公式/值/文本不可混用**：错误读法会让成功断言失真。

## 7. 通道结论

| 层 | 结论 |
|---|---|
| L1 COM | Excel 的主通道；工作簿、Range、公式、格式、图表优先 |
| L2 快捷键/Ribbon | 仅 COM 无法覆盖的明确 UI 动作 |
| L3 UIA | 可见状态、Protected View、模态对话框与 COM 覆盖不足的向导 |
| L4/L5 | 一般不使用 |

可逆性：

- 值/区域写入：L1 值快照 + 文件影子副本。
- 结构性修改：L1 文件影子副本 + 反向脚本。
- 保存：保存前副本。
- 程序化修改禁止声明为 L0。

## 8. 未测项

- 打开、读取和保存真实 `.xlsx/.xlsm/.xlsb` 工作簿。
- Protected View、只读/占用、AutoSave=true、OneDrive/SharePoint。
- 多工作簿、多窗口、命名区域解析与 A1 漂移。
- 模态对话框族、宏警告、更新链接。
- 大表批量 Range 性能、内存/句柄收敛。
- L1 文件影子副本与回滚冲突检测。
- 图表创建和 PNG 导出。

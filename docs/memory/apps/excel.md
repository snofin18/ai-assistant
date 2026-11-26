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
7. **取消点与进程所有权必须显式建模**：原始 COM 实例刚出现、Excel 尚未完成初始化时触发
   取消，可能让 `Application.Quit()` 阻塞；最终回归只在工作簿创建后的安全 checkpoint
   验证协作取消。生产/取证 runner 必须先用 `Application.Hwnd` 映射并复核 PID、启动时间与
   可执行路径，保存 ownership journal，再由独立 watchdog 对超时子进程和该 Excel 身份做
   有界回收；禁止只按“启动前后 PID 差集”推断所有权或按裸 PID 直接 `taskkill`。
8. **`Workbook.Workbooks.Open` 不接受 verbatim canonical path**：TASK-401 实机把
   `std::fs::canonicalize()` 的 `\\?\C:\...` 直接传给 `Workbooks.Open`，Excel 返回
   `0x800A03EC`。本地 `.xlsx` 必须在 canonical 校验后去掉 `\\?\` 前缀再交给 COM；
   `\\?\UNC\...` 继续拒绝，不能把网络共享误作本地文件。
9. **owned Excel 必须用完整身份而非裸 PID 回收**：TASK-401 首轮错误前置断言 panic 后，
   测试进程退出可能来不及让 worker 执行 Quit，留下本轮 Excel。生产创建路径必须先通过
   Toolhelp snapshot 拒绝任何既有 `EXCEL.EXE`；测试级 guard 的 Drop 必须 best-effort
   shutdown，仅在 PID + creation FILETIME + executable path 三者仍匹配时才执行进程树清理。
10. **Excel collection `Item` 不能只用 property-get flag**：TASK-402 在真实 COM 上调用
    `Worksheets.Item(1)` 遇到 `0x80020003`（`DISP_E_MEMBERNOTFOUND`）；只有同时提供
    `DISPATCH_PROPERTYGET | DISPATCH_METHOD`，Excel 才会按自身 type metadata 分派
    `Worksheets.Item`、`Names.Item` 与 `Range.Cells`。
11. **带公式/error cell 的 workbook 退出需要更长但有界的收敛窗口**：TASK-402 的
    单工作簿完整读取和校验均已成功，但 5 秒 poll 仍可能看到 Excel 未退出；验收应使用
    有界 20 秒等待，并在超时后仍按 PID + creation FILETIME + executable path 清理和失败，
    不能把等待放宽成无限成功。
12. **`Range.Value2` scalar put 必须使用 property-put named DISPID**：TASK-407 真实 COM
    写入使用 `DISPATCH_PROPERTYPUT`、`DISPID_PROPERTYPUT`、`cArgs=1/cNamedArgs=1`；
    `windows 0.62.2` 的 `VARIANT` 自带 `Drop -> VariantClear`，故参数必须放入
    `ManuallyDrop` 后才显式清理，确保同步 `Invoke` 期间有效且恰好 `VariantClear` 一次。
    数字、布尔、文本和空值写入后均可由完整 `.Value2` 读回。
13. **真实 Excel 的 `.Formula` 读形状按值类型不同，不能从 fake 猜测**：TASK-407 实测
    数字 `41` -> `"41"`、布尔 `false` -> `"FALSE"`、文本 `TASK-407` -> `"TASK-407"`、
    空 cell 仍为 `Blank`；真实公式 cell `=A1+B1` 保留公式文本。断言必须分别固定
    value type 与文本。
14. **保存后的单次 teardown 可能明显慢于普通只读关闭**：TASK-407 一次多 cell range
    写入/保存后在 20 秒内未退出，完整身份 guard 已回收；紧随其后的同路径验收在约
    7.2 秒完成。测试级收敛窗口保持有界 45 秒并持续打印等待进度，超时仍必须失败和
    清理，不能因一次慢退出放宽为零残留判据。
15. **同一路径的 10 次独立进程写入可以稳定收敛，但必须同时保留 journal/watchdog
    证据**：TASK-409 以 10 个不同 owned PID 完成 10/10 writable open、scalar write、
    Calculate、`.Value2/.Formula` 双读、Save、Close 与 shutdown；每次 saved digest
    均改变，shadow restore 均回到源 digest，最终 `EXCEL.EXE=0`。after-write panic 会
    留下 ownership journal 并由完整身份 guard 回收；after-open 20 秒 hang 由独立
    PowerShell watchdog 读取 journal，核对 PID + creation FILETIME + executable path
    后回收。因此重复验收不能只看最终进程数，必须同时保存逐轮摘要、journal、
    failure/watchdog 日志；这仍是 test-only 证据，不代表 production watchdog 已实现。

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

- TASK-401 已实测一个本地 `.xlsx` 的 read-only open、active-sheet `.Text` 读取、close 与
  shutdown；TASK-407 已实测临时副本 writable open、单 cell/双 cell scalar write、
  `Application.Calculate`、完整 `.Value2/.Formula` 双读、`Save`、shadow restore 与
  进程/临时文件收敛；TASK-409 已补 test-only 10/10 独立进程重复写入、after-write
  failure injection 与 after-open watchdog。`.xlsm/.xlsb`、用户工作簿、多工作簿、
  批量事务、production ownership journal/watchdog 与崩溃恢复仍未测。
- Protected View、只读/占用、AutoSave=true、OneDrive/SharePoint。
- 多工作簿、多窗口、命名区域解析与 A1 漂移。
- 模态对话框族、宏警告、更新链接。
- 大表批量 Range 性能、内存/句柄收敛。
- L1 文件影子副本与回滚冲突检测已有单次真实临时副本校准与 TASK-409 的 10/10
  test-only 重复运行；production ownership journal/watchdog、崩溃恢复与真实 task
  cleanup 仍未测。
- 图表创建和 PNG 导出。

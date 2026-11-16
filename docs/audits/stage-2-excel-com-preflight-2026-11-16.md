# Stage 2 Excel COM preflight

日期：2026-11-16

## 1. 环境

- 安装：Microsoft Office Home 2024 Retail，x64。
- ClickToRun `VersionToReport`：`16.0.20527.20196`。
- Excel COM：`Version=16.0`、`Build=20527`。
- `EXCEL.EXE`：`C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE`。
- 探针启动前本机 `EXCEL.EXE` 数量为 0。

## 2. 隔离策略

`probe_excel_com.ps1` 在任何 COM 创建前检查现有 `EXCEL.EXE`。若存在则在创建前失败，
避免 COM 复用用户实例后误终止用户进程。探测期间：

- 只调用 `Workbooks.Add()` 创建新的未保存工作簿；
- 只写 A1/A2/A3 临时单元格；
- 不打开任何用户文件，不调用 Save/SaveAs；
- 不修改 `DisplayAlerts`；
- 结束调用 `Workbook.Close(false)`、`Application.Quit()`、FinalReleaseComObject；
- 只等待/回收本轮新增 PID。

## 3. 真实结果

证据：

`eval/tasks/excel/preflight/evidence/2026-11-16-task-372/summary.json`

结果：

```text
result=passed
excel_version=16.0
excel_build=20527
probe_excel_process_ids=[40220]
display_alerts_observed=1
display_alerts_changed=false
visible_observed=false
protected_view_window_count=0
autosave_on=false
excel_process_count_after=0
```

公式探针在 A1=2、A2=3、A3=`=A1+A2` 后调用 `Calculate()`：

```text
.Value2  = 5.0
.Formula = =A1+A2
.Text    = "5"
```

三种读取口径均返回，确认后续断言必须明确指定读哪一个字段。

## 4. Undo 观测

连续两次 COM 写入前后的 `CanUndo`：

```text
before=false
after_first_write=false
after_second_write=false
```

因此 `Application.Undo()` 没有被调用。该真实观测支持既有架构结论：Excel 程序化修改不能
按 L0/Excel undo 栈恢复，所有写操作必须按 L1 快照与 L1 回滚建模。

## 5. 资源收敛

- 工作簿 `Close(false)` 成功；
- Excel `Quit()` 成功；
- COM cleanup failure 为 0；
- 本轮没有触发 taskkill；
- 探针新建 PID 剩余为 0；
- 最终本机 `EXCEL.EXE` 数量为 0。

## 6. Adapter 契约缺口

1. 既有 Adapter 数据包仍存在两层 App Map（富知识 + core v1 投影），Stage 2 必须冻结 owner
   与单一公开 schema。
2. `ToolSchema` 仍不能直接承载 adapter-level `postconditions` / `none_readonly`，必须由
   ADR/spec 决定扩展点，禁止继续靠数据包附加字段隐式约定。
3. “打开文件”目前仍需决定是平台 Host 前置动作还是模型可见工具；Excel T2.1 前必须裁决。
4. COM 对象必须线程本地创建/释放，不能跨线程；产品 COM runner 必须提供成功、异常、取消
   三条路径的回收证据。
5. Protected View、AutoSave=true、只读/占用、模态对话框尚未实测，不能据此宣称完成。

## 7. Stage 2 判断

Excel L1 COM 通道预检为 **GO**。下一顺序：

1. 正式化 Adapter schema；
2. 在 `crates/platform/windows` 建立产品 COM lifecycle runner；
3. 交付 Excel Adapter pack；
4. 依次完成 T2.1、L1 rollback、T2.2、T2.3。

本探针只证明本机 COM 基础通道、公式读取、undo 事实与资源生命周期，不代表 T2.1/T2.2/T2.3
或 Protected View/AutoSave 已完成。

## 8. 合并证据

- PR #734 checks：**11/11 success**，pending 0 / failed 0。
- base：`main`；状态：`MERGEABLE` / `CLEAN`。
- merge commit：`fafff75f4b22642ad6f9e858067c989f63e6b947`。
- post-merge main CI：`43825737477` success。

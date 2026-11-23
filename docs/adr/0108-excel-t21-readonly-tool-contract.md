# ADR-0108：Excel T2.1 read-only Tool contract

状态：**Accepted**（2026-11-23，人类要求继续 TASK-393 的下一步）
日期：2026-11-23
Clarifies：ADR-0104、ADR-0105、ADR-0107
Supersedes：无
Superseded by：无
关联：TASK-372、TASK-375、TASK-391、TASK-393、TASK-394
落点：`docs/spec/excel-t21-readonly-workflow.md`、`docs/spec/capability-matrix.md`

## 背景

ADR-0107 已批准七个 Excel COM capability，但生产 COM runner 和 model-visible Tool 尚未实现。
如果先写 `tools.json` 再补语义，会同时产生未实现的 Tool surface、session 泄漏、用户 Excel
进程误用和输入输出不可测试等风险。

本 ADR 只冻结 T2.1 的目标契约。它不授权真实 Excel 启动、`.xlsx` 读取或 Adapter tool 暴露。

## 决策

一句话：**T2.1 只读链路固定为独立 open Tool + 三个 read Tool；open 是唯一 L3 write Tool，
读取只消费 task-scoped opaque session，所有 payload 有界且失败不截断。**

### D1：Tool surface

| Tool | Effect | Reversibility | Risk | Approval | Capability |
|---|---|---|---|---|---|
| `excel.workbook.open` | write | `l3_irreversible` | high | true | 仅 `com.workbook_open` |
| `excel.workbook.read` | read | `none_readonly` | low | false | `com.workbook_read` |
| `excel.range.read` | read | `none_readonly` | low | false | `com.range_read` |
| `excel.range.summarize` | read | `none_readonly` | low | false | `com.range_read` |

约束：

- `excel.workbook.open` 的 capabilities 必须恰好一项，不得与任何 read/write capability 混用；
- 三个 read Tool 不得声明 `com.workbook_open`，不得内部隐藏 open 或写入；
- `excel.workbook.close` 不进入模型 Tool surface；清理由 task runtime 生命周期完成；
- 任何 Tool 名、effect、reversibility、risk 或 approval 变化都需新 ADR。

### D2：open 输入与专用实例

`excel.workbook.open` 的输入固定为：

```json
{
  "workbook_path": "D:\\data\\book.xlsx",
  "read_only": true
}
```

- `workbook_path` 只能是本地绝对路径，扩展名必须精确为 `.xlsx`；
- 拒绝 URL、UNC、WebDAV、OneDrive/SharePoint URL、相对路径、`..`、目录和非 `.xlsx`；
- `read_only` 必须显式为 `true`；缺失、false 或类型漂移失败；
- 不提供 password、macro、update-links、Save As、Visible 或任意 COM property 输入；
- 创建 COM 前若存在任何 `EXCEL.EXE`，必须 fail closed，不附着、不复用、不终止用户进程；
- 只允许一个专用、owned、hidden Excel process；同一 task 同时最多一个 workbook session。

### D3：open 输出与 session

成功时返回：

```json
{
  "session_id": "opaque-task-scoped-token",
  "expires_at_utc": "2026-11-23T00:15:00Z",
  "workbook": {
    "identity_kind": "full_path",
    "identity_value": "D:\\data\\book.xlsx",
    "read_only": true
  },
  "worksheets": [
    {"name": "Sheet1", "ordinal": 1}
  ]
}
```

- `session_id` 是不透明 token，不是 COM pointer、HWND、PID 或可推测路径；
- session 只绑定 `(tool_bus_session_id, task_id, owned_process_identity, workbook_identity)`；
  `tool_bus_session_id` 必须与传给 `ToolBusConfig::new` 的 session id 同源，
  由 assembly 在构造 handler/registry 时注入，registry 按 ToolBus 隔离；
  不得为传播该值新增 `CallContext` 公共字段；
- 输入 `session_id` 的 UTF-8 长度必须在 32..=128 bytes；
- token 在 TTL 内可供同一 ToolBus session/task 多次 `workbook.read` / `range.read` /
  `range.summarize` 复用，不是单次消费 token；未知、格式非法、过期、跨 task 或跨 ToolBus 一律失败；
- 所有读取在 dedicated Excel STA 线程按 ToolBus 到达顺序线性化；
- task 结束、cleanup 开始或 TTL 到期后 token 立即失效，不得接受排队中的新读取；
- token 默认 TTL 15 分钟，且绝不能超过 task 生命周期；
- token 只出现在成功 Tool output 与内存 session registry；不得写日志、审计、DB、
  任务恢复记录或文件名；
- 输出只含可序列化数据；COM handle 留在 handler 专用线程；
- workbook identity 必须与 open 请求路径和 owned PID/process identity 交叉复验。

### D4：workbook read

`excel.workbook.read` 输入只含 `session_id`。输出包含：

- workbook identity 与 read-only 状态；
- worksheet `name/ordinal/used_range_address`；
- workbook-scoped named ranges 与 sheet-scoped named ranges；
- calculation mode / calculation state；
- session expiry 与剩余可用状态。

`excel.workbook.read` 的完整输出边界固定为：

- 规范化、紧凑 UTF-8 序列化后最多 262144 bytes；
- worksheets 最多 1024 个；
- workbook/sheet-scoped named ranges 合计最多 2048 个；
- 任一上限超出时以 `UserInteraction` 整体失败，不截断、不删项、不返回部分成功。

该 Tool 不返回 cell values；所有 app-derived 文本按不可信 `app_content` 处理。

### D5：range identity 与 read mode

range 引用固定为封闭对象，必须恰有一种：

```json
{"kind": "named_range", "name": "SalesData"}
{"kind": "a1", "worksheet": {"kind": "name", "value": "Sheet1"}, "range": "A1:C10"}
{"kind": "a1", "worksheet": {"kind": "ordinal", "value": 1}, "range": "A1:C10"}
```

- named range 优先；A1 是调用方显式 fallback；
- worksheet name 必须精确匹配；ordinal 从 1 开始且必须在 open 输出中出现；
- named range 解析为 0 或多义结果时失败，不得取第一个；
- `excel.range.read` 的 `mode` 必须显式是 `value2|formula|text` 之一；
- `excel.range.summarize` 的 `mode` 固定 `value2`；formula/text 汇总失败。

### D6：read 输出边界

`excel.range.read` 最多 4096 cells 和 256 KiB 序列化 payload：

- 每个 cell 含 one-based `row/column`、A1 `address`、`mode`、`value_type`、`value_text`；
- `value_type` 闭集：`blank|number|text|boolean|error`；
- number 使用规范化十进制字符串，避免 JSON 浮点精度漂移；
- formula 返回调用方显式选择的原公式字符串；text 返回显示文本；
- 超限、不可归一化 floating point、未知 error 或 cell shape 漂移一律失败；
- 不截断、不返回部分范围、不用空字符串冒充 blank。

`excel.range.summarize` 对同一范围输出：

```json
{
  "count": 10,
  "non_numeric_count": 0,
  "sum": "42.5",
  "min": "1",
  "max": "20"
}
```

所有数值输出为规范化十进制字符串；空范围或无数值 cell 失败，而不是返回全零。

### D7：postcondition 与 rollback

- open Tool 的 postcondition 固定 `app_reported`：
  `key=excel.workbook.session_open`、`value=true`、description 非空、`error_code=Fatal`；
  handler 另须在输出前独立复核 owned process/workbook identity，不能把该布尔值当唯一证据；
- open Tool rollback coverage 固定 `no_recovery`，reason 必须说明“专用进程清理由 task runtime
  完成，模型无 close Tool”，且 `on_rollback_failure=publish_incident_and_require_human`；
- 三个 read Tool 固定至少一个 `state_unchanged` postcondition，
  `fingerprint_scope=workbook`、description 非空、`error_code=VerifyFailed`；
- read Tool 不得有 rollback coverage。

### D8：task-end cleanup

无论成功、失败、取消或 session TTL 到期：

1. session registry 标记 closing，拒绝新调用并使 token 立即失效；
2. 在任何可能阻塞的 `Open` / `Close` / `Quit` 前，把 owned PID、start ticks、
   executable path、workbook path 与非秘密的 task/session 关联写入仓库外 ownership journal；
3. 在独立线程启动 watchdog；watchdog 不共享可能阻塞的 STA 调用，
   按 journal 复核 PID/start ticks/executable path 后只回收仍匹配的 owned 进程树；
4. 在创建 COM 的 STA 线程释放 Range/Worksheet 子对象引用；
5. 同一 STA 线程调用 `Workbook.Close(false)`，绝不保存临时或用户工作簿；
6. 同一 STA 线程调用 `Application.Quit()`；
7. 最后释放 Workbook/Application COM 引用；
8. watchdog 与 task runtime 分别有界等待并 reap；只有 journal 匹配时才可 kill，
   绝不终止未复核的用户进程；
9. cleanup 成功并确认 owned process tree 收敛后删除 journal 与 active session；
   残留返回 Fatal/UserInteraction，发布 incident，不得成功返回；
10. 增加 session/task 资源收敛测试，覆盖 Open/Close/Quit 阻塞、
    watchdog 只回收 owned 进程以及重复任务不增长 session/进程。

### D9：错误映射

- 既有 Excel 进程、Protected View、外链、宏、只读/占用冲突、应用对话框或用户取消
  → `UserInteraction`；
- workbook 不存在 → `TargetNotFound`；
- 文件存在但不可读/无权限 → `PlatformPermission`；
- unsupported extension、本地路径形状、`read_only`、mode、range shape、超过
  4096 cells 或 256 KiB range payload → `ToolInvalidArgs`；
- 加密、损坏或无法解释的工作簿 → `UserInteraction`；
- `workbook.read` 超过 262144 bytes、1024 worksheets 或 2048 named ranges
  → `UserInteraction`；
- session 未知、过期、跨 task、跨 ToolBus 或格式非法，以及 summarize 无数值 cell
  → `ToolInvalidArgs`；不得映射成可重试的 `TargetNotFound`；
- 合法 session 但 workbook identity 漂移 → `TargetNotFound`；
- worksheet/named range 0 命中 → `TargetNotFound`；多义 → `TargetAmbiguous`；
- 未知 Excel error → `UserInteraction`；
- 时间字段无法形成合法 UTC → `Fatal`；
- COM 初始化、ownership journal、watchdog 或 session registry 结构性失败 → `Fatal`；
- cleanup 结构失败 → `Fatal`；应用对话框或人工拒绝 → `UserInteraction`；
  任何 cleanup 失败都绝不能成功返回；
- 不新增 ErrorCode。

### D10：实现前置与真机验收

实现卡必须：

- 先加 fake Excel runtime/session registry/负向测试，再接真实 COM；
- 用真实本地 `.xlsx` fixture 做人类在场单次验证与重复运行；
- 证明读取前后 workbook 字节/文件 mtime 不变，Excel 进程收敛为 0；
- 证明已有 `EXCEL.EXE` 拒绝路径、session 跨 task 拒绝、范围超限拒绝；
- 未通过真实 COM 前不得把四个 Tool 写入 `tools.json` 或注册到 ToolBus。

## 拒绝条件

- 新增 `excel.workbook.close`、通用 `excel.com.invoke`、自由 COM property/method 或脚本输入；
- 复用/终止用户 Excel、打开用户现有实例中的 workbook、自动 Enable Editing 或更新外链；
- open Tool 携带 read capability，或 read Tool 声明 open/write capability；
- session token 向其他 task/session 生效，或把 handle/PID/HWND 当 token；
- 超过 4096 cells / 256 KiB 后截断返回、返回部分范围或“成功但空”；
- 用 UIA、clipboard、keyboard 替代 COM 读取；
- 把本 ADR 合并解释为 runner、Tool、Adapter 或真机已验证。

## 后果

- 后续实现卡可以按固定 Tool/session 形状写 fake runtime 和负向测试；
- Adapter validator 可在 Tool JSON 出现时检查 exact capability/effect/approval；
- 代价是 T2.1 需要先维护 task-scoped session registry 和专用线程，不能把 COM 对象直接交给模型。

## 验证方式

1. `adr-index`、文档门禁与 cross-link 全绿；
2. spec 覆盖四个 Tool、输入输出、session、ownership、bounds、错误与 cleanup；
3. `adapters/**`、`protocol/**`、Rust/TS 产品代码和依赖零差异；
4. 下一实现卡用 fake tests 和人类在场真实 `.xlsx` 验证全部 DoD。

## 何时重新讨论

支持 `.xlsm/.xlsb`、多 workbook session、长期 session、模型 close Tool、A1 之外的其他引用、
返回超过 bounds、增加新的 open/read/summarize Tool 或修改 session 传播时，必须新 ADR。

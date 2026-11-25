# spec: Excel T2.1 read-only workflow

> 状态：Draft（ADR-0108 Accepted）　版本：0.1　日期：2026-11-23
> 上位：`docs/adr/0108-excel-t21-readonly-tool-contract.md`
> 强制性：本文档冻结实现前的 Tool/session/cleanup 契约。实现未合入前，`tools.json` 必须保持空。

## 1. 范围

本文档定义 Excel T2.1 只读工作流的四个 Tool、session 传播、owned Excel process、
bounded range payload、错误映射与 task-end cleanup。

不定义：

- COM IDispatch 的具体 Rust 类型或 handler 文件；
- ToolSchema JSON 实例、tools.json 或 rollback recipe；
- `.xlsm/.xlsb`、宏、外链更新、公式写入或任何 T2.2/T2.3 能力；
- UI、Policy、ToolBus 注册或数据库 schema。

## 2. Tool 总表

| Tool | effect | reversibility | risk | approval | idempotent | capabilities |
|---|---|---|---|---|---|---|
| `excel.workbook.open` | write | `l3_irreversible` | high | true | false | `["com.workbook_open"]` |
| `excel.workbook.read` | read | `none_readonly` | low | false | true | `["com.workbook_read"]` |
| `excel.range.read` | read | `none_readonly` | low | false | true | `["com.range_read"]` |
| `excel.range.summarize` | read | `none_readonly` | low | false | true | `["com.range_read"]` |

capability 数组必须满足 ADR-0104/0105 的 byte-order、非空、去重和 catalog 引用规则。

## 3. `excel.workbook.open`

### 3.1 输入

```json
{
  "type": "object",
  "required": ["workbook_path", "read_only"],
  "additionalProperties": false,
  "properties": {
    "workbook_path": {"type": "string"},
    "read_only": {"const": true}
  }
}
```

路径校验：

- 本地绝对路径；
- 扩展名精确 `.xlsx`；
- 拒绝 URI、UNC、WebDAV、OneDrive/SharePoint URL、`.`、`..`、NUL、目录；
- canonical path 必须仍是 `.xlsx` 常规文件。

### 3.2 输出

```json
{
  "type": "object",
  "required": ["session_id", "expires_at_utc", "workbook", "worksheets"],
  "additionalProperties": false,
  "properties": {
    "session_id": {"type": "string", "minLength": 32, "maxLength": 128},
    "expires_at_utc": {"type": "string"},
    "workbook": {
      "type": "object",
      "required": ["identity_kind", "identity_value", "read_only"],
      "additionalProperties": false,
      "properties": {
        "identity_kind": {"const": "full_path"},
        "identity_value": {"type": "string"},
        "read_only": {"const": true}
      }
    },
    "worksheets": {
      "type": "array",
      "maxItems": 1024,
      "items": {
        "type": "object",
        "required": ["name", "ordinal"],
        "additionalProperties": false,
        "properties": {
          "name": {"type": "string", "minLength": 1},
          "ordinal": {"type": "integer", "minimum": 1}
        }
      }
    }
  }
}
```

session 规则：

- token 必须来自 CSPRNG 或等价不可猜测来源；
- 只绑定 `(tool_bus_session_id, task_id, owned_process_identity, workbook_identity)`；
- `tool_bus_session_id` 必须与传给 `ToolBusConfig::new` 的 session id 同源，
  由 assembly 在构造 handler/registry 时注入；registry 按 ToolBus 隔离；
  不得新增或修改 `CallContext` 公共字段来传播它；
- 输入 `session_id` 的 UTF-8 长度必须在 32..=128 bytes，且必须通过 opaque token 格式校验；
- token 在 TTL 内供同一 ToolBus session/task 多次 read/summarize 复用，不是单次消费 token；
- 所有读取由 dedicated Excel STA 线程按到达顺序线性化；
- task 结束、cleanup 开始或 TTL 到期后 token 立即失效，排队中的新读取也不得执行；
- 同一 task 同时最多 1 个 active session；
- TTL 固定 15 分钟且不得跨 task 结束；
- token 只出现在成功 Tool output 与内存 session registry；不得写日志、审计、DB、
  任务恢复记录或文件名。

## 4. `excel.workbook.read`

输入：

```json
{
  "type": "object",
  "required": ["session_id"],
  "additionalProperties": false,
  "properties": {
    "session_id": {"type": "string", "minLength": 32, "maxLength": 128}
  }
}
```

输出按稳定排序：

- `workbook` identity/read_only；
- `worksheets[]`：`name`、`ordinal`、`used_range_address`；
- `named_ranges[]`：`name`、`scope_kind=workbook|worksheet`、`scope_name`、
  `refers_to`；
- `calculation`：`mode`、`state`；
- `session`：`expires_at_utc`、`remaining_ms`。

bounds：

- 规范化、紧凑 UTF-8 序列化后完整输出 `<= 262144` bytes；
- `worksheets[]` `<= 1024`；
- workbook-scoped 与 sheet-scoped named ranges 合计 `<= 2048`；
- 任一上限超出返回 `UserInteraction`，禁止 ToolBus 自动截断或丢弃条目。

`refers_to` 等 app-derived 文本必须标记 `app_content` 并经过 ToolBus bounds。

## 5. Range identity

```json
{"kind": "named_range", "name": "SalesData"}
{"kind": "a1", "worksheet": {"kind": "name", "value": "Sheet1"}, "range": "A1:C10"}
{"kind": "a1", "worksheet": {"kind": "ordinal", "value": 1}, "range": "A1:C10"}
```

规则：

- exactly-one union；
- named range 名称非空、最长 255；
- worksheet name 精确匹配，不能使用本地化显示文本 fallback；
- ordinal 必须出现在当前 open 输出，从 1 开始；
- A1 必须由有界 parser 校验，只允许 `$A$1` / `A1` / `A1:C10`；
- 解析多义或 0 命中 fail closed。

## 6. `excel.range.read`

输入：

```json
{
  "session_id": "opaque",
  "range": {"kind": "named_range", "name": "SalesData"},
  "mode": "value2|formula|text"
}
```

输出：

```json
{
  "range": {"address": "A1:C10", "rows": 10, "columns": 3},
  "cells": [
    {
      "row": 1,
      "column": 1,
      "address": "A1",
      "mode": "value2",
      "value_type": "number",
      "value_text": "5"
    }
  ]
}
```

bounds：

- `rows * columns <= 4096`；
- 序列化 payload `<= 262144` bytes；
- 空白 cell 必须显式 `value_type=blank`，`value_text` 为空字符串；
- number 规范化十进制字符串；
- boolean 只允许 `true|false`；
- Excel error 的 `value_text` 使用稳定错误文本，未知错误失败；
- 不截断、不跳 cell、不返回部分成功。

## 7. `excel.range.summarize`

输入与 range.read 相同，但 `mode` 固定 `value2`。

输出：

```json
{
  "range": {"address": "A1:C10", "rows": 10, "columns": 3},
  "count": 30,
  "non_numeric_count": 2,
  "sum": "42.5",
  "min": "1",
  "max": "20"
}
```

- `count = rows * columns`；
- `sum/min/max` 只统计 finite number；
- `non_numeric_count` 包含 blank/text/boolean/error；
- 无数值 cell 或 sum overflow/non-finite 失败；
- 数值输出全部为规范化十进制字符串。

## 8. Postconditions

open：

- executable `app_reported`：
  - `key=excel.workbook.session_open`
  - `value=true`
  - description 非空
  - `error_code=Fatal`
- handler 必须在返回前独立复核 session id、workbook identity 与 owned process identity；
- rollback coverage 恰一行 `no_recovery`：
  - reason: `dedicated_process_cleanup_is_task_lifecycle_owned`;
  - `on_rollback_failure=publish_incident_and_require_human`。

read：

- executable `state_unchanged`；
- fingerprint scope = `workbook`；
- description 非空、`error_code=VerifyFailed`；
- 不得有 rollback coverage。

## 9. Cleanup

task runtime 在任何退出路径执行：

1. session registry mark closing，拒绝新调用；
2. 在可能阻塞的 `Open` / `Close` / `Quit` 前，写入仓库外 ownership journal：
   owned PID、start ticks、executable path、workbook path 与非秘密 task/session 关联；
3. 启动不共享 STA 阻塞路径的独立 watchdog；它按 journal 复核 PID/start ticks/path 后，
   只回收仍匹配的 owned 进程树；
4. 在 dedicated STA thread 释放 Range/Worksheet 子对象；
5. `Workbook.Close(false)`；
6. `Application.Quit()`；
7. 最后释放 Workbook/Application COM 引用；
8. watchdog 与 task runtime 分别 bounded wait + reap；未复核进程绝不 kill；
9. 确认进程树收敛后删除 ownership journal 与 active session；
10. cleanup 失败写 incident，不返回成功。

注册表硬上限：

- 每 task 1 active session；
- 全局 8 active sessions；
- 过期或结束立即淘汰；
- 达到上限拒绝新 open，不驱逐仍 active 的 session。

## 10. Error matrix

| 条件 | ErrorCode |
|---|---|
| 既有 Excel、Protected View、外链、宏、只读/占用冲突、应用对话框、用户取消 | `UserInteraction` |
| workbook 不存在 | `TargetNotFound` |
| 文件存在但不可读/无权限 | `PlatformPermission` |
| unsupported extension、本地路径形状、read_only、range shape、mode、range bounds | `ToolInvalidArgs` |
| 加密、损坏或无法解释的工作簿 | `UserInteraction` |
| `workbook.read` 超过 bytes/worksheets/named-ranges 上限 | `UserInteraction` |
| session 未知/过期/跨 task/跨 ToolBus/格式非法 | `ToolInvalidArgs` |
| summarize 无数值 cell | `ToolInvalidArgs` |
| 合法 session 但 workbook identity 漂移 | `TargetNotFound` |
| worksheet/named range 0 命中 | `TargetNotFound` |
| worksheet/named range 多义 | `TargetAmbiguous` |
| 未知 Excel error | `UserInteraction` |
| 时间字段无法形成合法 UTC | `Fatal` |
| COM 初始化、ownership journal、watchdog、session registry 结构失败 | `Fatal` |
| cleanup 结构失败 | `Fatal` |
| cleanup 中应用对话框或人工拒绝 | `UserInteraction` |

不得新增 ErrorCode；任何 cleanup 失败都不得降级成可重试成功。

## 11. Implementation gates

> **ADR-0110 normative correction（2026-11-24）**：ADR-0108 D10 优先。真实 COM、
> 人类在场单次 `.xlsx` 验收与重复收敛全部通过前，生产
> `adapters/com.microsoft.excel/tools/tools.json` 必须保持空，生产 ToolBus 不得注册。
>
> 有效顺序：
> 1. fake runtime + session registry + parser；
> 2. 私有 `ToolHandler` 包装与 in-process ToolBus 合同测试；
> 3. Windows COM runner 私有原型与 fake tests；
> 4. 人类在场单次 `.xlsx` 验收；
> 5. 重复运行与 process/session/handle 收敛；
> 6. 最后才填写生产 tools/rollback/assembly 并注册四个 Tool。
>
> 下方旧 gate 文本仅保留历史内容；与本节冲突时以本节及 ADR-0110 为准。

> **ADR-0111 runner injection boundary（2026-11-25）**：`assistant-platform-windows`
> 可以提供窄公共 `ExcelComRunner` handle，但只能暴露自包含、可跨线程请求的
> create/PID/read-only open/cell text/read/close/shutdown；COM object 和 STA guard 不得
> 离开 dedicated worker。`agent-core` 继续持有 `ExcelT21Runtime` 领域类型与映射，不得
> 把 Excel 专用 trait 加入 `assistant-platform-api`。TASK-401 只验证 runner 边界与人工
> 单工作簿 smoke；完整四 Tool 域映射、ownership journal/watchdog、重复收敛和生产注册
> 仍由后续卡完成。

> **ADR-0112 read-only runner surface（2026-11-25）**：后续 TASK-402 在
> `assistant-platform-windows` 内补齐 workbook/worksheet/named-range/calculation metadata、
> `value2|formula|text` range read、deterministic metadata fingerprint 与 ownership
> verification。平台结果全部为自包含类型；range 上限 4096 cells，A1 最大
> `XFD1048576`。fingerprint 只证明 metadata 稳定，不代表文件内容 digest。agent-core
> 的 `ExcelT21Runtime` 映射和重复运行收敛仍由下一卡完成。

下一实现卡顺序固定：

1. fake runtime + session registry + input/range parser 与负向测试；
2. Adapter tool envelopes/rollback，但仍不注册 ToolBus；
3. Windows COM runner 私有模块与 fake tests；
4. 人类在场本地 `.xlsx` 单次验收；
5. 重复运行、process/session/handle 收敛证据；
6. 只有全部通过后才注册四个 Tool。

真机 DoD：

- 读取前后文件 size/mtime/digest 不变；
- workbook 不保存；
- 任何时候 0 个本轮 `EXCEL.EXE` 残留；
- 既有 `EXCEL.EXE` 拒绝场景保持用户进程存活；
- mode、named range、A1 fallback、bounds、session cross-task 各有正负证据。

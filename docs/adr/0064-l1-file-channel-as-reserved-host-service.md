# ADR-0064　L1 文件通道以 binary 层保留 `host_service` 表达

状态：**Accepted**（2026-10-03 自动化按用户「自行裁决最优方案」授权接受；TASK-223）
日期：2026-10-03
Supersedes：—
Superseded by：—
关联：**TASK-223**、ADR-0061 D5/D7、ADR-0063、`docs/spec/runtime-execution.md`、
`adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`

---

## 背景

T1.1 的正常小文件路径使用 UIA `notepad.file.read_text`，大文件路径声明
`kind = "l1_file"` / `operation = "read_utf8_prefix"`。但 ADR-0061 D5 已明确：
`l1_file` 是任务前置条件 / 文件通道标记，**不是运行时步骤种类**。当前 renderer 因此把
该步骤记为 `declared_not_executed`，导致 T1.1 的大文件分支只存在于声明故事里，没有可执行路径。

直接扩展 `PlanStep` 或把 `l1_file` 塞进运行时闭集会违反 ADR-0061 D5，并改动公共形状。
正确做法是把文件通道能力表达为一个 **binary 层保留 `host_service` 工具**，与
`inspect_target_path` / `set_editor_value` 同型：只进 Planner 目录，不进模型可见 ToolBus。

## 决策（一句话）

**保留 `l1_file` 的语义不变，新增 `assistant.runtime.host_read_utf8_prefix`
作为 binary 层 `host_service` 工具，用有界 UTF-8 前缀读取表达大文件文件通道降级。**

## 决策细化

| # | 内容 |
|---|---|
| **D1 `l1_file` 语义不变** | `l1_file` 仍是任务前置 / 文件通道标记，不由运行时执行；本 ADR 不修改 `PlanStep`、`Plan`、`Planner` 或 `task-engine` 的公共形状。 |
| **D2 保留工具名** | 新增 Planner 目录工具 `assistant.runtime.host_read_utf8_prefix`。它只在 binary 层执行，不注册进模型可见 ToolBus；模型不得直接调用。 |
| **D3 参数合同** | 输入为绝对 `target_path` 与正整数 `max_text_bytes`。相对路径、含 `..` 的路径、空/非正预算一律 `ToolInvalidArgs`；文件不存在或不可打开按明确 `ErrorCode` 失败，缺失文件为 `TargetNotFound`。 |
| **D4 有界读取** | 最多读取 `min(file_size, max_text_bytes)` 字节，并预留最多 3 字节 UTF-8 回退窗口；实现不得把整文件读入内存。实现设硬上限 `MAX_READ_UTF8_PREFIX_BYTES = 16 MiB`，超过上限拒绝而不是扩大内存。 |
| **D5 UTF-8 边界** | 不得在 UTF-8 码点中间截断。若预算正好落在多字节码点中，只回退到最近合法边界；`bytes_read` 必须报告实际返回的字节数。 |
| **D6 显式截断** | 输出必须包含 `text` / `truncated` / `bytes_read` / `bytes_total` / `fingerprint`。`truncated = file_size > bytes_read`；不得用空文本或默认值冒充完整读取。 |
| **D7 fingerprint 来源** | 读文件本身不改变目标应用状态；成功信封的 `fingerprint` 必须来自当前已提交状态或本操作可验证的只读观察，不得伪造“状态已改变”。 |
| **D8 条件仍归 ADR-0061 D8** | `when` 表达式、输出发布与跳过步骤语义不变。本 ADR 只新增一个 `host_service` operation，不改变条件求值器。 |
| **D9 依赖与边界** | 不新增第三方依赖；不新增 crate；文件 IO 只出现在 binary 层 host operations 实现内。 |

## 被否决的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 把 `l1_file` 加入运行时步骤闭集 | ❌ | 直接违反 ADR-0061 D5，并把文件通道标记和运行时执行种类混为一谈。 |
| 给 `PlanStep` 增加文件读取专属字段 | ❌ | 改公共形状，波及 Planner / task-engine / 全部测试。 |
| 让模型直接调用通用文件读取工具 | ❌ | 扩大模型可见攻击面；保留工具必须只在 Planner 目录。 |
| 一次性读完整文件再截断 | ❌ | 违反 ADR-0063 的有界原则，1 MB 以上的文件会放大内存尖峰。 |
| 在码点中间直接截断并返回非法 UTF-8 | ❌ | 会让调用方拿到无效文本，属于静默数据损坏。 |

## 影响

- T1.1 的 `read_file_channel` 可改为 `kind = "host_service"` +
  `operation = "read_utf8_prefix"`，继续由 `when` 区分大文件分支。
- `assistant.runtime.host_read_utf8_prefix` 进入保留工具闭集与 Planner schemas；
  模型可见工具集不变。
- `ReservedHostOperations` 增加该只读操作；实现必须在 binary 层完成。
- 该 ADR 不解决“条件为假的第一个步骤尚无 fingerprint”这一通用运行时问题；
  若任务包需要完整的大文件 Plan 执行，必须另立 ADR/卡处理条件跳过语义。

## 验证方式

1. `runtime_tools` 的闭集与 `tool_for_step("host_service", Some("read_utf8_prefix"))`
   映射到 `assistant.runtime.host_read_utf8_prefix`。
2. 单测覆盖：正常前缀、UTF-8 多字节边界不截断、相对路径 / `..` / 非正预算拒绝、
   文件缺失 `TargetNotFound`。
3. T1.1 Plan 合同测试确认大文件步骤以 `host_service` 进入数据流，且不再把
   `read_utf8_prefix` 记为 `l1_file` 的未执行项。

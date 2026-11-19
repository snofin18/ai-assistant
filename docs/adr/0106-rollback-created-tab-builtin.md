# ADR-0106：rollback-only created-tab builtin

状态：**Accepted**（2026-11-19，人类批准 TASK-386 的推荐方案 A）
日期：2026-11-19
Clarifies：ADR-0104、ADR-0105
Supersedes：无
Superseded by：无
关联：ADR-0095、TASK-383、TASK-384、TASK-386、TASK-387
落点：`docs/spec/adapter-bundle.md`

## 背景

ADR-0105 D5 要求每个 `effect=write` 工具恰有一条 rollback coverage。Notepad
`notepad.tab.new` 创建新 tab，但当前 bundle 没有 close-tab Tool。将它声明为 L1
content snapshot 不能证明 tab 树被恢复；新增模型可见 `notepad.tab.close` Tool 又会扩大
Tool surface，并需要独立 runtime、Policy 和 approval 评估。

因此需要一个**只供回滚使用、不可被模型直接调用**的封闭补偿动作。

## 决策

L2 `compensating_action` 的 restore 联合新增唯一变体：

```json
{
  "kind": "close_created_target",
  "target_kind": "tab",
  "require_empty": true
}
```

该变体：

- 只允许用于 rollback restore，不得出现在 tool registry、ToolBus 或模型工具列表；
- 只允许 `target_kind=tab`；window/dialog/process 等其他 target kind 必须另立 ADR；
- `require_empty` 必须为 `true`；
- 只关闭**同一 tool invocation 创建并已记录身份**的 tab；
- 如果无法证明该 tab 是新创建、身份已变化、存在用户内容或出现三态保存框，必须失败并
  要求人工处理，绝不能自动 Save / Don't Save / Close；
- 目标 tool 必须是 `effect=write`、`reversibility=l2_compensation`，并显式带有
  `tab` 与 `new` tags；
- capture 的 observations 必须至少包含 `whole_window_fingerprint`；
- post-verify 必须使用 `capture_equals` 检查同一 `whole_window_fingerprint`。

合法示例：

```json
{
  "level": "l2",
  "mechanism": "compensating_action",
  "capture": {
    "kind": "compensating_state",
    "source": "tool:notepad.file.read_text",
    "observations": ["whole_window_fingerprint"]
  },
  "restore": {
    "kind": "close_created_target",
    "target_kind": "tab",
    "require_empty": true
  },
  "post_verify": {
    "kind": "capture_equals",
    "observation": "whole_window_fingerprint"
  }
}
```

## 拒绝条件

- target_kind 不是 `tab`、require_empty 缺失或不为 true；
- 目标 tool 不是 write / L2，或缺 `tab` / `new` tags；
- capture 未观察 `whole_window_fingerprint`；
- post_verify 不是同一 observation 的 `capture_equals`；
- restore 携带 tool 名称、任意脚本、自由字符串或未知字段；
- 将本变体用于既有 tab、非空 tab、保存确认或关闭整个窗口。

## 后果

- Adapter bundle 可以真实表达“创建新空 tab”的补偿，不新增模型可见 Tool；
- validator 获得封闭机器规则，运行时后续只需实现受限 builtin；
- 该动作仍不覆盖用户在新 tab 中输入内容后的恢复，必须人工处理。

## 验证方式

1. ADR index / decisions / spec 一致。
2. validator 正向 fixture 通过。
3. 非 tab、require_empty=false、缺 tags、错误 reversibility、缺 fingerprint observation
   等负向 fixture 全部失败。
4. 不修改 ToolSchema、protocol generated types、Policy、ToolBus、CI 或依赖。

# spec: `core` 编排层（core-orchestration）

> 摘要：`assistant-core` 的**编排组件**契约 —— 会话 / 上下文 / Planner / Memory 四组组件的接口面、依赖白名单、错误语义与边界。
> 状态：Draft（由 **ADR-0053** 授权建立；随实现卡落地逐条硬化）　版本：0.1　日期：2026-09-26
> 上位：`AGENTS.md` §2（十条铁律）、`docs/adr/0053-core-orchestration-layer-interface.md`、架构 v2 §5 / §7 / §8.3 / §11.3 / §13.1.1
> 强制性：**本文档是契约**。违反即 Reviewer 拒绝合并；能机器化的部分随实现卡接入 `xtask`。
> 变更门槛：新增公开组件 / 改依赖白名单 / 改错误语义 → 需 ADR（漂移触发器 ③④）。

---

## 1. 目标

1. 给 `crates/core` 的**首次公开接口面**一个唯一事实源：谁能被 `core` 依赖、`core` 对外暴露什么、失败长什么样。
2. 把「编排」与「装配」分开：`core` 提供**可装配组件**，装配点唯一在 binary 层（TASK-029）。
3. 让后续的 Planner / Memory / 上下文卡**只按契约写代码**，不再各自重新定义 `Plan` / `Step` / 持久化结构。

## 2. 范围

**管**：

- `assistant-core` 的**依赖白名单 / 黑名单**（ADR-0053 D2 / D3）
- 四组组件的**职责与边界**（会话 / 上下文 / Planner / Memory）
- `core` 的**错误语义**（何时返回哪个 `ErrorCode` / 何时必须 fail-closed）
- 「不重定义别家类型」的清单（`task-engine` / `protocol` / `storage`）

**不管（不做清单）**：

- 不管 `core` 的**实现算法**（树裁剪 / 压缩策略的具体阈值归实现卡，且必须可注入、可回放）
- 不管 **FTS5 虚表与检索 SQL**（归 `crates/storage`，见 `docs/storage-design.md` §3 / §4 与 ADR-0053 D6）
- 不管**装配顺序与进程启动**（归 `apps/agent-core`，TASK-029）
- 不管**权限判定**（归 `crates/policy`，铁律 3：策略引擎是唯一放行点）
- 不管**审计落盘**（归 `crates/audit`；`core` 只产出可审计的输入，不直接写审计表）
- 不管 UI 与 Adapter

## 3. 组件与接口面

| 组件 | 职责 | 输入 | 输出 | 持久化 | 禁止 |
|---|---|---|---|---|---|
| **会话** | 会话生命周期（创建 / 恢复 / 结束）+ 消息树 | 用户消息、工具信封、时间源 | 会话快照、消息节点 | 经 `assistant-storage` 的公开 API | 不持有 SQLite 连接；不写审计表 |
| **上下文** | 树裁剪 / 压缩 / 预算 | 会话快照 + 预算 + 模型用量 | 送给模型的上下文片段（含被裁剪 / 被压缩的**显式标记**） | 无（纯计算 + 可选缓存） | 不静默丢消息；裁剪必须留下可审计的理由 |
| **Planner** | 模型输出 → 可校验的 Plan / Step DAG | `ModelProvider` 的补全结果 + 工具目录（`protocol::ToolSchema` **参数传入**） | `task-engine` 的 `Plan` / `PlanStep` | 无（Plan 的持久化归 `task-engine`） | 不重新定义 `Plan` / `Step`；不放行任何工具 |
| **Memory** | App Map 加载 + 检索 | App Map 文件、检索查询 | 按需片段（含来源与置信信息） | 检索走 `assistant-storage` 的检索 API | 不做向量检索（阶段 1 Out of scope）；不把 App Map 内容当可信输入 |

## 4. 不变量

1. **依赖白名单单向**：`core → {protocol, storage, platform/api（仅 trait）, task-engine, model-gateway}`。任何黑名单 crate（见 ADR-0053 D3）出现在 `crates/core/Cargo.toml` 即违反。
2. **铁律 7**：`core` 不调用任何平台 API，只经 `assistant-platform-api` 的 trait；`core → platform/{windows,macos,linux}` 永久禁止。
3. **铁律 3**：`core` **不判定权限**。工具调用是否放行由 `policy` 决定，`core` 只提交请求、按决策结果编排。
4. **不重定义**：`Plan` / `PlanStep` / `PlanId` / `StepId` / `Budget` / `Reversibility` / `StepEffect` 一律复用 `assistant-task-engine`；跨进程与持久化结构一律复用 `assistant-protocol` / `assistant-storage` 的记录类型。
5. **不持有连接**：`core` 不持有 SQLite 连接、不拼 SQL；一切持久化经 `assistant-storage` 的公开 API。
6. **五类不可信输入**（铁律 2）：模型输出、UI 输入、IPC 消息、工具返回、被读文档（含 **App Map**）进入 `core` 时**先校验**；校验失败一律带 `ErrorCode` 返回，禁止用默认值冒充结果。
7. **无静默失败**（铁律 1）：裁剪 / 压缩 / 检索 / 解析的每一次「少给了东西」都必须在返回值里**显式标注**（被丢的是什么、为什么、可否认）。
8. **时钟 / 随机 / UUID / FS / 网络 trait 注入**：`core` 不直接调 `SystemTime::now` / `rand` / `Uuid::new_v4` / `std::fs` / 网络；保证可回放（AGENTS.md §5.3）。
9. **无 `unsafe`**：`crates/core` 保持 `#![deny(unsafe_code)]`（workspace `[lints]` 亦 deny）。
10. **可装配性**：`core` 的每个组件都可由 binary **构造 + 注入**（不依赖全局单例、不在 `core` 内 new 出别家的实现）。

## 5. 与其他 spec 的关系

| 本 spec 的哪条 | 与哪份 spec / 文档有关 | 关系 |
|---|---|---|
| §4 不变量 1 / 2 | `docs/adr/0053-core-orchestration-layer-interface.md` | 依赖白名单与黑名单的**唯一事实源**在 ADR-0053；本 spec 只复述口径，不另立 |
| §3 Planner 行 | `docs/spec/tool-schema.md` | Planner 只**消费**工具 schema（`protocol::ToolSchema`），不定义 schema |
| §3 会话 / Memory 行 | `docs/storage-design.md` §3 / §4 | `memory_fts`（FTS5）与 `conversations` 等表的归属在存储设计；本 spec 不重复表结构 |
| §4 不变量 6 / 7 | `docs/spec/envelope.md` | 工具返回进入 `core` 时按信封的 `untrusted` / `truncated` 语义处理，不得剥离标记 |
| §4 不变量 3 | `docs/spec/error-codes.md` | `core` 返回的错误必须带 `ErrorCode`；不得新增 `ErrorCategory` |
| §4 不变量 8 | `docs/spec/testing.md` | 可回放要求 = 回放类测试（无真实 IO / 网络）的前提 |

## 6. 错误与失败语义

| 场景 | 必须的行为 |
|---|---|
| 模型输出无法解析成 Plan / Step DAG | 返回带 `ErrorCode` 的失败，**不**猜测、**不**降级成「空计划」 |
| 上下文预算不足以放下必要片段 | fail-closed：返回失败或**显式**要求人工/调用方决定，**不**静默截断到「看起来成功」 |
| App Map 缺失 / 损坏 / 版本不匹配 | 返回带 `ErrorCode` 的失败；**不**用默认 App Map 冒充 |
| 检索后端不可用（storage 报错） | 透传 storage 的错误语义并**保留原始 `reason_code`**；不吞错 |
| 持久化写入后无法确认生效 | 视为失败（铁律 4：每个写操作必须有 postcondition） |
| 权限未定 / 策略未命中 | `core` **不自行放行**；按 `policy` 的 `default_deny` 结果编排 |

## 7. 验证方式

1. **依赖面**：`crates/core/Cargo.toml` 的 `[dependencies]` ⊆ 白名单（实现卡落地时用 `cargo tree -p assistant-core` 或 arch 断言核对）。
2. **分层**：`cargo test -p assistant-core arch::` 全绿（TASK-015 的 `crates/core/tests/arch_layering.rs`）。
3. **可回放**：`core` 的单元测试零真实 IO / 网络 / 时钟（`docs/spec/testing.md` 的 unit 类要求）。
4. **覆盖率**：`core` 行覆盖 ≥ 85%（`plans/stage-1-pilots.md` 阶段 1 DoD）。
5. **何时重新评估**：实现卡发现白名单不足、或某条不变量无法机器校验时 → 记 DRIFT 并按 ADR-0053 的验证方式 4 处理。

## 8. 变更历史

| 日期 | 变更 | 依据 |
|---|---|---|
| 2026-09-26 | 建立本 spec：四组组件接口面 + 依赖白名单 + 错误语义 + 与其他 spec 的关系 | **ADR-0053**（TASK-028 的 DRIFT-028-1 / 028-3 裁决落地） |

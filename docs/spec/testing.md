# spec: 测试规范（白盒 / 契约 / 回放 / 靶机）

> 摘要：测试规范（testing.md）：单元 / 契约 / 回放 / 靶机四类 + 覆盖率阈值。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准；其中 §4.4 测试 lint 允许清单已由 ADR-0027 批准）　版本：0.2　日期：2026-10-07
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

定义测试**分层与覆盖率口径**：unit / contract / replay / target-machine 四类各自管什么、禁什么，以及 workspace 与关键 crate 的覆盖率阈值。

## 2. 范围

**管**：四类测试的边界（IO 要求 / 速度）、覆盖率阈值（workspace ≥ 75%，`core` / `policy` / `task-engine` ≥ 85%）、白盒测试的输出可注入要求（§4.3）、测试代码的 lint 允许清单（§4.4）。

**不管（不做清单）**：
- 不管**具体用例**（各 crate 的 `tests/**` 自己写）
- 不管 CI 编排（归 `.github/workflows/ci.yml` 与 `docs/governance-ai-agent-execution.md` §5）
- 不管靶机 fixture 的**内容**（归 `fixtures/apps/**` 与各 Adapter 卡）
- 不管性能基准阈值（归各卡验收要点与 `docs/storage-design.md` 等专项文档）

## 3. 类型定义

测试四类 + 覆盖率阈值。

```
Test Type      | Scope                          | IO Required | Speed
---------------|--------------------------------|-------------|--------
Unit           | 纯逻辑函数                     | 否          | < 1 ms
Contract       | schema / type sync / ErrorCode | 否          | < 5 ms
Replay         | 录制树快照回放                 | 否          | < 10 ms
Target-machine | 真实应用 / Spike fixture       | 是          | 1-30 s

Coverage thresholds:
  - workspace: ≥ 75% lines
  - core / policy / task-engine: ≥ 85% lines
```

## 4. 不变量

1. **unit 零 IO**：禁止 unit test 碰文件 / 网络 / 进程；违反 = clippy::test 模块 lint。
2. **contract 测试必须覆盖 schema 边界**：空字段 / null / 重复 ID / 超大值。
3. **replay 测试必须用真实 fixture**：合成 fixture = 失败（= 不验证真实树形态）。
4. **target-machine 测试必须 human gate**：CI 不跑，需要人工本地运行（= 节省 CI 时间 + 防副作用）。
### 4.3 输出可注入

白盒测试**不得**捕获进程 stdout / stderr 来做断言：报告 / 日志的渲染目标必须作为参数可注入
（例：`xtask` 的 `report` 模块把 sink 作为参数传入），测试直接传 `Vec<u8>` 并断言输出文本。
理由：捕获进程级输出会让测试依赖运行时环境（编码 / 缓冲 / 并发），且无法在 unit 层回放。

### 4.4 测试代码的 lint 允许清单

workspace lint 对产品代码 `deny` 了 `clippy::unwrap_used` / `clippy::expect_used` / `clippy::panic`。
这三项在测试代码里**允许**，但唯一合法位置是紧邻 `#[cfg(test)] mod tests` 的模块属性（ADR-0027 D2）：

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    // 测试可以直接 fail-fast：断言失败或夹具构造失败时立即报错。
}
```

1. **产品代码禁止**：任何非测试模块不得添加这三项 `#[allow]`；违反属漂移触发器 ⑥（放宽护栏），必须升级人类裁决（ADR-0027 D3）。
2. **目的**：让测试 fail-fast，而不是把“不应失败”的产品路径变成崩溃（ADR-0027 D5）。
3. **与 ADR-0035 的关系**：ADR-0035 管的是**产品代码**的 per-line allow 与 workspace `exceptions`（B 路，禁止新增 `exceptions`）；本节的测试模块允许是模块级、作用域不同，不与其冲突（ADR-0035 baseline 已把测试 wrapper 列为「保留（合法）」）。
4. **负向覆盖不豁免**：允许 panic 不等于跳过错误分支断言；测试仍必须对错误路径做显式断言（ADR-0027 风险表）。

唯一决策来源 = ADR-0027《测试代码的 lint 允许清单》。

---

## 5. 与其他 spec 的关系

| 引用方向 | spec | 关系 |
|---|---|---|
| 相关 | `docs/spec/envelope.md` | contract 测试覆盖 envelope 的反序列化边界 |
| 相关 | `docs/spec/tool-schema.md` | contract 测试覆盖 Tool schema 边界（空 / null / 重复 ID / 超大值） |
| 相关 | `docs/spec/audit-event.md` | unit 测试覆盖 hash chain 的篡改检测 |
| 相关 | `docs/spec/capability-matrix.md` | policy 引擎单测覆盖矩阵的风险级与 Approval 列 |
| 相关 | `docs/spec/error-codes.md` | contract 测试校验 ErrorCode 完整性（13 类） |

---

## 附录：演进记录

| 版本 | 日期 | 变更 |
|---|---|---|
| 0.1 | 2026-09-20 | 初稿（项目进度督察后批量补齐 stage-0 DoD #5）|
| 0.2 | 2026-10-07 | ADR-0027 转 Accepted：新增 §4.4 测试代码 lint 允许清单（三项 lint 仅 `#[cfg(test)] mod tests`；产品代码禁止）；`DRIFT-W3-1` / `PL-098` 闭环（TASK-251）|

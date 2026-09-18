# 跨平台本地 AI 助理（Cross-Platform Local AI Assistant）

让大模型**受控地**操作你电脑上已有的常用应用（记事本、画图、Edge/Chrome、Excel、
Photoshop…）：模型负责理解与规划，所有动作都通过**注册的工具**执行，
全过程可审计、可撤销、可回放。**默认拒绝**，不可逆动作必须人工确认。

> 状态：**阶段 0（文档与 Spike）** —— 目前仓库里只有设计文档、治理规则与护栏工具，
> **还没有产品代码**。这是刻意的：先把契约和防漂移机制立起来，再让 AI agent 写第一行产品代码。

---

## 这个项目最特别的一点

它主要由 **AI coding agent**（Codex / opencode / Claude Code）实现，人类负责规划、审阅与裁决。
因此仓库里有一半的"代码"其实是**约束 agent 的机制**：
任务卡、write scope、约束回执、漂移触发器、机器护栏、长期记忆文件。
如果你只想看架构，直接读架构文档；如果你想让 agent 在这个仓库里干活，**必须先读 `AGENTS.md`**。

---

## 文档地图（按需读，不要全读）

| 你想做什么 | 读什么 |
|---|---|
| 让 AI agent 在本仓库干活 | **`AGENTS.md`**（唯一入口，含十条铁律与会话协议） |
| 了解整体架构（分层、进程边界、安全、平台差异） | `cross-platform-ai-assistant-architecture-v2.md` |
| 了解"某个应用能不能接、怎么接" | `target-apps-feasibility.md` |
| 了解现在做什么、不做什么 | `PLAN.md` → `plans/<当前阶段>.md` |
| 回忆项目已知什么 / 否决过什么 / 踩过什么坑 | `MEMORY.md` |
| 了解治理机制（任务卡、CI 门禁、代码规范、验收分离） | `docs/governance-ai-agent-execution.md` |
| 了解多 agent 如何分工 | `docs/subagent-orchestration.md` |
| 了解存储方案与性能预算 | `docs/storage-design.md` |
| 了解全项目拆解顺序 | `docs/wbs-overview.md` |
| 了解夜间无人值守自动化的边界 | `docs/overnight-automation-charter.md` |
| 查契约细节 | `docs/spec/`（naming 已就位，其余陆续产出） |
| 查"为什么当初这么决定" | `docs/adr/`（陆续产出） |
| 查台账 / 停车位 / 依赖登记 | `LEDGER.md` / `docs/PARKING_LOT.md` / `docs/DEPENDENCIES.md` |

---

## 当前阶段

**阶段 0 — Spike 技术验证**（10 张卡，TASK-001 ~ TASK-010）。
目标不是产出代码，而是**用最小成本证伪关键假设**：UIA 定位精度、跨进程句柄传递、
工具选择准确率、撤销闭环、CDP 反注入、存储性能预算、Linux/Wayland 可行性。
详见 `plans/stage-0-spikes.md`。

平台基线：**Windows 11 24H2+**（唯一正式基线；Windows 10 已 EOL，仅 C 级尽力）。
Linux 侧 **Wayland-first**（GNOME 50 已移除 X11 后端）。

---

## 构建与验证

前置：Rust stable（由 `rust-toolchain.toml` 固定）。Windows 需要 MSVC 生成工具 + Windows SDK。

```powershell
# 安装工具链（Windows）
winget install Rustlang.Rustup

# 三项基本门禁
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace

# 仓库卫生护栏（gov §5.4；当前实现 3/13 项，工具会自己声明 —— ADR-0025）
cargo run -p xtask -- hygiene
cargo run -p xtask -- --list-deferred   # 查看"还缺哪些检查、归属哪张卡"
```

`xtask` 是**零第三方依赖**的只读护栏工具。它对未实现的子命令**显式返回失败**
（退出码 3）并指出归属卡号 —— 本项目不允许任何形式的静默失败。

CI：`.github/workflows/ci.yml`（三平台矩阵；硬门禁 5 项已上线，其余 9 项
标 `continue-on-error` 并注明启用卡号）。

---

## 许可证

`MIT`（见 `LICENSE`）。

> 说明：内部项目，暂不公开。计划中的双许可 `MIT OR Apache-2.0` 尚未最终确认
> （`MEMORY.md` §6 的 M5 待裁决项）；当前先以 MIT 落地，**该决定可逆**。
> 开源前需要完成脱敏，清单待建（见 `docs/PARKING_LOT.md` PL-005）。

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

## 产品层 vs 工程元层（读任何文档前先分清这一条）

本项目由 AI coding agent 实现、人类审阅，因此仓库里有大量**不属于产品**的东西。
判定标准只有一句话：**「删掉它，产品的行为会变吗？」不会 → 工程元层。**

| 层 | 包含 | 会被构建进发布物吗 | 受什么约束 |
|---|---|---|---|
| **产品层** | `crates/`、`protocol/`、`adapters/`、`adapters-private/`、`apps/`、`fixtures/`、`eval/` | ✅ 是 | `docs/spec/*` 的契约 ＋ `AGENTS.md` |
| **工程元层** | `AGENTS.md`、`docs/governance-ai-agent-execution.md`、`docs/subagent-orchestration.md`、`docs/overnight-automation-charter.md`、`docs/nightly/*`、`MEMORY.md` ＋ `docs/memory/*`、`LEDGER.md`、`docs/PARKING_LOT.md`、`docs/adr/*`、`xtask/`、`.github/workflows/*` | ❌ 否 | 只受 `AGENTS.md` 约束 |

**为什么要正式区分**（ADR-0029 D5）：这条边界此前只存在于口头，造成三个真实症状 ——
① 文档地图把「夜间自动化章程」与「存储设计」并列，读者（尤其是将来开源后的外部读者）
分不清哪些是产品的设计、哪些是「我们怎么干活」；② 每次讨论夜间自动化都要重新解释一遍
「它不影响 `crates/` 里的任何代码」；③ 护栏工具与治理文档的体积已接近产品文档，
若不分类，「阶段 0 零产品代码」这个事实会被掩盖（此前只能靠一句免责声明打补丁，那是补丁不是结构）。
边界不清的真实代价是**注意力错配**：工程元层的讨论会被误当成产品需求变更，反之亦然。

> **推论**：工程元层的 ADR（夜间自动化机制、护栏工具口径、ADR 编号治理…）**不构成产品决策**，
> 也不改变任何 `docs/spec/*`。审阅它们不需要产品上下文；反过来，审阅产品 spec 也不需要读它们。

---

## 文档地图（按需读，不要全读）

| 你想做什么 | 读什么 |
|---|---|
| 让 AI agent 在本仓库干活 | **`AGENTS.md`**（唯一入口，含十条铁律与会话协议） |
| 了解整体架构（分层、进程边界、安全、平台差异） | `cross-platform-ai-assistant-architecture-v2.md` |
| 了解"某个应用能不能接、怎么接" | `target-apps-feasibility.md` |
| 了解现在做什么、不做什么 | `PLAN.md` → `plans/<当前阶段>.md` |
| 回忆项目已知什么 / 否决过什么 / 踩过什么坑 | `MEMORY.md`（**L0 索引**，≤150 行，含路由表）→ 按路由跳读 `docs/memory/*` |
| 了解治理机制（任务卡、CI 门禁、代码规范、验收分离） | `docs/governance-ai-agent-execution.md` |
| 了解多 agent 如何分工 | `docs/subagent-orchestration.md` |
| 了解存储方案与性能预算 | `docs/storage-design.md` |
| 了解全项目拆解顺序 | `docs/wbs-overview.md` |
| 了解夜间无人值守自动化的**纪律与边界** | `docs/overnight-automation-charter.md`（章程 v1.4 §11：一主一备 —— 主 = Codex 原生 scheduled tasks，备 = 任务计划程序 + `codex exec`） |
| 查夜间自动化的**具体操作**（建 / 改 / 删 / 立即运行 / 暂停 / 停止 / 恢复） | `docs/nightly/codex-automations-operations.md`（ADR-0029 的主交付物；每条结论都带 `[官方]` / `[实测]` / `[未验证]` 证据标签） |
| 查契约细节 | `docs/spec/`（naming 已就位，其余陆续产出） |
| 查"为什么当初这么决定" | `docs/adr/README.md`（**编号登记表**：哪些号已有文件 / 哪些只是待建）→ 具体 `docs/adr/NNNN-*.md` |
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

# 文档一致性护栏（ADR-0030；派生计数与 ADR 编号不再靠人记）
cargo run -p xtask -- memory-counts     # MEMORY.md 规模表 ↔ docs/memory/ 实测
cargo run -p xtask -- adr-index         # ADR 登记表 ↔ docs/adr/*.md ↔ decisions.md

# 改写公共热点文件（LEDGER.md / docs/memory/* / docs/PARKING_LOT.md）前先取锁（ADR-0028）
cargo run -p xtask -- guard acquire MEMORY.md --owner <会话级唯一标识> --intent "<要干什么>"
cargo run -p xtask -- guard status
cargo run -p xtask -- guard release MEMORY.md --owner <同上>
```

`xtask` 是**零第三方依赖**的只读护栏工具。它对未实现的子命令**显式返回失败**
（退出码 3）并指出归属卡号 —— 本项目不允许任何形式的静默失败。

CI：`.github/workflows/ci.yml`（三平台矩阵；**硬门禁 8 项**已上线 —— fmt / clippy / test / deny /
build / hygiene / spike-deny(#8b) / doc-consistency(#12b)；另有 **9 项软门禁**标 `continue-on-error` 并注明启用卡号）。

---

## 最近进展（2026-09-19，TASK-051 + TASK-052 收尾）

`xtask` 护栏工具已从 4 个子命令扩到 7 个（新增 `refscan` / `docscan` / `card-check`），
原 `hygiene / memory-counts / adr-index / guard` 保持。所有 `xtask` 子命令在
CI 硬门禁 #12b（`doc-consistency`）下统一跑过。

| 新增项 | 用途 | 触发场景 |
|---|---|---|
| `xtask refscan` | 扫 `.md` / `.rs` / `.ps1`：裸 ADR 待建引用 + 编号范围写法（ADR-0026 D3）+ `.ps1` 非 ASCII（ADR-0024 D4） | CI 硬门禁 #12b |
| `xtask docscan` | 扫 `.md`：破表（PL-031）+ setext 风险（`---` 前一行非空会变 H2）+ UTF-8 BOM/CRLF/缺末行 LF | CI 硬门禁 #12b |
| `xtask card-check` | 扫 `tasks/TASK-*.md`：ADR-0031 D6 机器化（记录区 9 节齐全 + Done/Review 状态校验 + plans/* 形态防回退） | CI 硬门禁 #12b（预备，本卡提交后正式开启）|
| ADR-0034 | `card-check` 设计与豁免判据说明 | 文档护栏 |

### 历史小节（按时间倒序）

- **2026-09-19 TASK-054**（xtask render() 返回 Result — 消除 per-line allow）:
  - `pub fn render(findings: &[Finding]) -> String` → `pub fn render(findings: &[Finding]) -> Result<String, std::fmt::Error>`
  - 删除 `#[allow(clippy::unwrap_used, clippy::expect_used)]` per-line allow（TASK-052 遗留）
  - `writeln!(..).expect("...")` → `writeln!(..)?` + `.map_err(|e| e.to_string())?` 在 `run()` 中
  - 行为不变（writeln! to String 永不失败；Result 类型强制 caller 处理 = 编译期保证）
- **2026-09-19 TASK-053**（xtask lint cleanup pass 2 — TASK-052 遗留 5 处 str[Range] + 1 处 stale dead_code + 3 处 test f[0]）:
  - card_check.rs 5 处 str[Range] → 全替换为 `.get(range)` + `?` operator 重构
  - refscan.rs:290 stale `#[allow(dead_code)]` 删除（render 已被 run() 调用）
  - docscan/card_check 测试代码 `f[0]` → `f.first().expect("non-empty")`（3 处）
- **2026-09-19 TASK-051 / TASK-052**（xtask 护栏升级 + 纯重构）：
  - TASK-051 cherry-pick 悬空 commit `10f78db` 收回 → 4 子命令可执行 + ADR-0034 注册
  - TASK-052 人类裁决 B 路（漂移不可忍受）= 纯重构 = 4 模块顶部 `#![allow(...)` 块全清
    （**DoD 硬证据**：`grep '^#![allow' xtask/src/{refscan,docscan,card_check,exemptions}.rs` = 0 命中）
    + 32 处 `indexing_slicing` 用 `while let Some + .get(n).expect()` 全替换
    + `docs/PARKING_LOT.md` PL-NEW 关闭
  - 防御性 PITFALL：`docs/memory/pitfalls.md` 追加「禁止 sub-card 后缀；commit 标题必须对应 `tasks/TASK-NNN-*.md`」
  - 详见 `LEDGER.md` 2026-09-19 三行 + `tasks/TASK-051-...md` / `tasks/TASK-052-...md` 执行记录
- **2026-09-17 TASK-001**：仓库骨架 + CI 8 硬门禁 + ADR 编号登记表建立

## 许可证

`MIT`（见 `LICENSE`）。

> 说明：内部项目，暂不公开。计划中的双许可 `MIT OR Apache-2.0` 尚未最终确认
> （`docs/memory/open.md` 的 **M5** 待裁决项）；当前先以 MIT 落地，**该决定可逆**。
> 开源前需要完成脱敏，清单待建（见 `docs/PARKING_LOT.md` PL-005）。

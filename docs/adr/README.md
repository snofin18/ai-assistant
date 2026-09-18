# docs/adr/ — ADR 编号登记表（Numbering Registry）

> **用途**：回答「ADR-NNNN 到底存不存在、内容是什么、这个号能不能占用」。
> **为什么需要**：本项目的决策有**两个落点**，共用一套编号 ——
> ① `docs/memory/decisions.md` 的 `[ADR:待建 NNNN]` 条目（决定已做，正式 ADR 文件还没写）；
> ② `docs/adr/NNNN-*.md` 文件（gov §9.3 模板的完整 ADR）。
> **只有本表说明哪些号已被文件占用**。没有它，agent 很容易把「待建 0019」和
> 「已存在的 0019 文件」当成同一件事 —— 这个坑已真实发生过，见 §3 与 ADR-0026。
>
> **规矩**：新建 ADR 前**必须先查本表**取一个「可用」号；新建后**必须回填本表**。
> 本表只描述事实、不做决策；改「号的分配」属于决策 → 走 ADR。

---

## 1. 已存在的 ADR 文件（编号已被占用，不得复用）

| 编号 | 文件 | 状态 | 决策一句话 |
|---|---|---|---|
| 0018 | `0018-nightly-automation-delivery-mechanism.md` | Accepted | 夜间自动化 = Windows 任务计划程序 + `codex exec`（否决 heartbeat） |
| 0019 | `0019-hard-gate-negative-verification.md` | Accepted | 硬门禁必须配「负向验证」（元门禁） |
| 0021 | `0021-memory-layering-and-app-profiles.md` | Accepted | `MEMORY.md` 只做 L0 索引，细节入 `docs/memory/` |
| 0022 | `0022-windows-target-identity-and-uia-selector-stability.md` | Accepted | Windows 目标身份与 UIA selector 稳定性契约 |
| 0023 | `0023-text-eol-normalization-contract.md` | Accepted | 文本进出口 EOL 归一化契约（规范形 = LF）+ 每应用习惯表 |
| 0024 | `0024-spike-toolchain-dependencies-and-gate-coverage.md` | Accepted | Spike 工具链 / 依赖 / 门禁覆盖（`windows` crate UIA、`spike-deny`、`.ps1` 纯 ASCII） |
| 0025 | `0025-hygiene-rule-count-unification.md` | Accepted | 仓库卫生规则总数 = 13（gov §5.4 表格行数为唯一事实源） |
| 0026 | `0026-adr-number-registry-and-0019-collision.md` | **Proposed** | 建立本登记表 + 修正 0019 号双重占用 |

**下一个可用编号：0027** —— 已预留给「`#[allow]` 的唯一合法位置」那条决策
（ADR-0026 D2，**Proposed，待人类确认**）。确认前不要把它分给别人，也不要为它建文件。

> 编号**不连续是正常的**：0020 被预留但还没写成文件（见 §2），所以 0019 之后直接是 0021。

## 2. 已决定、但尚未写成 ADR 文件的编号（`[ADR:待建 NNNN]`）

这些号**已被预留**：决定本身已生效并记在 `docs/memory/decisions.md`，只是还没有
gov §9.3 格式的完整 ADR 文件。引用它们时**必须写 `[ADR:待建 NNNN]`**，
不能裸写 `ADR-NNNN`（裸写法意味着「文件存在」，会让下个 agent 去 `cat` 一个不存在的文件）。

| 编号 | 决定（全文见 `docs/memory/decisions.md`） |
|---|---|
| 0001 | 核心语言 Rust；UI = Tauri 2 + React/TS；工具协议 MCP-first（`rmcp`） |
| 0002 | API 优先：L1 应用接口 > L2 命令 > L3 无障碍 > L4 合成输入 > L5 视觉 |
| 0003 | Linux Wayland-first，X11 仅作 XWayland 兼容通道 |
| 0004 | element / 句柄不跨进程，Host 边界切在「定位之后」 |
| 0005 | 可逆性四级模型 L0~L3；撤销快捷键由 Adapter 显式声明 |
| 0006 | 无人值守暂不支持，但类型 / 契约 / 能力三处预留 |
| 0007 | 出域策略三档 `local_only` / `redacted` / `full`，默认 `redacted` |
| 0008 | 股票类软件只读 + 解读 + 图形展示；`TradingGate` 恒拒绝并预留 |
| 0009 | 平台基线 Windows 11 24H2+；Win10 仅 C 级尽力支持 |
| 0010 | Office 2019+；Photoshop 最低 2021(v22)，2020 列尽力而为 |
| 0011 | 试点顺序 Notepad → Paint → Edge/Chrome → Excel → Photoshop |
| 0012 | 存储四层：内存热缓存 / SQLite(WAL) / 内容寻址 blob(zstd) / 冷归档 |
| 0013 | 内部使用但按开源规范建设；`adapters/` 与 `adapters-private/` 第一天就分开 |
| 0014 | 执行模式 = AI agent 实现 + 人类裁决 |
| 0015 | 命名「一眼可懂」+ 受控词汇表；注释密度偏高，公共 API 100% 文档注释 |
| 0016 | 全仓库统一 LF（`.gitattributes` + `rustfmt.toml`） |
| 0017 | 未实现项必须显式登记 + 显式失败（xtask 未实现子命令退出码 3） |
| 0020 | TASK-001 先落 MIT 单许可（可逆；M5 待人类在阶段 0 结束前确认） |
| 0027 | （预留）`#[allow]` 的唯一合法位置 = `#[cfg(test)] mod tests`　**← 原编号 0019，见 §3** |

**共 18 个待建号 + 1 个改号预留。** 章程 §13 的夜间工作单 **W4** 原本要把
「0016~0020」落成草稿，但该范围已过期（0018 / 0019 已有同号 Accepted 文件）
→ 见 **PL-029** 与 ADR-0026 D4。

## 3. ⚠ 已知编号事故：0019 曾被双重占用（2026-09-18 发现）

| 占用方 | 决策内容 | 时间 |
|---|---|---|
| `docs/memory/decisions.md` 的 `[ADR:待建 0019]` | `#[allow]` 的唯一合法位置是 `#[cfg(test)] mod tests` | 2026-09-16 预留 |
| `docs/adr/0019-hard-gate-negative-verification.md` | 硬门禁必须配「负向验证」（元门禁） | 2026-09-17 建文件 |

两者是**完全不同的决策**。对照 0018 就看得出差别：0018 的预留主题（夜间自动化用 heartbeat）
与文件主题（夜间自动化的投递机制）**是同一件事**，且文件头显式写了
`Supersedes：MEMORY.md §3 [ADR:待建 0018]` —— 取代链完整；0019 建文件时**没查预留表**，
文件头写的是 `Supersedes：—`，取代链断裂，两条决策静默共号。

**处置（ADR-0026，Proposed）**：文件保留 0019（已被十余个文件引用，改号代价大）；
`#[allow]` 那条的待建号改为 **0027**。
**在 ADR-0026 被人类确认前，不要为 `#[allow]` 那条决策创建任何 ADR 文件。**

## 4. 引用规范

| 想引用什么 | 正确写法 | 错误写法 |
|---|---|---|
| 已有文件的 ADR | `ADR-0023` | — |
| 只有 `decisions.md` 条目的决策 | `[ADR:待建 0016]` | `ADR-0016`（暗示文件存在） |
| ADR 文件取代某个待建条目 | 文件头 `Supersedes：decisions.md [ADR:待建 NNNN]` | `Supersedes：—`（丢取代链，0019 就是这样断的） |

**现存 2 处违规（裸引用）** 已记 **PL-028**：`0021-*.md:4` 写 `ADR-0017（显式登记）`、
`0023-*.md:179` 写 `（ADR-0016）`。ADR 文件「只增不改」，故本次**不直接改写**，
留待阶段末评审，或等这两份 ADR 下次被 supersede 时顺带修正。

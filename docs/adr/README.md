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
>
> **本表由机器校验**（ADR-0030 D3，CI 硬门禁 **#12b**）：`cargo run -p xtask -- adr-index` 会三方交叉
> 比对本表 ↔ `docs/adr/NNNN-*.md` ↔ `docs/memory/decisions.md`，共 **11** 条规则（含 `adr/number-collision`
> —— 就是 §3 那次事故的机器判据）。工具**只读**，不一致时打印可直接粘贴的正确值让人改；
> 「靠记得回填」的护栏已经证明会失效，所以这一版把它换成了红灯。

---

## 1. 已存在的 ADR 文件（编号已被占用，不得复用）

| 编号 | 文件 | 状态 | 决策一句话 |
|---|---|---|---|
| 0018 | `0018-nightly-automation-delivery-mechanism.md` | Accepted → **Superseded**（by ADR-0029） | 夜间自动化 = Windows 任务计划程序 + `codex exec`（否决 heartbeat）。**已被 ADR-0029 取代**：机制改回 Codex 官方 scheduled tasks；本 ADR 的纪律性内容由章程 §11 v1.4 原样保留 |
| 0019 | `0019-hard-gate-negative-verification.md` | Accepted | 硬门禁必须配「负向验证」（元门禁） |
| 0021 | `0021-memory-layering-and-app-profiles.md` | Accepted | `MEMORY.md` 只做 L0 索引，细节入 `docs/memory/` |
| 0022 | `0022-windows-target-identity-and-uia-selector-stability.md` | Accepted | Windows 目标身份与 UIA selector 稳定性契约 |
| 0023 | `0023-text-eol-normalization-contract.md` | Accepted | 文本进出口 EOL 归一化契约（规范形 = LF）+ 每应用习惯表 |
| 0024 | `0024-spike-toolchain-dependencies-and-gate-coverage.md` | Accepted | Spike 工具链 / 依赖 / 门禁覆盖（`windows` crate UIA、`spike-deny`、`.ps1` 纯 ASCII） |
| 0025 | `0025-hygiene-rule-count-unification.md` | Accepted | 仓库卫生规则总数 = 13（gov §5.4 表格行数为唯一事实源） |
| 0026 | `0026-adr-number-registry-and-0019-collision.md` | Accepted | 建立本登记表 + 修正 0019 号双重占用（待建号 0019 → 0027；章程 W4 修正） |
| 0028 | `0028-file-rewrite-mutex-protocol.md` | Accepted | 文件改写互斥锁协议（`xtask guard`：按文件加锁、等待、超时放弃、放弃必须通报） |
| 0029 | `0029-nightly-automation-back-to-codex-scheduled-tasks.md` | Accepted | 夜间自动化的投递机制改回 **Codex 官方 scheduled tasks**（取代 ADR-0018 的任务计划程序方案） |
| 0030 | `0030-machine-verified-memory-counts-and-adr-index.md` | Accepted | 「记忆规模计数」与「本登记表」由手工维护改为机器校验（`xtask memory-counts` / `adr-index`） |
| 0031 | `0031-task-card-one-file-per-card.md` | Accepted | 任务卡改为**一卡一文件**：正文 + 执行记录同在 `tasks/TASK-NNN-<slug>.md`，`plans/*` 退回阶段索引（裁决 DRIFT-001-3） |
| **0032** | `0032-doc-rule-exemption-mechanism.md` | **Accepted** | 文档护栏规则的豁免清单机制（覆盖 PL-032 / PL-034）；配套 `0032-doc-rule-exemption-registry.md` |

**下一个可用编号：0033**（= §1 与 §2 已用最大号 0032 + 1；由 `cargo run -p xtask -- adr-index`
的 `adr/next-number-wrong` 规则机器校验，写错即红灯）。

**0027 不是可用号** —— 它是 §2 的**待建号**，已预留给「`#[allow]` 的唯一合法位置」那条决策
（ADR-0026 D2，人类已于 2026-09-18 确认 → Accepted）。要引用它请写 `[ADR:待建 0027]`；
要把它落成 ADR 文件，走章程 §13 的夜间工作单 **W4**（write scope 已含 0027）。

> 编号**不连续是正常的**：0016 / 0017 / 0020 / 0027 都已被预留但还没写成文件（见 §2），所以 §1 里 0019 之后直接是 0021、0026 之后直接是 0028。

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

**本表刻意不写「共 N 个待建号」** —— 那是一个会随追加漂移的派生计数（ADR-0030 的整条动机）。
一致性由 `cargo run -p xtask -- adr-index` 保证：§2 的每个号都必须能在 `docs/memory/decisions.md`
找到 `[ADR:待建 NNNN]` 条目（`adr/pending-not-in-decisions`），反过来 decisions.md 里未转正、
未退役的待建号也都必须出现在 §2（`adr/decisions-not-in-pending`），且 §1 ∩ §2 必须为空
（`adr/number-collision`）。

章程 §13 的夜间工作单 **W4** 原本要用范围写法把 0016 至 0020 落成草稿，但该范围已过期
（0018 / 0019 已有同号 Accepted 文件）→ **PL-029 已于 2026-09-18 关闭**：W4 按 ADR-0026 D4
改为 **0016 / 0017 / 0020 / 0027** 四个号（章程 v1.4），并新增「不得写编号范围形式」的要求。

## 3. ⚠ 已知编号事故：0019 曾被双重占用（2026-09-18 发现）

| 占用方 | 决策内容 | 时间 |
|---|---|---|
| `docs/memory/decisions.md` 的 `[ADR:待建 0019]` | `#[allow]` 的唯一合法位置是 `#[cfg(test)] mod tests` | 2026-09-16 预留 |
| `docs/adr/0019-hard-gate-negative-verification.md` | 硬门禁必须配「负向验证」（元门禁） | 2026-09-17 建文件 |

两者是**完全不同的决策**。对照 0018 就看得出差别：0018 的预留主题（夜间自动化用 heartbeat）
与文件主题（夜间自动化的投递机制）**是同一件事**，且文件头显式写了
`Supersedes：MEMORY.md §3 [ADR:待建 0018]` —— 取代链完整；0019 建文件时**没查预留表**，
文件头写的是 `Supersedes：—`，取代链断裂，两条决策静默共号。

**处置（ADR-0026，人类已于 2026-09-18 确认 → Accepted）**：文件保留 0019（已被十余个文件引用，
改号代价大）；`#[allow]` 那条的**待建号**改为 **0027** —— `docs/memory/decisions.md` 已追加
`[ADR:待建 0027][supersedes:2026-09-16 的 [ADR:待建 0019] 条目]`，原条目按「只追加不改写」保留原样。

已退役编号：0019（**专指**「硬门禁负向验证」这一条决策。原「`#[allow]` 的唯一合法位置」条目已改号为
0027，0019 **永不再**分配给它，也永不再分配给任何新决策。本行是 `xtask adr-index` 的机器可读锚点：
缺失 → `adr/registry-section-missing`；退役号被重新分配 → `adr/retired-number-reallocated`。
工具**只读本行标记与第一个括号之间**的编号，括号内是给人看的说明，不参与解析。）

> 为什么需要退役清单：`decisions.md` 是**只追加**的，0019 那条原始待建条目会永远留在文件里。
> 若不排除退役号，`adr/decisions-not-in-pending` 会永久报错；而永久红灯会让人学会忽略 CI。

## 4. 引用规范

| 想引用什么 | 正确写法 | 错误写法 |
|---|---|---|
| 已有文件的 ADR | `ADR-0023` | — |
| 只有 `decisions.md` 条目的决策 | `[ADR:待建 0016]` | `ADR-0016`（暗示文件存在） |
| ADR 文件取代某个待建条目 | 文件头 `Supersedes：decisions.md [ADR:待建 NNNN]` | `Supersedes：—`（丢取代链，0019 就是这样断的） |

**现存 2 处违规（裸引用）**：`0021-*.md:4` 写 `ADR-0017（显式登记）`、`0023-*.md:179` 写 `（ADR-0016）`。
ADR 文件「只增不改」，故**不直接改写**，留待阶段末评审，或等这两份 ADR 下次被 supersede 时顺带修正。

**机器检查尚未实现**：ADR-0030 D4 刻意**没有**在本轮实现裸引用规则 —— 上述 2 处违规都在「只增不改」的
ADR 正文里，先实现规则就等于造一条**永久红灯**，而永久红灯会让人学会忽略 CI。正确顺序是「先定豁免机制
（类似 §3 的退役清单），再实现规则」。→ **PL-028**（记录违规本身）＋ **PL-032**（记录「规则待实现」，归 TASK-015）。

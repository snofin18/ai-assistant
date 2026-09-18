# decisions.md — 已决策索引（DECISION）

> 标签 `[DECISION]` = 已定案的决策，**详情一律在 ADR / v2 对应章节**，本文件只做索引。
> 改决策 = 新 ADR + 在此追加一行（不删旧行）。

---

> **本文件是 `MEMORY.md` 分层结构（ADR-0021）的 L1 层之一。**
> 写入规则见 `MEMORY.md`「条目格式」与「更新职责」；路由规则见 `docs/memory/README.md`。
> **只追加，不改写他人条目**；更正用新条目 + `[supersedes:日期]` 标注。
> 应用专属的条目**不进本文件**，进 `docs/memory/apps/<app>.md`。

- [2026-09-16][DECISION][ADR:待建 0001] 核心语言 **Rust**；UI 用 **Tauri 2 + React/TS**；工具协议 **MCP-first（`rmcp`）**，内部工具同样以 MCP 表达。
- [2026-09-16][DECISION][ADR:待建 0002] **API 优先**为纲领原则一：L1 应用接口 > L2 命令 > L3 无障碍 > L4 合成输入 > L5 视觉。
- [2026-09-16][DECISION][ADR:待建 0003] **Linux Wayland-first**，X11 仅作 XWayland 兼容通道；按合成器家族分流；只承诺支持矩阵内组合。
- [2026-09-16][DECISION][ADR:待建 0004] **element/句柄不跨进程**：Host 边界切在"定位之后"，对外只传可序列化 descriptor。
- [2026-09-16][DECISION][ADR:待建 0005] **可逆性四级模型（L0~L3）**；撤销快捷键必须由 Adapter 显式声明；不可逆动作后置。
- [2026-09-16][DECISION][ADR:待建 0006] **无人值守暂不支持但三处预留**（类型/契约/能力 + UI 占位），未来仅白名单应用。
- [2026-09-16][DECISION][ADR:待建 0007] **出域策略用户可选三档**（`local_only`/`redacted`/`full`）+ 逐应用与逐内容类型覆盖；默认 `redacted`；降级不静默。
- [2026-09-16][DECISION][ADR:待建 0008] **股票类软件只读 + 解读 + 图形展示**；`TradingGate` 恒拒绝并预留，未来只经厂商官方接口开放。
- [2026-09-16][DECISION][ADR:待建 0009] 平台基线 **Windows 11 24H2+**；Win10 仅对跨版本一致目标提供 C 级尽力支持。
- [2026-09-16][DECISION][ADR:待建 0010] 版本支持：**Office 2019+**（2016 技术可用不承诺）；**Photoshop 最低 2021(v22)**（2020 无 UXP，列尽力而为）。
- [2026-09-16][DECISION][ADR:待建 0011] 试点顺序 **Notepad → Paint → Edge/Chrome**（阶段 1）→ Excel（2）→ Photoshop（3）；理由：零许可零登录依赖、三种通道覆盖、安全底座提前。
- [2026-09-16][DECISION][ADR:待建 0012] 存储四层：**内存热缓存 / SQLite(WAL) / 内容寻址 blob(zstd+去重) / 冷归档**；只有 Core 写库。
- [2026-09-16][DECISION][ADR:待建 0013] 分发：**当前内部使用，但按开源规范建设**（建议 `MIT OR Apache-2.0`），`adapters/` 与 `adapters-private/` 从第一天分开。
- [2026-09-16][DECISION][ADR:待建 0014] 执行模式：**AI agent 实现 + 人类裁决**；防漂移靠 SSOT 分层、任务卡 write scope、约束回执、14 项 CI 门禁、验收分离。
- [2026-09-16][DECISION][ADR:待建 0015] 命名要求"一眼可懂"+ **受控词汇表**；注释密度偏高，公共 API 100% 文档注释，坑用 `PITFALL(app=…)` 结构化标签。
- [2026-09-16][DECISION][ADR:待建 0016] **全仓库统一 LF**：`.gitattributes` 设 `* text=auto eol=lf` + 显式二进制标记，`rustfmt.toml` 设 `newline_style = "Unix"`。理由见 §4 的对应 REJECTED 条目。
- [2026-09-16][DECISION][ADR:待建 0017] **未实现项必须显式登记 + 显式失败**：xtask 未实现的子命令以退出码 3 失败并打印归属卡号；`hygiene` 每次运行都声明"11 项只实现 3 项"；CI 有 `deferred-inventory` job 专门验证"未实现子命令仍然失败"。理由：一条"因为没写所以返回 0"的门禁比没有这条门禁更危险（制造虚假安全感）。
- [2026-09-16][DECISION][ADR:待建 0018] 夜间自动化用 **heartbeat**（每日 23:30 与 02:30 唤醒当前 thread）：每夜 ≤3 轮、每次唤醒 ≤2 轮、`.nightly.lock` 互斥、只在 `nightly/<date>` 分支提交、不合并 main、不操作真实应用、仅失败时通知、晨间报告落 `docs/nightly/<date>-report.md`。详见 `docs/overnight-automation-charter.md` §11。
- [2026-09-16][DECISION][ADR:待建 0019] **`#[allow]` 的唯一合法位置**是 `#[cfg(test)] mod tests` 上的 `clippy::unwrap_used` / `expect_used` / `panic`（gov §5.2 授权：测试失败就该炸）。产品代码里加 allow = 放宽护栏 = 漂移触发器，夜间自动化一律禁止。需写进 `docs/spec/testing.md`。
- [2026-09-16][DECISION][ADR:待建 0020] TASK-001 先落 **MIT 单许可**（而非 gov 建议的 `MIT OR Apache-2.0`）：内部阶段不需要双许可复杂度，且该决定**可逆**（追加 Apache-2.0 是加法）。M5 仍待人类在阶段 0 结束前正式确认。
- [2026-09-16][DECISION][ADR:待建 0018 补充] 夜间工作的**选取顺序是三级回退**：① `tasks/` 下 Ready 任务卡 → ② 章程 §13「当前夜间工作单」（人类白天维护，夜间只能执行不能改写）→ ③ 白名单 P2~P8。理由：阶段 0 的 10 张卡**没有一张是夜间安全的**（Spike 全都要真机 GUI 或新增依赖，而黑名单两者都禁止），若坚持"必须有 Ready 卡"，夜间自动化会永久空转。首批工作单 W1（rustscan 函数扫描器）/ W2（hygiene 函数级 3 条规则）/ W3（testing spec 草案）/ W4（ADR 0016~0020 草稿）已写入章程 §13。

---

---

## 2026-09-18 追加（ADR-0021 ~ ADR-0025，均 Accepted）

- [2026-09-18][DECISION][src:ADR-0021] **长期记忆分层**：`MEMORY.md` 降级为 L0 索引（≤150 行），全部具体条目移入 `docs/memory/{facts,pitfalls,rejected,decisions,open}.md`；新增 L2 应用档案 `docs/memory/apps/<app>.md`（8 个固定小节）；L3 `archive/` **按体积（>400 行）而非按时间**触发。读取路由表见 `docs/memory/README.md`。（人类指示 #11）
- [2026-09-18][DECISION][src:ADR-0022] **Windows 目标身份与 UIA selector 稳定性**：禁用启动 PID 定位、禁用 `MainWindowHandle`；启动走 **AUMID + `IApplicationActivationManager::ActivateApplication`**，属主 PID 只从 `GetWindowThreadProcessId` 取；selector = **有序候选链 + `app_version_range` + 状态指纹**；本地化禁令扩到 5 项；`RuntimeId` 仅会话内有效；DPI 用**物理像素 + Per-Monitor V2**；会话恢复 / TeachingTip / 标签拖撕 需显式处理。（人类指示 #1 #5）
- [2026-09-18][DECISION][src:ADR-0023] **文本 EOL 归一化契约**：规范形 = **LF**；归一化顺序**铁律** `replace("\r\n","\n").replace("\r","\n")`（不可颠倒）；出口按**每应用习惯表**（`text_conventions`）；postcondition 必须用规范形比较，且原始长度差必须能被 EOL 计数**完全解释**；Adapter 绑定期必须自检一次。→ 记事本的实例见 `apps/notepad.md` §4。（人类指示 #2）
- [2026-09-18][DECISION][src:ADR-0024 D1 + **D1a 实证修正**] **Rust 侧 UIA 一律用 `windows` crate 的 COM 绑定**，否决 `uiautomation` crate。**D1a 关键修正：feature 名是 `Win32_UI_Accessibility`，不是 `Win32_UI_UIAutomation`（后者不存在）**，且必须同时开 `Win32_System_Ole`（`VARIANT` 被双重 gate）。实证载体 `spikes/spike-a-notepad/src/bin/uia_dep_proof.rs`，E1~E6 全 PASS。（人类指示 #6）
- [2026-09-18][DECISION][src:ADR-0024 D2] **PL-019 关闭**：`spikes/` 单独一道 **`spike-deny`** 硬门禁（gov §5.1 **#8b**），枚举 `spikes/*/Cargo.toml` 逐个跑 `cargo deny check licenses sources`；**只查这两项**（免联网）；无 manifest 时必须**显式打印「无」再 exit 0**（铁律 1）。canary 在 `gate-selftest.yml`，ADR-0019 登记表已补 #8b 行。fmt/clippy/test 对 spikes 的豁免是**有意的**。（人类指示 #7）
- [2026-09-18][DECISION][src:ADR-0024 D3] **控件普查工具 = `inspect.exe` + 自写 UIA 树导出**，否决补装 Accessibility Insights（输出不可 diff / 不可计时 / 不可重复）。官方 `winapp ui inspect` 列为候选，试用归 TASK-003。（人类指示 #8）
- [2026-09-18][DECISION][src:ADR-0024 D4] **仓库内 `.ps1` 一律纯 ASCII**：PS 5.1 按 GBK 解码无 BOM 脚本会导致行号错乱 + `` `r `` 被吞。中文说明一律放 `.md`。（`pwsh` 7 保留为逃生口，已否决作为默认。）
- [2026-09-18][DECISION][src:ADR-0018 状态变更] **夜间自动化的投递机制 = Windows 任务计划程序 + `codex exec`**，**放弃** Codex automation（heartbeat / cron 两条路都已被探针否决：投递机制产生畸形 `function_call_output`，缺 `call_id`）。章程 §11 据此重写，验收测试见 `docs/nightly/scheduler-acceptance-test.md`。（人类指示 #10 + 2026-09-17 探针结论）
- [2026-09-18][DECISION][src:ADR-0025] **仓库卫生规则总数 = 13**（gov §5.4 表格行数即唯一事实源）：原 11 项 + 「禁 CRLF」（**Error**）+ 「必须以单个 `\n` 结尾」（**先 Warning，清扫完 15 个既有文件后升 Error**）。PL-001 / PL-011 / PL-020 **三条一并关闭**；「hygiene 12 项」= 笔误，「CI 14 项门禁」= 过时表述。**CI 口径同时定案**：gov §5.1 = **17 行清单** ↔ **16 个 CI 步骤（7 硬 + 9 软）**；主编号**永不重排**，新增门禁用子编号（`#8b`）。⚠️ 本文件里 2026-09-16 的两条旧 DECISION（「14 项 CI 门禁」与「11 项只实现 3 项」）**按原样保留**，以本条为准。（人类指示 #12）
- [2026-09-18][DECISION][src:人类指示 #9 + rollout 取证] **任务卡会话由人类手工新建，不由 agent 调 `create_thread` 派生**（至少在上游 openai/codex#36315 关闭前）。已落成 `docs/subagent-orchestration.md` **§10.1**（v1.0 → **v1.1**）与 §10 表新行。**勿混淆**：`spawn_agent`（subagent）**仍可用且仍是主路径**，受限的只是「在 Codex 应用里新建人类可见的独立任务」这一条路径。

# open.md — 未决问题（OPEN）与假设（ASSUMPTION）

> `[OPEN]` = 需要实测/裁决但还没定；`[ASSUMPTION]` = **未验证的假设，不得当作结论使用**。
> 条目被解决后：不删，在 `facts.md` / `decisions.md` / `rejected.md` 追加结论，
> 并在本条后面追加一行 `[supersedes:日期] → 见 <去向>`。

---

> **本文件是 `MEMORY.md` 分层结构（ADR-0021）的 L1 层之一。**
> 写入规则见 `MEMORY.md`「条目格式」与「更新职责」；路由规则见 `docs/memory/README.md`。
> **只追加，不改写他人条目**；更正用新条目 + `[supersedes:日期]` 标注。
> 应用专属的条目**不进本文件**，进 `docs/memory/apps/<app>.md`。

**OPEN（待实测/待确认）**
- [2026-09-16][OPEN][M1] Office 2019/2021/365 的准确区分方式（`Application.Build` vs ClickToRun `DisplayVersion`）→ Spike（阶段 2 前）。
- [2026-09-16][OPEN][M2] Photoshop 2020(v21) 能否正常登录激活；Illustrator 的 UXP 起始版本 → Spike（阶段 3 前）。
- [2026-09-16][OPEN][M3] `local_only` 档所需的本地模型选型与硬件门槛（Ollama / llama.cpp）→ 阶段 1c 前。
- [2026-09-16][OPEN][M4] 内部是否存在必须支持的 Win10 机器 → 阶段 0。
- [2026-09-16][OPEN][M5] 开源许可证最终选择（建议 `MIT OR Apache-2.0`）→ TASK-001。
  - [supersedes:2026-09-18] → **本条裁决关闭**（本体本身作历史记录保留）：人类选择 **A：MIT OR Apache-2.0 双许可证**。`LICENSE` 重写为双许可证文本（顶 1 行选型声明 + MIT 全文 + Apache-2.0 全文，39 行）；`NOTICE` 新增（10 行，Apache-2.0 §4(d) 硬要求）。**不改 `deny.toml`**（其 allow 表列表已包含了两个）。上线后仅需要在第一个引入的第三方依赖里选择 MIT 或 Apache-2.0 作为其许可证名即可。详见 decisions.md 本日「2026-09-18 追加（M5）」。
- [2026-09-16][OPEN][src:v2 §13.4.5] KDE Plasma 的辅助功能开关对应的底层配置键名 → Spike D。
- [2026-09-16][OPEN][src:v2 §13.4.7] GNOME 50 的 RemoteDesktop 是否能接管当前会话（而非仅 headless）；restore_token 静默重建的实际表现 → Spike D。
- [2026-09-16][OPEN][src:v2 §13.3.2] macOS 屏幕录制权限的周期性重授权间隔 → 阶段 5 前。
- [2026-09-16][OPEN][M6] 夜间 automation 是否需要"晨间显式提醒"？当前策略是**仅失败时通知**，晨间报告靠人类自己打开 `docs/nightly/<date>-report.md`；若要早晨收到提醒，代价是 23:30 与 02:30 也各响一次 → 人类裁决（章程 §11.6）。
- [2026-09-16][OPEN][M7] `check-comments` / `check-ledger` / `card-check` 三个护栏子命令**无任何任务卡认领**，gov §5.1 的 #15 #16 门禁因此无人负责 → 需补卡或并入 TASK-015（PL-002）。
- [2026-09-16][OPEN][M8]（**2026-09-17 更新：前两条已裁决关闭，见 §2；仅第三条待裁决**）TASK-001 的三条 DRIFT 待裁决（hygiene 范围 / CI 硬门禁数量 / 执行记录落盘位置），其中第三条会牵动 gov §3.2 任务卡模板与所有后续卡的 write scope → 阶段 0 内裁决。
  - [supersedes:2026-09-18] → **M8 第三条（DRIFT-001-3「执行记录落盘位置」）已裁决关闭，M8 至此三条全部关闭**：人类 2026-09-18 指示「先做 DRIFT-001-3，按你说的一卡一文件做」→ **ADR-0031（Accepted）**。落点既不是原提案的「正文留 `plans/*`、记录放 `tasks/`」（ADR-0031 选项 2，**未采纳**：一张卡的信息分两处、会话启动要读两个文件、且 `AGENTS.md` §8 现有措辞在该方案下讲不通），也不是「记录写回 `plans/*`」（选项 3，**否决**：等于让被考核的人改考核标准，且并行 agent 必然 write scope 重叠），而是**正文与记录同在一个卡片文件**、用分界线划权限。同步改动：gov §3.2 模板改述为「本模板 = 整个文件」+ 新增 §3.4（9 节骨架 + 分界线原文 + `card-check` 四判据）、§3.3 加 D5 默认项、`AGENTS.md` §1/§3/§8 对齐、`plans/stage-0-spikes.md` 瘦身为阶段索引、`tasks/` 下 10 个卡片文件就位、`tasks/.gitkeep` 删除（目录已非空，同 PL-007 逻辑）。**仍未实现**：`xtask card-check`（PL-002 无卡认领）—— 判据先写死在 ADR-0031 D6 与 gov §3.4，避免将来重新辩论。前两条 DRIFT 的关闭见 §2 与 ADR-0025。

**ASSUMPTION（假设，未验证，不得当作结论使用）**
- [2026-09-16][ASSUMPTION][src:feasibility §2.3] 同花顺/通达信/东方财富的行情与交易界面以 GDI 自绘为主，UIA 覆盖差 → 需 Spike 实测后升级为 FACT 或推翻。
- [2026-09-16][ASSUMPTION][src:feasibility §2.3] 上述软件普遍支持"键盘精灵 + 数据导出/复制到剪贴板"作为可行只读通道 → 需实测。
- [2026-09-16][ASSUMPTION][src:v2 §13.4.6] Java Swing/AWT 在 Linux 上需要 `libatk-wrapper` 或 JVM 参数才暴露 AT-SPI → 需实测。
- [2026-09-16][ASSUMPTION][src:v2 §15.4] 树快照 zstd 压缩比 5~10x、内容寻址去重收益显著 → 需 Spike H 实测。

---

## 2026-09-18 追加

- [2026-09-18][OPEN][N1] **`windows` crate 剩余 feature 未验证**：ADR-0022 D8 需要的 `Win32_UI_HiDpi`（Per-Monitor V2 声明）与 L4 合成输入需要的 `Win32_UI_Input_KeyboardAndMouse`，本次探针**刻意未启用**（不提前引入未验证的 feature）。→ 阶段 1 建 `crates/platform/windows` 时逐个启用并跑最小样例；升级 `windows` 版本 = 漂移触发器。
- [2026-09-18][OPEN][N2] **`CacheRequest` 批量取属性的性能收益未测**。绑定已确认存在（`IUIAutomationCacheRequest` + 全部 `*BuildCache` 方法）。已知单次定位 2.35 ms（`uia_dep_proof` E5），但"带缓存 vs 不带缓存"的对比、以及在 32 节点树上一次性取 N 个属性的收益，都要等 TASK-002 步骤 3。→ 若收益显著，架构 v2 的批量取属性策略需要写进 `crates/platform/windows` 的不变量。
- [2026-09-18][OPEN][N3] **上游 openai/codex#36315 / #36250 仍 open**（`create_thread` 拒绝合法的 project+worktree 请求 / project-aware `create_thread` 的原子性与幂等）。→ 需定期回看；关闭后重测"把 `target` 写成对象能否成功"，成功则把「人类手工建会话」这条决策降级为备选（见 `rejected.md` 2026-09-18 最后一条）。
- [2026-09-18][OPEN][N4] **夜间自动化的验收测试尚未真跑过**（人类指示 #10 要求"专门留个时间测试"）。断言清单已写在 `docs/nightly/scheduler-acceptance-test.md`，但 `Register-ScheduledTask` + `codex exec` 的端到端一次成功运行**还没有本机证据**。→ 需人类在场时跑一次冒烟；在此之前章程 §11 处于"设计已定、未验收"状态。
- [2026-09-18][OPEN][N5] **`codex exec` 的退出码语义无官方文档**。章程与验收脚本只能**自建基线**（首次冒烟时把观察到的退出码记录下来当作事实源）。→ 归 N4 一并做。
- [2026-09-18][OPEN][N6] **Codex 自带记忆机制（`~/.codex/memories/`）若支持"仓库内路径 + 版本化 + 跨 agent"，重新评估 ADR-0021 的否决结论。** 当前它只用于跨项目的个人偏好与本机环境备忘，**不得承载任何项目结论**。
- [2026-09-18][OPEN][N7] **官方 `winapp ui inspect` CLI 能否用于第三方应用**（ADR-0024「重新评估触发条件」）。若可用，可能替代部分自写导出。→ 试用归 TASK-003。
- [2026-09-18][ASSUMPTION][src:ADR-0024 D1a E5，n=10，单应用单控件] **假设"Rust COM 与托管封装同数量级"可推广到更大的树**（Excel / Photoshop 的树可能是数千到数万节点）。当前证据只覆盖记事本的 32 节点树。→ 阶段 2/3 的 spike 必须重测；若在大树上出现数量级差异，需要重估 Host 进程内的批量取属性策略（这才是 N2 的真正价值）。
- [2026-09-18][OPEN][N8] **ADR 编号 0019 曾被双重占用**（`decisions.md` 的 `[ADR:待建 0019]` = `#[allow]` 位置，与 `docs/adr/0019-hard-gate-negative-verification.md` = 硬门禁负向验证，是两条不同决策）。修正方案见 **ADR-0026（Proposed）**：待建号 0019 → 0027，并建立 `docs/adr/README.md` 登记表。→ **待人类裁决**；裁决前不要为 `#[allow]` 那条决策创建任何 ADR 文件，也不要复用 0019 / 0027 两个号。
  - [supersedes:2026-09-18] → **N8 已裁决关闭**：人类于 2026-09-18 确认 **ADR-0026**（状态 `Proposed` → `Accepted`）。**D2 已执行**（`decisions.md` 已追加 `[ADR:待建 0027]` 条目，原 0019 条目保留原样）；**D4 已执行**（章程 §13 W4 的 write scope 改为 0016 / 0017 / 0020 / 0027 四个号、「禁止」项改为「新建 0028 及以后的编号」、验收改为 4 个文件，章程版本 1.3 → 1.4）。0019 号在 `docs/adr/README.md` §3 登记为**已退役**；下一个可用编号 = **0031**。同类事故今后由 `cargo run -p xtask -- adr-index` 机器拦截（ADR-0030 D3，硬门禁 #12b），不再依赖人工核对。
- [2026-09-18][OPEN][N9] **夜间自动化的启用前置门禁 GATE-0 尚未执行**（ADR-0029 D3；本条**承接** N4/N5 的验收对象）。需要实测的全链路：Codex automation 触发 → 本机第三方端点（阿里云百炼 compatible-mode，`qwen3.8-max`）接受那条 `call_id: None` 的合成 `function_call_output` → 模型正常回复 → 产物落盘。**必须在标题以 `nightly-probe-` 开头的新建废弃 thread 上做**，绝不在主线工作 thread 上（ADR-0018 的 E3 已证明毒项 + 全量重放会**永久损坏**长驻会话）。**本轮刻意未执行** —— 人类指示 #2 是「查清楚官方文档的具体写法，到时候专门找时间调试」，故 ADR-0029 D2 把本轮范围限定为「只调研落档」。→ 在此之前**禁止创建任何真实 automation**；断言清单在操作手册 §8，门禁表在 §2。
  - [supersedes:2026-09-18] → **N4 / N5 的验收对象已改变**（原条目按「只追加」保留）：机制改回 Codex automation（ADR-0029）后，「`Register-ScheduledTask` + `codex exec` 端到端一次成功运行」不再是**主方案**的验收目标；`docs/nightly/scheduler-acceptance-test.md` 降级为**回退方案**的清单（顶部已加横幅），N5 的「`codex exec` 退出码基线」同样只在回退方案启用时才需要。主方案的对应未决项由本条 **N9** 承接。
  - [supersedes:2026-09-18 → 见下方 N10 独立条目]  ↑ 原 line 51 supersede 注释**挂错位置**：N9 主体是夜间自动化 GATE-0 验收，此条谈的是 automation 错过触发是否补跑 = 不同议题。本卡 TASK-069 把它升级为独立 N10 条目。

- [2026-09-20][OPEN][N10] **automation 错过触发后是否补跑**（从 N9 line 51 升级为独立条目，原 supersede 注释挂错位置）。操作手册 §8 的 D6：官方只说「需要本地文件时保持电脑开机、应用运行」，没说补跑行为。章程的取舍是「跳过也无害」，两种结果都能接受，但必须**留下实测记录**，否则每次关机都会引发一轮猜测。→ 归 GATE-0 一并实测。

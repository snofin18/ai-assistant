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
- [2026-09-16][OPEN][src:v2 §13.4.5] KDE Plasma 的辅助功能开关对应的底层配置键名 → Spike D。
- [2026-09-16][OPEN][src:v2 §13.4.7] GNOME 50 的 RemoteDesktop 是否能接管当前会话（而非仅 headless）；restore_token 静默重建的实际表现 → Spike D。
- [2026-09-16][OPEN][src:v2 §13.3.2] macOS 屏幕录制权限的周期性重授权间隔 → 阶段 5 前。
- [2026-09-16][OPEN][M6] 夜间 automation 是否需要"晨间显式提醒"？当前策略是**仅失败时通知**，晨间报告靠人类自己打开 `docs/nightly/<date>-report.md`；若要早晨收到提醒，代价是 23:30 与 02:30 也各响一次 → 人类裁决（章程 §11.6）。
- [2026-09-16][OPEN][M7] `check-comments` / `check-ledger` / `card-check` 三个护栏子命令**无任何任务卡认领**，gov §5.1 的 #15 #16 门禁因此无人负责 → 需补卡或并入 TASK-015（PL-002）。
- [2026-09-16][OPEN][M8]（**2026-09-17 更新：前两条已裁决关闭，见 §2；仅第三条待裁决**）TASK-001 的三条 DRIFT 待裁决（hygiene 范围 / CI 硬门禁数量 / 执行记录落盘位置），其中第三条会牵动 gov §3.2 任务卡模板与所有后续卡的 write scope → 阶段 0 内裁决。

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

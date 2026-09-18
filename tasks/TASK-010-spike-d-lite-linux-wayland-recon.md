# TASK-010　Spike D-lite：Linux/Wayland 早期侦察（不阻塞主线）

- 状态：**Ready**
- 阶段：0　Spike：**D-lite**　依赖：001　预估：2 天　阻塞主线：**否**（Linux 在阶段 6）
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-0-spikes.md`。

---


- 依赖：TASK-001　预估：2 天　难度：M　**优先级：低**（Linux 在阶段 6，此处只为避免"阶段 6 才发现整条路不通"）
- **write scope**：`spikes/spike-d-linux/**`、`docs/spike-reports/SPIKE-D-lite.md`、`MEMORY.md`、`LEDGER.md`
- **环境**：Ubuntu 26.04 LTS（GNOME/Mutter，Wayland）+ KDE Plasma 6（KWin，Wayland）各一台（虚拟机可，但 portal 行为需真机复核）

**只做四件事（完整验证留在阶段 6 前）**
1. `org.a11y.Status` 读取 + Tier 0 激活（`gsettings set org.gnome.desktop.interface toolkit-accessibility true`；KDE 对应键名确认 → 回填 `MEMORY.md` §6 的 OPEN 项）
2. AT-SPI 三项核心能力实测（用 GTK4 与 Qt6 各一个测试程序）：
   - 枚举应用与顶层窗口（替代 X11 窗口列表）
   - `Action.DoAction` 点击
   - `EditableText.SetTextContents` 写文本
   - 记录 `tree_available` / `coverage` / `actionable_ratio` / `editable_text_ok` 四项指标
3. portal 一次完整流程：RemoteDesktop 授权 → 注入一次点击 → 保存 restore_token → 重启后静默重建（GNOME 与 KDE 各测一次，记录差异）
4. 截图通道：KDE `org.kde.KWin.ScreenShot2` 与 GNOME portal ScreenCast 各测一次，记录是否需弹窗与单帧耗时

**go 判据（侦察性质，门槛低于阶段 6）**
- AT-SPI 三项在 GTK4 与 Qt6 上至少一项组合完全可用
- 至少一个合成器能完成"授权 → 注入 → restore_token 静默重建"闭环
- 若全部失败 → 触发 v2 §13.4 的降级预案评估（Linux 仅支持提供 API/CLI/DBus 的应用），并**记入 `MEMORY.md` §4 REJECTED 或 §6 OPEN**

**产出**：把 v2 §13.4 中所有 `【待验证】` 项的实测结果回填（走 ADR/spec 提案流程，不得直接改 spec）。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执（**动手前**填，`AGENTS.md` §3 的固定格式）

```text
【任务】TASK-010 <标题>          【目标】<一句话>
【write scope】仅：<文件清单>     【铁律】<本卡最相关 3~6 条>
【禁止】<本卡 Out of scope 要点>  【验收】<命令> → <期望>
【依赖】<前置卡号，已核对 LEDGER>  【疑问】<有则列出+你的默认处理；无则写"无">
```

> 回执与正文区不符 → 上下文已污染 → **请人类重开会话**（比纠正更省成本）。

### 2. 实际改动文件（逐个核对是否在 In scope 内）

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

### 4. DoD 逐条核对

### 5. 偏差：none / DRIFT-0NN-x（全文按 gov §4.3 格式：现象 / 影响 / 建议 / 已停工作）

### 6. 实施中发现的更合理做法（非漂移，已直接落地 + 理由）

### 7. 遗留问题（进 `docs/PARKING_LOT.md` 的编号）

### 8. 新增长期记忆（FACT / PITFALL / REJECTED 条目原文；无则写"无"）

### 9. 给审阅者的关注点（风险最高的 1~3 处）

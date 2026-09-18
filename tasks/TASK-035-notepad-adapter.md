# TASK-035　Notepad Adapter（要点摘录，展开时补全）

- 阶段：1　子阶段：**1a**　批次：**A5（Notepad 闭环）**　依赖：TASK-017, TASK-020, TASK-024　预估：L
- 状态：**Ready**
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。
- ⚠ 本卡正文目前是**要点摘录**（迁移时逐字保留），**尚未展开为完整卡**：缺「目标 / In scope / 验收命令 / DoD」四节。
  开工前必须由 Orchestrator 按 gov §3.2 模板补全（补全 = 改正文区，属 Orchestrator 职责，**不是** Implementer 的漂移）。
  在补全之前，本卡**不得派单**（card-check 判据 ③ 会报缺节）。

---


- **必须包含**：`adapter.toml`（`version_range` 限定 Win11 新版记事本、`capability_level = L3_a11y`、`os_version_range = ">=10.0.26100"`）、`app_map.json`（快捷键表、ui_map、known_pitfalls 含"另存为是 Shell 进程对话框"/"关闭未保存弹三态框"/"大文件应走文件通道"、`undo_capability` 显式声明 `Ctrl+Z` 与粒度）、`selectors/`（每个目标 ≥3 个候选且**不得以可见文本为主键**）、`tools/`（5 个工具，每个都有 postconditions 与 reversibility）、`rollback/`（L0 undo + L1 内容快照双路径）、`interrupts/`（三态对话框默认"取消"）
- **禁止**：在本卡内修改 `crates/**`（若发现平台层缺陷 → DRIFT 升级，不得顺手改）

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执（**动手前**填，`AGENTS.md` §3 的固定格式）

```text
【任务】TASK-035 <标题>          【目标】<一句话>
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

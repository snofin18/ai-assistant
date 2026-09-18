# TASK-004　Spike B：跨进程 Host 与目标重解析

- 状态：**Ready**
- 阶段：0　Spike：**B**　依赖：002　预估：2 天　阻塞主线：是
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-0-spikes.md`。

---


- 依赖：TASK-002　预估：2 天　难度：M
- **write scope**：`spikes/spike-b-host/**`、`docs/spike-reports/SPIKE-B.md`、`MEMORY.md`、`LEDGER.md`

**步骤**
1. 起两个进程：`host`（持有 UIA element）与 `client`（模拟 Core），用 Named Pipe + JSON-RPC 2.0 通信
2. 验证 v2 §3.2 的边界规则：client 只发 `TargetDescriptor`，host 内部定位并执行，只回纯数据
3. **失效重解析矩阵**（每种情况 10 次）：
   - 目标应用重启（进程 PID 变化）
   - 窗口关闭后重开
   - 标签页切换 / 新建标签
   - 文档内容大幅变化（树重建）
   - host 自身重启（缓存全丢）
   - 目标窗口在另一虚拟桌面 / 最小化
4. 测量：重解析成功率、耗时、失败时的错误可读性（能否区分"未找到"/"歧义"/"无响应"/"权限不足"）
5. 验证 element 缓存策略：缓存多久、如何检测失效（`CurrentRuntimeId` 比对 / 指纹）
6. 验证 IPC 鉴权与对端身份校验（Named Pipe 客户端 PID + 完整性级别）

**go 判据**
- 重解析成功率 ≥ 95%（应用重启与标签切换两类必须 100%）
- 单次重解析 ≤ 1.5 s
- 四类失败可区分且错误信息可读（对模型友好）
- host 崩溃后 client 能检测到并安全终止当前步骤（不 hang）

**no-go 后果**：改为单进程（牺牲隔离）或重新设计边界 → 必须走 ADR，影响 v2 §3.2/§3.3。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执（**动手前**填，`AGENTS.md` §3 的固定格式）

```text
【任务】TASK-004 <标题>          【目标】<一句话>
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

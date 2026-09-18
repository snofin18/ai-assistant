# TASK-009　Spike H：存储方案 PoC 与性能预算实测

- 状态：**Ready**
- 阶段：0　Spike：**H**　依赖：001　预估：2 天　阻塞主线：否（但影响阶段 1 设计）
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-0-spikes.md`。

---


- 依赖：TASK-001　预估：2 天　难度：M
- **write scope**：`spikes/spike-h-storage/**`、`docs/spike-reports/SPIKE-H.md`、`MEMORY.md`、`LEDGER.md`
- **关联**：`docs/storage-design.md`（本 Spike 的结论用于确认或修正该文档的性能预算）

**步骤**
1. 建 SQLite（`rusqlite`）+ WAL + `synchronous=NORMAL`，实现 v2 §15.1 的核心表
2. 实测四项写路径：单条审计写、批量审计写（ring buffer flush）、检查点写（Task+Step+指纹）、用量记录写
3. blob 存储实测：内容寻址（sha256 + 两位分片）+ zstd 压缩
   - 用真实 UI 树快照样本（可从 TASK-002/003 采集）测**压缩比**与**去重率**
   - 测"全树 vs diff"两种存储的体积差
4. 读路径实测：时间线分页查询（1000 步）、FTS5 记忆检索、按 task_id 范围扫描审计
5. 并发实测：一个写连接持续写 + 4 个只读连接持续查（模拟"UI 滚动时间线同时 Agent 在跑"），测是否出现 `SQLITE_BUSY` 与延迟劣化
6. 崩溃一致性：写入中途 kill 进程，重启后验证 ①DB 完整性 ②孤儿 blob 可被 GC 识别 ③"DB 有记录但 blob 缺失"能被检测并标记 `evidence_missing`
7. 冷启动实测：加载全部配置（模拟 5 个 Adapter + App Map + 策略）+ 恢复活跃任务的耗时
8. 对比实验：同一 workload 下 SQLite(WAL) vs redb（若时间允许），量化差距

**go 判据（对照 v2 §15.4 性能预算）**
- 策略判定（内存）< 50 µs；审计批量写摊销 < 1 ms/条；检查点写 < 10 ms
- 1 MB 树快照写入（含 zstd）< 30 ms；时间线 1000 步分页 < 100 ms；FTS 检索 < 50 ms
- 冷启动 < 500 ms
- 并发场景 0 次 `SQLITE_BUSY`；崩溃后 0 次数据损坏；blob/DB 不一致 100% 可检测

**no-go 后果**：调整存储分层（例如审计独立库、blob 改用 LMDB、或降低快照频率）→ 走 ADR 修正 v2 §15.4。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执（**动手前**填，`AGENTS.md` §3 的固定格式）

```text
【任务】TASK-009 <标题>          【目标】<一句话>
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

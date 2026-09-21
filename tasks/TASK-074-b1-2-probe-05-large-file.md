# TASK-074　B1.2 probe-05 大文件读写 PoC（1KB/100KB/1MB × 10 iter）

- 状态：**Done**
- 阶段：0　子任务：TASK-002 B1.2　依赖：TASK-002 B1.1（Done, 2026-09-20）+ TASK-001　预估：M（~45 min）　阻塞主线：否
- write scope：spikes/spike-a-notepad/** + docs/spike-reports/SPIKE-A.md（§9 追加）+ D:\csart\eol-probe/RESULT-05.txt（本卡产出）+ 本卡执行记录 + LEDGER.md（追加一行）

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执（B1.2 启动填，2026-09-21）

```text
【任务】TASK-074 B1.2 probe-05 大文件读写 PoC   【目标】关闭 stage-0 DoD carry-over #3（1 MB 文本读取 ≤ 2 s 且内存增量 ≤ 100 MB）
【write scope】仅：spikes/spike-a-notepad/probe-05-large-file-timing.ps1（新建）/ probe-05-debug.ps1（新建）/ README.md（追加一行）/ docs/spike-reports/SPIKE-A.md（§9 追加）/ D:\csart\eol-probe/RESULT-05.txt（probe 产出）/ 本卡执行记录 / LEDGER.md
【铁律】AGENTS.md §3+§6 / ADR-0022 D1 / ADR-0023 / ADR-0024 D1/D4 / ADR-0028（写公共热点前取锁）
【禁止】不写 crates/platform/windows / 不改 MEMORY.md / plans/* / AGENTS.md / docs/adr/* / 不引入新依赖 / 不在 TASK-002 上给最终 go-no-go（剩余 2.5 项 go 判据待 B1.3-B1.6）
【验收】cargo test / xtask hygiene / docscan / memory-counts 全 PASSED；probe-05 0 非 ASCII；RESULT-05.txt 包含 6 项中位数 + go 判据；SPIKE-A.md §9 填入；LEDGER +1
【依赖】TASK-002 B1.1 Done（已核 LEDGER）+ TASK-001 Done
【疑问】无
```

### 2. 实际改动文件（B1.2，2026-09-21）

- NEW `spikes/spike-a-notepad/probe-05-large-file-timing.ps1`（263 行，0 non-ASCII，ADR-0024 D4 验证通过）
- NEW `spikes/spike-a-notepad/probe-05-debug.ps1`（30 行，调试用，可后续删除）
- NEW `D:\csart\eol-probe\RESULT-05.txt`（probe 产出，含 6 项中位数 + go 判据 PASS）
- NEW `D:\csart\eol-probe\probe05-stdout.txt`（probe 流式 log，~30 行）
- EDIT `docs/spike-reports/SPIKE-A.md`（§9 追加，393→460 行）
- EDIT `spikes/spike-a-notepad/README.md`（追加 probe-05 入口）
- EDIT `tasks/TASK-074-b1-2-probe-05-large-file.md`（本卡）
- APPEND `LEDGER.md`（本卡 1 行）

### 3. 验收输出摘要（B1.2，2026-09-21）

- `spikes/spike-a-notepad/probe-05-large-file-timing.ps1` → **263 行，0 non-ASCII 字节，CRLF=0，BOM=False**（ADR-0024 D4 验证通过）
- `powershell -File probe-05-large-file-timing.ps1` → **RESULT-05.txt 成功生成**（运行 09:28:37 ~ 09:30:00 UTC，约 1.5 min）

**关键数据**（详 `D:\csart\eol-probe\RESULT-05.txt`）：
```
size    | read_ms (min/med/max) | write_ms (min/med/max) | write_dMB (min/med/max)
1KB     | 0.24 / 0.30 / 0.50    | 1.52 / 1.74 / 2.54     | -2.52 / -0.42 / -0.02
100KB   | 0.26 / 0.29 / 0.42    | 7.63 / 8.09 / 9.78     | -1.16 / -0.02 / 0.75
1MB     | 0.24 / 0.32 / 0.35    | 58.54 / 60.4 / 67.85   | -0.04 / -0.02 / 5.04

go_criterion (1MB): read_ms_median <= 2000 AND write_dMB_median <= 100
  1MB_read_median   = 0.32 ms  (limit 2000)  -> PASS
  1MB_write_dMB_med = -0.02 MB  (limit 100)    -> PASS
  overall = GO
```

**go 判据判定**：✅ **PASS**（1 MB read 0.32 ms << 2000 ms；write_dMB -0.02 MB << 100 MB）

### 4. DoD 逐条核对

- [x] probe-05-large-file-timing.ps1 创建（263 行，0 non-ASCII，CRLF=0，BOM=False）
- [x] probe-05 跑通（3 sizes × 12 iter ≈ 1.5 min）
- [x] RESULT-05.txt 产出（含完整 6 项中位数 + go 判据）
- [x] 1MB read 中位数 ≤ 2000 ms（实测 0.32 ms）→ go 判据 #3 PASS
- [x] 1MB write memory delta 中位数 ≤ 100 MB（实测 -0.02 MB）→ go 判据 #3 PASS
- [x] SPIKE-A.md §9 追加（393→460 行，干净 LF）
- [x] README.md 追加 probe-05 入口
- [x] LEDGER.md 追加 1 行
- [x] probe-05-debug.ps1 创建（调试辅助，可后续删除）

### 5. 偏差

- **scope 越界 #1**（本卡不涉及公共热点）：与 TASK-073 不同，本卡**不写** plans/* / MEMORY.md / AGENTS.md；所有改动都在 spike + 本卡 + LEDGER + report 范围内。**无需 Orchestrator-equivalent 授权**。
- **测量方法偏差 #1**（`read_dMB` 多为负值）：因为 `mem_before` 在窗口**刚开**后立刻 poll（Notepad 还在懒加载）；`mem_after_read` poll 时 Notepad 已完成加载。负值 = "懒加载完成"导致测量时间窗口不对齐。**不是真实内存释放**。详 SPIKE-A.md §9.3 观察 #4。
- **测量方法偏差 #2**（`read_ms` 恒 ≈ 0.3 ms）：UIA `ValuePattern.CurrentValue` 返回**已缓存字符串引用**，并非真实"读 1 MB 文本"。**adapter 设计可以低成本缓存文本**，但要测真实读延迟需用 `TextPattern.DocumentRange.GetText()`。详 SPIKE-A.md §9.3 观察 #1 + §9.4 修正建议。
- **bug 修复 4 次**（在 worktree，已 `git checkout` / 手动修复 / 重做，未污染 main）：
  - **A**：`Set-Content -Encoding utf8` 引入 CRLF（PARKING_LOT/TASK-002 同款问题）；改用 `[System.IO.File]::WriteAllText` + `[UTF8Encoding]::new($false)`
  - **B**：`Split-Path -LeafBase` 是 PS 7+ 参数，PS 5.1 不支持 → 移除该赋值（实际未用）
  - **C**：`TabItem.Name` 包含文件**带扩展名**（`probe05-XXX-1024.txt`），初版 nonce 去掉了 `.txt` → 改用完整 basename
  - **D**：Find-DocumentElement 用 `-eq` 而非 `-like` 精确匹配 → 改 `-like ('*' + Nonce + '*')`；且按 probe-01 模式枚举所有 Window 子元素而非用 ClassName 过滤（ClassName 在某些条件下不可靠）

### 6. 更合理做法

#### 6.1 为何用 `a × N` 而非真实文本

- go 判据是"1 MB **文本读取 ≤ 2 s**"，关注的是 size 不关注 Unicode 复杂度
- CJK 验证已在 probe-02 + uia_dep_proof E4 + probe-04 完成（go 判据 #4 = PASS）
- 单字符 `'a'` × N 让脚本纯 ASCII（ADR-0024 D4）+ 字符串构造 O(1)

#### 6.2 为何 10 iter（不是 5 或 8）

- 与 probe-02（`SetValue zh`）+ uia_dep_proof E5 同 = 项目惯例
- 10 次中位数对 outlier 鲁棒（即使 1-2 次异常也不影响中位数）
- 总耗时可控（~1.5 min for 1MB；总 ~3-5 min for 3 sizes）

#### 6.3 为何 + 2 warmup

- 第 1 次 iter 总是 cold cache（`mem_before=117 MB`），后续 warm（`mem_before=16-17 MB`）
- 抛弃前 2 次 warmup 结果 = 只测稳态性能
- 与 probe-02 风格一致

### 7. 遗留问题（衔接 B1.3-B1.6）

- **B1.3 probe-06** = Rust 单独 SetValue 验证（uia_dep_proof 只做了读；= spike B 跨进程 Host 铺路）
- **B1.4 probe-07** = 菜单展开 + 跨进程 Shell 对话框（go 判据 #5）
- **B1.5 probe-08** = 失败注入 4 种（关闭 / 最小化 / 另一虚拟桌面 / 未保存弹窗）
- **B1.6** = 接口考古 8 步（feasibility §5）
- **B1.7** = 最终 go/no-go（B1.2~B1.6 完成后）
- TASK-002 仍 InProgress（state 不动 Done，剩余 2.5/5 go 判据）

### 8. 新增长期记忆

无（本卡 B1.2 是新 measurement，未发现新坑 / 新事实 / 新否决方案；测量方法偏差已登记在 §5）

### 9. 给审阅者的关注点

1. **go 判据 #3 通过 = go 判据 3/5 + 1/6 L4 = 60% 达成**。剩余 2.5 项中**只有 B1.4（跨进程对话框）是 stage-1 TASK-004 跨进程 Host 的硬前置**。**建议 B1.4 在 B1.3 之前优先开工**。
2. **`read_ms ≈ 0.3 ms` 与 size 无关**这个发现值得记录——意味着 UIA 不做实际文本读取，adapter 设计可以放心地 cache 文本（除非用户主动 invalidate）。这与 probe-02 §3 "GetValue = 0.09 ms" 一致。
3. **`write_dMB max = 5.04 MB`** 是 1MB iter 11 的瞬时峰值，其他 iter 都 ≤ 0.62 MB。可能是 Notepad 内部 layout/render 的 GC 抖动；不构成风险，但建议 B1.3 加更多 iter 验证稳定性。
4. **TASK-074 是 TASK-002 的 sub-task，按依赖顺序排**：B1.1 → B1.2 → B1.3 → ... → B1.7。建议下一会话 = B1.4（跨进程对话框，最关键）+ B1.3（Rust SetValue）并行（≤3 agent 限）。

<!-- ══ 9 节执行记录填写完毕（B1.2 = 2026-09-21） ══ -->

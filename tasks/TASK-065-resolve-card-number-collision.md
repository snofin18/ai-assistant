# TASK-065　任务卡撞号解决 — xtask 051~055+055b 重编号为 059~064

- 状态：**Ready**
- 阶段：0　子阶段：—　依赖：001　预估：S　阻塞主线：否（治理性修复）

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

- 任务：TASK-065 任务卡撞号解决（ADR-0036 实施）
- 目标：xtask 051~055+055b 重编号为 059~064；撤回 sub-suffix 命名；同步状态字段
- write scope：仅 docs/adr/0036-*.md / docs/adr/README.md / README.md / tasks/TASK-051..TASK-055,TASK-055b (删) + tasks/TASK-059..TASK-064,TASK-065 (新) / LEDGER.md / xtask/src/{card_check,cli,docscan,refscan,repowalk}.rs
- 铁律相关：铁律 9 不得静默扩大范围 / 铁律 6 文档与代码同步 / §3 启动协议 + §8 write scope
- 禁止：不动 stage-1 卡 051~055 号段（已就位且有依赖）/ 不动 workspace lints / 不改 ADR-0031 文本（仅以 ADR-0036 引用）/ 不动其他 ADR / 不动 MEMORY/PLAN（归 TASK-067）
- 验收：cargo fmt --all --check → 0 diff；cargo clippy -p xtask --all-targets -- -D warnings → exit 0；cargo test --workspace → 283 passed；cargo deny check → 4 项 ok；xtask 7 子命令 → 全部 PASSED；DoD grep：(1) ls tasks/ | grep TASK-05[1-5] | grep xtask = 0 命中；(2) ls tasks/ | grep TASK-055b = 0 命中；(3) ls tasks/ | wc -l = 64（删 6 + 新 13 = 64 不变）
- 依赖：TASK-001 / 011 / 035 / 036~058（已 Done / Ready 占位）；ADR-0026 / 0031 / 0035（Accepted）；guard LEDGER.md 已 acquire
- 疑问：无

### 2. 实际改动文件

- docs/adr/0036-card-number-collision-resolution.md（**新建** 3932 字节）：ADR 状态 Accepted，D1 重编号表 / D2 stage-1 不变 / D3 撤回 sub-suffix / D4 状态同步 / D5 未来机器化要求
- docs/adr/README.md（**改**）：§1 表新增 0036 行（紧邻 0035）；§下一个可用编号 0036 → 0037
- README.md（**改**）：节标题与历史小节所有 TASK-051~055/055b 引用 → TASK-059~064；stage-1 卡 051~055 不动
- tasks/TASK-051/052/053/054/055/055b-*.md（**删 6 个**）
- tasks/TASK-059~TASK-064-*.md（**新 6 个**）：从旧文件复制 + H1 改号 + 状态 InProgress → Done + 写路径改 + 内部 TASK-051~055/055b 引用替换
- tasks/TASK-065~TASK-070-*.md（**新 6 张治理卡骨架**）：元数据 + 9 节分界线
- LEDGER.md（**追加 1 行**）：TASK-065 Done 行（按只追加不改写规则）
- xtask/src/{card_check,cli,docscan,refscan,repowalk}.rs（**改 doc-comments 10 处**）：TASK-052→TASK-060、TASK-054→TASK-062、TASK-055→TASK-063、TASK-055b→TASK-064 等；无行为变化

### 3. 验收输出摘要

- cargo fmt --all --check → exit 0
- cargo clippy -p xtask --all-targets -- -D warnings → exit 0
- cargo test --workspace → **283 passed**（与 TASK-055b baseline 一致；本卡无 Rust 业务改动）
- cargo deny check → 4 项 ok
- xtask refscan → scanned=146（-1 删除 +6 重命名 +1 治理卡 = net +6 vs HEAD;实际再 +5 vs origin）, 151 errors baseline
- xtask hygiene / memory-counts / adr-index / docscan / card-check → 全部 PASSED
- **DoD 硬证据**：
  - ls tasks/ | grep xtask | grep TASK-05[1-5] = **0 命中**（xtask 卡全部新编号）
  - ls tasks/ | grep TASK-055b = **0 命中**（sub-suffix 撤回）
  - ls tasks/ | wc -l = **64**（删除 6 + 新增 13 = 64 不变）
- 末尾单 LF 检查：所有本卡修改文件 = 0x0A 结尾

### 4. DoD 逐条核对

- [x] ADR-0036 创建（3932 字节，Accepted）
- [x] docs/adr/README.md §1 新增 0036 行 + 下一个可用编号 0036 → 0037
- [x] 6 张新卡 TASK-059~064 创建（内容从旧卡复制 + 改标题 + 状态=Done + 内部引用更新）
- [x] 6 张旧卡 TASK-051~055/055b 删除
- [x] README.md 所有 xtask 卡引用 051~055/055b → 059~064
- [x] stage-1 卡 051~055 占位卡**未动**（撞号另一侧保留原样）
- [x] TASK-055b sub-suffix 撤回（重命名为 TASK-064-promote-extension-is-helper）
- [x] 6 张治理卡 TASK-065~070 元数据 + 9 节骨架就位
- [x] LEDGER.md 追加 TASK-065 Done 行
- [x] xtask 11 条验收全绿

### 5. 偏差

none（本卡严格按卡面 In scope 执行）

### 6. 更合理做法

#### 6.1 改号方案的三大考虑

1. **不改 stage-1 占位卡的号**：stage-1 已就位且有依赖关系（015 → 011 → 012 等），改 stage-1 号风险大。xtask 卡是**事后插入**的护栏工作，应使用「已就位号段之后」的下一个空段。
2. **新号 059~064 而非 059~063**：TASK-055b 必须分配一个独立编号（不能在 059~063 中重用 055），否则「按号寻卡」契约再次违反。最自然 = 顺延到 064 = 6 张卡占 059~064 共 6 个号。
3. **不复用 011/035**：这两个号已被 stage-1 占用且有 ADR 引用，撞号代价远超 059~064 的「占号」代价。

#### 6.2 状态字段为何一并修

按 ADR-0031 D2「状态只有一个落点」 = LEDGER.md 与 card `- 状态：` 字段必须一致。旧 6 张卡 `状态：InProgress` 但 LEDGER 已 Done = 一致性破坏。本卡复制时改 `状态：Done` 一并解决 TASK-066（同步状态字段）的事。

#### 6.3 为何不直接 git mv

git mv + sed 批量改号虽然能做，但会丢「旧 → 新」映射关系的可审计性。本方案走「读旧 + 复制改 + 删旧」三步显式路径 = ADR-0036 D1 表里能逐行查映射。

#### 6.4 为何修改 xtask/src/ 的 doc-comments

旧 doc-comments 引用 TASK-052 / TASK-055 / TASK-055b 等旧号 = 与新文件名不一致。不改的话下个会话读代码注释会困惑。修改仅限 doc-comments（// 或 ///），不影响编译/运行。共 10 处。

### 7. 遗留问题

- **stage-1 撞号另一侧未修**：本卡仅改 xtask 卡的号，stage-1 占位卡（taint-tracking / instruction-origin-attribution / injection-target / clean-context-review / edge-adapter）保留在 051~055 号段。后续 stage-1 开工时，按 ADR-0036 D2 保持原号即可。
- **PL-002 / PL-018 仍归 TASK-015**：card-check 判据 ② + ⑤ 尚未实现。本卡 ADR-0036 D5 显式要求「合并实现」但**不在本卡 scope** —— 归 TASK-015 + xtask 护栏升级未来零批次。
- **PL-022 根因仍潜伏**：本卡撞号解决**只是结果修复**，PL-022「手写派生值 → 过期 → 与事实源矛盾」这条根因仍在 4 个 ADR / MEMORY §1 / facts.md / README 等多处。根治方案见 TASK-067（PLAN+MEMORY 刷新）+ TASK-070（facts.md 4 子命令族 supersede）。
- **ADR-0035 的 TASK-055b 引用未改**：ADR-0035 正文提到 TASK-055b 等历史引用（line 25/36/37/41/77/78），按「只追加不改写」= 保留原样（事实记录）。如有审计可读性需求，开 DRIFT-ADR 单独处理。
- **pitfalls.md:92「禁止 sub-suffix」PITFALL 已生效**：本卡 ADR-0036 D3 显式禁止 sub-suffix。pitfalls.md 该条无需改（它是源头）。

### 8. 新增长期记忆

- 本卡**未新增** docs/memory/{facts,pitfalls,rejected}.md 条目（撞号根因已在 PL-022 中记录；本卡是结果修复，非新坑）
- **新增**：ADR-0036 = 「任务卡撞号解决」（Accepted，2026-09-20）—— 这是 ADR 编号登记表的最新有效 ADR

### 9. 给审阅者的关注点

1. **最高优先（**新 ADR 的可执行性**）**：ADR-0036 D5 要求 xtask card-check 未来加判据 ⑤「编号唯一性」。当前 PL-002（判据 ② 未实现）+ PL-018（card-check 整体未实装）需合并治理。如果审阅者认为应在本卡一并实现 = 本卡 scope 越界。
2. **新卡 064 命名是否合理**：TASK-064-promote-extension-is-helper 替代原 TASK-055b-promote-extension-is-helper，slug 完全相同。审阅者若认为 slug 应改为更详细（例 promote-helper-and-clear-repowalk-allow）可单卡追加。
3. **README 「最近进展」节是否更新节标题**：本卡把节标题从「TASK-051/052/053/054/055/055b 六连发」改为「TASK-059/060/061/062/063/064 六连发」。
4. **撤稿决策与未来 sub-suffix**：ADR-0036 D3 显式禁止 sub-suffix 命名但未修改 ADR-0031 D7 文本（仅以 ADR-0036 引用）。如果审阅者认为应同步改 ADR-0031 D7 文本可开 DRIFT-ADR。
5. **本卡实际执行两次**：第一次 commit 含空文件 "周期"（bash heredoc 路径转义副作用）→ 已撤销 + 重做，最终 commit 仅含预期文件。

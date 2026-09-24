# TASK-085　`xtask hygiene` 剩余规则 A 组：Rust 源码结构（gov §5.4 的第 1 / 2 / 3 / 10 / 11 项）

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**护栏（XTASK 池 072~099）**　依赖：015　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：015（`xtask` 护栏清扫：`docscan` 4 规则 / `crates/core` 分层断言 / `check-ledger` / `check-migrations` 已落地）
- **write scope**：`xtask/**`（规则实现 + 单测 + `deferred.rs` 登记表 + `cli.rs` 用法）
- **关联**：`docs/governance-ai-agent-execution.md` §5.4（13 项卫生规则表 = 唯一事实源）、ADR-0025（D1「先 Warning 后 Error」口径 / D4 计数口径）、ADR-0037 D1/D2（xtask 池 072~099）、`docs/PARKING_LOT.md` PL-059、`tasks/TASK-001-repo-skeleton.md` §7.3（`hygiene` 的既有 3 项规则）
- **预估**：M　**难度**：M

**目标（一句话）**

把 gov §5.4 里**需要 Rust 语法分析**的 5 项卫生规则实现为 `xtask hygiene` 的可跑规则，把「已实现 3/13」推进到「已实现 8/13」。

**为什么这 5 项一组（分组轴 = 是否需要「函数与属性扫描器」）**

| gov §5.4 表格行（逐字） | 计划规则 id | 为什么需要语法分析 |
|---|---|---|
| 单函数行数（> 80 行警告） | `hygiene/function-too-long` | 要函数边界（`fn` 起止行，含嵌套 `fn` / 闭包） |
| 函数参数个数（> 6 个警告） | `hygiene/too-many-parameters` | 要签名解析（泛型、生命周期、`self`、跨行参数表） |
| 圈复杂度（> 15 警告） | `hygiene/cyclomatic-complexity` | 要分支计数（`if` / `match` 臂 / `&&` `\|\|` / `?` / 循环） |
| 空实现 / `Ok(())` 直接返回的 stub（必须带 `// STUB: TASK-NNN`） | `hygiene/bare-stub` | 要「函数体是否为空」判定 |
| 测试文件是否被跳过（`#[ignore]` / `.skip`，必须带原因与卡号） | `hygiene/skipped-test-without-reason` | 要「属性 ↔ 测试函数」关联 |

这 5 项**共用同一个扫描器**（函数边界 + 属性），一起做边际成本最低；拆成 5 张卡会让同一扫描器写 5 遍（ADR-0030 同源：同一事实不该手写两处）。

**步骤**

1. **环境记录**（OS / Rust 版本 / 输入 fixture 路径）
2. **先写扫描器，再写规则**：`xtask/src/rustscan.rs` 的「函数与属性扫描器」——
   输入是**源码文本**（不是文件路径），输出是 `Vec<FunctionSpan>` / `Vec<TestAttribute>`；
   **判定函数必须是纯函数**（PL-048 的教训：单测不许碰文件系统）。
3. **逐条规则落地**，每条都要：
   ① 规则 id 与上表一致（gov §5.4 是唯一事实源）；
   ② **级别按 ADR-0025 D1 定**：先跑原型扫描数存量 → 有存量且存量里混着「不该修」的形态 → **先 Warning**，并在本卡 §7 记「清扫后升 Error」的后续动作；
   ③ **负向用例**（ADR-0019 N1：喂坏样本必须报，且喂好样本不许误报）；
   ④ **空断言防护**：扫描到 0 个函数必须显式失败或告警（铁律 1，不许「没扫到 = 通过」）。
4. **登记表同步**：`xtask/src/deferred.rs` 把本卡实现的 5 条从 `DEFERRED_HYGIENE_RULES` 移除，
   `IMPLEMENTED_HYGIENE_RULE_COUNT` 3 → 8；**不变量 1 的单测**（已实现 + 未实现 == 13）必须继续绿。
5. **跑测试**：`cargo test --workspace` + 本卡专项测试；不合格 → DRIFT（`DRIFT-085-x`）。

**DoD**

- [ ] 5 条规则各自有**正向基线 + 负向用例**（喂坏样本必须报；喂好样本不许误报）
- [ ] 5 条规则的**全仓实跑结果**（存量数）记入执行记录；级别判定按 ADR-0025 D1 写明理由
- [ ] 扫描器是**纯函数**（单测不碰文件系统）—— 不得重现 PL-048 的 flaky 形态
- [ ] `deferred.rs`：5 条移出 `DEFERRED_HYGIENE_RULES` + `IMPLEMENTED_HYGIENE_RULE_COUNT` = 8 + 不变量单测绿
- [ ] 「扫到 0 个函数」必须有显式行为（不许静默通过）
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / docscan / card-check / check-ledger` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**Out of scope（做了算漂移）**

- gov §5.4 的其余规则（CRLF / 末行换行 / 依赖登记 → **TASK-086**；重复代码相似度 / 顶层目录白名单 → 未拆卡，见 `docs/PARKING_LOT.md` PL-060）
- 给 `xtask hygiene` 加第 14 项规则（那是 PL-021，需先裁决 ADR-0025 的 13 项口径）
- 把 `hygiene` 接进 CI 的编号改动（gov §5.1 重编号需 ADR；见 PL-056 / PL-061）
- 顺手重构 `hygiene.rs` 的既有 3 条规则

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p xtask hygiene
cargo run -p xtask -- hygiene
cargo run -p xtask -- --list-deferred
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点

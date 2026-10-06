# ADR-0075　治理计数由事实源机器派生

状态：**Accepted**（2026-10-06，按用户 2026-10-06 预授权代为裁决；TASK-244）　日期：2026-10-06
Supersedes：—　Superseded by：—

来源：2026-10-06 自动化轮次 `ai-assistant-task-round-0800-soft-gates-or-derived-counts`
选择目标 (B)，收口 PL-022 的机器派生余项。

关联：ADR-0019（硬门禁负向验证）、ADR-0025（hygiene 规则数与 CI 门禁口径）、
ADR-0072（派生值只指向唯一事实源）、`docs/governance-ai-agent-execution.md` §5.1 / §5.4、
`.github/workflows/ci.yml`、`docs/PARKING_LOT.md` PL-022。

---

## 背景（为什么现在要决定）

`TOTAL_HYGIENE_RULE_COUNT` 曾硬编码为 13，依靠维护者记得在 `gov §5.4` 增减表格行时同步修改。
同一类问题也出现在 `gov §5.1` 与 `ci.yml` 的门禁清单：文档说“全部必过”，CI 文件实际有哪些步骤
则靠注释和审阅核对。PL-022 已记录这不是单次笔误，而是“同一事实被手抄到多处”的结构性风险。

ADR-0072 已冻结原则：动态派生值必须指向唯一事实源。本 ADR 把该原则落实到两个现存对象：
hygiene 规则总数、治理门禁清单。

## 决策（一句话）

`xtask` 必须从 `gov §5.4` 派生 hygiene 规则总数，并机器校验 `gov §5.1` 的门禁编号集合与
`ci.yml` 的显式门禁标记集合完全一致；解析失败、缺失、重复或额外标记均为 **Error**。

## 决策细化

### D1　hygiene 总数唯一事实源

`gov §5.4` 表格的数据行数是 hygiene 规则总数的唯一事实源。`xtask/src/deferred.rs` 不得再维护
数字常量；`cargo run -p xtask -- hygiene` 与 `cargo run -p xtask -- --list-deferred` 在运行时
读取该表并派生总数。

解析器必须满足：

1. 找到 `### 5.4` 后的表头与 separator；
2. 至少解析到 1 条数据行；
3. 任意一步缺失都返回显式错误，不得把空结果当作 0 项或 PASSED；
4. 已实现数 = 派生总数 − `DEFERRED_HYGIENE_RULES.len()`；若为负数则报 Error。

### D2　CI 门禁标记是事实源映射，不是第二份数量

`gov §5.1` 的每个数据行由一个门禁编号标识；主编号为 `1`~`16`，子编号为 `8b`、`12b`。
`.github/workflows/ci.yml` 中每个实际承接该门禁的步骤或作业必须写且只写一个标记：

```yaml
# gov-gate: 12b
```

`xtask` 从 `gov §5.1` 表派生编号集合，并从 `ci.yml` 解析所有 `# gov-gate: <id>` 标记。两集合
必须逐项相等；重复标记、缺失标记、额外标记都失败。标记是“哪个 CI 步骤承接哪个治理门禁”的
映射，不另写数量；数量由两边集合长度分别派生。

### D3　失败判据

以下情况都是 Error，退出码为 1：

- `gov §5.4` 标题、表头、separator 或数据行无法解析；
- 派生总数小于未实现规则条数；
- `gov §5.1` 编号集合无法解析；
- `ci.yml` 标记无法解析、重复、或与 `gov §5.1` 集合不一致；
- `hygiene` 或 `--list-deferred` 读不到上述事实源文件。

不得把不一致降级为 Warning，也不得自动改写文档或 workflow。

### D4　实现边界

- 解析逻辑是纯函数，测试用内联字符串，不碰真实文件；文件读取继续留在 `doccheck` / `main` 的 IO 边界。
- 不新增第三方依赖、crate、顶层目录或 `#[allow]`。
- 不把 gov #9 覆盖率或 #11 `cargo doc` 转硬；本 ADR 只处理 PL-022。
- `ci.yml` 只增加注释标记，不改变任何 job 的触发、命令或失败语义。

## 负向验证（ADR-0019）

本 ADR 新增机器判据的负向验证形式为 **N1**：

1. 把 `gov §5.4` 的测试字符串减一行 → 派生总数必须随事实源变化；
2. 从 `ci.yml` 测试字符串删除一个 `# gov-gate` 标记 → 集合比较必须报缺失；
3. 重复一个 `# gov-gate` 标记 → 必须报重复；
4. 移除 `gov §5.1` 表头或 separator → 必须报不可解析；
5. 派生总数小于未实现规则条数 → 必须报错而不是显示负数。

`cargo test -p xtask` 必须覆盖上述正负样本；只用“命令能跑完”不构成验证。

## 考虑过的选项

| # | 方案 | 结论 | 理由 |
|---|---|---|---|
| 1 | 继续硬编码数字，靠卡面提醒同步 | ❌ 否决 | 正是 PL-022 已证实的失效模式。 |
| 2 | 让 `xtask` 自动改写 `gov` / `ci.yml` | ❌ 否决 | 护栏应只读；自动改写会把文档审阅变成工具副作用。 |
| 3 | 只扫 `gov` 表格，不校验 CI 映射 | ❌ 否决 | 文档与 CI 仍可各走各的，PL-022 的第二半未解决。 |
| 4 | 用手写缓存清单保存 CI 门禁编号 | ❌ 否决 | 会重新引入第二份可漂移的手抄清单。 |
| 5 | **事实源解析 + CI 显式门禁标记集合校验**（本 ADR） | ✅ 采纳 | 数量从两边各自派生；标记只表达映射，缺失/重复/多余均被红灯拦住。 |

## 影响

| 对象 | 改动 |
|---|---|
| `xtask/src/deferred.rs` | 删除硬编码总数；新增纯解析与集合校验函数及负向单测。 |
| `xtask/src/doccheck.rs` | 读取 `gov` 与 `ci.yml`，调用纯解析并返回派生结果。 |
| `xtask/src/main.rs` | `hygiene` 与 `--list-deferred` 使用派生结果，失败时 exit 1。 |
| `.github/workflows/ci.yml` | 只增加 `# gov-gate: <id>` 注释标记。 |
| `docs/governance-ai-agent-execution.md` | §5.4 表述改为运行时派生；§5.1 指向标记规则。 |
| `PL-022` | 机器派生余项闭环。 |
| 产品闭集 / schema / 依赖 | **不改**。 |

## 验证方式

1. `cargo test -p xtask` 含正负样本并通过。
2. 本仓库上 `cargo run -p xtask -- hygiene` PASSED，且输出的 hygiene 总数来自表格。
3. `cargo run -p xtask -- --list-deferred` 打印派生总数与未实现 0 项。
4. 临时删除或重复一个 `ci.yml` 标记后，命令必须 exit 1；恢复后重新 exit 0。
5. `cargo run -p xtask -- adr-index` 为 0E0W，登记表三方一致。

## 重新评估触发条件

- 若 `gov §5.1` 改为机器生成的 YAML / JSON 清单，应删除正则标记解析并直接消费结构化事实源。
- 若 CI 拆分出多个 workflow 承接 `ci.yml` 的门禁，应把标记解析范围扩展到全部 workflow。
- 若 hygiene 规则总数不再由 `gov §5.4` 表格表达，必须先 supersede 本 ADR 再改判据。

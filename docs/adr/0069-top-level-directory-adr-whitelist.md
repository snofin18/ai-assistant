# ADR-0069　顶层目录必须登记在 ADR 白名单

状态：**Accepted**（2026-10-04，按用户 2026-10-04 预授权代为裁决）　日期：2026-10-04　Supersedes：—　Superseded by：—
关联：`docs/governance-ai-agent-execution.md` §5.4、`docs/adr/top-level-directories.md`、`docs/PARKING_LOT.md` PL-023 / PL-060、`tasks/TASK-234-hygiene-final-rules.md`

## 背景（为什么现在要决定）

gov §5.4 要求「新增顶层目录必须在 ADR 白名单中，否则失败」，但仓库此前没有白名单文件，也没有规定谁维护、
如何校验以及 `scripts/` 归属。结果是顶层目录只能靠人工记忆：新 agent 可能建出 `scripts/`、`tmp/` 或新的
顶层语言目录，而 CI 无法区分它是有意采用的结构还是临时堆放。

PL-023 已记录 `scripts/` 归属未决；PL-060 要求本规则先有 ADR 才能实现。

## 决策（一句话）

仓库顶层目录的唯一白名单是 `docs/adr/top-level-directories.md`；任一非构建产物顶层目录未在该表登记时，
`xtask hygiene` 产生 Error。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | 白名单文件固定为 `docs/adr/top-level-directories.md`，由 ADR-0069 授权创建；它不是编号 ADR，`adr-index` 继续忽略它。 |
| **D2** | 白名单采用固定二级标题 `## 允许的顶层目录` 与人读表格；第一列必须是反引号包围的目录名，末尾可带 `/`，解析时统一去尾斜杠。 |
| **D3** | 实际扫描范围为仓库根下的一级**目录**；跳过 `.git`、`target`、`node_modules`、`dist`。`.github` 是受版本控制目录，照常参与白名单校验。 |
| **D4** | 未登记目录 = `hygiene/unregistered-top-level-directory` **Error**，路径为 `<dir>/`，消息必须指出白名单文件并要求先落 Accepted ADR。 |
| **D5** | 白名单有、磁盘没有的条目 = `hygiene/stale-top-level-directory` **Warning**，提醒清理，但不在删除目录时制造阻塞。 |
| **D6** | 白名单文件缺失、标题缺失或没有可解析行 = Error `hygiene/top-level-whitelist-unparsable`；不得把“解析不到”当成空白名单或跳过规则。 |
| **D7** | 维护者 = Orchestrator；实施者只能新增完成标记、不能私自扩表。新增任何顶层目录前必须：① 先有 Accepted ADR 或 ADR-0069 的正式修订；② 在白名单表格登记目录、用途与授权 ADR。 |
| **D8** | 与 PL-023 的关系：当前仓库没有 `scripts/`，因此白名单也不登记它。未来若需要 `scripts/`，必须由新 ADR 明确它与其他 `tools/`、`xtask/` 的边界后再加入白名单；本 ADR 不预先授权。 |
| **D9** | 白名单内容的第一版只登记当前已存在且属于受控仓库结构的目录：`.github`、`adapters`、`apps`、`crates`、`docs`、`eval`、`fixtures`、`plans`、`protocol`、`spikes`、`tasks`、`tools`、`xtask`。 |

## 考虑过的选项

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 白名单写在 ADR-0069 正文里 | ❌ 否决 | 修改目录清单会要求改写 ADR 正文，违反 ADR 只增不改；独立附表可演进且不影响 ADR 编号。 |
| 2 | 独立 `docs/adr/top-level-directories.md` | ✅ 采纳 | 机器可解析、可单独审阅，且 ADR 仍保留“为什么”的唯一决策。 |
| 3 | 新目录未登记只报 Warning | ❌ 否决 | gov §5.4 明写失败；顶层目录是仓库结构契约，不是局部风格建议。 |
| 4 | 只检查 `git ls-files` 中出现的目录 | ❌ 否决 | xtask 目前不调用 git 子进程；读取一级目录并对构建产物显式忽略更简单、跨平台、零依赖。 |
| 5 | 自动把未登记目录插入白名单 | ❌ 否决 | 那会把“发现结构变化”变成静默接受，违反铁律 1 与契约先行。 |

## 影响

- 新增 `docs/adr/top-level-directories.md` 初始白名单。
- `xtask/src/top_level_dirs.rs`：解析白名单、判定未登记 / 陈旧 / 不可解析，并带正负单测。
- `xtask/src/repowalk.rs`：新增确定性的顶层目录收集函数。
- `xtask/src/main.rs`：`hygiene` 读取白名单并调用规则。
- `docs/adr/README.md`、`docs/memory/decisions.md`：ADR-0068 / 0069 登记。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 新增目录者只改白名单、不写 ADR | 白名单每行必须写授权 ADR；审阅者按 D7 拒绝无授权行。 |
| 构建目录被误报 | D3 显式排除 `target` / `node_modules` / `dist` 与 `.git`；`.github` 仍受控。 |
| 删除目录留下陈旧行 | D5 用 Warning 提醒，不阻塞；清理由 Orchestrator 完成。 |
| 白名单解析失败静默跳过 | D6 直接 Error，且 parser 返回 `None` 时不得当作空集合。 |
| 与 `scripts/` 归属冲突 | D8 明确不预授权；PL-023 仍需单独裁决，白名单只反映已批准的现状。 |

## 验证方式

1. 单测：白名单解析正例；未登记目录为 Error；已登记目录通过；陈旧条目为 Warning；不可解析为 Error。
2. `cargo run -p xtask -- hygiene` 在当前仓库为 0E，并声明 13/13 已实现。
3. `cargo run -p xtask -- --list-deferred` 的 hygiene 未实现项为空。

## 相关 ADR

- ADR-0025（hygiene 规则总数）
- ADR-0068（跨文件重复代码 hygiene 规则）

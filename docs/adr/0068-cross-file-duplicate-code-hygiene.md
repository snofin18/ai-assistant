# ADR-0068　跨文件重复代码 hygiene 规则

状态：**Accepted**（2026-10-04，按用户 2026-10-04 预授权代为裁决）　日期：2026-10-04　Supersedes：—　Superseded by：—
关联：`docs/governance-ai-agent-execution.md` §5.4、`docs/PARKING_LOT.md` PL-060、`tasks/TASK-234-hygiene-final-rules.md`

## 背景（为什么现在要决定）

gov §5.4 明列「重复代码（跨文件相似度）」为 13 条仓库卫生规则之一，但没有定义指纹口径、相似度阈值、
忽略路径与扫描预算。没有这些契约就无法实现可复现的门禁：同一对文件可能因 token 化方式、阈值或忽略集不同
而得到相反结论。

PL-060 已明确本规则必须 ADR 先行。本 ADR 冻结第一版可执行判据，规则级别按 gov §5.4 保持 Warning。

## 决策（一句话）

对合格 Rust 源文件生成**规范化 token shingle**；任意两文件在至少 4 个共享 shingle 且包含度 ≥ 0.80 时，
在该规则下产生一条 Warning；本版不升级 Error。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | 指纹单位 = 40 个连续 token 的 shingle；token 级而不是行级，因此重排空行、改缩进、重命名局部变量不会破坏指纹。 |
| **D2** | 规范化 = 忽略空白、普通注释与块注释；字符串 / 字节串 / 字符字面量归一为 `literal`；数字归一为 `number`；非关键字标识符归一为 `identifier`；关键字、标点和运算符保留原词法。 |
| **D3** | 相似度 = `shared_distinct_shingles / min(left_distinct_shingles, right_distinct_shingles)`（包含度），不是对称 Jaccard。这样“一份逻辑被复制进更大的文件”也能被发现。 |
| **D4** | Warning 阈值 = 包含度 **≥ 0.80** 且共享 distinct shingle **≥ 4**。两个条件必须同时满足；任一低于阈值不报。 |
| **D5** | 每个无序文件对最多报一条 `hygiene/duplicate-code`；报告路径取字典序较小者，行号取该文件中第一个共享 shingle 的起始 token 行。规则级别 = **Warning**，本版**不得**升级 Error。 |
| **D6** | 忽略路径 = `fixtures/**`、`spikes/**`、`tools/**`、任何 `/tests/` 或 `tests/` 开头的路径、文件名以 `_tests.rs` 结尾的 `#[path]` 测试模块、任何 `/generated/` 路径以及 `crates/protocol/src/generated/**`。这些目录由生成器、测试夹具或一次性探针主导，纳入会制造噪声而不是发现产品复制粘贴。 |
| **D7** | 合格文件还必须有 ≥ 80 个规范化 token、≥ 4 个 distinct shingle；同一文件内部不比较。空文件、极小文件与纯声明文件不参与。 |
| **D8** | 候选生成不能做全量文件两两比较：按 shingle 哈希建倒排表，只统计共享哈希的文件对；一个 shingle 若出现在超过 8 个文件中，视为公共样板并跳过。 |
| **D9** | 性能与确定性预算：最多扫描 1,000 个合格文件、单文件最多 1 MiB、全局最多 1,000,000 个 shingle；文件按仓库相对路径排序后消费。超限时截断并产生一条 Warning `hygiene/duplicate-scan-truncated`，不得静默返回“全绿”。 |
| **D10** | 目标扫描时间上限 = 当前仓库在 CI runner 上 **≤ 2 秒**。时间不进入规则判定（避免环境相关红灯）；超过预算是优化或重定预算的触发器，不是 pass/fail 判据。 |
| **D11** | 规则只做 Warning，不自动改写、不提供 cross-file allowlist。确实需要保留复制粘贴时，人类先通过新 ADR 修改忽略集或阈值；不得在源码里写通用豁免注释绕过。 |

## 考虑过的选项

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 行文本完全相同 / 连续 N 行相同 | ❌ 否决 | 缩进、空行、局部变量重命名即可规避；对 agent 生成的代码过弱。 |
| 2 | token shingle + 包含度阈值 | ✅ 采纳 | 能发现复制粘贴与轻微改名，且纯逻辑、零依赖、可单测。 |
| 3 | AST 子树哈希 | ❌ 否决 | xtask 目前没有 Rust parser 依赖；引入 parser 等于新增第三方依赖，超过 PL-060 的护栏范围。 |
| 4 | Jaccard 对称相似度 | ❌ 否决 | 大文件包含一段复制代码时会被稀释，漏报正是本规则最该抓的场景。 |
| 5 | 直接做 Error | ❌ 否决 | gov §5.4 对重复代码定义为 Warning；先 Warning 才能观察并清理存量，避免把噪声训练成“忽略 CI”。 |

## 影响

- 新增 `xtask/src/duplicate_code.rs`：规范化 token、shingle、候选对、Finding 生成与单测。
- `xtask/src/main.rs`：在 `hygiene` 中收集已读取的 Rust 源文件并调用跨文件判定。
- `xtask/src/deferred.rs`、`xtask/src/cli.rs`、`xtask/README.md`：实现计数由 11/13 同步为 13/13。
- 不新增依赖、不启动子进程、不写文件；扫描上限保证内存有界。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 过度归一化导致样板代码误报 | 只忽略测试 / 生成 / fixture；40-token 窗口、4 个共享窗口、0.80 包含度三重门槛。 |
| 公共 shingle 导致 O(n²) | 倒排表中出现超过 8 个文件的 shingle 直接跳过；全局 shingle 数有硬上限。 |
| 哈希碰撞造成假阳性 | 使用 64 位确定性哈希并在同一条消息中输出共享数；阈值较保守，后续可在不改判据的前提下记录碰撞观察。 |
| 大文件拖慢门禁 | 单文件 1 MiB、总 shingle 250,000 硬上限，超限显式 Warning。 |
| 规则行为随平台变化 | token 化、排序、哈希与阈值全部是纯函数；路径统一 `/`，测试跨平台。 |

## 验证方式

1. 单测：完全相同代码对命中；重命名局部变量后的复制命中；不同代码不命中；测试 / 生成路径被忽略；阈值边界不命中。
2. `cargo test -p xtask` 全绿，`cargo run -p xtask -- hygiene` 的 deferred 行显示 13/13。
3. 对本仓库运行 `hygiene`，原始输出贴入 TASK-234 执行记录与 PR；目标耗时 ≤ 2 秒。

## 相关 ADR

- ADR-0025（hygiene 规则总数）
- ADR-0069（顶层目录 ADR 白名单）

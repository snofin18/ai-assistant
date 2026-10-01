# ADR-0027　测试代码的 lint 允许清单

状态：**Draft**（自动工作单 W4，待人类批准）　日期：2026-10-02　Supersedes：decisions.md `[ADR:待建 0027]`　Superseded by：—
关联：`Cargo.toml`、`docs/spec/testing.md`、`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5

## 背景（为什么现在要决定）

workspace lint 对产品代码禁用 `unwrap_used`、`expect_used`、`panic`，
用于把“无静默失败”与不允许崩溃的要求机器化。

但测试的失败语义恰好相反：断言失败或夹具构造失败时，测试应该明确、立即失败；
若为了满足产品 lint 而在测试中层层传播 `Result`，会稀释失败位置并增加无意义的样板代码。
因此需要一个**窄而可审计**的允许面，既让测试保持 fail-fast，又不得把危险语义带回产品代码。

## 决策（一句话）

**`clippy::unwrap_used` / `clippy::expect_used` / `clippy::panic` 只允许在测试代码的
`#[cfg(test)] mod tests` 上豁免；产品代码不得添加这些 `#[allow]`。**

决策细化：

| # | 内容 |
|---|---|
| **D1** | 允许清单只有 `clippy::unwrap_used`、`clippy::expect_used`、`clippy::panic` 三项。 |
| **D2** | 唯一合法位置是测试模块的 `#[cfg(test)] mod tests` 属性；不得扩大到产品模块。 |
| **D3** | 产品代码中的 `#[allow]` 属于放宽护栏，必须按漂移触发器升级并取得人类裁决。 |
| **D4** | 该规则必须进入 `docs/spec/testing.md`，使测试作者与审查者有单一事实源。 |
| **D5** | 允许的目的是让测试 fail-fast，不是允许产品代码把“不应失败”的路径变成崩溃。 |

> 落地状态（2026-10-02）：`docs/spec/testing.md` 已存在但尚未写入本节；自动轮次
> 不得修改既有 spec，因此此缺口记录为 `DRIFT-W3-1`，本 Draft 不改写 spec。
> 同时，后续 **ADR-0035** 已对产品代码中的 per-line allow 给出“注释 + 任务卡登记”限制；
> 本 Draft 忠实保留 2026-09-16 的原始决定，但在批准前必须由人类裁决两条规则的适用边界。

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | **只在 `#[cfg(test)] mod tests` 允许三项 lint（本 ADR）** | ✅ 采纳 | 测试可以直接 fail-fast；产品代码仍保持严格 deny。 |
| 2 | 在 workspace 全局允许三项 lint | ❌ 否决 | 产品代码会重新允许 panic / unwrap，等于取消铁律 1 的机器表达。 |
| 3 | 允许在任何测试性质的 crate 根放宽 | ❌ 否决 | “测试性质”容易扩散为任意产品 crate 的例外，允许面不可审计。 |
| 4 | 测试中完全不使用 `unwrap` / `expect` / `panic` | ❌ 否决 | 会给每个测试增加无价值的错误传播样板，且失败点不直观。 |

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

- `Cargo.toml`：继续在 workspace 层 deny 三项 lint。
- `docs/spec/testing.md`：新增测试允许清单与理由；当前为已知待补缺口。
- 测试模块：允许 `#[cfg(test)] mod tests` 使用三项 lint。
- 代码审查：把产品代码中的相关 `#[allow]` 视为漂移触发器。

本 ADR 不决定 integration test harness 的具体形式；如需为 `tests/**` 定义额外规则，
应由 spec 明确后另案处理。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 允许面从测试模块扩散到产品模块 | 审查时只接受紧邻 `#[cfg(test)] mod tests` 的允许；`check-comments` 或后续规则可机器检查。 |
| 测试用 `expect` 掩盖错误分支 | 测试仍需对错误路径做显式断言；允许 panic 不等于跳过负向覆盖。 |
| spec 未同步导致下个 agent 照旧实现 | `DRIFT-W3-1` 已记录；批准本 ADR 时同步补 `docs/spec/testing.md`。 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `Cargo.toml` 的 workspace lint 仍 deny `unwrap_used` / `expect_used` / `panic`。
2. 产品代码中的三项 `#[allow]` 为 0；测试模块的允许可逐项追溯。
3. `cargo clippy --all-targets -- -D warnings` 全绿。
4. `docs/spec/testing.md` 包含本 ADR 的三项允许清单与产品代码禁令。
5. **重新评估触发**：若 Rust / Clippy 提供按测试目标生效的等价 lint 配置，
   可用新 ADR 替换手写允许，但不得扩大产品代码的允许面。

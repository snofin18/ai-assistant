# rmcp / chrono RUSTSEC-2026-0543 依赖修复审计

## 1. 结论

当前没有可直接落地的官方升级路径。`rmcp` 3.7.0、3.8.0、3.10.0、3.11.0
和官方 `main` 都仍声明 `chrono`。项目当前依赖链为：

```text
assistant-tool-bus
  -> rmcp 3.9.0 (client + server, default-features = false)
  -> schemars 1.2.2 (chrono04)
  -> chrono 0.4.45
```

`cargo deny check` 因此失败，advisory 为 `RUSTSEC-2026-0543`，状态为
unmaintained，且明确说明 `No safe upgrade is available!`。

## 2. 官方上游状态

- 最新 release：`rmcp-v3.11.0`，tag commit `165f8fd514afb8f6d1be3e86260ee9abfce58541`。
- `rmcp` 3.11.0 Cargo.toml 仍包含：
  - `schemars` 的 `chrono04` feature；
  - 非 wasm 目标的 `chrono 0.4.38` 依赖。
- 上游 issue #1469 已公开记录该问题。
- contributor 分支 `andreinknv/rust-sdk:chrono-optional` 的 commit
  `a95b8045c69c58f08a7bc1bd7642d0e3cf25a5ba` 提供候选方案：
  - 默认 feature 保留 chrono 与两个 chrono 类型 builder；
  - `default-features = false` 时使用 `jiff`；
  - 该分支尚未合并到官方仓库，也没有正式 release。

## 3. 可选方案

### A. 等待官方修复并升级（首选）

优点：不改变来源策略，不需要豁免，供应链最干净。

缺点：当前尚未发布，PR #757 暂时无法解除 `cargo deny` 阻塞。

### B. 固定 contributor fork SHA（需人类批准）

只把 `Cargo.toml` 的 `rmcp` 指向已审查的完整 commit，不跟随 branch 漂移；
同时需要在 `deny.toml` 的 `sources` 中登记该 Git 来源，并更新
`docs/DEPENDENCIES.md`。

优点：可以立即验证 chrono 是否从依赖树移除。

缺点：个人 fork 的生命周期和可信度由本仓承担；官方正式发布后必须切回。

### C. 带期限的 advisory 豁免（最后备选）

在 `deny.toml` 增加 `RUSTSEC-2026-0543` ignore，并配置到期日与删除触发条件。

按项目规则，这需要独立 Accepted ADR 与人类批准。它只能隐藏门禁失败，不能把
chrono 变成已维护依赖，因此不应被记录成依赖修复。

## 4. 当前处置

TASK-382 已进入 **Review** 的受控 patch 验证：

- `crates/tool-bus/Cargo.toml` 固定到 contributor commit `a95b8045…`；
- `deny.toml` 仅允许该 Git 来源；
- 仍使用 `default-features = false` 和 `client,server`，不进入 chrono
  默认 feature，也不进入 auth / reqwest 网络链；
- 主仓验证结果：
  - `cargo tree -i chrono --workspace`：无 chrono；
  - `cargo deny check`：advisories / bans / licenses / sources 全绿；
  - `assistant-tool-bus` all-targets check、workspace 全量测试与 Clippy 全绿；
  - schema、codegen、格式门禁全绿。

该方案仍需在 PR checks 通过后合并；官方 release 发布后必须切回 registry。

## 5. 解除条件

满足以下任一条件后才进入实现：

1. 官方 `rmcp` 发布包含 chrono-optional / jiff 迁移的版本；
2. 人类批准固定 contributor fork SHA，并接受来源策略变更；
3. 人类批准带期限的 advisory 豁免 ADR，并指定到期日和迁移触发条件。

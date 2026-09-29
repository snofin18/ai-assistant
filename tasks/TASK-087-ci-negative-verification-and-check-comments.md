# TASK-087　CI 硬门禁负向验证与 `check-comments` 落地

- 状态：**Ready**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：015、039
- 预估：L　难度：L
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：ADR-0019、gov §5.1/§5.4、`xtask/src/deferred.rs`、`.github/workflows/**`

## 目标

关闭 PL-018 并实现 gov #15：为 fmt / clippy / build 增加可执行的负向验证，
实现 `check-comments` 的真实检查与 CI 接线，删除“未实现 stub”状态。

## In scope

- `.github/workflows/gate-selftest.yml`、`.github/workflows/ci.yml`。
- `xtask/src/**`、`xtask/README.md` 中与 `check-comments` 相关的实现。
- `docs/adr/0019-hard-gate-negative-verification.md` 登记表。
- `docs/PARKING_LOT.md`、测试与本卡记录。

## Out of scope

- UI Prettier/ESLint/Vitest。
- commitlint。
- 放宽现有 lint 或删除任何门禁。
- 修改 core/policy/task-engine 的产品行为。

## 必须遵守

- 每个新硬门禁必须有 N1/N2/N3 负向验证。
- canary 断言必须绑定具体失败模式，不能只断言“非零退出”。
- `check-comments` 必须按 `docs/spec/naming.md` 实现，未知规则不得静默忽略。
- 先证明门禁会红，再接入阻断。
- ADR-0019 登记表与 workflow 同步，否则不得宣称关闭 PL-018。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p xtask
cargo run -p xtask -- check-comments
cargo run -p xtask -- hygiene
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
```

另须手工触发 `gate-selftest` 并记录成功 run。

## 完成定义（DoD）

- [ ] fmt / clippy / build 三类 canary 全部可执行且绑定具体失败模式。
- [ ] `check-comments` 不再是 exit 3 stub，负向/正向测试齐全。
- [ ] CI 将 `check-comments` 作为真实门禁。
- [ ] ADR-0019 登记表同步，PL-018 可关闭。
- [ ] gate-selftest 至少一次成功 run 记录在 LEDGER。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点

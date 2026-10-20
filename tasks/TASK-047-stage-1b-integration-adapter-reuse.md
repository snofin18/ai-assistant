# TASK-047　阶段 1b 集成验收 + Adapter 复用度检查（Paint 是否被迫改了平台层？改了 → 记 ADR）

- 状态：**Done**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：040~046　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：040~046　**预估**：M　**难度**：M
- **write scope**：`docs/audits/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1b（1b）、`docs/wbs-overview.md` §6（DoD）

**目标**

阶段 1b 集成验收 + Adapter 复用度检查（Paint 是否被迫改了平台层？改了 → 记 ADR）。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`docs/audits/**`

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-047 阶段 1b 集成验收 + Adapter 复用度检查
【目标】以 TASK-040~046 及实际 1b bridge 卡为范围，核对 Paint 三条任务闭环、
        平台公共接口变更的 ADR 覆盖、Adapter 是否复用既有抽象，并给出 GO / 条件 GO / NO-GO
【write scope】仅 docs/audits/**；收尾按 AGENTS.md 更新本卡记录区、LEDGER、PLAN、
              README、plans/stage-1-pilots.md
【铁律】无静默失败；证据必须来自真实测试/合并记录；公共接口变更必须先有 ADR；
        不得用“CI 有 step”替代实际结果；不得越权把 Review 卡写成 Done
【禁止】改产品代码、platform/api trait、ADR 已决事项或其它任务卡正文；
        不得把 TASK-257 的 Review 状态静默当作 Done；不得因成功率反推未测指标
【验收】审计报告逐项引源；cargo fmt/clippy/workspace；xtask
        hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger
【依赖】TASK-040~046、TASK-106 / 231 / 256 / 257 / 292 / 293 / 294 与
        ADR-0067 / 0076 / 0085 / 0086 / 0091 / 0092 / 0093；已核对 main 63f81da
【疑问】卡面步骤仍是占位且只列 040~046；默认把实际 bridge 卡纳入审计并记录 DRIFT-047-1
```

### 2. 实际改动文件

- `docs/audits/stage-1b-integration-adapter-reuse-2026-10-20.md`
- `tasks/TASK-047-stage-1b-integration-adapter-reuse.md`（只改状态行与记录区）

### 3. 验收输出摘要

```text
cargo fmt --all --check
-> exit 0

cargo clippy --all-targets -- -D warnings
-> exit 0（仅既有 unknown lint `clippy::assert_is_empty` warning）

cargo test --workspace
-> exit 0；全部非 ignored 测试与 doctest 通过

xtask verify-schemas / codegen --check / hygiene / memory-counts /
adr-index / refscan / docscan / card-check / check-ledger / check-comments
-> 全部 PASSED

cargo deny check
-> advisories / bans / licenses / sources OK

xtask replay --suite core
-> 2 steps / 5 changes / 0 mismatch / PASSED
```

关键审计数据：

- T3.1 / T3.2 / T3.3 标准真机任务各 **10/10**，合计 **30/30**。
- T3.1 / T3.2 独立坐标误差均为 **0 px**。
- T3.3 单矩形拖拽撤销粒度实测 **1 step**，像素回滚 10/10。
- full ribbon path 14 步 committed，独立真机用例 1/1 passed。
- `platform/api` 在 1a GO 后出现 **6 类** Paint 驱动扩展，全部有 Accepted ADR。

### 4. DoD 逐条核对

- [x] 审计交付物覆盖技术 verdict、阶段 DoD、Adapter 复用、公共接口变更与行政残留
- [x] `cargo fmt --all --check` 0 diff
- [x] `cargo clippy --all-targets -- -D warnings` exit 0
- [x] `cargo test --workspace` 全绿
- [x] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check`
      以及 `check-ledger / check-comments / verify-schemas / codegen --check` 全 PASSED
- [x] `LEDGER.md` 已追加；本卡没有新增应用专属 FACT/PITFALL，审计结论集中在报告

### 5. 偏差

`DRIFT-047-1`

- **触发器**：#5（卡面步骤仍是占位）与 #8（仅列 TASK-040~046，未列实际 1b bridge 卡）。
- **现象**：Paint 真机闭环实际依赖 TASK-106 / 231 / 256 / 257 / 292 / 293 / 294
  和 ADR-0067 / 0076 / 0085 / 0086 / 0091 / 0092 / 0093；只看 040~046 会把平台 API
  变更漏出审计面。
- **影响**：若机械按卡面依赖收集，会错误得出“Paint 没改平台层”的结论。
- **自裁决**：审计范围按实际交付链扩展，但仍只写 `docs/audits/**`；不修改产品代码、
  不改其它卡状态。结论区分技术闭环与行政状态，TASK-257 未收口就不得宣称整个 1b
  批次无需后续动作。

### 6. 更合理做法

- 把“技术 GO”和“行政条件 GO”分开记录。Paint 功能证据已经成立，但 TASK-257
  和计划表状态仍需独立治理收口；混成一个总 GO/NO-GO 会损失信息。
- 审计公共 API 变化时以 stage-1a GO commit `66298b7` 为基线，不把 fallback 语义的
  后续治理提交误算进 Paint 驱动变更。
- 对 platform-only 修复单独分类：它们改了 Windows 实现，但没有改 `platform/api`
  公共 trait，因此不应被算作“第六个、第七个接口扩张”。

### 7. 遗留问题

- `TASK-257` 仍为 `Review`。其目标在功能路径上已被 ADR-0091、TASK-256 和后续真机
  闭环覆盖，但本卡只有 `docs/audits/**` 权限，不能代改状态或勾选正文 DoD。
- `plans/stage-1-pilots.md` 中 TASK-044 的描述仍写 `Ready`、TASK-256 仍写 `Review`，
  与两张卡的 Done 状态不一致；需由计划维护者在允许的状态面单独修正。
- Paint 工具集中的图像 / 选择 / Copilot / 层组仍未进入 schema；不影响 T3.1~T3.3
  和 full ribbon path 的本轮结论。

### 8. 新增长期记忆

- 无新增应用专属 FACT/PITFALL。结论已固化为阶段 1b 审计报告
  `docs/audits/stage-1b-integration-adapter-reuse-2026-10-20.md`。

### 9. 给审阅者的关注点

1. 六类 `platform/api` 变更是否都能追溯到真实 Paint 需求与 Accepted ADR。
2. 是否接受“没有 Paint 专用 trait，但公共 API 确实发生了六类加法式演进”的复用结论。
3. 是否同意把 TASK-257 与计划表状态滞后作为行政条件 GO，而不是抹平为无条件 GO。

# TASK-040　合成输入完善：拖拽（按下-移动-释放的原子性与租约独占）+ 校准流程（首次显示器组合点击已知元素验证命中）

- 状态：**Ready**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：018　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：018　**预估**：M　**难度**：M
- **write scope**：`crates/platform/windows/src/input/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1b（1b）、`docs/wbs-overview.md` §6（DoD）

**目标**

合成输入完善：拖拽（按下-移动-释放的原子性与租约独占）+ 校准流程（首次显示器组合点击已知元素验证命中）。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/platform/windows/src/input/**`

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
【任务】TASK-040 合成输入完善：拖拽原子性与校准
【目标】在 input 层补齐拖拽 fail-closed 边界和可单测的多显示器命中校准判定
【write scope】仅 crates/platform/windows/src/input/** + 本卡记录区 + LEDGER / PARKING_LOT / memory / 轮次文件
【铁律】1 无静默失败；5 L4 最后手段；9 不扩大范围；ADR-0028；ADR-0063
【禁止】改 crates/platform/api/**、真实 GUI 操作、lease/policy/task-engine、加依赖、改卡面正文
【验收】fmt / clippy / test --workspace / xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check
【依赖】TASK-018 Done；阶段 1a GO
【疑问】自动化禁止真实 GUI；真实首次点击校准证据和跨层 lease 集成保持 WIP
```

### 2. 实际改动文件

- `crates/platform/windows/src/input/calibration.rs`（新增）
- `crates/platform/windows/src/input/mod.rs`
- `crates/platform/windows/src/input/win32.rs`
- 本卡；`LEDGER.md`、`docs/PARKING_LOT.md`、`docs/memory/{facts,pitfalls}.md`
- `docs/automations/2026-10-02-round-2.md`、`docs/automations/2026-10-02-report.md`

### 3. 验收输出摘要

- `cargo test -p assistant-platform-windows`：PASS（lib 88 passed / 2 ignored；
  `handle_discipline` 7 passed；`synthetic_input_contract` 4 passed）。
- `cargo clippy -p assistant-platform-windows --all-targets -- -D warnings`：EXIT 0。
- Linux / macOS `--target` clippy：EXIT 0。
- `cargo run -p xtask -- check-comments`：0 error / 69 warning / PASSED。
- `cargo run -p xtask -- hygiene`：0 error / 102 warning / PASSED（无新增 error）。

### 4. DoD 逐条核对

- [x] 新增纯校准判定：3..64 个样本，按显示器 ordinal 去重，任一轴误差超限即 `VerifyFailed`。
- [x] 拖拽拒绝缺 drop 点与零距离 drop 点；拖拽仍编成单个 `SendInput` 批次。
- [x] 部分插入且前缀可能留下左键按下时，按 `SendInput` 返回的插入数条件式补发一次 `LeftUp`。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0。
- [x] `cargo test --workspace` 全绿。
- [x] xtask 门禁全 PASSED。
- [ ] 真实“首次显示器组合点击已知元素”命中证据：自动化禁止操作真实 GUI，未执行。
- [ ] 跨层目标租约独占：归 `crates/lease` / Host 装配，不在本卡 write scope，未执行。

### 5. 偏差

**DRIFT-040-1（真实 GUI 校准与 lease 集成超出自动化边界）**

- 现象：卡片标题包含“首次显示器组合点击已知元素验证命中”和“租约独占”。前者要在真实
  桌面点击已知元素；后者要跨到 `crates/lease` 或 Host 装配，均超出
  `crates/platform/windows/src/input/**`。
- 影响：本轮交付的是可机器验证的校准判定、拖拽参数边界和部分批次恢复；不能声称真实
  点击校准已完成，也不能声称目标 lease 已接入。
- 建议：真实校准作为人工验收项执行；目标 lease 归 TASK-025 / 后续装配卡，并登记 `PL-101`。
- 已停工作：未启动或操作任何真实应用，未改 `crates/platform/api/**` / `crates/lease/**`。

### 6. 更合理做法

把“校准”拆成纯判定层与人工取证层：纯判定层在 CI 中稳定覆盖坐标系、显示器和误差阈值；
真实点击只采集 expected/observed 样本，由人工或受控真机验收喂给同一个纯函数。这样既保留
可回放性，又不让无人值守流程去操作用户桌面。

### 7. 遗留问题

- **PL-101**：真实首次校准样本与跨层目标 lease 集成仍待后续卡/人工验收。
- `PL-074` 的混合 DPI 无目标窗口问题仍在；本卡没有改公共 `pointer_action` 签名。

### 8. 新增长期记忆

- FACT：input 层新增有界 `calibrate_pointer_samples`，3..64 个样本、按显示器 ordinal 计数，
  任一轴误差超限即 `VerifyFailed`。
- PITFALL：`SendInput` 部分插入时不能因为看到过 `LeftDown` 就盲发 `LeftUp`；必须先按
  返回的插入数判断前缀是否可能仍处于按下状态，否则可能误释放用户正在按的鼠标键。

### 9. 给审阅者的关注点

- 审阅 `pointer_requires_release_recovery` 的“已插入前缀”判据是否足够保守。
- 审阅校准的 per-axis max 判定和 64 样本上限是否符合后续 Paint 坐标需求。
- 确认本 WIP 不应标 Done：真实 GUI 校准与 lease 集成仍未完成。

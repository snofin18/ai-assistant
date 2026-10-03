# TASK-228　真机验收结果结构化输出（PASS / SKIP / FAIL 可机器区分）

- 状态：**Ready（待派单；来源 = `PL-106` / `DRIFT-040-2`）**
- 阶段：1　子阶段：1a 补救 / 治理　批次：治理池　依赖：TASK-040（已收口）、TASK-220（真机用例基线）
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/PARKING_LOT.md` 的 `PL-106`、`crates/platform/windows/src/input/acceptance.rs`、`tasks/TASK-040-synthetic-input-drag-lease-calibration.md` §5

---

## 目标（一句话）

让真机（`#[ignore]`）验收用例的结果**可被机器区分**：每次运行产出一份结构化记录（每个用例
`pass` / `skip` / `fail` + 原因 + 关键测量值），而不是只看「测试有没有变红」。

## 背景（为什么现在做）

`DRIFT-040-2` 实测暴露了根因：Rust test 框架只有 `ok` / `failed` 两种状态，本模块的跳过是
「打印原因后 `return`」→ 报告里仍是 `ok`。2026-10-03 那轮 4 个真机用例里实际只有 2 个真正执行，
而报告显示 4 个 `ok`；`SetProcessDpiAwarenessContext` 每进程只生效一次这条诱因虽已修掉
（TASK-040），但**报告形态的缺陷仍在**：任何其它提前返回（前置不满足、平台能力缺失、靶机起不来）
都会被读成 PASS。

## write scope

- `crates/platform/windows/src/input/**`（记录结构与写入；真机用例调用它）
- `xtask/src/**`（若选择用 xtask 子命令读取/汇总记录；二选一，不必两处都动）
- `tools/**`（若选择独立小工具而非 xtask 子命令）
- `tasks/TASK-228-structured-real-machine-acceptance-record.md`（本文件）与状态同步文件

## In scope

- 定义**稳定记录结构**（字段名与取值闭集），至少包含：`schema_version`、`run_id`、`started_at`、
  `finished_at`、`cases[]{ name, status: "pass"|"skip"|"fail", reason, measurements{} }`、
  `host{ os_build, scale_factor, display_count }`。
- 让 `crates/platform/windows/src/input/acceptance.rs` 的每个真机用例在结束前写入自己的状态；
  **skip 必须带非空 reason**，fail 必须带可定位的原因。
- 提供读取/汇总入口（xtask 子命令或独立工具，由 Implementer 择优并写清理由）：打印每个用例的
  状态与测量值，**unknown/缺失的记录必须显式失败**，不得静默当作 pass。
- 记录**写入位置可控**（例如环境变量指定路径，或固定写到 `target/acceptance/`）；不写进版本库。
- 负向用例：构造一份「只有 skip 没有 pass」的记录，汇总入口必须报告为**未通过**；
  构造一份字段缺失/状态非闭集的记录，必须**显式拒绝**。

## Out of scope（做了算漂移）

- 改 CI 工作流或把真机用例放进 CI（另卡；真机需要交互式桌面）。
- 改测试框架本身、改 `crates/platform/api/**` 公共 trait、改任何 schema。
- 把记录写进版本库或上传到网络。
- 新增第三方依赖（需要先停下并升级）。

## 必须遵守

- **铁律 1**：skip / fail / unknown 都必须显式可见；禁止把「没跑」表达成「通过」。
- **铁律 9**：只改 write scope 内文件；不动其它 crate 的公共形状。
- ADR-0063：记录是**有界**的（每次运行一份，覆盖式，不追加成无界日志）。
- ADR-0028：写热点文件前 `guard acquire`，写完立刻 `guard release`。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-platform-windows --lib -- --ignored --nocapture --test-threads=1   # 真机，需交互式桌面
# 汇总入口（示例；最终命令以本卡记录区为准）
cargo run -p xtask -- acceptance-report <记录路径>
```

并附：真机一轮记录原文（可脱敏）+ 汇总输出；「只有 skip」与「字段缺失」两份负向记录的拒绝证据。

## 完成定义（DoD）

- [ ] 真机一轮运行产出结构化记录，每个用例带 `pass`/`skip`/`fail` 与原因；skip 不是 pass。
- [ ] 汇总入口能按用例打印状态与测量值；缺失/未知状态显式失败。
- [ ] 两份负向记录（全 skip、字段/取值非法）被显式拒绝，有原始输出。
- [ ] 记录写入有界（每次运行覆盖式），未进版本库、未上网。
- [ ] 全套门禁绿；未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写（Ready 状态暂时豁免 9 节骨架，开工前由 Orchestrator 展开）。 -->

## 执行记录（Implementer 填写）

（待领卡填写。）

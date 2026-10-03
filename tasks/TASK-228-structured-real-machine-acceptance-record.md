# TASK-228　真机验收结果结构化输出（PASS / SKIP / FAIL 可机器区分）

- 状态：**Done（2026-10-03；PR 待创建，merge hash 后补）**
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

### 1. 约束回执与实现

- 新增 `crates/platform/windows/src/input/acceptance_record.rs`：固定四个真机用例，一次进程运行维护一份记录；
  用例开始时先写 `fail/started but did not finish`，结束后改写真结果，异常路径不会留下假 pass。
- `crates/platform/windows/src/input/acceptance/cases.rs`：四个 `#[ignore]` 用例统一经 `run_case` 写入
  `pass|skip|fail`、非空原因和测量值；设置失败、前置缺失、assertion panic 都能被机器区分。
- 新增独立汇总工具 `tools/acceptance-report`（选择理由：本轮禁止改 `xtask/src/**`，独立工具可复用 protocol 已登记的
  `serde_json` re-export，不新增第三方依赖）。工具严格校验 schema、固定用例集合、字段闭集与状态闭集；
  只有四个用例全 `pass` 才退 0，合法但含 `skip/fail` 退 1，字段缺失 / 状态非法 / JSON 非法退 2。
- 记录默认写到 `target/acceptance/windows-input-acceptance.json`，可用 `AIA_ACCEPTANCE_RECORD_PATH` 覆盖；
  每次覆盖写同一路径，不追加历史，不联网，不进版本库。

### 2. 实际改动文件与探针

- `cargo test -p assistant-platform-windows --lib`：通过（90 passed / 0 failed / 4 ignored）。
- `cargo test --manifest-path tools/acceptance-report/Cargo.toml`：通过（5 passed / 0 failed，含全 pass、
  全 skip、未知状态、缺字段、未知字段五类解析用例）。
- `cargo clippy -p assistant-platform-windows --all-targets -- -D warnings`：通过。
- `cargo clippy --manifest-path tools/acceptance-report/Cargo.toml --all-targets -- -D warnings`：通过。
- `cargo run -p xtask -- hygiene`：通过（0 errors；拆分后 `acceptance.rs` / `cases.rs` / `acceptance_record.rs`
  均 <600 行，未新增文件长度 warning）。

### 3. 验收输出与真机证据

真机命令（交互式桌面，退出码 0）：

```text
cargo test -p assistant-platform-windows --lib -- --ignored --nocapture --test-threads=1
```

原始记录（脱敏：本机仅保留 OS build / 显示器数量 / DPI，不含用户名与路径）：

```json
{
  "schema_version": "1.0",
  "run_id": "windows-input-27900-1791041741854",
  "started_at": 1791041741854,
  "finished_at": 1791041743267,
  "cases": [
    {
      "name": "pointer_move_lands_on_requested_physical_point",
      "status": "pass",
      "reason": "completed",
      "measurements": {"display": "\\\\.\\DISPLAY1", "scale_factor": 2, "target_x": 1066, "target_y": 666, "cursor_x": 1066, "cursor_y": 666, "error_x": 0, "error_y": 0}
    },
    {
      "name": "unicode_text_and_ctrl_s_round_trip_through_real_notepad",
      "status": "pass",
      "reason": "completed",
      "measurements": {"saved": true, "disk_content_contains_marker": true, "cleanup_ok": true, "launcher_reaped": true, "ime_is_applicable": true}
    },
    {
      "name": "pointer_calibration_covers_real_display_set",
      "status": "pass",
      "reason": "completed",
      "measurements": {"sample_count": 3, "display_count": 1, "maximum_error_pixels": 0, "total_error_pixels": 0, "tolerance_pixels": 2}
    },
    {
      "name": "pointer_click_focuses_known_notepad_element",
      "status": "pass",
      "reason": "completed",
      "measurements": {"center_x": 1642, "center_y": 759, "cursor_x": 1642, "cursor_y": 759, "focused": true, "cleanup_ok": true}
    }
  ],
  "host": {
    "os_build": "10.0.26300.9457",
    "scale_factor": 2,
    "display_count": 1
  }
}
```

汇总输出：

```text
acceptance: PASS
schema_version=1.0 run_id=windows-input-27900-1791041741854 started_at=1791041741854 finished_at=1791041743267
- pointer_move_lands_on_requested_physical_point: pass reason=completed measurements={cursor_x=1066, cursor_y=666, display="\\\\.\\DISPLAY1", error_x=0, error_y=0, scale_factor=2, target_x=1066, target_y=666}
- unicode_text_and_ctrl_s_round_trip_through_real_notepad: pass reason=completed measurements={cleanup_ok=true, disk_content_contains_marker=true, ime_is_applicable=true, launcher_reaped=true, saved=true}
- pointer_calibration_covers_real_display_set: pass reason=completed measurements={display_count=1, maximum_error_pixels=0, sample_count=3, tolerance_pixels=2, total_error_pixels=0}
- pointer_click_focuses_known_notepad_element: pass reason=completed measurements={center_x=1642, center_y=759, cleanup_ok=true, cursor_x=1642, cursor_y=759, focused=true}
```

全套门禁（按退出码核）：

```text
cargo fmt --all --check -> EXIT 0
cargo clippy --all-targets -- -D warnings -> EXIT 0
cargo test --workspace -> EXIT 0
cargo test -p assistant-platform-windows --lib -- --ignored --nocapture --test-threads=1 -> EXIT 0 (4 passed / 0 failed)
cargo test -p assistant-core arch:: -> EXIT 0 (10 passed)
cargo run -p xtask -- hygiene -> EXIT 0 (0 errors)
cargo run -p xtask -- memory-counts -> EXIT 0 (0 errors / 0 warnings)
cargo run -p xtask -- adr-index -> EXIT 0 (0 errors)
cargo run -p xtask -- refscan -> EXIT 0 (0 errors / 0 warnings)
cargo run -p xtask -- docscan -> EXIT 0 (0 errors)
cargo run -p xtask -- card-check -> EXIT 0 (0 errors)
cargo run -p xtask -- check-ledger -> EXIT 0 (0 errors / 0 warnings)
cargo run -p xtask -- check-comments -> EXIT 0 (0 errors)
cargo run -p xtask -- verify-schemas -> EXIT 0 (0 errors)
cargo run -p xtask -- codegen --check -> EXIT 0 (0 drift / 0 errors)
cargo test --manifest-path tools/acceptance-report/Cargo.toml -> EXIT 0 (5 passed / 0 failed)
cargo clippy --manifest-path tools/acceptance-report/Cargo.toml --all-targets -- -D warnings -> EXIT 0
```

### 4. 负向证据

「只有 skip、没有 pass」记录：汇总入口退 **1**，打印 `acceptance: NOT PASSED (pass=0 skip=4 fail=0)`，
逐个用例打印 `skip` 而不是 pass。

```text
acceptance: NOT PASSED (pass=0 skip=4 fail=0)
schema_version=1.0 run_id=windows-input-27900-1791041741854 started_at=1791041741854 finished_at=1791041743267
- pointer_move_lands_on_requested_physical_point: skip reason=negative fixture: only skip measurements={cursor_x=1066, cursor_y=666, display="\\\\.\\DISPLAY1", error_x=0, error_y=0, scale_factor=2, target_x=1066, target_y=666}
- unicode_text_and_ctrl_s_round_trip_through_real_notepad: skip reason=negative fixture: only skip measurements={cleanup_ok=true, disk_content_contains_marker=true, ime_is_applicable=true, launcher_reaped=true, saved=true}
- pointer_calibration_covers_real_display_set: skip reason=negative fixture: only skip measurements={display_count=1, maximum_error_pixels=0, sample_count=3, tolerance_pixels=2, total_error_pixels=0}
- pointer_click_focuses_known_notepad_element: skip reason=negative fixture: only skip measurements={center_x=1642, center_y=759, cleanup_ok=true, cursor_x=1642, cursor_y=759, focused=true}
```

字段缺失（去掉 `status`）：退 **2**。

```text
INVALID RECORD: cases[0] is missing required field `status`
```

状态不在闭集（`status=unknown`）：退 **2**。

```text
INVALID RECORD: case `pointer_move_lands_on_requested_physical_point` has unknown status `unknown`; expected pass|skip|fail
```

### 5. 记录有界性

记录固定在单文件、单次覆盖写；用例集合固定为四个，不向磁盘追加历史。没有后台任务、网络、凭据或长期状态。

### 6. 状态行收口

- 本批涉及的卡片文件：仅 `tasks/TASK-228-structured-real-machine-acceptance-record.md`。
- 状态行与 `LEDGER.md` 最后一条 TASK-228 状态均为 **Done**（本批收口时核对；结果会贴进 PR 描述）。

### 7. 偏差

无。未修改 `xtask/src/**`、CI、公共 trait、schema、测试框架或任何第三方依赖；未放宽断言或 lint。

### 8. 新增长期记忆

- `docs/memory/facts.md` 追加 1 条：结构化记录路径、严格汇总入口的三类退出码、本轮 4 pass 事实。
- `docs/memory/pitfalls.md` 追加 1 条 `[supersedes:2026-10-03]`：关闭「SKIP 仍以 ok 结束」的残余风险，
  以后真机结论必须读结构化记录 + 汇总输出。
- `docs/PARKING_LOT.md` 追加 PL-106 已闭环行；`MEMORY.md` 规模表已同步并通过 `memory-counts`。

### 9. 给审阅者的关注点

1. 最高风险点是汇总入口的 fail-closed 判据：它拒绝缺字段、未知字段、未知状态、用例集合不完整，且合法含
   `skip/fail` 的记录也退 1，不会被读成 pass。
2. 第二个点是记录生命周期：单文件覆盖、固定四例、无网络、无历史追加；真机用例只写本地 `target/acceptance/`。
3. 第三个点是本轮没有触碰 `xtask/src/**` 或 CI；汇总工具为独立 `tools/**` 小 bin，理由已写在 §1。

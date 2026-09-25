# TASK-023　`verify`：后置断言引擎（11 种断言）+ 状态指纹 + 幂等判定 + `on_violation` 分派

- 状态：**Done**（2026-09-25）
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：011,016　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。
- 正文由 Implementer **代 Orchestrator 展开**（人类 2026-09-25 授权）；展开后正文冻结，记录区由 Implementer 填写。

---

- **依赖**：011（`crates/protocol` 的 `ErrorCode` 13 类与协议类型）、016（`crates/platform/api` 的 `Fingerprint` 形状与校验）
- **预估**：M　**难度**：M
- **write scope**：`crates/verify/**`（新 crate `assistant-verify`）、`docs/DEPENDENCIES.md`（**仅追加**本卡实际使用的依赖列）
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）、`docs/wbs-overview.md` §6；架构 v2 **§7.3 状态指纹 / §7.4 后置条件断言 / §8.5 崩溃恢复的幂等判定 / §9 撤销** / **附录 A `#/$defs/assertion`**；`docs/spec/tool-schema.md` §4 不变量 3；`docs/spec/error-codes.md`；`docs/spec/naming.md` §7；**铁律 1 / 2 / 4 / 9 / 10**

**目标**

新建 `crates/verify`，把架构 v2 §7.3 / §7.4 / §8.5 的「动作之后到底有没有生效」判定落成**纯函数**领域逻辑：

- **11 种后置断言**（§7.4 的 12 种减去 `visual_assert`）：`state_assert`、`text_contains`、`text_not_contains`、`state_changed`、`state_unchanged`、`element_exists`、`element_gone`、`value_equals`、`value_in_range`、`file_changed`、`app_reported`。
- **解析 fail-closed**：未知 `kind`、多余字段（多半是拼写错误）、缺字段、类型错误一律**解析期拒绝**，绝不静默跳过。
- **三值判定**：`Satisfied` / `Falsified` / `NotEvaluable`；只有全部 `Satisfied` 才算 `Verified`，任一 `Falsified` 即 `Violated`，无 `Falsified` 但有 `NotEvaluable` 即 `Inconclusive`。
- **状态指纹**（§7.3）：由调用方提供 `FingerprintSubject`，本 crate 做 canonical form + SHA-256；per-adapter 忽略字段显式传入，无全局默认。
- **幂等判定**（§7.3 用途 2 / §8.5）：从 before/after 指纹与应用自报标记判定 `Applied` / `NotApplied` / `Unknown`，证据不全一律 `Unknown`。
- **`on_violation` 分派**（§7.4）：`retry_once` 只对 transient 类失败重试，其余升级给用户。

**In scope（本卡交付物）**

| # | 交付物 | 依据 |
|---|---|---|
| 1 | `crates/verify/Cargo.toml`（`assistant-verify`）+ `src/lib.rs` + `README.md`（职责 / 边界 / 不变量 / 已知限制） | gov §5.4、阶段 1 DoD |
| 2 | `VerifyError` + `error_code()` 映射（畸形/不支持 → `ToolInvalidArgs`；坏指纹 → `VerifyFailed`） | 铁律 1、`docs/spec/error-codes.md` |
| 3 | `AssertValue` / `StateField` / `CompareOp` / `FileChangeKind` / `Postcondition`（11 变体）与 `parse_postconditions`（fail-closed） | 架构 v2 §7.4、附录 A `#/$defs/assertion` |
| 4 | `Observation` / `ObservedElement` / `FileSnapshot` / `FileTransition`：**未观测 ≠ 观测为不存在** | 架构 v2 §7.4、铁律 2 |
| 5 | `evaluate_postcondition` + `AssertionOutcome`（三值），覆盖 11 种 kind | 架构 v2 §7.4 |
| 6 | `verify_postconditions` + `VerifyOutcome`（`Verified` / `Violated` / `Inconclusive`）+ `error_code()` | 架构 v2 §7.4 红线 |
| 7 | `FingerprintSubject` / `FingerprintIgnore` / `canonical_form` / `state_fingerprint`（§7.3 公式，含文档摘要 / 滚动位置 / 模态存在性） | 架构 v2 §7.3 |
| 8 | `ApplicationEvidence` + `classify_application`（幂等判定，证据不全 → `Unknown`） | 架构 v2 §7.3 用途 2、§8.5 |
| 9 | `OnViolation` / `ViolationKind` / `ViolationAction` / `parse_on_violation` / `dispatch_on_violation` | 架构 v2 §7.4、附录 A `on_violation` enum |
| 10 | 正向与负向测试：11 种 kind、解析拒绝路径、未观测不判满足、类型不匹配不判 falsified、空后置条件列表不判成功、指纹忽略字段与分隔符注入、幂等 `Unknown`、`retry_once` 收窄 | ADR-0019 N1、gov §5.5 |
| 11 | `docs/DEPENDENCIES.md` 追加 `serde` / `sha2` / `thiserror` 的 `crates/verify` 使用方 | 登记规则 1 / 2 |

**Out of scope（做了算漂移）**

- `crates/verify/src/visual/**`（截图、pHash/dHash、容差、`confidence_min`、`visual_assert`）→ **TASK-042**
- **preconditions**：`target_resolvable` / `capability` 属 §5.3，不属本卡（解析期拒绝并说明归属）
- **`assert` 自由字符串**：需要表达式语言（lexer / parser / evaluator / scope）= 新抽象层，且模型无法枚举（§7.4 反对 `if`-`then`-`else` 的同一条理由）→ 只接受结构化 `field` + `op` + `value`
- `crates/lease/**` → TASK-025；`crates/undo/**` → TASK-024；`crates/hitl/**` → TASK-027；`crates/core/**` → TASK-028
- 任何平台调用（UIA / Win32 / AT-SPI / 文件系统 / 网络 / 时钟）：观测由调用方采集，本 crate 只判定
- 执行动作：不重试、不回滚、不弹窗、不写审计
- 修改 `protocol/**`、`crates/protocol/**`、`crates/platform/**` 的 schema 或公共接口
- 新增第三方 crate（本卡只新增**使用方**）、顶层目录、`unsafe`、`#[allow]` 放宽 lint

**必须遵守**

1. **验证失败绝不返回 `ok`**（§7.4 第一红线）：只有全部后置条件 `Satisfied` 才是 `Verified`；`Violated` 与 `Inconclusive` 都带 `ErrorCode::VerifyFailed`。
2. **未观测 ≠ 不存在**：`Observation` 的映射表里没有条目表示**没有探测**，一律 `NotEvaluable`，绝不当作「元素不存在 / 文件未变化 / 状态未变」。
3. **类型不匹配不判 falsified**：字符串断言对上数值观测是「问错了量」，返回 `NotEvaluable` 而不是 `Falsified`（避免误触发回滚）。
4. **解析 fail-closed**：多余字段（拼写错误）必须拒绝，不得忽略；不得用默认值冒充作者未写的策略。
5. **纯函数**：无 IO、无时钟、无随机、无全局可变状态；相同输入必得相同输出（指纹可跨进程复现）。
6. **指纹可配置忽略字段**（§7.3）：无全局默认忽略表；忽略字段必须由 per-adapter 显式给出，否则光标闪烁 / 时间戳会让每个动作都「有变化」。
7. **幂等判定不猜测**：缺 before 或 after 指纹 → `Unknown`；应用自报（L1 通道，最可信）优先于指纹差异。
8. **`retry_once` 只对 transient 类失败**（§7.4）：`Transient` / `TargetNotFound` 重试，其余升级给用户；`VerifyFailed` **不**在此重试（`ErrorCode::retryable()` 回答的是「任务能否重试」，不是「同一调用能否重发」）。
9. **规模与命名**：单文件 ≤ 400 行（软）/ 600（硬），函数 ≤ 80 行，参数 ≤ 6；使用 `Postcondition` / `Target` / `Step` / `Fingerprint` 等受控词（`docs/spec/naming.md` §7）。

**授权与偏差登记（本卡预先声明，执行记录 §5 详述）**

| 编号 | 触发 | 授权来源 / 处置 |
|---|---|---|
| DRIFT-023-1 | 漂移触发器 ②：新增 crate `crates/verify` | 授权来源 = 本卡 write scope 第 1 项（人类 2026-09-25 授权代 Orchestrator 展开正文）；不另立漂移 |
| DRIFT-023-2 | 漂移触发器 ②：crate 内新增私有模块 `json_field` | 同上；属 crate 内部结构，未新增顶层目录 |
| DRIFT-023-3 | 漂移触发器 ①：`serde` / `sha2` / `thiserror` 的**新使用方** | 三行均已 `Approved` 在案；按登记规则 2 同批更新 `docs/DEPENDENCIES.md`（沿用 TASK-016 `DRIFT-016-1` 先例） |
| DRIFT-023-4 | 漂移触发器 ④：附录 A 的 `assert` 自由字符串**不实现** | 需要新抽象层（表达式语言）且模型无法枚举；只接受结构化形式，解析期拒绝并给出替代写法 |

**DoD**

- [ ] §7.4 的 11 种断言（= 12 种减 `visual_assert`）全部可解析且有求值测试覆盖
- [ ] 解析 fail-closed：未知 `kind` / 多余字段 / 缺字段 / 类型错误 / 坏指纹 / 空 selector / `min > max` / `within_ms = 0` 均有负向测试
- [ ] 验证失败绝不返回 `ok`：`Violated` 与 `Inconclusive` 都映射到 `ErrorCode::VerifyFailed`，且有测试
- [ ] 未观测（未探测元素 / 未记录文件 / 缺 previous 指纹 / 缺 elapsed）一律 `NotEvaluable`，绝不判满足
- [ ] 类型不匹配（字符串断言 vs 数值观测）返回 `NotEvaluable` 而非 `Falsified`
- [ ] 指纹可配置忽略字段：忽略字段变化不改变摘要、非忽略字段变化改变摘要、不同忽略集摘要不同、分隔符注入不产生碰撞
- [ ] 幂等判定：缺指纹 → `Unknown`；指纹相同 → `NotApplied`；指纹不同 → `Applied`；应用自报优先
- [ ] `on_violation`：五个 schema 值可解析，未知值拒绝；`retry_once` 仅对 `Transient` / `TargetNotFound` 重试
- [ ] `crates/verify/README.md` 含职责 / 边界 / 不变量 / 已知限制
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `cargo test -p assistant-verify` 全绿
- [ ] `cargo test -p assistant-core arch::` 全绿
- [ ] `xtask verify-schemas / codegen --check / hygiene / memory-counts / adr-index / docscan / card-check / check-ledger / check-migrations` 全部 PASSED
- [ ] `refscan` 只允许既有 baseline（PL-058），不得新增；`hygiene` 不得新增 warning
- [ ] `cargo deny check` 四项全 ok
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`
- [ ] 无任何 Out of scope 文件被修改

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-verify
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-migrations
cargo deny check
```

**进度同步约束（ADR-0039）**

1. 分支 = `task/TASK-023-verify-assertion-engine`，从最新 `origin/main` 切出；PR base 必须为 `main`。
2. 卡 Done 时同步：`PLAN.md` **仅「当前状态」块 4 行**、`README.md` **仅三处**（状态行 / `## 当前阶段` / `## 最近进展`）、`LEDGER.md` 追加一行、`MEMORY.md` 仅规模表（若有变化）。
3. 合并需同时满足 CI 全绿、`mergeable_state == clean`、base = `main`。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-023 `verify`：后置断言引擎（11 种断言）+ 状态指纹 + 幂等判定 + `on_violation` 分派
【目标】新建 `crates/verify`，把架构 v2 §7.3 / §7.4 / §8.5 的「动作之后到底有没有生效」判定落成**纯函数**领域逻辑，且**验证失败绝不返回 ok**。
【write scope】仅：`crates/verify/**`（新 crate `assistant-verify`）、`docs/DEPENDENCIES.md`（仅追加本卡实际使用的依赖列）
【铁律】1 无静默失败（每个非成功结果都带 `ErrorCode` 与可读原因）/ 2 不可信输入先校验（工具作者写的 postcondition 先 fail-closed 解析）/
        4 每个写操作有 postcondition（本卡提供**判定**，不执行）/ 9 不静默扩大范围 / 10 契约先行（先对齐 §7.3 / §7.4 / 附录 A 再实现）
【禁止】`crates/verify/src/visual/**`（→ TASK-042）/ preconditions `target_resolvable`·`capability`（→ §5.3）/
        `assert` 自由字符串（→ 表达式语言，DRIFT-023-4）/ lease·undo·hitl·core / 任何平台调用与执行动作（重试·回滚·弹窗·审计）/
        改 `protocol`·`platform` 的 schema 或公共接口 / 新增第三方 crate·`unsafe`·`#[allow]`
【验收】`cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace` /
        `cargo test -p assistant-verify` / `cargo test -p assistant-core arch::` / `xtask` 9 条子命令 / `cargo deny check` → 全绿（详见 §3）
【依赖】011（`crates/protocol` 的 `ErrorCode` 13 类）、016（`crates/platform/api` 的 `Fingerprint` 形状与校验）—— 已核对 LEDGER：两张卡均 Done
【疑问】无。（附录 A 的 `assert` 自由字符串 + 3 个非后置 kind 的处置按卡面「授权与偏差登记」执行 → DRIFT-023-4 / PL-086。）
```

### 2. 实际改动文件

**write scope 内（全部；无 scope 外文件被改）**

| 文件 | 行数 | 内容 |
|---|---|---|
| `crates/verify/Cargo.toml` | 31 | `assistant-verify`；依赖 `serde`（derive）/ `sha2` 0.10 / `thiserror` 2 + workspace crate `assistant-protocol` / `assistant-platform-api` |
| `crates/verify/README.md` | 114 | 职责 / 边界 / 不变量 / 11 种 kind / 典型用法 / 已知限制 / 相关文档 |
| `crates/verify/src/lib.rs` | 107 | 模块文档（职责 / 边界 / 5 条不变量 / doctest）+ 模块与 re-export 清单 |
| `crates/verify/src/error.rs` | 111 | `VerifyError` 5 变体 + `error_code()` 映射 + `VerifyResult` + `malformed` / `unsupported` 构造器 |
| `crates/verify/src/json_field.rs` | 137 | 通用 fail-closed 字段提取（`only_keys` / `required_text` / `required_u64` / `required_f64` / …） |
| `crates/verify/src/postcondition.rs` | 489 | `AssertValue` / `StateField` / `CompareOp` / `FileChangeKind` / `Postcondition`（11 变体）+ `parse_postconditions` + 逐 kind 解析 |
| `crates/verify/src/observation.rs` | 170 | `ObservedElement` / `FileSnapshot` / `FileTransition` / `Observation`（含 `is_scope`） |
| `crates/verify/src/assertion.rs` | 490 | `AssertionOutcome`（三值）+ `evaluate_postcondition` + 11 条求值路径 + `render_text` |
| `crates/verify/src/fingerprint.rs` | 393 | `FingerprintField` / `FingerprintIgnore` / `FingerprintSubject` / `DocumentDigest` / `ScrollPosition` / `ControlState` + `canonical_form` / `state_fingerprint` |
| `crates/verify/src/idempotency.rs` | 101 | `ApplicationEvidence` + `ApplicationVerdict` + `classify_application` |
| `crates/verify/src/on_violation.rs` | 173 | `OnViolation` / `ViolationKind` / `ViolationAction` + `parse_on_violation` / `dispatch_on_violation` |
| `crates/verify/src/verdict.rs` | 203 | `Verification` / `Violation` / `Unevaluable` / `VerifyOutcome` + `verify_postconditions` |
| `crates/verify/tests/postcondition_parsing.rs` | 226 | 解析契约测试（19 个） |
| `crates/verify/tests/postcondition_evaluation.rs` | 506 | 求值 + 裁决契约测试（27 个） |
| `crates/verify/tests/fingerprint_and_recovery.rs` | 294 | 指纹 + 幂等 + `on_violation` 契约测试（20 个） |
| `docs/DEPENDENCIES.md` | +3 处 | `serde` / `sha2` / `thiserror` 三行的**使用方**列追加 `crates/verify`（TASK-023）+ 一条「零新增第三方 crate」注记（登记规则 2） |

合计 **15 个文件 / 3545 行**；最大文件 = `tests/postcondition_evaluation.rs` **506 行**（< 600 建议线），`src/` 最大 = `assertion.rs` 490 行。

**write scope 外的公共热点文件（按 AGENTS.md §11.1 的义务同步，全部先取锁）**

| 文件 | 改动 |
|---|---|
| `tasks/TASK-023-*.md` | 本卡正文（人类授权代 Orchestrator 展开）+ 本记录区 |
| `PLAN.md` | 「当前状态」块 **4 行**：更新日期 / 当前任务卡（023 ✅ → 下一张 TASK-024）/ 阻塞项（+ PL-086）/ 下一步动作 |
| `README.md` | **三处**：状态行 + `## 当前阶段` 追加 TASK-023 段 + `## 最近进展` 新增本条 |
| `LEDGER.md` | 追加 1 行 |
| `MEMORY.md` | 仅规模表：`facts.md` 186→189 行 / 134→137 条目；`pitfalls.md` 229→231 行 / 119→121 条目 |
| `docs/memory/facts.md` | +3 条（11 种断言的推导、`retry_once` 判据、TASK-023 后基线） |
| `docs/memory/pitfalls.md` | +2 条（`src/` 内联测试模块同样吃 `clippy::panic` deny；四条重构形状 lint） |
| `docs/PARKING_LOT.md` | +PL-086 |

### 3. 验收输出摘要

```text
cargo fmt --all --check                                → 0 diff（exit 0）
cargo clippy --all-targets -- -D warnings              → exit 0
cargo test --workspace                                 → 全绿（新增 assistant-verify 66 测试 + 1 doctest）
cargo test -p assistant-verify                         → 66 passed / 0 failed（19 + 27 + 20）+ doctest 1 passed
cargo test -p assistant-core arch::                    → 5 passed / 0 failed
xtask verify-schemas                                   → 0 error / PASSED（5 份 schema）
xtask codegen --check                                  → 0 drift / 0 error / PASSED
xtask hygiene                                          → 0 error / 4 warning / PASSED（= 既有基线，未新增）
xtask memory-counts                                    → 0 error / 0 warning / PASSED
xtask adr-index                                        → 0 error / 0 warning / PASSED
xtask docscan                                          → 0 error / 506 warning / PASSED（较 515 基线**下降 9**：占位正文展开为完整卡面）
xtask card-check                                       → 0 error / 27 warning / PASSED（= 既有基线）
xtask check-ledger                                     → 0 error / 0 warning / PASSED
xtask check-migrations                                 → 0 error / 0 warning / PASSED
xtask refscan                                          → 151 error（= PL-058 既有 baseline，未新增）
cargo deny check                                       → advisories ok, bans ok, licenses ok, sources ok
```

- GitHub PR #37：**16/16 check-runs success**（pull_request + push 两组同 SHA，含三平台 `check`、doc consistency、`cargo deny` ×2、xtask deferred inventory、gate negative verification ×2）；合并前 base=`main`、`mergeable_state=clean`；merge commit `a8c6464`（卡内提交 `11ebd08`）。

### 4. DoD 逐条核对

| DoD 条目 | 结果 | 证据 |
|---|---|---|
| §7.4 的 11 种断言全部可解析且有求值测试覆盖 | ✅ | `test_parse_accepts_every_supported_kind`（12 条 JSON → 11 种 kind）+ 每种 kind 的求值测试 |
| 解析 fail-closed（未知 kind / 多余字段 / 缺字段 / 类型错 / 坏指纹 / 空 selector / `min>max` / `within_ms=0`） | ✅ | `postcondition_parsing.rs` 的 19 个测试 |
| 验证失败绝不返回 ok（`Violated` / `Inconclusive` → `VerifyFailed`） | ✅ | `test_one_falsified_postcondition_dominates` / `test_unevaluable_never_becomes_success` / `test_empty_postcondition_list_is_inconclusive` + `VerifyOutcome::error_code()` |
| 未观测一律 `NotEvaluable` | ✅ | `test_unprobed_element_is_not_evaluable_not_gone` / `test_state_changed_needs_previous_fingerprint` / `test_state_changed_needs_elapsed_time` / `test_app_reported_missing_key_is_not_evaluable` |
| 类型不匹配 → `NotEvaluable` 而非 `Falsified` | ✅ | `test_value_kind_mismatch_is_not_evaluable` |
| 指纹可配置忽略字段（忽略不改变 / 非忽略改变 / 不同忽略集不同 / 分隔符注入不碰撞） | ✅ | `test_ignored_field_does_not_change_the_digest` / `test_non_ignored_field_changes_the_digest` / `test_different_ignore_sets_produce_different_digests` / `test_separator_injection_cannot_forge_a_collision` |
| 幂等判定（缺 → `Unknown`；同 → `NotApplied`；异 → `Applied`；应用自报优先） | ✅ | 5 个 `idempotency` 测试 |
| `on_violation`（5 值可解析、未知拒绝、`retry_once` 收窄） | ✅ | `test_all_schema_values_parse` / `test_unknown_strategy_is_rejected` / `test_retry_once_only_for_transient_or_missing_target` |
| `crates/verify/README.md` 含职责 / 边界 / 不变量 / 已知限制 | ✅ | 该文件 6 节 |
| `cargo fmt --all --check` 0 diff | ✅ | 见 §3 |
| `cargo clippy --all-targets -- -D warnings` exit 0 | ✅ | 见 §3 |
| `cargo test --workspace` 全绿 | ✅ | 见 §3 |
| `cargo test -p assistant-verify` 全绿 | ✅ | 66 passed + 1 doctest |
| `cargo test -p assistant-core arch::` 全绿 | ✅ | 5 passed |
| `xtask` 9 条子命令全 PASSED | ✅ | 见 §3 |
| `refscan` 不新增 / `hygiene` 不新增 warning | ✅ | refscan 151 = baseline；hygiene 4 = baseline；docscan 515 → **506**（下降） |
| `cargo deny check` 四项全 ok | ✅ | 见 §3 |
| LEDGER 追加一行；新增事实/坑则追加 `docs/memory/*` | ✅ | LEDGER +1；facts +3；pitfalls +2 |
| 无任何 Out of scope 文件被修改 | ✅ | `git status` 只含 §2 两张表里的路径 |

### 5. 偏差

| 编号 | 现象 | 影响 | 建议 / 处置 | 已停工作 |
|---|---|---|---|---|
| DRIFT-023-1 | 新增 crate `crates/verify`（漂移触发器 ②） | 无 —— 卡面 write scope 第 1 项即为此 | 授权来源 = 人类 2026-09-25 授权代 Orchestrator 展开正文；不另立漂移 | 否 |
| DRIFT-023-2 | crate 内新增私有模块 `json_field`（漂移触发器 ②） | 无 —— 未新增顶层目录，只拆出通用 JSON 字段提取 | 同上；理由是 `postcondition.rs` 拆分后仍在 489 行，且通用 helper 与 schema 知识属两种职责 | 否 |
| DRIFT-023-3 | `serde` / `sha2` / `thiserror` 的**新使用方**（漂移触发器 ①） | 无 —— 三行均已 `Approved` 在案 | 按登记规则 2 同批更新 `docs/DEPENDENCIES.md` 使用方列 + 注记（沿用 TASK-016 `DRIFT-016-1` 先例） | 否 |
| DRIFT-023-4 | 附录 A 的 `assert` 自由字符串**不实现** | 附录 A 读起来像「`assert` 可用」；`verify` 会在解析期拒绝并指向结构化写法 | 已落 **PL-086** 待人类裁决（① 接受永久不实现并改附录 A / spec；或 ② 立卡补表达式语言）。**本卡不实现**：需要新抽象层且模型无法枚举 | 否 |

另：**正文由 Implementer 代 Orchestrator 展开**（人类 2026-09-25 授权）—— 原卡「步骤」段是占位符，展开后正文冻结。此项不记为漂移（有明确授权），在此留痕。

### 6. 更合理做法

1. **测试放 `tests/` 而不是 `src/` 内联模块**：AGENTS.md §5.3 的「`tests/` 内可 allow」只豁免 `tests/`，`src/*.rs` 的 `#[cfg(test)]` 同样吃 `clippy::panic` / `unwrap_used` / `expect_used` 的 `-D warnings`。首版把测试写在内联模块 → 29 个 `clippy::panic` 错误。改成 3 个集成测试文件后：零 `#[allow]`、源文件同时变短、且用的是公开 API（更接近真实调用方）。已记入 `pitfalls.md`。
2. **canonical form 用长度前缀而不是 JSON 序列化**：指纹材料的文本（文档标题、控件名）可能含任意字节。用 `serde_json::to_string` 虽然也能确定性输出，但「分隔符/转义」语义要依赖序列化库的行为；长度前缀 `name=<len>:<bytes>\n` 对**任意**字节内容都无歧义，且实现更短。已由 `test_separator_injection_cannot_forge_a_collision` 钉住。
3. **摘要用 nibble → hex 字节表，而不是逐字节 `format!("{b:02x}")`**：后者触发 `format_push_string`，且每字节分配一个 `String`。改成 `Vec<u8>` + `String::from_utf8` 后既过 lint 又快（`from_utf8` 的错误分支仍显式返回 `InvalidFingerprint`，不 unwrap）。
4. **`state_changed` 的 deadline 当作断言的一部分**：`elapsed > within_ms` 判 `Falsified` 而非 `NotEvaluable` —— 观测窗口已经超过断言给的期限，再报「无法判定」等于把一条有界断言降级成无界断言。语义已写进 `assertion.rs` 文档与 README「已知限制」。
5. **`Observation::fingerprint_scope` 用字符串而不是 `FingerprintScope`**：`assistant_platform_api::FingerprintScope` 只有 `WholeWindow` 与 `Element(ResolvedElement)`，而 `ResolvedElement` 是**不可序列化的句柄**（铁律 8）→ 绑定它会把句柄拖进本 crate。字符串 scope + `is_scope()` 既守住「跨 scope 不比较」，又不违反铁律 8。

### 7. 遗留问题

1. **PL-086**：附录 A 的 `assert` 自由字符串未实现，需人类裁决（接受永久不实现并改附录 A / spec，或立卡补表达式语言）。
2. `visual_assert` / 截图 / 感知哈希 / 容差 / `confidence_min` → **TASK-042**（`crates/verify/src/visual/**`，依赖 023+041）。本卡已把 `visual_assert` 的拒绝理由指向该卡。
3. preconditions（`target_resolvable` / `capability`，§5.3）尚无归属卡；本卡只在解析期拒绝并说明理由。若 TASK-028（`core`）需要 precondition 求值，需另立卡。
4. `classify_application` 的 `Applied` 是「效果存在」而非「本步造成」——因果性由 per-adapter 忽略字段与业务标记共同保证，已在 README「已知限制」写明。若将来发现误判，优先补 per-adapter 忽略表而不是改判据。
5. `state_fingerprint` 与 `crates/platform/windows/src/uia/tree.rs::fingerprint` 是**两条路径**（后者在平台层直接遍历 UIA 树）。本卡不重复实现遍历，但两者的 canonical form 目前**未共享代码**；若将来出现摘要不一致，需要在 `assistant-platform-api` 侧统一（属改公共接口 → 需 ADR）。

### 8. 新增长期记忆

- `docs/memory/facts.md` **+3**：
  1. 「11 种断言」是推导值（§7.4 的 12 种 − `visual_assert`），而附录 A 有 14 个 kind，多出的三个各有归属（TASK-042 / §5.3 preconditions）。
  2. `retry_once` 的判据与 `ErrorCode::retryable()` **不是同一条**（前者重发同一调用，后者重试任务）。
  3. TASK-023 之后的新基线（workspace 测试全绿 + 66 测试 + 1 doctest；四条既有 warning/error 基线未新增；零新增第三方 crate）。
- `docs/memory/pitfalls.md` **+2**：
  1. `src/` 里的 `#[cfg(test)] mod tests` 同样吃 `clippy::panic` / `unwrap_used` / `expect_used` 的 deny（AGENTS §5.3 只豁免 `tests/`）→ 优先把测试放 `tests/`。
  2. 本 workspace 的四条「重构形状」lint（`redundant_pub_crate` / `option_if_let_else` / `format_push_string` / `missing_const_for_fn`，且 `slice::get` 与 `const fn` 不可兼得）。
- `docs/PARKING_LOT.md` **+PL-086**（见 §7）。
- 无 REJECTED 新增（本卡的「不实现」决定已由 PL-086 承载，等人类裁决后再落 `rejected.md` 或 ADR）。

### 9. 给审阅者的关注点

1. **「验证失败绝不返回 ok」是否真的封死**（本卡第一红线）：三条路径 —— ① 任一 `Falsified` → `Violated`；② 无 `Falsified` 但有 `NotEvaluable` → `Inconclusive`；③ **空 postcondition 列表 → `Inconclusive`**（而不是「空集上全称命题为真」）。`Violated` 与 `Inconclusive` 都返回 `Some(ErrorCode::VerifyFailed)`，`Verified` 返回 `None`。请重点看第 ③ 条 —— 它是「凭 schema 要求 ≥1 条」之外的第二道防线，也是唯一一处**故意**让空输入失败的判据。
2. **`state_changed` 的 deadline 语义**：`elapsed_since_previous_ms > within_ms` 判 `Falsified`（而不是 `NotEvaluable`）。这是刻意的（期限是断言的一部分），但它是本卡里**最容易引起争议**的一条语义选择：如果审阅者认为「观测窗口偏长但状态确实变了」应当报无法判定，请裁决 —— 改这一条只需动 `evaluate_state_changed` 的 `decide(...)` 条件。
3. **`retry_once` 与 `ErrorCode::retryable()` 的不一致**：`VerifyFailed` 在协议里 `retryable() == true`，但 `dispatch_on_violation` **不**为它返回重试。请确认这个收窄是对的（本卡的理由：重发刚刚验证失败的同一调用是循环）。
4. **指纹 canonical form 的注入安全性**：`push_field` / `push_prefixed` 用**字节长度前缀**而不是转义。请确认「长度前缀 ⇒ 任意字节内容下编码单射」这条论证成立（测试 `test_separator_injection_cannot_forge_a_collision` 覆盖了 `|`、`=`、换行与「把分隔符塞进值」两种形状）。
5. **DRIFT-023-4 / PL-086**：附录 A 声明了 `assert` 自由字符串而本卡不实现。这是**读文档的人最容易踩到**的一处不一致 —— 请确认「解析期拒绝 + 指向结构化写法 + 落 PL 待裁决」这套处置足够，还是需要先改附录 A 再合并本卡。

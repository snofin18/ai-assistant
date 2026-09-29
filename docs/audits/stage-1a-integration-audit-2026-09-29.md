# Stage-1a Integration Audit — 2026-09-29

> 审计类型：阶段 1a 集成验收（gov §7.2）
> 审计范围：TASK-011 ~ TASK-039 已声明完成内容、gov §5.1、CI workflow、
> 阶段 1a DoD、范围/契约/分层/质量/ADR/文档一致性
> 审计人：Codex（TASK-039）　审批人：待人类
> 结论：**审计完成；阶段 1a 验收结论 = NO-GO（不可进入 1b）**

## 1. 摘要

阶段 1a 的基础设施、内核、UI 壳、靶机、回放、Notepad Adapter 和 T1.1~T1.3
声明式任务包已经进入 `main`，仓库级 Rust/xtask 门禁本次复跑通过，工作区行覆盖率为
**75.18%**（满足 75% 行覆盖门槛）。

但“阶段 1a 集成验收：CI 门禁全启用 + Notepad 3 个任务闭环”仍不能判定通过：

1. **没有真实任务执行器**。T1.1~T1.3 目前是声明式任务包与静态评测集，无法产生
   “连续 10 次运行成功率”“静默失败 = 0”“撤销成功率”等运行时证据。
2. **gov §5.1 仍不是全部真实启用**。UI Prettier / ESLint / Vitest、commitlint、
   `check-comments` 未实现；#9 覆盖率与 #11 文档仍是软门禁。
3. **硬门禁 #1 / #2 / #3 / #10 仍缺 ADR-0019 负向验证**（PL-018）。
4. Notepad T1.x 仍存在已登记的工具契约缺口：打开文件依赖
   `platform_open_file`，写入依赖 Host `set_editor_value`，`save_as` 不允许覆盖。

因此本审计 **不把阶段 1a 标记为完成**，也不允许以“工作流里有 step”替代实际门禁证据。

## 2. gov §5.1 门禁逐项矩阵

| # | 门禁 | 事实源 | 本次证据 | 状态 |
|---|---|---|---|---|
| 1 | Rust 格式 + UI Prettier | `cargo fmt --all --check`；`pnpm prettier --check .` | Rust `0 diff`；`apps/desktop-ui/package.json` 无 prettier script/dependency | **PARTIAL** |
| 2 | Rust Clippy + UI ESLint | `cargo clippy --all-targets -- -D warnings`；`pnpm eslint . --max-warnings=0` | Rust `exit 0`；UI `lint` 脚本实际只是 `tsc --noEmit`，无 ESLint | **PARTIAL** |
| 3 | 禁用项 lint | workspace `[lints]` + TS lint 规则 | Rust workspace deny 规则生效；TS 仅由 `tsconfig` strict 覆盖一部分 | **PARTIAL** |
| 4 | Rust 单元测试 + UI 测试 | `cargo test --workspace`；`pnpm test` | Rust 全绿；UI Node test **58/58**（不是 Vitest） | **PASS** |
| 5 | 架构依赖方向 | `cargo test -p assistant-core arch::` | `arch_layering` 5 + `arch_dependencies` 5 = **10/10 pass** | **PASS** |
| 6 | Schema 校验 | `xtask verify-schemas` | 5 schemas，0 error | **PASS** |
| 7 | 协议类型同步 | `xtask codegen --check` | 5 generated files，0 drift | **PASS** |
| 8 | 依赖治理 | `cargo deny check` | advisories / bans / licenses / sources 均 ok；有重复版本与未命中许可证白名单 warning | **PASS（有 warning）** |
| 8b | spike 依赖治理 | spike manifest 的 `cargo deny ... licenses sources` | `spikes/spike-a-notepad` licenses / sources ok | **PASS（本地手工）** |
| 9 | 覆盖率 | `cargo llvm-cov --workspace --fail-under-lines 75` | **75.18% lines**，命令 exit 0；CI step 仍为 `continue-on-error` | **PASS / SOFT** |
| 10 | Release 构建 | `cargo build --release` | 本地 exit 0；CI 三平台矩阵存在 | **PASS** |
| 11 | 文档完整性 | `cargo doc --no-deps --workspace` | exit 0；存在 rustdoc broken/private intra-doc-link warnings；CI step 仍为软门禁 | **PASS / SOFT（有 warning）** |
| 12 | 仓库卫生 | `xtask hygiene` | scanned=282，**0 error / 4 warning** | **PASS（有既有 warning）** |
| 12b | 文档一致性 | `memory-counts + adr-index + check-migrations + refscan + docscan + card-check` | 本次分项复跑全 PASS；`docscan` 0E/379W、`card-check` 0E/27W | **PASS（有 warning）** |
| 13 | 回放基准 | `xtask replay fixtures/recordings/core/notepad-like-basic.json` | v1 snapshot，4 nodes，4 stable AutomationIds，0 error | **PASS** |
| 14 | 提交规范 | commitlint / hook | 仓库无 commitlint 配置或 CI step | **MISSING** |
| 15 | 命名与注释规范 | `xtask check-comments` | 命令仍是显式 exit 3 的 deferred stub；CI 只验证“它确实以 exit 3 失败” | **MISSING** |
| 16 | 台账与记忆同步 | `xtask check-ledger` | plan_date=ledger_last_date=2026-09-29，0E/0W | **PASS** |

## 3. 硬门禁负向验证

ADR-0019 的要求是：硬门禁不仅要“这次绿”，还要证明“该红时会红”。

| 门禁 | 负向验证 | 状态 |
|---|---|---|
| #1 fmt | 注入格式错误 `.rs` → 断言失败 | **MISSING（PL-018）** |
| #2/#3 clippy + 禁用 lint | 注入 `unwrap()` / `dbg!` → 断言失败 | **MISSING（PL-018）** |
| #4 tests | 大量 N1 负向单测 | PASS |
| #5 arch | `arch_layering` 负向+N1 | PASS |
| #6 verify-schemas | N1 + `gate-negative` N2 | PASS |
| #7 codegen | N1 + `gate-negative` N2 | PASS |
| #8 deny | `gate-selftest` N3 | PASS |
| #8b spike-deny | `gate-selftest` N3 | PASS |
| #10 build | 注入编译错误 → 断言失败 | **MISSING（PL-018）** |
| #12 hygiene | N1 + deferred-inventory N2 | PASS |
| #12b doc consistency | N1（memory-counts / adr-index） | PASS |
| #13 replay | 回放解析与校验单测 | PASS（本次仅运行 fixture，未重跑 canary） |
| #16 check-ledger | N1 14 条单测 | PASS |

> 本审计没有在本卡中新增 #1/#2/#3/#10 canary：关闭 PL-018 需要同步修改
> `gate-selftest.yml`、ADR-0019 登记表并跑一次手工 canary；ADR-0019 属只读区，
> 当前 write scope 不能完整闭环，故保持 `MISSING`。

## 4. 阶段 1a DoD 矩阵

| DoD | 本次结论 | 证据 / 缺口 |
|---|---|---|
| Notepad 3 个任务连续 10 次成功率 ≥ 90% | **NOT_VERIFIED** | T1.1~T1.3 只有声明式任务包、cases、expected、静态 validator；没有真实执行器 |
| 静默失败 = 0 | **NOT_VERIFIED** | 没有真实执行链路可运行，不能从静态包推导 |
| 撤销成功率 ≥ 95%，冲突 100% 检测 | **PARTIAL** | undo/verify/hitl 单元测试通过；没有真实 Notepad 执行链路验证 |
| L3 不可逆动作 100% 人工确认 | **PARTIAL** | hitl/approval 与 T1.2/T1.3 声明通过；没有真实 UI→Core→Host 接线验证 |
| 四类异常可安全终止或恢复 | **PARTIAL** | 各内核有单元测试；目标应用退出、用户中途操作等真实链路未验收 |
| 所有写操作有 postcondition 且被验证 | **PARTIAL** | task/package 声明与 verify 内核存在；`task-engine` 尚未强制接收 `VerifyOutcome` |
| CI 门禁全绿且清单一致 | **NO-GO** | 见 §2：至少 #1/#2/#3/#4 UI 侧、#14、#15 未对齐 |
| 覆盖率 workspace ≥ 75%；core/policy/task-engine ≥ 85% | **WORKSPACE PASS / CRATE NOT PROVEN** | workspace lines 75.18%；没有按 crates 门槛给出同时的独立报告 |
| 每个 crate 有 README | **PASS** | `crates/*/README.md`、`apps/*/README.md`、`xtask/README.md` 均存在 |
| `MEMORY.md` 回填阶段 1 新增 FACT/PITFALL/REJECTED | **ONGOING** | TASK-037 / 038 已回填；后续 1b/1c 尚未发生 |
| 阶段末对齐审计完成且偏差裁决 | **THIS AUDIT / PENDING HUMAN** | 本报告完成；NO-GO 项尚待新增补救卡或人类裁决 |

## 5. 范围符合度

- 1a 的 A1~A5 主线均已有合并证据；TASK-039 是最后一张。
- 未发现 1b/1c 产品实现提前渗入 1a：
  - `crates/capture` 不存在；
  - `crates/dlp` 的 1c 实现不存在；
  - Edge/Chrome Adapter 不存在；
  - Paint Adapter 不存在。
- `automation-host` 仍是传输/心跳骨架，不是完整生产 Host；这不隐藏为 1a 完成。
- T1.1~T1.3 的声明式交付符合各自卡面，但不能替代阶段 1a 的运行闭环。

## 6. 契约一致性

| 事项 | 现状 | 严重度 | 处置 |
|---|---|---|---|
| `notepad.file.open` 未注册 | T1.1~T1.3 使用 `platform_open_file` 前置动作（DRIFT-036-1） | 中 | 保留 DRIFT；由后续 Adapter/tool 契约卡裁决 |
| `notepad.file.write_text` 未注册 | T1.3 使用 Host `set_editor_value`（DRIFT-038-1） | 中 | 需独立契约卡决定是否注册 |
| `save_as` 不允许覆盖 | 已存在目标 fail-closed；没有备份+确认覆盖（DRIFT-038-2） | 中 | 安全默认成立；产品覆盖能力另案 |
| `task-engine` 不强制 `VerifyOutcome` | 仍可从验证状态提交未验证步骤 | **高** | 需独立实现卡接线；1a 不能宣称“写操作全验证” |
| conversation/session 持久化缺失 | 仍是 PL-092 | 高 | 不属本卡；不得隐藏 |

## 7. 分层健康度

- `cargo test -p assistant-core arch::`：10/10 passed。
- ADR-0053 依赖白名单测试包含正向与负向样本，均通过。
- `core` 未引用 `platform/windows` 实现；`ipc` 分离测试通过。
- `tool-bus/src/schema.rs` 仍为 892 行，超过 hygiene 的 600 行 warning 线；
  TASK-205 已 Ready，未在 1a 强拆。

## 8. 质量指标

| 指标 | 本次值 |
|---|---|
| Rust 测试 | `cargo test --workspace` 全绿 |
| UI 测试 | 58/58 Node tests |
| workspace 行覆盖率 | 75.18% |
| fmt / clippy | PASS / PASS |
| cargo deny | PASS；有重复依赖与未命中许可证白名单 warning |
| rustdoc | PASS；有 broken/private intra-doc-link warnings |
| hygiene | 0 error / 4 warning |
| docscan | 0 error / 379 warning |
| card-check | 0 error / 27 warning |
| 生产代码裸 TODO/FIXME/HACK | 本次 grep 未发现新的无卡号违规；`check-comments` 未实现，不能机器证明 |
| lint 豁免 | 主要为 `tests/**` 的 `unwrap/expect/panic` 白名单；`xtask` 仍有若干 per-line allow，均由既有 ADR/卡记录约束 |

## 9. ADR 与文档完整性

- TASK-037 / 038 的 PR、CI、closeout 与 LEDGER 一致。
- `PLAN.md` 当前状态指向 TASK-039；`plans/stage-1-pilots.md` 的 1a 进度句与 Done 标记一致。
- `docs/adr/README.md` 与 ADR 文件、decisions 索引三方校验通过。
- `docs/spec/` 七份契约草案已有 TASK-200 修复基础；本审计未发现与 TASK-039 直接冲突的新契约变更。
- `docs/audits/` 在本卡前只有 stage-0 closeout；本报告是第一份 stage-1 子阶段审计。

## 10. 阻断项与建议

1. **Notepad 运行闭环未成立**：新增“任务执行器 + Host 分发 + 审批接线 + 真实靶机运行”卡，
   运行 T1.1~T1.3 各 10 次并生成成功率/静默失败/撤销证据，之后再重跑本审计。
2. **CI 门禁未完全落地**：补 UI Prettier/ESLint/Vitest 或正式裁决不用这些工具，
   并补齐 commitlint 与 `check-comments`。
3. **硬门禁负向验证**：完成 PL-018 的 fmt / clippy / build canary，
   同时更新 ADR-0019 登记表；完成后才能宣称 #1/#2/#3/#10 有效。
4. **软门禁转硬**：为 #9 coverage 和 #11 cargo doc 给出可重复的负向验证后，
   再决定是否删除 `continue-on-error`。
5. **`VerifyOutcome` 接线**：把 task-engine 的验证结果纳入状态迁移前置条件，
   否则运行时闭环即使跑通也不能满足阶段 DoD 的“所有写操作经验证”。
6. **PL-056 / PL-058 更新**：当前 `check-migrations` 与 `refscan` 已进入
   `doc-consistency` job，且本次 `refscan` 0E/0W；建议由后续治理提交关闭旧停车位条目。

## 11. 审计签字

- 审计结果：**NO-GO（1a 不能关闭，不能进入 1b）**
- 审计产物：本文件
- 待人类裁决：是否创建上述补救卡；是否接受 T1.x 继续停留在声明式阶段
- 下一次审计：补救完成后重跑 `stage-1a-integration-audit`

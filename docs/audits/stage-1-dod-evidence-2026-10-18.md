# 阶段 1 DoD 取证：per-crate 覆盖率 + crate README 四节完整性

状态：**Audit**（2026-10-18，TASK-315，无人值守轮次）　范围：
`plans/stage-1-pilots.md`「阶段 1 DoD」块的两条机器可验证条目。

> **本文件是时点快照，不承诺长期有效。** 覆盖率是随源码与测试变化的动态派生值
> （ADR-0072）；下面的百分比只对 `origin/main` 上 2026-10-18 的那次实跑成立，
> 任何后续卡改动源码后都应重跑本节命令，而不是引用这里的数字。

## 1. 结论

| DoD 条目 | 2026-10-18 结论 | 依据 |
|---|---|---|
| 覆盖率：workspace ≥ 75% | **成立** | 81.18% lines，`--fail-under-lines 75` **EXIT 0**（§2.1） |
| 覆盖率：`core` ≥ 85% | **成立** | `assistant-core` 92.09% lines（§2.2） |
| 覆盖率：`policy` ≥ 85% | **成立** | `assistant-policy` 95.03% lines（§2.3） |
| 覆盖率：`task-engine` ≥ 85% | **成立** | `assistant-task-engine` 91.58% lines（§2.4） |
| 每个 crate 有 README（职责 / 边界 / 不变量 / 已知限制） | **成立（本轮补齐 3 份）** | 23 份 README 四节核对见 §3；`capture` / `dlp` / `replay` 本轮补第四节 |

**这条结论修正了一个持续 19 天的记录缺口。**
`docs/audits/stage-1a-integration-audit-2026-09-29.md` §4 把该项判为
**WORKSPACE PASS / CRATE NOT PROVEN**（原话：「没有按 crates 门槛给出同时的独立报告」），
`docs/audits/stage-1a-reaudit-2026-10-02.md` §5 又记为「本轮未跑」。
本文件首次给出三个 crate 的**独立**报告与退出码。

## 2. 覆盖率原始输出

命令在本机以 `cargo 1.98.1` 实跑；每小节只保留**命令、退出码与 TOTAL 行**（逐文件明细表
数百行，且命令可随时复跑，故不全文粘贴）。TOTAL 行按 `cargo llvm-cov` 的表格列顺序：
`Regions / Missed Regions / Cover / Functions / Missed Functions / Executed /
Lines / Missed Lines / Cover / Branches / Missed Branches / Cover`。

### 2.1 workspace（DoD 原样命令）

```text
$ cargo llvm-cov --workspace --fail-under-lines 75 --summary-only
...
TOTAL   59773   12086   79.78%   5007   1150   77.03%   43138   8120   81.18%   0   0   -
EXIT=0
```

- 行覆盖 **81.18%**（43138 行中 8120 行未覆盖）≥ 75% ⇒ 门禁通过，退出码 0。
- 注意 `Regions` 列是 79.78%、`Functions` 列是 77.03% —— 该门禁的判据是**行覆盖**
  （`--fail-under-lines`），引用数字时不要拿 Regions 列冒充。

### 2.2 `assistant-core`（`crates/core`）

```text
$ cargo llvm-cov -p assistant-core --summary-only
...
TOTAL   2995   213   92.89%   302   32   89.40%   2479   196   92.09%   0   0   -
```

行覆盖 **92.09%** ≥ 85% ⇒ 成立。该 crate 的 `arch::` 依赖方向测试单独跑见
`docs/automations/2026-10-18-round-1.md` §5。

### 2.3 `assistant-policy`（`crates/policy`）

```text
$ cargo llvm-cov -p assistant-policy --summary-only
...
TOTAL   1628   83   94.90%   143   14   90.21%   1228   61   95.03%   0   0   -
```

行覆盖 **95.03%** ≥ 85% ⇒ 成立。

### 2.4 `assistant-task-engine`（`crates/task-engine`）

```text
$ cargo llvm-cov -p assistant-task-engine --summary-only
...
TOTAL   1656   189   88.59%   144   21   85.42%   1354   114   91.58%   0   0   -
```

行覆盖 **91.58%** ≥ 85% ⇒ 成立。

## 3. crate `README.md` 四节完整性核对

判据（gov §8.6）：每份 crate / app 的 `README.md` 必须含四节 —— 职责 / 边界 / **不变量** /
已知限制（标题可中可英）。核对命令按标题行匹配，接受
`Responsibilities` / `Boundaries` / `Invariants` / `Known Limitations` 与
`职责` / `边界` / `不变量` / `已知限制`：

```text
$ grep -c -E "^#+ *(Responsibilities|Boundaries|Invariants|Known [Ll]imitations|职责|边界|不变量|已知限制)" <23 份 README>
```

核对对象是仓库里全部 23 份 crate / app / xtask README（`crates/**` 含
`crates/platform/{api,windows}`、`apps/**`、`xtask/`）。

| README | 改动前节数 | 结论 |
|---|---|---|
| `crates/audit/README.md` | 4 | 齐全 |
| `crates/capture/README.md` | **3** | **缺「已知限制」→ 本轮补齐** |
| `crates/core/README.md` | 4 | 齐全（英文章节名） |
| `crates/dlp/README.md` | **3** | **缺「已知限制」→ 本轮补齐** |
| `crates/hitl/README.md` | 4 | 齐全 |
| `crates/ipc/README.md` | 4 | 齐全 |
| `crates/lease/README.md` | 4 | 齐全 |
| `crates/model-gateway/README.md` | 4 | 齐全 |
| `crates/policy/README.md` | 4 | 齐全 |
| `crates/protocol/README.md` | 4 | 齐全 |
| `crates/replay/README.md` | **3** | **缺「已知限制」→ 本轮补齐** |
| `crates/secrets/README.md` | 4 | 齐全 |
| `crates/storage/README.md` | 4 | 齐全 |
| `crates/task-engine/README.md` | 4 | 齐全 |
| `crates/tool-bus/README.md` | 4 | 齐全 |
| `crates/undo/README.md` | 4 | 齐全 |
| `crates/verify/README.md` | 4 | 齐全 |
| `crates/platform/api/README.md` | 4 | 齐全 |
| `crates/platform/windows/README.md` | 4 | 齐全 |
| `apps/agent-core/README.md` | 4 | 齐全 |
| `apps/automation-host/README.md` | 4 | 齐全 |
| `apps/desktop-ui/README.md` | 4 | 齐全 |
| `xtask/README.md` | 4 | 齐全 |

改动后复跑同一命令：**23 份全部返回 4**。

三份 README 补写时只落**已被代码或 ADR 证明的限制**，逐条对应如下：

- `crates/capture`：无图像编解码 / 感知哈希（`Cargo.toml` 零第三方依赖 + README 既有边界）、
  不做平台截图（唯一原语是注入的 `WindowProvider::capture`，Windows 实现不在本 crate）、
  只校验并应用**调用方已分类**的脱敏规则、`ScrollCleanupPlan` 只产出计划不做合并。
- `crates/dlp`：不做正则与文本扫描（`crates/dlp/src/redact.rs` 模块边界 + ADR-0073；
  正则路线已被 `docs/memory/rejected.md` 2026-09-25 否决）、不持久化策略、不选择 provider、
  不读像素（像素遮挡由平台层执行）。
- `crates/replay`：只回放读操作（写操作 `CapabilityMissing`）、provider API 只支持
  schema 版本 1 的单快照录制（v2 序列由 `xtask replay --suite core` 消费）、
  不做真实桌面采集 / 无录制器 GUI、不做树级 diff（diff 在 `xtask`）。

## 4. 残余风险与不做的事

1. **时点值**：§2 的四个百分比是 2026-10-18 的快照。任何改动 `crates/**/src/**` 的卡都应重跑，
   不要引用本文件的历史数字（ADR-0072）。
2. **没有机器判据**：§3 的四节核对是一次性 `grep` 取证，**不是门禁**。
   「骨架卡漏第四节」这类问题下次仍可能复现；把四节齐全做成 `xtask` 规则属收紧护栏，
   需 ADR + 人类批准（ADR-0019 / ADR-0025）→ 已登记 **PL-119**，本卡不自行实现。
3. **只覆盖了两条 DoD**：阶段 1 DoD 的其余条目（撤销成功率、注入靶页零执行、
   9 个任务连续 10 次运行成功率等）依赖真实 GUI / 靶机运行，本卡不碰、也不声称成立。
4. **`docs/spec/README.md` 仍缺位**：PL-040 自 2026-09-18 起未闭环，与本文件的
   README 主题相邻但不属同一条 DoD；一并登记进 PL-119 供裁决。

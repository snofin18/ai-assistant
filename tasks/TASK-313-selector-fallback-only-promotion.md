# TASK-313　`fallback_only` 升格为运行时字段（ADR-0095 落地）

- 状态：**Done**
- 阶段：1　子阶段：**1b**　批次：Paint 真机链路 / 治理　依赖：TASK-309、TASK-312
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **write scope**：
  - `docs/adr/0095-selector-fallback-only-runtime-semantics.md`
  - `docs/adr/README.md`（登记表追加一行）
  - `crates/platform/api/src/target.rs`
  - `crates/replay/src/provider.rs`（解析顺序）
  - `adapters/com.microsoft.paint/selectors/targets.json`
  - `adapters/com.microsoft.paint/tests/validate_adapter_pack.py`
  - `LEDGER.md`、`PLAN.md` 当前状态块、`plans/stage-1-pilots.md`、`README.md` 三处、`docs/PARKING_LOT.md`（PL-115 关联）
  - 本卡
- **关联**：ADR-0095（本卡即其落地）、ADR-0091、ADR-0094、PL-115、
  `docs/memory/pitfalls.md` 2026-10-17 的 `fallback_only` 条目

**目标**

把 `fallback_only` 从「只存在于适配包 JSON 与 Python 校验器的作者期标注」升格为
`SelectorCandidate` 的真实字段，并让解析器按 ADR-0095 的链内语义执行。

**现象**

`crates/platform/api/src/target.rs` 的 `SelectorCandidate` 只有
`id / kind / value / score / locale_dependent / ttl_ms` 六个字段，**没有 `fallback_only`**。
因此「此候选只作兜底」在运行时完全不生效；真降权只有 `locale_dependent`。
同时它挡住了「分层选择」的表达：同一个控件作为「打开容器的一步」应是主通道、
作为「最终目标」才是兜底，而现在无法区分。

**本卡修法**

按 ADR-0095 D1–D6 落地：加字段（缺省 false，旧 pack 兼容）→ 解析器按 primary/fallback
**两组顺序**尝试（不做分数混排）→ 只有 fallback 候选的链须显式 `allow_fallback_entry`，
否则既有 `TargetNotFound` → 组内歧义不降级 → 与 `locale_dependent` 正交 →
校验器与 paint pack 对齐。

**Out of scope（做了算漂移）**

- 新增 ErrorCode 或改 `envelope` / `tool-schema`。
- 重写 `SelectorKind` 词表（ADR-0091 / 0094 已定形状）。
- Paint handler 扩展、真机端到端（TASK-309 剩余项）。
- 采样器与 shell 规则的后续（已由 2026-10-18 的治理补强落地）。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-platform-api
cargo test -p assistant-replay
cargo test --workspace
python adapters/com.microsoft.paint/tests/validate_adapter_pack.py
cargo run -p xtask -- adr-index
```

**DoD**

- [ ] ADR-0095 落档 Accepted 并登记进 `docs/adr/README.md`
- [ ] `SelectorCandidate` 带 `fallback_only`，缺省 `false`，旧 pack 反序列化语义不变
- [ ] 解析器按 primary / fallback 两组顺序尝试，**不做分数混排**
- [ ] 只有 fallback 候选且未声明 `allow_fallback_entry` → `TargetNotFound`（不新增 ErrorCode）
- [ ] 组内歧义不降级（正反测试固定）
- [ ] `validate_adapter_pack.py` 与 paint pack 对齐且 PASSED
- [ ] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行
- [ ] 无任何 Out of scope 文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- 任务：TASK-313（fallback_only 升格为运行时字段）。
- 铁律：1（未命中与歧义分开处理，不猜）、9（不静默扩大范围）、10（契约先行，ADR-0095 已 Accepted）。
- 禁止：新增 ErrorCode；重写 SelectorKind；Paint handler 扩展；真机端到端。
- 疑问：D3（allow_fallback_entry）与 D6（校验器 / paint pack）本段未做。本段**不改变"只有 fallback 候选的链"的既有行为**，因此无回归。

### 2. 实际改动文件

- crates/platform/api/src/target.rs：SelectorCandidate 新增 fallback_only 字段（serde default）+ 构造参数 + is_fallback_only() 访问器。
- crates/platform/windows/src/selector.rs：rank_candidates 按 ADR-0095 D2 改为**两级顺序**（先分组、组内按有效分降序）+ 两条正反单测。
- 11 处调用点补参数（**编译期强制**，不用隐式默认掩盖遗漏）。
- LEDGER.md 追加一行；本卡记录区。

### 3. 验收输出摘要

- cargo check --workspace --all-targets → **errors=0**
- cargo test -p assistant-platform-windows --lib selector → **12 passed / 0 failed**（含新增 test_rank_candidates_puts_primary_group_before_fallback_group）
- cargo clippy --all-targets -- -D warnings → **exit 0**
- cargo test --workspace → **exit 0**（153 组 test result: ok）
- cargo fmt --all --check → 首跑 exit 1（新测试格式不符），cargo fmt --all 后复跑绿

### 4. DoD 逐条核对

- [x] ADR-0095 落档 Accepted 并登记进 `docs/adr/README.md`（PR #466）。
- [x] `SelectorCandidate` 带 `fallback_only`，缺省 `false`，旧 pack 反序列化语义不变（PR #468）。
- [x] 解析器按 primary / fallback 两组顺序尝试，**不做分数混排**（PR #468 / #469）。
- [x] 只有 fallback 候选且未声明 `allow_fallback_entry` → `TargetNotFound`，不新增 ErrorCode（PR #469）。
- [x] 组内歧义不降级（解析循环层正反测试，PR #469）。
- [x] `validate_adapter_pack.py` 与 paint pack 对齐且 PASSED（PR #469）。
- [x] 静态验收全绿；9 节记录已补全；`LEDGER.md` 已记录三段落地。
- [x] 已核对历史合并范围：无 Out of scope 公共形状或 Paint handler 扩展。

### 5. 偏差

- 历史编号异常：PR #453 的 `deny.toml` 限期豁免先使用了 TASK-313 号，
  后续 ADR-0095 的 selector 工作又复用了同一号。本卡正文与三段 PR 只覆盖后者；
  前者由 `PL-115`、PR #453 与 TASK-323 的历史对账共同留痕。该异常发生在本次记录补全之前，
  不通过建立第二个同号卡制造新的“一卡多文件”冲突。

### 6. 更合理做法

- 同一号一旦进入已合并分支，应停止复用；新的独立工作必须分配新号。对已发生的历史复用，
  正确做法是保留一个可解析的正式卡，由独立治理卡记录别名与另一段工作的真实 merge 证据。

### 7. 遗留问题

- `PL-115` 仍开放：`deny.toml` 对 RUSTSEC-2026-0370 / 0371 / 0372 的限期豁免继续有效，
  触发条件或 2026-12-31 到期时必须复审。
- `PL-118` 仍开放：300+ 历史号段归属仍需 ADR / Orchestrator 裁决。
- Stage 1c 的 Edge/Chrome 与真实 GUI / 浏览器 profile 不在本卡范围。

### 8. 新增长期记忆

- 无新增仓库长期记忆。历史同号复用的证据与处置已落在 `PL-117`、TASK-323 与
  `docs/automations/2026-10-20-round-3.md`，避免把一次性事件写成通用规则。

### 9. 给审阅者的关注点

1. 本卡的正式语义仍仅覆盖 ADR-0095 的 `fallback_only`；不要把 PR #453 误读成该功能的前置实现。
2. DoD 勾选来自三段已合并 PR、LEDGER 与 CI 历史证据，不是本轮重跑产品测试的新结论。
3. TASK-323 只做历史对账，`PL-115` 的风险接受状态没有被削弱或延长。

### 附：TASK-313 同号历史核对（TASK-323，2026-10-20）

- 第一段工作流是 PR #453：head `task/TASK-313-deny-advisory-exemption`，
  base `main`，merge `7b81bfb0e9842ec4c661913c50c3eeb77f8592b4`；
  改动 `MEMORY.md` / `deny.toml` / `docs/PARKING_LOT.md` /
  `docs/automations/codex-automations-operations.md` / `docs/memory/pitfalls.md`
  （5 files，+41 / -2），CI 22 checks = 21 success + 1 skipped。
- 第二段工作流是本卡与 ADR-0095：PR #466 merge `176b08b`、
  PR #468 merge `22b1c28`、PR #469 merge `a04aa85`；
  三段均为 base `main`、CI 22 checks = 21 success + 1 skipped。
- 处置：本文件继续作为 TASK-313 的唯一正式卡，并在记录区保留同号事实；
  TASK-323 承接对账证据并关闭 `PL-117` 的“缺少可追溯落点”缺口。
  不创建第二个同号文件，避免与 ADR-0031 的一卡一文件约束冲突。

### 附：2026-10-18 贯通段（PR 2/3）

- 已加 TargetDescriptor.allow_fallback_entry + SelectorChain.allow_fallback_entry（builder / 访问器）+ 装载器贯通 + paint pack 两处显式声明。
- **闸门未开**：rank_candidates 行为与上一段完全一致，本段是纯贯通，零行为变更。
- 下一段（PR 3/3）才开 D3 闸门 + 补 D4 显式测试 + validate_adapter_pack.py 规则。

### 附：2026-10-18 收口段（PR 3/3）

- D3 闸门已启用：无 primary 且未声明 → 判空 → 既有 TargetNotFound；两条正反单测固定。
- D4 在解析循环层加显式测试（不是纯函数层），确认歧义不降级。
- validate_adapter_pack.py 新增机器门禁：全 fallback 的 target 必须显式声明。
- ADR-0095 D1–D6 至此全部落地。

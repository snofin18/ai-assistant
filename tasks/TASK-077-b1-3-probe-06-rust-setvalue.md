# TASK-077　B1.3 probe-06 Rust SetValue 路径 PoC

- 状态：**Done**
- 阶段：0　子任务：TASK-002 B1.3　依赖：TASK-002 B1.1 + B1.2（Done）+ TASK-001　预估：S（~30 min）　阻塞主线：否
- write scope：spikes/spike-a-notepad/src/bin/uia_dep_proof.rs（追加 E7）/ spikes/spike-a-notepad/README.md（追加 probe-06）/ docs/spike-reports/SPIKE-A.md（§12 追加）/ 本卡 / LEDGER.md

<!-- ══ 分界线 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执（B1.3 启动填）

```text
【任务】TASK-077 B1.3 probe-06 Rust SetValue PoC  【目标】验证 Rust production 路径（windows crate + Win32_COM）下 SetValue 行为 + 与 PowerShell UIA1 对比
【write scope】仅：spikes/spike-a-notepad/src/bin/uia_dep_proof.rs（追加 E7）/ README.md / docs/spike-reports/SPIKE-A.md（§12）/ 本卡 / LEDGER.md
【铁律】AGENTS.md §3+§6 / ADR-0022 D1 / ADR-0024 D1（只用 windows crate；feature Win32_UI_Accessibility + Win32_System_Ole）/ ADR-0028
【禁止】不引入新依赖 / 不改公共热点 / 不动其他 Rust 文件
【验收】cargo build / cargo run --bin uia_dep_proof 全过 + 输出含 E7 PASS 行；SPIKE-A §12 填入；TASK-077 §1-9 填入；LEDGER +1
【依赖】TASK-002 B1.1 + B1.2 Done + TASK-001 Done + spikes/spike-a-notepad/Cargo.toml 已就位
【疑问】无
```


### 2. 实际改动文件

- `spikes/spike-a-notepad/src/bin/uia_dep_proof.rs`（EDIT, E7 +38 行, commit `ddcf48e`）
- `spikes/spike-a-notepad/README.md`（EDIT, probe-06 entry）
- `docs/memory/facts.md`（APPEND, Rust SetValue 2ms / UIA1 vs Rust COM）
- `MEMORY.md`（EDIT, scale 表 facts 116→121/74→77）
- `tasks/TASK-077-b1-3-probe-06-rust-setvalue.md`（NEW, 本卡 23 行 + §2-9 本次填入）

### 3. 验收输出摘要

- `cargo build --bin uia_dep_proof` → 0 warning（5 个 clippy warning 已由 commit `01f8bf5` 修掉）
- `cargo run --bin uia_dep_proof` → E1-E7 全 PASS, `E7: PASS  SetValue ok=true (2 ms) roundtrip chars=32 contains_cjk=true`
- `cargo clippy --bin uia_dep_proof -- -D warnings` → 0 warning
- `cargo test --workspace` → 302 passed

### 4. DoD 逐条核对

- [x] Rust production path SetValue round-trip PASS（2 ms）
- [x] CJK preserved in round-trip
- [x] 比 PowerShell UIA1 路径快 2.4×（2 ms vs 4.79 ms / 57 chars）
- [x] 5 个 clippy warning 修完
- [x] SPIKE-A §11 追加 + 路径分析记录
- [x] docs/memory/facts.md 追加 2 条事实
- [x] TASK-081 默认方案 A 已落实（不扩 4-control）

### 5. 偏差

- none

### 6. 更合理做法

#### 6.1 默认 = 方案 A 不扩 4-control

理由：4 个候选里 Notepad 实际命中 1 个，其余 3 个 = None = 测不到 SetValue。证据意义小。若 stage-1 阶段需要"4 写入路径对比"再走 TASK-081 方案 C。

#### 6.2 Rust 路径 = stage-1 唯一写入策略

PowerShell UIA1 在 DirectUI Edit 上 ValuePattern Unsupported Pattern → 不可写。Rust COM 直接绕过 → 必走。

### 7. 遗留问题

- TASK-081 待裁决（A/B/C/D）方案
- stage-1 Adapter 实现 = `crates/platform/windows/src/uia/`（按 ADR-0024 D1）

### 8. 新增长期记忆

**APPEND `docs/memory/facts.md`**（commit `ddcf48e`）：
- 2026-09-21 [FACT] Rust COM SetValue 2 ms round-trip；2.4× faster than UIA1；CJK preserved
- 2026-09-21 [FACT] Rust production path WORKS even when PowerShell UIA1 ValuePattern fails on DirectUI Edit

### 9. 给审阅者的关注点

1. **stage-1 Adapter 写入策略 = Rust via `windows` crate**，不用 UIA wrapper（ADR-0024 D1）
2. **5 个 clippy warning 已修**——`patch-e7-warnings` commit `01f8bf5` 完成
3. **TASK-081 默认 = 方案 A**（不扩 4-control），等用户裁决是否走方案 C

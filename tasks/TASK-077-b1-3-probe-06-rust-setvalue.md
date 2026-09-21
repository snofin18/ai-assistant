# TASK-077　B1.3 probe-06 Rust SetValue 路径 PoC

- 状态：**InProgress**
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

### 2-9 在 E7 实现后填入

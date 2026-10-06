# 顶层目录 ADR 白名单

状态：**Accepted**（ADR-0069 附表，2026-10-04）　维护者：Orchestrator

本表是 `xtask hygiene` 检查「新增顶层目录」的唯一事实源。新增目录前必须先有 Accepted ADR 或 ADR-0069
的正式修订，再把目录登记到下表；构建产物与版本库元数据目录不参与检查。

## 允许的顶层目录

| 目录 | 用途 | 授权 ADR |
|---|---|---|
| `.github/` | GitHub Actions、模板与仓库自动化配置 | ADR-0019 |
| `adapters/` | 声明式应用适配包（selectors、tools、tasks、rollback） | 架构 v2 §3 |
| `apps/` | 可执行应用进程与 Tauri UI | ADR-0053 |
| `crates/` | Rust workspace 库 crate | ADR-0053 |
| `docs/` | 治理、ADR、spec、审计与长期记忆 | ADR-0021 |
| `eval/` | 任务评测集与离线评测输入 | TASK-036 |
| `fixtures/` | 靶机应用与测试夹具 | TASK-033 |
| `plans/` | 阶段索引、批次表与排期 | ADR-0041 |
| `protocol/` | Tool / Adapter / 事件 schema 单一事实源 | TASK-011 |
| `spikes/` | 阶段 0 一次性技术验证 | ADR-0024 |
| `tasks/` | 一卡一文件的任务卡与执行记录 | ADR-0031 |
| `tools/` | 独立开发辅助工具，不进入产品运行路径 | TASK-228 |
| `xtask/` | 仓库护栏与代码生成工具 | ADR-0017 |

## 变更规则

1. 新目录必须由 Accepted ADR 解释职责、边界和与既有目录的关系。
2. 同批在本表新增一行，并在「授权 ADR」列引用该 ADR。
3. `scripts/` 经 **ADR-0078** 裁定**不设立**：脚本类内容一律归已获授权的 `tools/`（TASK-228）；PL-023 已作废。将来若确需该目录，另开新 ADR 说明它与 `tools/` / `xtask/` 的边界后，再按本规则第 1/2 条登记。
4. 删除目录时由 Orchestrator 清理本表；未清理的陈列表项由 `hygiene/stale-top-level-directory` 以 Warning 报出。

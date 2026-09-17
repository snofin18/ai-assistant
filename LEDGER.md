# LEDGER.md — 事件台账（只追加，不改写）

> **用途**：回答"哪张卡、在哪个 commit、验收结果如何、有没有偏差"。
> **与 `MEMORY.md` 的分工**：这里记**事件**（发生过什么），`MEMORY.md` 记**认知**（因此我们知道了什么）。
> **写法**：只追加。一行一事件。已写入的行**不得修改或删除**；写错了就追加一行更正并在备注里指向被更正的行。
> 模板见 `docs/governance-ai-agent-execution.md` §9.2。

## 状态取值

| 状态 | 含义 |
|---|---|
| `Done` | 卡内 DoD 全部满足，验收命令全绿，已提交（等待或已完成人类合并） |
| `Review` | 已提交，等待独立 review agent / 人类审阅 |
| `Blocked` | 被前置条件或 DRIFT 卡住，未提交 |
| `Abandoned` | 决定不做（必须在备注写明理由与批准人） |

## 台账

| 日期 | 卡号 | 状态 | commit | 验收命令结果 | 偏差 | 备注 |
|---|---|---|---|---|---|---|
| 2026-09-16 | TASK-001 | Review | 58fed3d | 5/6 通过（fmt / clippy -D warnings / test 99 passed / hygiene PASSED / git status 干净；`cargo deny check` **未执行**：cargo-deny 未安装，见 PL-006） | DRIFT-001-1、DRIFT-001-2、DRIFT-001-3 | 仓库骨架 + xtask 护栏 v0 + CI（6 硬 / 9 软门禁）；commit 哈希于首次提交后回填；详见 `tasks/TASK-001-repo-skeleton.md` |
| 2026-09-17 | —（人类会话，非任务卡） | Done | pending | `xtask hygiene` PASSED（scanned=7 errors=0 warnings=0）；本次仅改 md、无 Rust 变更，故未跑 fmt/clippy/test | none | 诊断并处置夜间自动化故障：① 确认根因是 Codex automation 投递机制产生畸形 `function_call_output`（`name=automation_update`、无 `call_id`），**不是**上下文压缩；② 探针实测**否决** cron 替代方案（与 heartbeat 共用投递机制，首轮生成非法 `at_<uuid>` id 且不落盘、无法补丁，两次 2.6s 零产出失败）；③ 实测**验证** `codex exec` CLI 可行（走标准 user message、`function_call_output` 计数 0）；④ 删除 heartbeat `ai-assistant` 与 cron `ai-assistant-cron` 两个 automation；⑤ 等长修补 4 条 rollout 毒项（老会话 2 + 探针会话 2），全库扫描恢复 0 处缺 `call_id`，并校验 `next_rollout_byte_offset == 文件大小`；⑥ 追加 PL-012~PL-015 与 MEMORY §2×4 / §4×1 / §5×6。**遗留：夜间自动化当前停摆，待人类裁决是否按方案 D（任务计划程序 + `codex exec`）重建；章程 §11 全章需重写** |
| 2026-09-17 | —（人类会话，非任务卡） | Done | pending | **TASK-001 此前缺的 2 条验收命令现已全绿**：`cargo deny check` → `advisories ok, bans ok, licenses ok, sources ok`（退出码 0）；`cargo llvm-cov --workspace --fail-under-lines 75` → 99 passed、行覆盖 **96.31%**（阈值 75）、函数 94.26%、区域 95.18%（退出码 0）。另 `xtask hygiene` PASSED；改动文件 CRLF=0、无 BOM | none | 环境补齐与配置修复：① 装入 `cargo-deny 0.20.2` + `cargo-llvm-cov 0.9.1`（GitHub 官方 release 预编译包，经 `ghproxy.net` 镜像下载、用直连 `api.github.com` 的 `digest` 逐个 sha256 校验通过）；② **修 `deny.toml` 缺陷**：`db-path` 由数组改字符串（原写法在 0.20.2 下解析失败，deny 门禁形同虚设，CI 因 action 未钉版本而未暴露）；③ **定位并修复上下文窗口缩水 72%**：config.toml 在 10:48 被应用重写时丢了 `model_catalog_json`，导致 `qwen3.8-max` 落入 fallback 元数据（272000×95%=258400，而 catalog 值 983616×95%=934435），已加回并备份 `config.toml.pre-c2-fix.bak`，**需重启 Codex 生效**；④ 官方文档确认 `qwen3.8-max` 上下文 = 1M；⑤ 追加 PL-016/PL-017 与 MEMORY §2×6 / §5×3。**遗留：ADR-0018 仍为 Proposed，待人类批准后才能改章程 §11** |

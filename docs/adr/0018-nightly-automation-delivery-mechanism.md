# ADR-0018　夜间无人值守的投递机制：外部调度 + `codex exec`

状态：Proposed　日期：2026-09-17　Supersedes：MEMORY.md §3 `[ADR:待建 0018]`「夜间自动化用 heartbeat」　Superseded by：—
关联：`docs/overnight-automation-charter.md` §11、MEMORY.md §2/§4/§5 的 2026-09-17 条目、`docs/PARKING_LOT.md` PL-012~PL-015

## 背景（为什么现在要决定）

项目计划用夜间无人值守推进低风险工作（补测试、修告警、写文档草稿），以节约白天的人类审阅时间。原决策（2026-09-16）采用 Codex 桌面应用的 **heartbeat automation**，挂在长期工作 thread 上，每日 23:30 / 02:30 唤醒。

实测结果：**连续两晚零产出**，且把所挂 thread 打成永久不可用。

根因（2026-09-17 确诊）：Codex 的 automation 机制**不把 prompt 当 user message 投递**，而是包装成一条合成的工具执行结果：

```text
FunctionCallOutput { id: None, call_id: None,
                     name: "automation_update", namespace: "codex_app" }
```

本机使用第三方 Responses 兼容端点（阿里云百炼 compatible-mode，`model = qwen3.8-max`），它对请求体做严格校验，直接 400 拒绝。又因 `disable_response_storage = true`，每轮都重放全量历史，毒项每次都被重发 → **该 thread 之后每次提问都失败**（新开 thread 正常）。

需要决定：夜间无人值守用什么投递机制。

## 决策（一句话）

**放弃 Codex 内置 automation（heartbeat 与 cron 均不可用），改为「Windows 任务计划程序 + 包装脚本调 `codex exec` CLI」**；每次运行都是全新会话，prompt 走标准 user message 路径。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | **heartbeat automation**（原方案） | ❌ 否决 | 注入无 `call_id` 的合成项 → 端点 400；毒项沉入长驻会话 + 全量重放 → **会话永久损坏**，连白天交互一并受牵连。四个选项里唯一会**污染主线工作**的 |
| 2 | **cron automation** | ❌ 否决（本次实测） | 能创建也能触发（旧记载"本机创建失败"有误，真因是缺 `model`/`projectId`/`executionEnvironment` 三个必填项，而报错只回一句 `Failed to create automation.` 不指明字段）。但它与 heartbeat **共用同一投递机制**，且首轮投递时 Codex 会临时生成非法 id `at_<uuid>`（百炼要求 `msg_` 前缀），该 id **从不落盘** → 等长补丁法无效。探针两次运行均 2.6s 内零产出失败 |
| 3 | **换官方 OpenAI 端点跑 automation** | ⏸ 保留为备选 | 官方端点容忍这些畸形项，automation 应可正常工作。但需要官方额度与密钥；且一旦切回第三方端点问题复现 → 不作为主方案 |
| 4 | **`codex exec` CLI + 外部调度** | ✅ **采纳** | 实测走标准 user message 投递：rollout 中 `function_call_output` 计数 **0**、无 `automation_update`、无 `at_` 前缀 id、模型正常回复、退出码 0。额外好处：① 不依赖 Codex 桌面应用是否开着；② 无 jitter，定时更准；③ 每次天然是全新会话，顺带解决上下文累积漂移 |

### 实测证据

| 项 | 值 |
|---|---|
| cron 探针 | 临时 automation `probe-call-id`，约定 11:16 / 11:24，实际 11:17:54 / 11:25:54 触发（**+2 分钟 jitter**，来自 `~/.codex/automations/.run-jitter-salt`） |
| cron 探针会话 | `01a0ad5e-ec81-…`、`01a0ad66-4013-…`；各含 1 条 `call_id=None, name=automation_update` |
| cron 失败详情 | `duration_ms ≈ 2615`、`last_agent_message = null`；错误 `Invalid 'id': message id must be a string starting with 'msg_', got 'at_d95b52a4-…'` |
| `at_` id 是否落盘 | **否**。两个 rollout 全文只有 `msg_` 与 `fco_` 两种前缀 |
| CLI 验证会话 | `01a0adcd-2b86-…`（`codex-cli 0.154.0-alpha.6.2`，`exec --skip-git-repo-check`），assistant 正常回复，19422 tokens，退出码 0 |
| 对比：老 heartbeat 会话为何可修 | 毒项落盘 id 是合法的 `fco_01a0aad8-…`，只缺 `call_id` → 补 `hb_…` 即恢复问答 |

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

1. **`docs/overnight-automation-charter.md` §11 全章重写**（章程第 6 行：修改本章程需 ADR，即本 ADR）：
   - §11.1「为什么是 heartbeat 而不是 cron」整节作废 → 改为「为什么两者都不用」
   - §11.2 互斥锁 `.nightly.lock`：**保留**（人类与夜间任务仍可能撞车）
   - §11.3「当夜」定义与轮次编号：**保留**，但"每次唤醒最多 2 轮"改为"每次运行最多 2 轮"
   - §11.4 晨间报告触发条件：**保留**
   - §11.5 机制性上报事件：删去"thread 触发 auto-compact""需把 automation 重新指向新 thread"两条（不再适用），新增"包装脚本未释放锁""CLI 非零退出"
   - §11.6 通知策略：**必须重写** —— CLI 路径没有应用内通知，失败只体现在退出码与日志文件，需要新的失败可见性方案
   - §11.7 运行前提：从"依赖 Codex 桌面应用运行"改为"依赖 Windows 任务计划程序；电脑关机/睡眠仍不补跑"
   - §10 自检表「重开会话是正常操作」一行：现在**真的做到了每轮新会话**，从"机制上做不到"改为"机制上已保证"
2. **新增 `scripts/nightly-run.ps1`**（包装脚本）：获取锁 → 调 `codex exec` → 输出重定向到 `docs/nightly/logs/` → 无论成败释放锁 → 以 CLI 退出码作为任务计划的成功判据
3. **新增顶层目录 `scripts/`** → 按 gov §5.4「新增顶层目录必须在 ADR 白名单中」，**本 ADR 即该授权**
4. **MEMORY.md §1 快照与 §3 DECISION** 需由 Orchestrator 覆写（见 PL-015）
5. **章程 §13 W4 的范围需调整**：0018 已由本 ADR 正式建立，W4 应只做 0016 / 0017 / 0019 / 0020 四条草稿
6. **注册 Windows 任务计划**（每日 23:30 / 02:30）—— 属人类操作或需人类明确授权

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| CLI 无人值守时若卡住，没有应用内 UI 可中断 | 包装脚本设**硬超时**（建议 90 分钟），超时 kill 并释放锁；任务计划侧设"运行时间上限" |
| 失败不可见（没有应用通知） | ① 退出码 + 日志文件；② 晨间报告 §① 必须记录本次运行退出码；③ 任务计划失败可挂事件日志/邮件（待定） |
| `codex exec` 在非受信目录会拒绝运行 | 脚本固定 `cd` 到仓库根（已在 `config.toml` 的 `[projects]` 中标 `trust_level = "trusted"`），并显式传 `--skip-git-repo-check` |
| CLI 版本升级改变 `exec` 行为 | 脚本首行把 `codex --version` 写进日志；升级 Codex 后必须重跑一次探针 |
| `qwen3.8-max` metadata 缺失导致上下文窗口被 fallback 成 828400（PL-014） | 脚本显式传 `-c model_context_window=1000000`；首次运行后核对日志实际值 |
| 夜间 agent 与人类同时改仓库 | `.nightly.lock` 互斥（§11.2 保留）+ 只在 `nightly/<date>` 分支提交 |
| 任务计划在无人登录时不运行 | 配置为"仅在用户登录时运行"（工作站夜间通常开机登录）；无人登录运行需存凭据，**不建议** |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

**验收（启用后第一晚必须全部满足）**：

1. 任务计划报告显示上次运行退出码 `0`
2. `docs/nightly/logs/<date>-<HHMM>.log` 存在，且含 `session id:` 与模型回复
3. 该会话 rollout 中 `function_call_output` 计数为 **0**（`fix_codex_callid.py` 干跑扫描全库仍为 `total 0`）
4. `docs/nightly/<date>-round-1.md` 与 LEDGER 追加行存在
5. `.nightly.lock` 在运行结束后**不存在**

**重新评估的触发条件**：

- Codex 官方修复 automation 投递机制（合成项带上合法 `call_id` 与 `msg_` 前缀 id）→ 可回到选项 1/2，换取应用内可见性
- 本机切换到官方 OpenAI 端点 → 选项 3 变得可行
- `codex exec` 在无人值守下连续 3 晚失败 → 暂停夜间自动化，回到纯白天推进

# 阶段 1 — 三试点闭环（Notepad → Paint → Edge/Chrome）

> 周期 10~12 周　状态：**进行中**（stage-0 已 2026-09-20 closeout；1a 批次 **A1 全部 Done**：TASK-011 / 012 / 013 / 014 / **015**；A2 **TASK-016 / 017 / 018 / 019 已 Done（2026-09-25）**，其中 TASK-019 = `automation-host` 进程 + `crates/ipc`：帧 / 握手 token / NamedPipe / 对端身份白名单 / 双向心跳 / 看门狗 、**TASK-020 = `crates/tool-bus`：MCP client(`rmcp`) + in-process server + draft-07 子集参数校验 + 统一返回信封 + 工具集指纹 + 动态挂载（含 > 40 告警）已 Done（2026-09-25）→ 下一张 = **TASK-021**（`policy`：白名单 / 风险分级 / 默认拒绝 / 审批决策））　上位文件：`PLAN.md`
> 依据：架构 v2.2 §20.2、feasibility v1.1 §3.0/§3（P1/P3/P5 档案）
> 全局拆解见 `docs/wbs-overview.md`；每张卡在开工前由 Orchestrator 按 gov §3.2 模板展开为 `tasks/TASK-NNN-*.md`

## In scope（冻结）

三个子阶段串行推进，每个子阶段有自己的 DoD：

| 子阶段 | 周期 | 目标 | 通道 |
|---|---|---|---|
| **1a** | 4~5 周 | 全部基础设施 + Notepad 3 个任务闭环 | L3 UIA + L1 文件契约 |
| **1b** | 3 周 | Paint 3 个任务闭环 | L4 坐标注入 + L5 视觉验证 |
| **1c** | 3~4 周 | Edge/Chrome 3 个任务闭环 + 安全底座验收 | L1 CDP + L3 UIA 外壳 |

## Out of scope（做了算漂移）

macOS/Linux 任何代码；Excel/Word/Photoshop Adapter；外部 MCP server 加载；WASM 插件；无人值守执行；技能市场与插件签名；向量检索记忆；股票类软件任何通道；图表可视化面（仅预留接口）；提权 Host（`automation-host-elevated`）；EgressProxy 独立进程（本阶段内联在 Core）；录制器（Recorder）；多 agent 并行执行任务。

## 阶段 1 DoD（不达标不进入阶段 2）

- [ ] 9 个任务连续 10 次运行成功率：Notepad ≥ 90%、Paint ≥ 75%、Edge ≥ 75%
- [ ] **静默失败 = 0**（任何"报告成功但实际未生效"都视为致命缺陷）
- [ ] 撤销成功率 ≥ 95%，撤销冲突 100% 被检测
- [ ] **注入靶页：0 次执行页面内指令**（安全一票否决）
- [ ] L3 不可逆动作 100% 经人工确认，且计划 UI 正确标注 point-of-no-return
- [ ] 四类异常可安全终止或恢复：断网、Core 崩溃、用户中途操作、目标程序退出
- [ ] 所有写操作有 postcondition 且被验证；所有失败可从时间线定位原因
- [ ] CI 门禁全绿：**gov §5.1 的 17 行清单 ↔ 16 个 CI 步骤（9 硬 + 7 软）**（含 arch test、schema 校验、类型同步、hygiene **13 项**、回放基准、spike-deny）
      > 口径由 **ADR-0025 D4** 统一（2026-09-18，PL-001 关闭）。原写「CI 14 项门禁」是过时表述。
- [ ] 覆盖率：workspace ≥ 75%，`core`/`policy`/`task-engine` ≥ 85%
- [ ] 每个 crate 有 README（职责/边界/**不变量**/已知限制）
- [ ] `MEMORY.md` 已回填阶段 1 新增的 FACT/PITFALL/REJECTED
- [ ] 阶段末对齐审计（gov §7.2）完成且偏差项已裁决

---

## 子阶段 1a：基础设施 + Notepad（TASK-011 ~ TASK-039）

### 批次 A1　地基层（**必须串行**，后续全部依赖）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **011** | `protocol` crate：schema 单一事实源 + Rust/TS 代码生成 + `ErrorCode` 枚举 | `protocol/**`、`crates/protocol/**`、`xtask/src/codegen*` | 001 | M | `codegen --check` 在 CI 通过；ErrorCode 覆盖 v2 §8.7 全部分类；**TS 类型 100% 生成，无手写重复** |
| **012** | 存储层：SQLite(WAL) + 迁移框架 + 核心表 + 内容寻址 blob（zstd/去重） | `crates/storage/**` | 011 | M | Spike H 的性能预算全部达标；孤儿 blob 可 GC；DB/blob 不一致可检测并标 `evidence_missing` |
| **013** | `audit`：追加不可改 + hash chain + ring buffer 批量 flush + `durability` 可配 | `crates/audit/**` | 012 | S | 无 UPDATE/DELETE 路径；篡改可被 hash chain 检出；batched 摊销 < 1 ms/条；`immediate` 模式可用 |
| **014** | `secrets`：OS keychain 封装（DPAPI/Keychain/Secret Service） | `crates/secrets/**` | 011 | S | 密钥不落盘明文、不入日志、不入 prompt；`zeroize` 生效；密钥访问被审计 |
| **015** | `xtask`：hygiene + arch test + verify-schemas + replay 骨架 | `xtask/**`、`crates/core/tests/arch*` | 011 | M | **arch test 能拦住 core→platform/windows 的依赖**；hygiene **13** 项检查全部生效（gov §5.4；口径见 **ADR-0025**，原「12 项」为笔误） |

> ★ 015 必须早做：它是后续所有卡的护栏。护栏晚于代码 = 漂移已经发生。
>
> **2026-09-24 顺序澄清（Orchestrator 代行）**：本表头写「必须串行」，但依赖列里只有 `013 ← 012` 是真串行；
> `014 ← 011`、`015 ← 011` 在 011 完成后即解锁，且三者 write scope（`crates/storage` / `crates/secrets` / `xtask`）互不重叠。
> **实际推进顺序仍按 011 ✅ → 012 → 013 → 014 → 015** —— 瓶颈是**人类审阅带宽**（AGENTS.md §3：并行度 ≤ 3、一会话 1~2 张卡），
> 不是依赖本身。
> ✅ **PL-037 已闭环（2026-09-24，人类裁决选项 ③ → `tasks/TASK-201-core-crate-skeleton.md`）**：`crates/core` 骨架已**提前落地**
> （`crates/core/{Cargo.toml,README.md,src/lib.rs}`；零第三方依赖、零 `pub` 项），
> 「`crates/core` 要到 TASK-028 才存在」这个硬阻塞已消除 → **015 现在可开工**。
> ✅ **2026-09-24 TASK-015 已把这段语义边界消掉**：`cargo test -p assistant-core arch::` 现在是 **`running 5 tests`**
> （`crates/core/tests/arch_layering.rs`：正向 2 + 负向 3），且 CI 的 `[SOFT #5]` 已**转硬**为 `[HARD #5]`
> （负向验证 = 该文件的三条 `arch::scan_flags_*` 用例，ADR-0019 N1）。同批把 `check-ledger`（gov #16）也转硬。
> ⚠ 上方 DoD 行的「9 硬 + 7 软」是本次转硬后的实测口径（原写「7 硬 + 9 软」）—— **只改派生计数，DoD 条目内容未动**。

### 批次 A2　平台层（011/015 完成后可 2 路并行）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **016** | `platform/api`：统一 trait + `CapabilityMatrix` + `TargetDescriptor` + `NormalizedPoint` + `Fingerprint` 类型 | `crates/platform/api/**`、`protocol/capability-matrix*` | 011 | M | trait 覆盖 v2 §13.1.1 全部方法（含 wait/fingerprint/capability）；**所有方法可取消且有超时** |
| **017** | `platform/windows`：UIA provider（树快照、selector 链解析、read_text、set_value、edit_text、invoke_action、bounds、fingerprint、window 枚举与状态） | `crates/platform/windows/src/uia/**`、`.../src/window/**` | 016 | L | Spike A 指标在真实记事本上复现；树遍历符合性能预算；**歧义/未找到/无响应三类错误可区分** |
| **018** | `platform/windows`：合成输入（SendInput）+ 焦点校验 + 坐标归一化（DPI/多屏）+ IME 处理 | `crates/platform/windows/src/input/**`、`.../src/coordinates/**` | 016 | M | 发送快捷键前 100% 校验前台窗口；Spike A2 的坐标精度矩阵全配置 ≤ 2 px；IME 开启时文本写入仍正确 |
| **019 ✅** | `automation-host` 进程 + `ipc`（JSON-RPC/NamedPipe + token + 对端身份校验 + 心跳 + 看门狗） | `apps/automation-host/**`、`crates/ipc/**` | 016 | M | Spike B 的重解析矩阵达标（≥95%）；**element 不出进程**（arch test 校验）；host 崩溃可被检测 |

### 批次 A3　内核层（016 完成后可 3 路并行，write scope 天然不重叠）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **020** | `tool-bus`：MCP client(`rmcp`) + in-process server + JSON Schema 校验 + **统一返回信封**（`untrusted`/`truncated`）+ 工具集指纹 + 动态挂载 | `crates/tool-bus/**` | 011 | L | 内部工具经 MCP 表达；信封字段齐全；工具数 > 40 时告警；schema 不合法直接拒 |
| **021** | `policy`：白名单 + 风险分级 + 参数校验（路径穿越/URL/长度/正则复杂度）+ 规则 DSL v0 + **默认拒绝** | `crates/policy/**` | 011 | L | v2 §12.2 五条示例规则全部可表达；决策是**纯函数**；每个 deny 带 `rule_id` 与可读 reason；策略判定 < 50 µs |
| **022** | `task-engine`：12 状态机 + Plan/Step DAG + 检查点 + 恢复 + 取消 + 预算 + 看门狗 | `crates/task-engine/**` | 011,012 | L | 状态迁移全部持久化；崩溃后能恢复；**"不确定是否执行过"必须走 NeedsHuman**（禁止猜测） |
| **023** | `verify`：后置断言引擎（11 种断言）+ 状态指纹 + 幂等判定 + `on_violation` 分派 | `crates/verify/**` | 011 | M | 断言类型齐全；**验证失败绝不返回 ok**；指纹可配置忽略字段（防抖） |
| **024** | `undo`：可逆性四级 + 锚点（内容快照/影子副本/步数级）+ 回滚剧本执行 + 冲突检测 + incident 上报 | `crates/undo/**` | 012,023 | L | Spike F 三条路径达标；冲突 100% 检测且默认最保守；撤销失败按 incident 处理 |
| **025** | `lease`：目标租约（exclusive/shared/intent + TTL + 续租 + 用户抢占 + 死锁避免） | `crates/lease/**` | 011 | S | 同目标同刻仅一个写租约；用户操作可强制释放；租约冲突是可读错误 |
| **026** | `model-gateway`：Provider trait（流式/取消/用量）+ 路由器 + 降级链 + 重试退避 + prompt cache 提示 + 成本计量 | `crates/model-gateway/**` | 011 | L | v2 §11.2 路由规则可配置；**取消能在 1 s 内中止请求**；每步记录 tokens/cost/latency |
| **027** | `hitl`：审批请求 + 授权范围（四维+TTL）+ 用户接管 + 暂停恢复 + 差异预览数据准备 | `crates/hitl/**` | 021,022 | M | 高风险禁止 persistent 授权；接管后交还需重新同步状态；审批超时行为明确 |
| **028** | `core`：会话管理 + 上下文管理（树裁剪/压缩/预算）+ Planner + Memory(App Map 加载/FTS5 检索) + 组装 | `crates/core/**` | 020~027 | L | arch test 通过（core 只依赖 trait）；上下文预算生效；App Map 按需片段注入 |

### 批次 A4　应用与 UI（028 完成后可 2 路并行）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **029** | 二进制骨架：`apps/agent-core` + `apps/desktop-ui`（Tauri 2 + React + TS + Tailwind）+ capabilities 最小化 + CSP | `apps/agent-core/**`、`apps/desktop-ui/src-tauri/**`、`apps/desktop-ui/*.config.*` | 028 | M | **webview 零系统权限**；CSP 严格；禁远端内容；IPC 全部走 token |
| **030** | UI：审批卡片（含 diff + 来源归因 + 授权范围）+ 执行时间线（含证据与撤销按钮） | `apps/desktop-ui/src/features/approval/**`、`.../timeline/**` | 029 | L | v2 §10.2 全部字段齐备；`app_content` 来源标红且默认拒绝；撤销按钮按可逆性分级禁用 |
| **031** | UI：元素拾取器 v0（悬停高亮 + 属性面板 + 一键生成 selector 候选链）+ 目标绑定向导 | `apps/desktop-ui/src/features/picker/**`、`.../binding/**` | 029,017 | L | 能对记事本生成 ≥ 3 种候选 selector 并写回 Adapter 草稿 |
| **032** | UI：策略面板 + **出域三档开关**（含逐应用覆盖）+ Capability Matrix 视图 + 成本面板 | `apps/desktop-ui/src/features/policy/**`、`.../capability/**`、`.../cost/**` | 029,021 | M | 三档可切换且即时生效；降级不静默；状态栏常驻显示当前出域级别 |

### 批次 A5　Notepad 闭环

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **033** | 靶机应用 v0：`notepad-like`（WinUI/WPF，全部控件有稳定 AutomationId，CLI 可注入故障：元素消失/超时/歧义多匹配/意外弹窗/忙碌） | `fixtures/apps/notepad-like/**` | 001 | M | 故障可脚本化触发；CI 可在 windows runner 上跑 |
| **034** | 录制回放框架 v0：树快照录制 + 离线回放（替代真实平台调用）+ `xtask replay` | `crates/replay/**`、`xtask/src/replay*`、`fixtures/recordings/**` | 017,022 | M | 逻辑层回归可在无真机的 CI 上跑；回放能复现 Spike 采集的真实快照 |
| **035** | Notepad Adapter：`adapter.toml` + `app_map.json` + `selectors/` + `tools/`（read_text/replace_text/save/new_tab/save_as）+ `rollback/` + `interrupts/`（未保存三态对话框默认"取消"） | `adapters/com.microsoft.notepad/**` | 017,020,024 | L | schema 校验通过；**声明 Win11 版本范围**；undo 快捷键显式声明；含 `version_range` 与 known_pitfalls |
| **036** | T1.1：打开文件 → 读全文 → 报告行数与关键词段落（只读） | `adapters/com.microsoft.notepad/tasks/**`、`eval/tasks/notepad/**` | 035 | S | 10 次连续成功率 ≥ 90%；大文件（1 MB）走 L1 文件通道降级并正确标 `truncated` |
| **037** | T1.2：全文替换「报表」→「报告」+ 保存（含审批 diff、L0 undo + L1 快照、后置断言） | 同上 | 036 | M | 替换计数正确；保存后标题无 `*`；撤销可回到锚点；**歧义时按 `error_if_ambiguous` 报错** |
| **038** | T1.3：新建标签 → 写入 → 另存为到指定路径（跨进程 Shell 对话框） | 同上 | 037 | M | 跨进程对话框解析成功率 ≥ 90%；**已存在文件绝不静默覆盖**（先备份 + 确认） |
| **039** | 阶段 1a 集成验收：CI 门禁全启用（gov §5.1 的 17 行清单 ↔ 16 个步骤：7 硬 + 9 软，ADR-0025 D4） + 9 项 DoD 中 1a 相关项 + 对齐审计 | `.github/workflows/**`、`docs/audits/**` | 033~038 | M | 全部绿灯；审计报告产出；偏差项已裁决 |

---

## 子阶段 1b：Paint（TASK-040 ~ TASK-047）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **040** | 合成输入完善：拖拽（按下-移动-释放的原子性与租约独占）+ 校准流程（首次显示器组合点击已知元素验证命中） | `crates/platform/windows/src/input/**` | 018 | M | 拖拽期间禁止其他写租约；校准失败则**禁用坐标通道**并上报 |
| **041** | 截图管线：窗口截图 + 脱敏（密码框/正则命中区域遮挡）+ 滚动清理 + 隐私模式（不保存截图） | `crates/capture/**`、`crates/dlp/src/redact*` | 017 | M | 只截目标窗口而非全屏；脱敏规则可配；存储上限与 TTL 生效 |
| **042** | 视觉验证：容差断言 + 感知哈希(pHash/dHash) + `visual_assert` 断言类型 + `confidence_min` | `crates/verify/src/visual/**` | 023,041 | M | 能稳定区分"画对/画错/没画上"，误判率 < 5%；**低置信结果必须标注且不得单独作为成功依据** |
| **043** | Paint Adapter：工具选择/颜色/图层（UIA）+ 画布坐标动作 + 像素快照回滚 + 缩放与滚动的坐标换算 | `adapters/com.microsoft.paint/**` | 040,042 | L | 画布坐标 ≠ 屏幕坐标的换算正确；工具状态"设置后回读"验证；图层前置条件生效 |
| **044** | T3.1：新建画布 → 选矩形工具与颜色 → 指定画布坐标画矩形 → 截图验证形状与颜色 | `adapters/com.microsoft.paint/tasks/**`、`eval/tasks/paint/**` | 043 | M | 10 次成功率 ≥ 75%；坐标命中误差 ≤ 2 px；容差断言生效 |
| **045** | T3.2：打开 PNG → 读尺寸与缩放 → 区域标记 → 另存为新文件 | 同上 | 044 | M | 不覆盖已有文件；缩放状态下坐标仍正确 |
| **046** | T3.3：绘制 → 用户点撤销 → **像素级验证回到快照** | 同上 | 045 | S | 像素级回滚确认；undo 粒度实测结论回填 `MEMORY.md` |
| **047** | 阶段 1b 集成验收 + Adapter 复用度检查（Paint 是否被迫改了平台层？改了 → 记 ADR） | `docs/audits/**` | 040~046 | S | 若 Paint 需要修改 `platform/api` trait → 必须 ADR（这是抽象是否正确的关键信号） |

---

## 子阶段 1c：Edge/Chrome + 安全底座（TASK-048 ~ TASK-058）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **048** | CDP provider：连接管理 + DOM 读写 + 点击 + 导航 + 截图 + 下载目录控制 + load/networkIdle 判定 | `crates/platform/windows/src/cdp/**`、`crates/platform/api/src/cdp*` | 016 | L | Spike G 指标复现；**依赖登记进 `docs/DEPENDENCIES.md`** |
| **049** | 专用 profile 管理：自定义 `--user-data-dir` 生命周期 + 独立窗口标识 + **禁止复制用户 profile** 的硬约束 + 首次登录引导 | `crates/browser-profile/**` | 048 | M | 136+ 约束正确处理；代码审查确认无任何 Cookie/凭据复制路径 |
| **050** | `dlp`：三档出域策略（`local_only`/`redacted`/`full`）+ 逐应用与逐内容类型覆盖 + 脱敏规则 + endpoint 白名单 + **降级不静默** | `crates/dlp/**` | 014,026 | L | 选 `local_only` 而无本地模型时**明确报错**而非改走云端；策略变更写审计 |
| **051** | 污点追踪 + 权限衰减：`untrusted` 内容引入后标记生效，宽授权降级为 `once`，高风险 deny | `crates/policy/src/taint**`、`crates/core/src/session**` | 021,020 | M | v2 §12.4 四层中第 2/3 层生效；污点只能由 SessionManager 清除（**不变量写进 README**） |
| **052** | 来源归因：每个动作记录 `instruction_origin`（user_request/plan_derived/app_content/tool_suggestion）+ UI 展示 | `crates/core/src/origin**`、`apps/desktop-ui/src/features/approval/**` | 027,030 | M | `app_content` 来源在卡片上标红且默认拒绝 |
| **053** | 注入靶页 fixture：可见指令 / 隐藏元素指令 / HTML 注释 / 伪系统提示 + 一个正常提取任务 | `fixtures/web/injection-target/**` | 001 | S | 页面可本地打开（`file://` 或本地 http），无需外网 |
| **054** | 干净上下文复核（第 4 层）：高风险动作前用不含外部内容的小模型复核一致性 | `crates/core/src/verify_review**`、`crates/model-gateway/**` | 026,051 | M | 不一致 → 拒绝并告警；复核调用本身不计入污点上下文 |
| **055** | Edge Adapter：CDP 工具集（读取/填表/导航/下载）+ 外壳 UIA 工具（地址栏/标签/下载栏）+ 站点黑名单（银行/支付/密码修改）+ interrupts（Cookie 横幅/登录墙/验证码 → NeedsHuman） | `adapters/browser.edge/**`、`adapters/browser.chrome/**` | 048,049 | L | **禁止绕过验证码**；黑名单命中即拒绝；标签身份用 URL+标题+自定义标记组合，不用 tab index |
| **056** | T5.1：打开指定站点 → 提取列表页结构化数据 → 写入本地 CSV | `adapters/browser.edge/tasks/**`、`eval/tasks/edge/**` | 055 | M | 10 次成功率 ≥ 75%；异步加载判定误判率 < 5% |
| **057** | T5.2：表单填写 → **停在提交前** → 展示 diff 与来源归因 → 用户确认后提交 | 同上 | 056 | M | L3 动作 100% 经确认；计划 UI 正确标注 point-of-no-return |
| **058** | T5.3：注入靶页安全验收 + **安全回归常驻 CI** + 阶段 1c/阶段 1 总验收与对齐审计 | 同上、`.github/workflows/**`、`docs/audits/**` | 053,054,055 | M | **0 次执行页面内指令**（一票否决）；安全回归在 CI 中每次 PR 都跑；阶段 1 全部 DoD 达成 |

---

## 并行编排建议（write scope 不重叠，≤3 agent）

```text
批次 A1（串行，1 agent）      011 → 012 → 013 → 014 → 015
批次 A2（2 agent 并行）       {016} → {017, 019} 并行；018 接在 017 后
批次 A3（3 agent 并行）       {020, 021, 026} → {022, 023, 025} → {024, 027} → {028}
批次 A4（2 agent 并行）       {029} → {030, 032} 并行；031 需 017 就绪
批次 A5（1~2 agent）          033 ∥ 034 → 035 → 036 → 037 → 038 → 039
批次 1b（2 agent）            {040, 041} → 042 → 043 → 044 → 045 → 046 → 047
批次 1c（2~3 agent）          {048, 050, 053} → {049, 051, 052} → 054 → 055 → 056 → 057 → 058
```

**关键路径**：`011 → 012 → 016 → 017 → 020/021/022 → 028 → 029 → 035 → 036~038 → 039`（约 6~7 周），因此 **011/012/016/017 四张卡不得延误**，且应由最强的 agent（或人类直接审阅）负责。

**人类审阅瓶颈提示**：批次 A3 有 8 张内核卡、每张 diff 可达 400 行 → 若并行 3 个 agent，人类每天需审阅约 1200 行。**建议 A3 批次并行度降为 2**，或把 022/024/028 三张最关键的卡改为人类逐行审阅。

---

## 卡片正文的位置（ADR-0031：**一卡一文件**）

> 本文件**只保留阶段级信息**：In/Out scope、阶段 DoD、批次表与并行建议。
> 每张卡的**正文**（目标 / write scope / In scope / 必须遵守 / 验收命令 / DoD）与**执行记录**都在
> `tasks/TASK-NNN-<slug>.md` 里，由 Orchestrator 在**开工前**按 gov §3.2 模板展开（会话启动只读自己那一个）。
> **本文件不再放任何卡片正文** —— 出现即被 `xtask card-check` 判为 Error（ADR-0031 D6 判据 ④）。
> 模板与填写要求见 **gov §3.2（模板）与 §3.4（9 节执行记录骨架 + 分界线原文）**。

**已展开的卡片文件**（其余 46 张在开工前逐张展开；本表只登记已存在的文件，**只记「完成标记」** —— 该卡是否 Done（**ADR-0041 D1**）；
状态的**权威落点**仍是卡片文件自身，ADR-0031 D2）：

| 卡号 | 批次 | 卡片文件（正文 + 执行记录） | 备注 |
|---|---|---|---|
| TASK-011 | A1 | `tasks/TASK-011-protocol-schema-codegen.md` | 完整卡（原 plans 的示例逐字搬运）；**已 Done** |
| TASK-035 | A5 | `tasks/TASK-035-notepad-adapter.md` | ⚠ **仅要点摘录**，展开前不得派单 |
| TASK-012 | A1 | `tasks/TASK-012-storage-layer-sqlite-wal-blob.md` | **完整卡**（2026-09-24 Orchestrator 展开）；**已 Done（2026-09-24）** |
| TASK-013 | A1 | `tasks/TASK-013-audit-append-hash-chain-flush.md` | **完整卡**（2026-09-24 Orchestrator 展开；**已 Done**） |
| TASK-014 | A1 | `tasks/TASK-014-secrets-os-keychain-wrapper.md` | **完整卡**（2026-09-24 Orchestrator 代行展开：`keyring` 4.2 后端 + `zeroize` + 访问审计注入点（fail-closed）；write scope 含 `docs/DEPENDENCIES.md` —— 登记表规则 1「先登记后引入」）；**已 Done（2026-09-24）** |
| TASK-015 | A1 | `tasks/TASK-015-xtask-hygiene-archtest-replay-skeleton.md` | **完整卡**（2026-09-24 Orchestrator 展开：`docscan` 4 条结构规则 + `crates/core/tests/arch*` 分层断言 + PL-047 迁移登记表扫描 + ADR-0039 D3 的 `check-ledger` 两条规则）；**已 Done（2026-09-24）** |
| TASK-016 | A2 | `tasks/TASK-016-platform-api-trait-capability-matrix.md` | **完整卡**（2026-09-24 Orchestrator 代行展开：零平台依赖的纯类型 + 3 个 trait 形状 + `CapabilityMatrix::validate()` 的 4 条不变量；**卡面有 3 个待裁决项 Q1 / Q2 / Q3**）；**已 Done（2026-09-24）** |
| TASK-017 | A2 | `tasks/TASK-017-platform-windows-uia-provider.md` | **完整卡**（2026-09-24 Orchestrator 代行展开：UIA provider 全 9 方法 + `WindowProvider` 全 5 方法 + 句柄纪律源码扫描断言；**卡面有 4 个待裁决项 Q1 ~ Q4**）；**已 Done（2026-09-24）** |
| TASK-018 | A2 | `tasks/TASK-018-platform-windows-synthetic-input-ime.md` | **完整卡**（2026-09-25 Orchestrator 代行展开：`SendInput` VK + `KEYEVENTF_UNICODE` 路径 + 发送前 100% 前台校验 + 显示器枚举 / DPI 换算 / 虚拟屏幕归一化 + `is_ime_open`；**卡面有 3 个待裁决项 Q1 ~ Q3**）；**已 Done（2026-09-25）** |
| **TASK-019 ✅** | A2 | `tasks/TASK-019-automation-host-ipc-named-pipe.md` | **已 Done（2026-09-25）** |
| **TASK-020 ✅** | A2 | `tasks/TASK-020-tool-bus-mcp-rmcp-server.md` | **完整卡**（正文含 4 个待裁决项 Q1 ~ Q4；Q5 为实现时新命中，见卡 §5）；**已 Done（2026-09-25）** |
| TASK-021 | A2 | `tasks/TASK-021-policy-whitelist-risk-default-deny.md` | Ready（批次表占位派单前补全） |
| TASK-022 | A2 | `tasks/TASK-022-task-engine-state-machine-dag-checkpoint.md` | Ready（批次表占位派单前补全） |
| TASK-023 | A2 | `tasks/TASK-023-verify-postcondition-assertion-engine.md` | Ready（批次表占位派单前补全） |
| TASK-024 | A2 | `tasks/TASK-024-undo-four-level-rollback-anchor.md` | Ready（批次表占位派单前补全） |
| TASK-025 | A2 | `tasks/TASK-025-lease-target-exclusive-shared-intent.md` | Ready（批次表占位派单前补全） |
| TASK-026 | A2 | `tasks/TASK-026-model-gateway-provider-router-fallback.md` | Ready（批次表占位派单前补全） |
| TASK-027 | A2 | `tasks/TASK-027-hitl-approval-scope-takeover.md` | Ready（批次表占位派单前补全） |
| TASK-028 | A2 | `tasks/TASK-028-core-session-context-planner-memory.md` | Ready（批次表占位派单前补全） |
| TASK-029 | A3 | `tasks/TASK-029-binary-skeleton-agent-core-desktop-ui.md` | Ready（批次表占位派单前补全） |
| TASK-030 | A3 | `tasks/TASK-030-ui-approval-card-timeline-evidence.md` | Ready（批次表占位派单前补全） |
| TASK-031 | A3 | `tasks/TASK-031-ui-element-picker-selector-candidates.md` | Ready（批次表占位派单前补全） |
| TASK-032 | A3 | `tasks/TASK-032-ui-policy-panel-egress-capability-cost.md` | Ready（批次表占位派单前补全） |
| TASK-033 | A3 | `tasks/TASK-033-target-app-notepad-like-fault-injection.md` | Ready（批次表占位派单前补全） |
| TASK-034 | A3 | `tasks/TASK-034-record-replay-framework-xtask-replay.md` | Ready（批次表占位派单前补全） |
| TASK-036 | A5 | `tasks/TASK-036-t1-1-open-read-full-text.md` | Ready（批次表占位派单前补全） |
| TASK-037 | A5 | `tasks/TASK-037-t1-2-replace-save-approval-diff-undo.md` | Ready（批次表占位派单前补全） |
| TASK-038 | A5 | `tasks/TASK-038-t1-3-newtab-saveas-cross-process-dialog.md` | Ready（批次表占位派单前补全） |
| TASK-039 | A5 | `tasks/TASK-039-stage-1a-integration-audit.md` | Ready（批次表占位派单前补全） |
| TASK-040 | 1b | `tasks/TASK-040-synthetic-input-drag-lease-calibration.md` | Ready（批次表占位派单前补全） |
| TASK-041 | 1b | `tasks/TASK-041-capture-window-redact-privacy.md` | Ready（批次表占位派单前补全） |
| TASK-042 | 1b | `tasks/TASK-042-visual-verify-phash-dhash-confidence.md` | Ready（批次表占位派单前补全） |
| TASK-043 | 1b | `tasks/TASK-043-paint-adapter-tools-canvas-coords.md` | Ready（批次表占位派单前补全） |
| TASK-044 | 1b | `tasks/TASK-044-t3-1-newcanvas-rect-color-screenshot.md` | Ready（批次表占位派单前补全） |
| TASK-045 | 1b | `tasks/TASK-045-t3-2-png-open-read-region-saveas.md` | Ready（批次表占位派单前补全） |
| TASK-046 | 1b | `tasks/TASK-046-t3-3-draw-undo-pixel-snapshot-verify.md` | Ready（批次表占位派单前补全） |
| TASK-047 | 1b | `tasks/TASK-047-stage-1b-integration-adapter-reuse.md` | Ready（批次表占位派单前补全） |
| TASK-048 | 1c | `tasks/TASK-048-cdp-provider-connect-dom-nav-download.md` | Ready（批次表占位派单前补全） |
| TASK-049 | 1c | `tasks/TASK-049-browser-profile-no-copy-user-profile.md` | Ready（批次表占位派单前补全） |
| TASK-050 | 1c | `tasks/TASK-050-dlp-three-tier-egress-local-only-redacted-full.md` | Ready（批次表占位派单前补全） |
| TASK-051 | 1c | `tasks/TASK-051-taint-tracking-permission-decay.md` | Ready（批次表占位派单前补全） |
| TASK-052 | 1c | `tasks/TASK-052-instruction-origin-attribution-ui.md` | Ready（批次表占位派单前补全） |
| TASK-053 | 1c | `tasks/TASK-053-injection-target-fixture-visible-hidden.md` | Ready（批次表占位派单前补全） |
| TASK-054 | 1c | `tasks/TASK-054-clean-context-review-small-model-fourth-layer.md` | Ready（批次表占位派单前补全） |
| TASK-055 | 1c | `tasks/TASK-055-edge-adapter-cdp-ua-blacklist-interrupts.md` | Ready（批次表占位派单前补全） |
| TASK-056 | 1c | `tasks/TASK-056-t5-1-open-site-extract-list-write-csv.md` | Ready（批次表占位派单前补全） |
| TASK-057 | 1c | `tasks/TASK-057-t5-2-form-fill-stop-before-submit-diff-origin.md` | Ready（批次表占位派单前补全） |
| TASK-058 | 1c | `tasks/TASK-058-t5-3-injection-target-security-ci-audit.md` | Ready（批次表占位派单前补全） |
| TASK-085 | XTASK 池 | `tasks/TASK-085-xtask-hygiene-rust-source-rules.md` | **完整卡**（2026-09-24 PL-059 归属修正新建：gov §5.4 第 1 / 2 / 3 / 10 / 11 项 = Rust 源码结构规则）；**Ready** |
| TASK-086 | XTASK 池 | `tasks/TASK-086-xtask-hygiene-file-level-and-registry-rules.md` | **完整卡**（2026-09-24 PL-059 归属修正新建：gov §5.4 第 8 / 9 / 13 项 = 文件级 + 依赖登记规则）；**Ready** |


> **迁移零丢失核对**（ADR-0031 验证方式 3）：两段正文共 **30** 行非空内容（TASK-011 28 行 + TASK-035 2 行），
> 迁移后逐行同序一致地出现在新卡片文件的正文区（脚本校验 missing=0、same-order=True）。
> 搬运方式是**逐字节复制、不改一个字**；新增的只有元数据块、分界线注释与执行记录骨架。


## 跨阶段治理卡（不在本阶段批次表内）

| 卡号 | 号段 | 卡片文件 | 备注 |
|---|---|---|---|
| TASK-200 | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-200-fix-spec-contract-drafts.md` | `docs/spec/*` 7 份契约草案的系统性缺陷（PL-038）；**已 Done（2026-09-24）** |
| TASK-201 | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-201-core-crate-skeleton.md` | `crates/core` 骨架提前（PL-037 选项 ③ 的落地物）；**已 Done（2026-09-24）** |
| TASK-202 | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-202-storage-migration-registry.md` | 存储迁移注册表（**ADR-0038**：storage 只提供机制、各 crate 自持迁移 + 唯一装配点）；PL-046 的落地物；**已 Done（2026-09-24）** |
| TASK-203 | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-203-audit-log-column-semantics.md` | `audit_logs` 列语义去重 + 显式链序（**ADR-0040**：删与 `id` 同义的 `hash`、加 `sequence`；迁移 0003 重建表）；PL-043 / PL-045 的落地物；**已 Done（2026-09-24）** |

## 任务卡号段分配（ADR-0037, 2026-09-20 起生效）

按 [docs/adr/0037-task-card-number-allocation-strategy.md](../../docs/adr/0037-task-card-number-allocation-strategy.md)：

| 号段 | 用途 | 现状 |
|---|---|---|
| 001~010 | stage-0 已 Done | 10 张（TASK-001~010）|
| 011 / 012 / 035 | stage-1 已迁入卡片文件 | 3 张（011 protocol / 012 storage = **完整正文**；035 notepad-adapter = **仅要点摘录、未展开**）|
| 013~034 / 036~058 | **stage-1 批次表占位**（本文件批次表）| **45 张 Ready**（开工前由 Orchestrator 逐张展开正文）|
| 059~070 | 已用 = xtask 护栏升级（6 张）+ governance（6 张）| 12 张 Done |
| 071 | ADR-0037 实施卡（号段分配策略；本 ADR 生效前建的治理卡）| 1 张 Done |
| 072~099 | XTASK 池 | 已用 072~079（stage-0 b1 探针卡）+ 083 / 084 + **085 / 086**（PL-059 归属修正新建）= **12 张**；**空位 080~082 / 087~099** |
| 100~199 | 业务池 | 已用 100 / 101（Win32-Input + probe 替换）；空位 102~199 |
| 200~299 | 治理池（audit/docs/memory 治理）| 已用 200（spec 修复卡）/ **201**（`crates/core` 骨架提前，PL-037）/ **202**（存储迁移注册表，ADR-0038，PL-046）；空位 203~299 |

**未来 xtask 护栏扩张** = 用 072~099；用满后用 200~299。sub-suffix 永久禁用。

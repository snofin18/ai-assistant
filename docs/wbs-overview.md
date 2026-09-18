# 全项目工作分解与执行顺序（WBS Overview）

> 版本：1.0　日期：2026-09-16　上位：`PLAN.md`
> 用途：给出**从第一行代码到可交付产品的完整分解与顺序**，使后续每一步都能"按卡推进"，不依赖临时决定。
> 卡号区间已预分配，避免后续冲突：阶段 0 = 001~010，阶段 1 = 011~058，阶段 2 = 059~080，阶段 3 = 081~100，阶段 4 = 101~130，阶段 5 = 131~160，阶段 6 = 161~200，阶段 7 = 201+。

---

## 1. 里程碑总览

| 阶段 | 名称 | 卡号 | 周期 | 里程碑（可对外展示的产出） | 关键验证 |
|---|---|---|---|---|---|
| 0 | Spike 技术验证 | 001~010 | 2~3 周 | 8 份 `SPIKE_REPORT` + 首批 spec/ADR | 关键假设被证实或证伪 |
| 1 | 三试点闭环 | 011~058 | 10~12 周 | **能对记事本/画图/Edge 执行 9 个真实任务，全程可审计可撤销** | 静默失败=0；注入靶页 0 命中 |
| 2 | Excel + Adapter 抽象 | 059~080 | 6~8 周 | 企业级 L1 COM 通道；Adapter 规范正式化 | COM 写入 100% 走快照；新应用边际成本 ≤ 3 人日 |
| 3 | Photoshop + 评测体系 | 081~100 | 6~8 周 | 纯脚本型应用支持；指标看板 | historyState 回滚 100% 可靠 |
| 4 | 扩量 + 股票只读 + 图表面 | 101~130 | 持续 | 8~10 个应用；股票数据解读与可视化 | 双通道交叉校验一致率 ≥ 99% |
| 5 | macOS | 131~160 | 8~10 周 | 同族应用（TextEdit/Preview/Safari/Numbers） | TCC 引导可用；更新后权限可恢复 |
| 6 | Linux（Wayland-first） | 161~200 | 10~12 周 | KWin 优先，GNOME 次之 | Spike D 完整版通过项才承诺 |
| 7 | 开放生态（可选） | 201+ | — | 外部 MCP、无人值守、交易闸门 | 每项都需独立 ADR |

**总量级**：阶段 0~3 约 26~31 周（6~8 个月）达到"三平台中的 Windows 深度可用 + 5 个应用"；全部 7 阶段约 12~16 个月（3 人 + AI agent 协作模式下）。

---

## 2. 依赖图（关键路径）

```text
阶段0
 001 仓库骨架 ─┬─ 002 Notepad UIA ──┬─ 004 跨进程Host ──────────────┐
              ├─ 003 Paint 坐标 ────┤                               │
              ├─ 005 工具选择       │                               │
              ├─ 006 UI 原型 ───────┤                               │
              ├─ 007 撤销闭环 ──────┤                               │
              ├─ 008 Edge CDP ──────┤                               │
              ├─ 009 存储 PoC ──────┤                               │
              └─ 010 Linux 侦察（旁路，不阻塞）                       │
                                    ↓（全部 go 才开阶段 1）           │
阶段1a  011 protocol ─→ 012 storage ─→ 013 audit ─→ 014 secrets ─→ 015 xtask(护栏)
                                    ↓
        016 platform/api ─→ 017 win/uia ─→ 018 win/input ─→ 019 host+ipc
                                    ↓
        020 tool-bus ┐ 021 policy ┐ 026 model-gateway ┐
        022 task-engine ┤ 023 verify ┤ 025 lease ┤ → 024 undo → 027 hitl → 028 core
                                    ↓
        029 apps 骨架 ─→ 030 审批/时间线 ┐ 031 拾取器 ┐ 032 策略/出域面板 ┘
                                    ↓
        033 靶机应用 ∥ 034 回放框架 ─→ 035 Notepad Adapter ─→ 036 T1.1 ─→ 037 T1.2 ─→ 038 T1.3 ─→ 039 1a验收
                                                                                              ↓
阶段1b  040 拖拽/校准 ∥ 041 截图/脱敏 ─→ 042 视觉验证 ─→ 043 Paint Adapter ─→ 044~046 T3.x ─→ 047 1b验收
                                                                                              ↓
阶段1c  048 CDP ∥ 050 dlp ∥ 053 注入靶页 ─→ 049 profile ─→ 051 污点 ─→ 052 归因 ─→ 054 复核
                                                    └────→ 055 Edge Adapter ─→ 056~057 T5.x ─→ 058 安全验收+阶段1总验收
```

**关键路径**（延误即整体延误）：
`001 → 002 → 011 → 012 → 016 → 017 → 020/021/022 → 028 → 029 → 035 → 036~038 → 039`

**旁路（可延后，不影响主线）**：010（Linux 侦察）、006（UI 原型可延到 029 前完成）、041/042（视觉验证在 1b 才需要）、050/053（1c 才需要）。

**★ 必须早做的护栏卡**：`011`（schema 与 ErrorCode）、`015`（xtask：arch test + hygiene）。护栏晚于业务代码 = 漂移已经发生且难以回收。

---

## 3. 阶段 2~7 的模块级分解（粗粒度，开工前再细化到卡）

### 阶段 2　Excel + Adapter 抽象正式化（059~080）

| 组 | 内容 |
|---|---|
| 抽象 | 在 3 个真实 Adapter（Notepad/Paint/Edge）之上**重构** Adapter 规范：`adapter.toml` schema 化、App Map schema 化、selector 链 schema 化、rollback recipe 语法、interrupts 模式语法 |
| COM 通道 | `platform/windows/src/com/`：COM 生命周期管理（显式释放、防残留进程）、就绪探测、`Application.Build` 版本区分（回填 M1） |
| Excel Adapter | 连接与身份（workbook+worksheet 级）、Protected View 探测、AutoSave/OneDrive 冲突探测、模态对话框族 interrupts、`forbidden_properties`（`DisplayAlerts`）、命名区域优先、`.Value`/`.Formula`/`.Text` 语义区分 |
| 可逆性 | **L1 快照为 Excel 的唯一可逆路径**（undo 栈被清空）；文件影子副本 + 原子替换 + 校验 |
| 自愈定位 | selector 成功率统计 → 分数回写 → `degraded` 告警（v2 §6.3） |
| App Map 生成器 | 由拾取器数据半自动生成 App Map 草稿（`tools/` 下的 devtool） |
| 评测 | 基准任务集框架 + 指标看板（成功率/步数/静默失败/人工干预/撤销成功率/定位命中率/耗时成本） |
| 任务 | T2.1 读取汇总、T2.2 写入命名区域+重算+校验+保存、T2.3 生成图表并导出 PNG |

### 阶段 3　Photoshop + 评测体系完善（081~100）

| 组 | 内容 |
|---|---|
| 脚本通道 | `platform/windows/src/scripting/`：UXP → JSX → COM 探测与选择；通道结果写入 Capability Matrix |
| PS Adapter | 就绪探测（启动 10~30 s + 首选项/登录/许可弹窗）、版本适配矩阵（v21~v27）、`historyState` 锚点（L0 强回滚）、文档激活与租约协调、大文件看门狗与可取消 |
| 长任务 | 任务分段与交接摘要（handoff）、预算控制、部分失败处理与报告 |
| 评测 | 回放基准扩充、真机抽样验收流程、指标趋势对比（Adapter 升级前后） |
| 任务 | T4.1 打开+列图层+导出 PNG、T4.2 可逆修改+直方图校验+historyState 回滚、T4.3 批量缩放导出+逐个校验+报告 |

### 阶段 4　扩量 + 股票只读 + 图表面（101~130）

| 组 | 内容 |
|---|---|
| 扩量 Adapter | Explorer（Shell COM + UIA）、Settings（`ms-settings:` + UIA）、7-Zip（CLI 优先）、Calculator、Word、PowerPoint、经典 Outlook（**只读**）、VS Code（CLI + 扩展） |
| 股票只读 | 数据源 Adapter（官方 API / 导出 / 剪贴板）、**统一行情数据模型**（与来源解耦）、双通道交叉校验、OCR 仅辅助 |
| 分析与图表 | `analysis.*` / `chart.*` 纯计算工具（内置 Rust 或 WASM）、UI 图表面（只读展示，不得成为操作通道） |
| 复用度验收 | 每新增一个 T1 应用的边际成本 ≤ 3 人日；否则回头修抽象 |
| 交易闸门 | 仅保留 `TradingGate` 类型与配置占位（**不实现**），启用需阶段 7 的 ADR |

### 阶段 5　macOS（131~160）

`platform/macos/`：AX provider（批量读取、AXObserver 事件、messaging timeout）、菜单 AXPress 优先、AppleScript/App Intents 通道、Vision OCR、CGEvent 兜底、坐标归一化（Quartz 左下原点 / Retina）、**TCC 权限自检与引导**、稳定 Developer ID 签名与公证、更新后权限迁移、同族应用 Adapter（TextEdit/Preview/Safari/Numbers/Photoshop mac 版）。

### 阶段 6　Linux Wayland-first（161~200）

先跑 **Spike D 完整版**（v2 §20.1 D 的 7 项）。然后：`platform/linux/atspi/`（树、Action、EditableText、Value、Selection、Component、事件）、`platform/linux/activation/`（Tier 0~3 激活方案 + 根因诊断 F1~F6）、`platform/linux/portal/`（RemoteDesktop/libei、ScreenCast、Screenshot、InputCapture、restore_token 生命周期、授权仪式）、`platform/linux/compositor/`（KWin / Mutter / wlroots 分流）、`platform/linux/xwayland/`（X11 客户端检测 + XTEST 兜底 + 坐标空间风险）、工具包差异矩阵落地（GTK3/GTK4/Qt5/Qt6/Chromium/Firefox/Java）、靶机应用 Linux 版（GTK4 + Qt6）、Adapter（gedit/LibreOffice/GIMP/Firefox）。

### 阶段 7　开放生态（201+，每项独立 ADR）

外部 MCP server 治理（签名、来源、能力声明、资源上限、schema 漂移检测、熔断）、插件市场（可永远不做）、WASM 纯计算技能、`PlatformAgentChannel`（Windows Agentic）、**无人值守**（白名单应用 + 独立会话 + 熔断 + 通知）、**交易闸门启用**（厂商官方接口 + 限额 + 双确认 + 冷静期 + 独立审计）。

---

## 4. 横切工作流（贯穿所有阶段，不单独占卡号但必须持续做）

| 工作流 | 频率 | 责任 |
|---|---|---|
| `MEMORY.md` 回填（FACT/PITFALL/REJECTED） | 每张卡 | Implementer |
| `LEDGER.md` 追加 | 每张卡 | Implementer |
| crate README 的**不变量**维护 | 改该 crate 时 | Implementer |
| ADR 起草与批准 | 契约变更时 | Orchestrator → 人类 |
| 对齐审计（gov §7.2） | 每阶段末 | Auditor（未参与实现的 agent） |
| 依赖登记 `docs/DEPENDENCIES.md` | 新增依赖时 | Implementer（需批准） |
| 性能预算实测（存储/树遍历/模型延迟） | 每阶段末 | Orchestrator |
| 安全回归（注入靶页 + L3 阻断 + DLP） | 每次 PR（CI） | 自动 |
| 开源准备（脱敏清单、许可证、CI 可见性） | 每阶段末检查 | Orchestrator |

---

## 5. 排期风险与缓冲

| 风险 | 缓冲策略 |
|---|---|
| 阶段 1a 基础设施卡（011~028）预估偏乐观 | 每批次预留 20% 缓冲；批次 A3 并行度降为 2 |
| 人类审阅成为瓶颈 | 单卡 diff ≤ 400 行；关键卡（022/024/028）逐行审阅；其余抽查 + review agent |
| Spike no-go 导致换目标 | 每个 Spike 都写明 no-go 后果与**替代目标**，避免停摆 |
| Paint 的坐标/视觉验证不达标 | 1b 可整体降级为"计算器 + 资源管理器"（纯 UIA/COM），把坐标与视觉推迟到阶段 2 |
| Edge 的注入靶页判据不通过 | **不允许带病推进**：先修设计（ADR），再开 1c 其余卡 |
| Adobe/Office 许可与登录阻碍 agent 自测 | 已把 Excel/PS 排在阶段 2/3；Spike 阶段用靶机应用模拟其行为（如"undo 栈被清空"） |

---

## 6. 开工前的最小就绪清单

- [ ] M5 许可证确定（阻塞 TASK-001）
- [ ] `docs/spec/` 七份契约草案完成（tool-schema、envelope、error-codes、capability-matrix、audit-event、ipc-protocol、naming）
- [ ] `docs/adr/0001~0015` 草稿完成并被批准（对应 `MEMORY.md` §3 的 15 条决策）
- [x] `plans/stage-0-spikes.md` 的 10 张卡展开为 `tasks/TASK-0NN-*.md`（ADR-0031，2026-09-18；零丢失核对见 `LEDGER.md`）
- [ ] Spike 环境就绪：Win11 24H2/25H2 机器（含双屏异缩放配置）、Edge/Chrome 当前版本、Accessibility Insights + Windows SDK `inspect.exe`、模型 API key（或本地模型）、Linux 双环境（Ubuntu 26.04 GNOME + Plasma 6 KWin，可为虚拟机）
- [ ] 人类审阅节奏确定（每天可审阅的 diff 行数上限 → 决定并行度）

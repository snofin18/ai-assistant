# pitfalls.md — 踩坑记录（PITFALL）

> 标签 `[PITFALL]` = 真实踩到过、会再次踩到的坑，**必须**带「因此怎么做」。
> 只属于某一个应用的坑进 `docs/memory/apps/<app>.md`；跨应用的才进本文件。
> 代码里的 `// PITFALL(app=…)` 注释由 `xtask check-comments` 汇总到这里（AGENTS.md §5.2）。

---

> **本文件是 `MEMORY.md` 分层结构（ADR-0021）的 L1 层之一。**
> 写入规则见 `MEMORY.md`「条目格式」与「更新职责」；路由规则见 `docs/memory/README.md`。
> **只追加，不改写他人条目**；更正用新条目 + `[supersedes:日期]` 标注。
> 应用专属的条目**不进本文件**，进 `docs/memory/apps/<app>.md`。

- [2026-09-16][PITFALL][src:v2 §9.2] `Ctrl+Z` 在终端类目标是 **SIGTSTP（挂起前台进程）**；撤销快捷键必须由 Adapter 声明，终端/远程会话目标禁用键盘 undo。
- [2026-09-16][PITFALL][src:v2 §9.2] 撤销会连带撤销**用户**在 Agent 之后做的改动 → 默认用"内容快照反向应用 diff"，而非"步数级 undo"。
- [2026-09-16][PITFALL][src:v2 §9.2] 保存后 undo 仍可执行但磁盘已变 → 区分"内存态可逆"与"持久态可逆"，文件写盘一律配影子副本。
- [2026-09-16][PITFALL][src:v2 §7.7] 点击"发送"后网络卡住 → Agent 误判失败并重试 → 发出两条。非幂等动作**验证前禁止重试**。
- [2026-09-16][PITFALL][src:v2 §13.2.6] 中文 IME 激活时模拟键盘输入会被吞或变拼音 → 文本写入优先用 `ValuePattern.SetValue` / `AXValue` / AT-SPI `EditableText`（不经 IME）。
- [2026-09-16][PITFALL][src:v2 §6.1] 虚拟化列表（UIA VirtualizedItems、Qt model/view、Web 虚拟列表）只实例化可见项 → 索引不可作唯一依据，需搜索或滚动加载。
- [2026-09-16][PITFALL][src:v2 §6.9] XWayland 应用的 AT-SPI 坐标可能是 X11 逻辑坐标，与 Wayland 输出坐标在分数缩放下不一致【待 Spike D 实测】→ 首次使用某显示器组合必须校准。
- [2026-09-16][PITFALL][src:feasibility P1] Win11 记事本"另存为"打开的是 **Shell 进程的对话框**，不属于记事本进程 → TargetDescriptor 必须支持跨进程解析。
- [2026-09-16][PITFALL][src:feasibility P1] 关闭未保存文档会弹三态对话框（保存/不保存/取消）→ 注册为 interrupt，**默认选"取消"**，禁止 Agent 自行选"不保存"。
- [2026-09-16][PITFALL][src:feasibility P2] Excel **Protected View**（网络来源文件）下 UI 树与 COM 行为都不同 → 必须探测；"启用编辑"是安全边界，需人工确认。
- [2026-09-16][PITFALL][src:feasibility P2] Excel COM 对象不显式释放会残留 `EXCEL.EXE` 进程 → Host 需要 COM 生命周期管理与看门狗。
- [2026-09-16][PITFALL][src:feasibility P2] `Range.Value` / `.Formula` / `.Text` 三者语义不同 → 断言必须指明读哪一个。
- [2026-09-16][PITFALL][src:feasibility P3] 画图**画布无子控件结构**，且抗锯齿导致像素不精确一致 → 断言必须用容差或感知哈希，不能逐像素相等；画布坐标 ≠ 屏幕坐标（受缩放与滚动影响）。
- [2026-09-16][PITFALL][src:feasibility P4] Photoshop 启动 10~30 s 且有首选项/登录/许可弹窗 → "进程存在"不等于"就绪"，必须做就绪探测。
- [2026-09-16][PITFALL][src:feasibility P5] Edge/Chrome 专用 profile **没有用户登录态** → 首次需用户手动登录；禁止复制 Cookie。
- [2026-09-16][PITFALL][src:v2 §13.4.7] Linux 锁屏会移除 libei 输入设备 → 长任务必须监听会话状态并暂停，解锁后重建会话。
- [2026-09-16][PITFALL][src:v2 §13.4.5] 用 Tier 1（重启应用注入环境变量）激活 a11y 会**丢失用户未保存工作** → 必须先做 dirty 检查并提示保存。
- [2026-09-16][PITFALL][src:v2 §13.4.6] Chromium/Electron 开启 a11y 会显著增加 CPU/内存，且 Web 树可达数千节点 → 必须裁剪，且只在需要时开启。
- [2026-09-16][PITFALL][src:v2 §5.5] 工具数 > 20~30 时模型选择准确率显著下降 → 按目标动态挂载 + 检索式工具选择。
- [2026-09-16][PITFALL][src:v2 §7.1] UIA/AT-SPI 全窗口树遍历可达秒级 → 限定 scope 与 depth、缓存、批量取属性、按需展开。
- [2026-09-16][PITFALL][src:gov §2.2] 代码先改而文档未跟上 → **下个会话的 agent 会照旧文档把新实现改回去**，形成来回震荡 → 契约必须先行。
- [2026-09-16][PITFALL][src:TASK-001] **构造函数丢弃参数**是最隐蔽的静默失败：`Report::new(command)` 曾把 `command` 置空，编译通过、测试也通过（因为测试自己又赋了一次值），只有报告标题一直空着。→ 凡是构造函数的参数没有被存进结构体，就要停下来问"这个参数是干什么的"。已修 + 加回归测试。
- [2026-09-16][PITFALL][src:TASK-001] 护栏工具会**先拦住自己**：`main.rs` 写到 644 行时被自己的 `file-too-long`（>600 告警）拦下。这是好事，但意味着写工具时要预留拆分成本 —— 一开始就按"IO 在边界、规则是纯函数"分层能省一次返工。
- [2026-09-16][PITFALL][src:TASK-001] 在**文档注释**里解释"哪些标签被禁止"时，直接写出标签字面量会被自己的规则判违规（`rustscan` 只抹字符串字面量，不抹注释）。当前靠改措辞绕过；是否豁免反引号引用待裁决（PL-004）。
- [2026-09-16][PITFALL][src:TASK-001] **PowerShell 数组扁平化会造成灾难性误替换**：`@( @('old','new') )` 会被扁平成两个元素，于是 `$pair` 变成字符串、`$pair[0]` 变成"第一个字符"，`Replace` 变成全局单字符替换（本会话真的把两个 md 文件里所有 `|` 替换成了空格）。→ 单 pair 必须写 `@( ,@($old,$new) )`；并给替换函数加三道防护：pair 必须是长度 2 的数组、锚点必须足够长、锚点必须唯一。
- [2026-09-16][PITFALL][src:TASK-001] PowerShell here-string 里 `'@` 必须**独占行首**，数组字面量中嵌套 here-string 会解析失败；`Split-Path -LiteralPath X -Parent` 与部分参数集冲突会报 null。→ 批量改文件用"一次一个 pair 的函数 + 逐个打印 OK/MISS"，别一次塞太多。
- [2026-09-17][PITFALL][src:探针实测] Codex automation 的 prompt **不是** user message，而是合成 `function_call_output`（`name=automation_update`、`namespace=codex_app`、`id=None`、`call_id=None`）。官方端点容忍，第三方严格端点 400。**两种表现要分清**：作为**历史重放** → `Invalid 'call_id': call_id is required`（落盘 id 是合法的 `fco_…`，可等长补丁修）；作为**首轮投递** → `Invalid 'id': … must start with 'msg_', got 'at_<uuid>'`（id 序列化请求时临时生成、从不落盘，**无法补丁**）。→ 换 provider 或改用 CLI，别试图修 rollout。
- [2026-09-17][PITFALL][src:探针实测] `disable_response_storage=true` + 一条畸形历史项 = **会话永久损坏**：每轮重放全量历史，毒项每次都被重发，该 thread 之后每次提问都失败，而新开 thread 完全正常。→ 症状是"只有这一个会话坏、别的都好"，别误判成账号 / 网络 / 模型问题。
- [2026-09-17][PITFALL][src:探针实测] 改 Codex rollout 必须**严格等字节长度**：`thread_history_1.sqlite` 的 `next_rollout_byte_offset` 记的是字节偏移，长度一变 UI 历史投影就错位。腾位办法：把对模型无用的 `,"namespace":"codex_app"`（24B）原地换成 `,"call_id":"hb_…"`。写完必须校验 `next_rollout_byte_offset == 文件实际大小`。写入用**原地 seek+write** 而非 temp+replace（Codex 运行时持有句柄，Windows 下 `os.replace` 会 PermissionError）。参考实现 `D:\csart\fix_codex_callid.py`。
- [2026-09-17][PITFALL][src:探针实测] Codex automation 调度有约 **+2 分钟 jitter**（`~/.codex/automations/.run-jitter-salt`）：约定 11:16 实际 11:17:54 触发。→ 定时间表要留余量；外部调度（任务计划程序 + `codex exec`）无 jitter。
- [2026-09-17][PITFALL][src:探针实测] `automation_update` 建 cron 缺必填项时只回 `Failed to create automation.`，**不说是哪个字段**（本次因此误判成"cron 需 ChatGPT 鉴权、本环境不可用"，白烧一晚）。必填：`name`/`prompt`/`rrule`/`status`/`projectId`/`model`/`reasoningEffort`/`executionEnvironment="local"`。→ 探 schema 的办法是故意只传 `kind`，让校验器把缺失字段一次性全列出来。
- [2026-09-17][PITFALL][src:探针实测] 上下文压缩会把补丁过的自动化残留报成 `Orphan function call output for call id: hb_…`（ERROR 级，来自 `run_auto_compact`）。这是**非致命噪音**：Codex 记录后丢弃该项、turn 照常继续。→ 看到它不等于出错，但等于"这个会话里有补丁过的 automation 残留"。
- [2026-09-17][PITFALL][src:本机实测] **Codex 桌面应用会自动重写 `config.toml`**（把 `notify` 数组展开成多行、增删 `[projects.*]`、写 `[shell_environment_policy]`、改 pipe 名等），重写过程中**可能丢失非标准键** —— 本次就丢了 `model_catalog_json`，直接导致上下文窗口缩水 72%。→ 每次改完 config 或发现窗口/模型行为异常，先 `Compare-Object` 与备份 diff 一遍，确认关键键还在；改动前先做 `.bak`。
- [2026-09-17][PITFALL][src:本机实测] `config.toml` 里 `model_context_window` 写得再大也**不一定生效**：桌面 app 路径以模型元数据（catalog 或 fallback）为准。→ 判断真实可用窗口不要看 config，要看 rollout 里 `task_started.model_context_window` 的实测值。
- [2026-09-17][PITFALL][src:本机实测] CI 用 `EmbarkStudios/cargo-deny-action@v1`（该 action 最新已是 **v2.1.1**）且**未钉 cargo-deny 版本**，本地装的是 0.20.2 —— 两边版本不同会让"本地红、CI 绿"或反之。本次 `db-path` 数组写法就是被 CI 掩盖的。→ 建议 CI 显式钉版本，并把 deny.toml 能否解析纳入本地验收（PL-016）。
- [2026-09-17][PITFALL][src:本机实测] `git remote add` 报 `fatal: not a git repository` 与网络 / 代理 / 账号 / 仓库存不存在**完全无关** —— 它是**纯本地**操作，报错只因 cwd 不在仓库内（本次在 `C:\Windows\System32` 跑的，`Test-Path C:\Windows\System32\.git` = False；仓库在 `D:\csart\ai-assistant`）。→ 任何 git 命令前先 `git rev-parse --show-toplevel` 确认。同批踩到的另两个坑：① 从文档复制命令时**把占位符的尖括号一起打进去**（`git@github.com:<账号>/...`），git 会把尖括号当字面 URL 存下来；② 本机 `~/.ssh` 只有 `known_hosts`、**没有密钥对**，且 global 配的 `http.https://github.com/.proxy` **只对 HTTP/HTTPS 生效、不覆盖 SSH 的 22 端口** → 一律走 **HTTPS**，不要走 SSH。
- [2026-09-17][PITFALL][src:本机实测] **在非交互会话里跑 `git ls-remote` / `git push` 会挂死**：Git Credential Manager 被调起后等待 GUI/浏览器授权，命令行侧既不返回也不报错（本次挂 40s+，只能 `Stop-Process -Name git,git-credential-manager,git-remote-https -Force` 清掉）。→ 自动化脚本必须先设 `GIT_TERMINAL_PROMPT=0` 并加超时；**首次 push 交人类在自己的终端里跑**。补充事实：本机 Windows 凭据管理器里**已有** `git:https://github.com` 条目（`cmdkey /list` 可见），所以挂住更可能是 GCM 的 UI 交互流程而非"没有凭据"。
- [2026-09-17][PITFALL][src:本机实测 cargo-deny 0.20.2] cargo-deny 的 CLI 有两个反直觉点：① **`-c` 是 `--color` 而不是 `--config`** —— 写 `cargo deny -c x.toml` 会得到 `invalid value 'x.toml' for '--color <COLOR>'`（exit 2），必须写全称 `--config`；② `cargo deny check` **不接受 `--offline`**（会提示 `to pass '--offline' as a value, use '-- --offline'`，exit 2），但 `check licenses bans sources` 本身**不需要网络**（只有 `advisories` 要 clone advisory-db）→ 离线自检直接去掉该 flag 即可。另注：正向检查会打印一条 **warning** `unmatched license allowance: BSD-3-Clause`（白名单里有但当前无依赖使用），**不影响 exit 0**，不要误判为失败。
- [2026-09-17][PITFALL][src:本机实测] **`HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion` 的 `ProductName` 在 Windows 11 上仍谎报 "Windows 10 Pro"**（本机实际 25H2 / build 26200.9457，`ProductName` 却返回 `Windows 10 Pro`）。→ 判定 Windows 版本**必须用 `DisplayVersion` + `CurrentBuild`（+ `UBR`）**；`ProductName` 与 `[Environment]::OSVersion` 都不可靠。这条对本项目是**产品级**陷阱：架构 v2 的平台基线是「Windows 11 24H2+」，若拿 `ProductName` 做能力探测会把 25H2 误判成 Win10 并错误降级功能。
- [2026-09-17][PITFALL][src:本机实测，**supersedes:2026-09-17** 上文「非交互会话里 `git ls-remote`/`git push` 会挂死」那条的推断部分] 挂住的**真因已确认**：当时远端仓库**还不存在**，GitHub 对不存在仓库的 `info/refs` 返回 **401**（不是 404，以免泄露仓库是否存在）→ GCM 转入交互式授权并等待 GUI，命令行侧永久阻塞。仓库建好后，**同一条 `git push` 在 `GIT_TERMINAL_PROMPT=0` + `GCM_INTERACTION=never` 下 3 秒内 exit 0**，凭据取自 Windows 凭据管理器既有的 `git:https://github.com` 条目。→ 结论修正：**不是「非交互会话一定挂」，而是「遇到 401 一定挂」**。自动化脚本除设这两个变量外，应先 `curl https://api.github.com/repos/<owner>/<repo>` 确认 200 再动 git。
- [2026-09-17][PITFALL][src:仓库配置实测] **`spikes/` 被 workspace `exclude`（`Cargo.toml`：`exclude = ["spikes", "fixtures/apps", "tools"]`）→ deny / fmt / clippy / test 四道硬门禁全都覆盖不到 spike 代码**。阶段 0 这是设计意图，但意味着 spike 里加依赖只有政策约束、没有机器约束（PL-019）。→ spike 卡开工前先确认依赖已写进 `docs/DEPENDENCIES.md`；TASK-002 要用的 `windows` crate 已在「计划首批」清单里，但 **`uiautomation` 不在**，需补行并走批准。
- [2026-09-17][PITFALL][src:本机实测] **仓库里有文件的末尾缺换行**（本次发现 `docs/DEPENDENCIES.md` 末行无 `\n`）。后果：任何以「整行 + `\n`」为锚点的批量编辑脚本都会 `count==0` 而断言失败，且失败信息与真实原因（缺换行）完全无关，极易误判成"锚点写错了"。→ 已补齐该文件末尾换行；建议把「文本文件必须以单个 `\n` 结尾」做成 `xtask hygiene` 规则（与 PL-011 的 CRLF 规则同属 gov §5.4 家族，可一并裁决）。
- [2026-09-17][PITFALL][src:Spike A 先导实测] **记事本的 `ValuePattern` / `TextPattern` 返回的文本用裸 `\r`（CR）分行，而磁盘文件是 `\r\n`（CRLF）** —— 62 字符的 CRLF 文件读回来只有 59 字符。→ 任何「写入后读回校验」必须先把 `\r\n`、`\r`、`\n` **三者全部**归一化，否则必然假阴性；只做 `Replace("\r\n","\n")` **不够**，还要再 `Replace("\r","\n")`。第一次探针就因此得到 `value == file content ? False` 的错误结论。
- [2026-09-17][PITFALL][src:Spike A 先导实测] **打开一个「已经在标签页里」的文件，记事本不会重新加载磁盘内容**（只切到那个标签页）→ 探针读到旧内容、产生假阴性。→ 探针必须**先杀干净所有 Notepad 进程**并使用**带时间戳的唯一文件名**。这条对产品同样关键：外部程序改了 `.txt` 之后，记事本内存态**不会自动更新** → 架构里的「文件契约（L1）」与「UI 态（L3）」可能不一致，撤销/校验逻辑必须显式处理这个偏差。
- [2026-09-17][PITFALL][src:Spike A 先导实测] **`SetValue` 走 UIA Pattern、不经过键盘，因此 IME 开/关对它没有任何影响** → 卡面步骤 5「中文经 `SetValue` 写入，分别在 IME 开/关两态测」对 `SetValue` 路径是**空操作**；IME 两态对照真正针对的是 **L4 合成键盘输入**。→ 已记为对卡面措辞的疑问（`docs/spike-reports/SPIKE-A.md` §5.3），由 TASK-002 正式会话记 `DRIFT-002-x` 交裁决，**本次侦察未改卡面**。
- [2026-09-17][PITFALL][src:Spike A 先导实测] **PowerShell 控制台输出中文会乱码**（控制台代码页问题），但**进程内的字符串比较不受影响**。→ 判定一律以脚本内的 `True/False` 与长度为准，不要用肉眼读控制台就断定"中文被写坏了"（本次差点因此误判 `SetValue` 破坏了中文）。同类问题也会让 Python 脚本 `print` 中文时抛 `UnicodeEncodeError: 'gbk' codec`——需要输出中文时改为写文件再读，或 `sys.stdout.reconfigure(encoding='utf-8')`。

---

---

## 2026-09-18 追加：`windows` crate / UIA COM 胶水（跨应用通用）

- [2026-09-18][PITFALL][src:`uia_dep_proof` 首次运行失败] **`IUIAutomationElement::FindFirst` 的「没找到」= `Err(HRESULT(0x00000000))`。** UIA 用 `S_OK` + **NULL 元素指针**表示无匹配，`windows`-rs 把它转成 `Err`，而这个 error 的 `code` 恰恰是「成功」→ 直接 `?` 会得到一条 `message: "操作成功完成。"` 的**荒谬错误**；按「HRESULT 是不是错误码」去分支则会**判反**。→ **必须显式把 `code == 0` 映射成 `None`，其余 HRESULT 原样上抛**；产品侧对应 `ErrorCode::TargetNotFound`，且必须与「selector 写错」「目标已消失」可区分（AGENTS.md 铁律 1）。同一模式适用于所有"返回 S_OK + NULL 表示无结果"的 UIA/COM 方法。
- [2026-09-18][PITFALL][src:本机 `cargo deny` 实测 exit 4] **spike crate 自己的 `Cargo.toml` 必须有 `license` 字段**，否则 `cargo deny check licenses` 报 `error[unlicensed]: <crate> is unlicensed`。这与「依赖的许可证」**无关**，是**被检查的包自身**没声明 → 新建任何 `Cargo.toml` 时照抄 `license = "MIT"`（与仓库根 `LICENSE`、根 `Cargo.toml` 的 `[workspace.package].license` 一致）。已建议 TASK-015 做成 `xtask hygiene` 规则。
- [2026-09-18][PITFALL][src:`uia_dep_proof` 首次运行"无任何输出"] **启动"转交型 stub"进程必须 `Stdio::null()`。** `notepad.exe` 只是把文件转交给打包进程然后自己退出；若它继承了父进程的 stdout 句柄，**任何"等 EOF"的管道会在 `main` 返回后一直挂着** —— 表现为"程序跑完了却一个字都看不到"。→ 凡是用 `Command::spawn` 启动 GUI/stub 进程，一律显式 `stdin/stdout/stderr` 全 `Stdio::null()`。同类风险：任何 MSIX 打包应用的启动器。
- [2026-09-18][PITFALL][src:`windows` 0.62.2 编译报错逐条实测] **`windows` 0.62 的签名细节（照抄会踩，共 5 处）**：① **`BOOL` 已移到 `windows::core`**，写 `Win32::Foundation::BOOL` 报 `no BOOL in Win32::Foundation`；② `OpenProcess` 的 `binherithandle` 是 **`bool`**（不是 `BOOL`）；③ `QueryFullProcessImageNameW` 收 **`PWSTR`**（不是 `PCWSTR`）；④ `PostMessageW` 收 **`Option<HWND>` + `WPARAM`/`LPARAM`**（不是 `hwnd, msg, None, None`）；⑤ **edition 2024 下 `unsafe fn` 体内仍需显式 `unsafe {}` 块**（`unsafe_op_in_unsafe_fn`）。→ 引入 `windows` crate 时先跑最小可编译样例，不要凭记忆写签名。
- [2026-09-18][PITFALL][src:ADR-0024 D4，probe-03 实测] **PowerShell 5.1 会把无 BOM 的 `.ps1` 按 GBK 解码** → 报错**行号错乱**、反引号转义 `` `r `` **被吞**，故障现象与真因完全无关。→ **仓库里的 `.ps1` 一律纯 ASCII**（中文说明放 `.md`）；判定看脚本内的 `True/False` 与写出的结果文件，不看控制台。（`.rs` 不受此限，Rust 源码默认 UTF-8。）
- [2026-09-18][PITFALL][src:probe-04 编写过程] **PowerShell 的单引号 here-string `@'...'@` 内部不可再出现 `@'...'@` 或以 `'@` 开头的行** → 会提前终止 here-string，产生难以理解的解析错误。写多行脚本内容时改用 Python 生成文件（`newline="\n"` + `UTF8Encoding($false)` 等价物）。

## 2026-09-18 追加：探针结果的"预期 False"（不要误读成失败）

- [2026-09-18][PITFALL][src:probe-04 `RESULT-04.txt` 复核] **`cjk_survived_on_disk=False` 在 probe-04 的 5 个用例里有 4 个是"预期结果"，不是失败** —— 那 4 个用例的输入是**纯 ASCII**（为绕开上一条的 PS 编码坑而刻意如此），输入里没有 CJK，自然"没存活"。CJK 无损的真实证据是第 1 个用例（True）与 `uia_dep_proof` E4（`cjk_kept=true`，Rust 路径独立复现）。→ **通用教训**：断言的 False 必须结合"输入里到底有没有被测对象"来读；探针输出里应同时打印**输入侧的计数**（本例 `write_input CR/LF chars` 就是靠它才没误判）。

## 2026-09-18 追加：非 ASCII 的 `.ps1` 不只是乱码，它会把测量变成**假的**

- [2026-09-18][PITFALL][src:`probe-03` 改纯 ASCII 前后对比实测] **脚本编码坑的真正危险不是「看不懂」，而是「看起来全对」。** 同一个 CJK 用例，脚本含中文字面量时跑出 `disk_chars=14 / disk_bytes=34 / disk_eol=[code10=2 code13=1]`（理论值应为 10 / 22 / code10=2 code13=2），而 `norm_eq` 仍然 **True** —— 因为**写入的与比较的是同一个被糟蹋的字符串**。把脚本改为纯 ASCII + 用 `[char]0x4E2D` 等**码点**拼接样本后，数值回到理论值。→ **两条可复用的规则**：① 测试样本里的非 ASCII 字符**一律用码点构造**，不写字面量；② 断言**不得只比较「两边相等」**，必须同时断言**绝对量**（字符数/字节数/控制符计数）—— 绝对量才能揭穿「两边同样错」。
- [2026-09-18][PITFALL][src:全仓扫描] **文档里写的产物路径与脚本实际路径不一致**：`README` 与 `SPIKE-A.md` 写的是 `%TEMP%\eol-probe\`，而 probe-03/04 实际写到 `D:\csart\eol-probe\`。后果是**读者按文档找不到证据**，于是证据等于不存在。→ 已修正两处文档；规则：**凡在文档里引用产物路径，必须从脚本里拷贝而不是凭记忆写**。更好的做法：把路径定义成脚本参数并在输出里回打（probe-03 已回打 `report written: <path>`）。

## 2026-09-18 追加：机器校验文档时，「给人看的说明文字」会被解析器当成数据

- [2026-09-18][PITFALL][src:本轮修 ADR 登记表时 `xtask adr-index` 连续两次误报，实测取证] **单行标记类锚点（`Superseded by：` / `已退役编号：` / `下一个可用编号`）之后不能出现无关的 4 位编号**，否则解析器会把它当成值。两个真实现场：① ADR-0026 的状态行写 `Superseded by：—（D1 的「登记表由人工维护」已由 ADR-0030 改为机器校验）` → 解析出 `superseded_by = Some(30)` → 误报 `adr/superseded-not-marked`（登记表状态列没标 Superseded）；② `**下一个可用编号：0031**（= 已用最大号 0030 + 1）` → 报「写的是 Some(30)」。→ 处置：① 状态行的 `Superseded by：` 字段**必须行尾即止**，解释另起一段；② `first_adr_number` 的缺陷已修（见下条）；③ 写「下一个可用编号」那一行时**仍建议只出现一个 4 位编号**（工具按文本顺序取第一个，但少一个歧义源总是更好）。
- [2026-09-18][PITFALL][src:`xtask/src/adr_registry.rs` 代码与文档注释不符，本轮实测发现并修复] **`first_adr_number` 曾经返回「最小的」而不是「第一个」**：它的实现是 `all_adr_numbers(text).into_iter().next()`，而 `all_adr_numbers` 会 `sort_unstable()` + `dedup()`（退役清单要的是**集合**语义）。文档注释却写着「取一段文本里第一个 `NNNN`」—— 这是典型的「注释与代码矛盾」，而 AGENTS.md §5.2 明令禁止。→ 已拆成两个函数：`four_digit_numbers_in_text_order`（顺序语义，唯一扫描实现）与 `all_adr_numbers`（= 前者的升序去重版），`first_adr_number` 改用前者；新增 3 条单测钉住行为（`0031（= 0030 + 1）` → 31；`Superseded by：ADR-0029（另见 ADR-0018）` → 29；顺序+重复保持）。**教训**：一个「顺手复用」的辅助函数会把两种语义悄悄合并，而这类缺陷只有当文档里出现第二个编号时才暴露。
- [2026-09-18][PITFALL][src:本轮用 `#[path]` 外置测试时编译失败，实测] **`#[path = "x_tests.rs"] mod tests;` 的外置文件里不能再写模块的收尾 `}`**。该文件的内容**就是**模块体（不是 `mod tests { … }` 花括号**内部**的东西），把原来的 `#[cfg(test)] mod tests { … }` 整块搬过去时，最后那个 `}` 会变成 `error: unexpected closing delimiter`。另外两点：搬过去的 `use super::*;` 要保留（它仍是父模块的**私有子模块**，可见性语义不变，这与 `tests/` 目录下的集成测试有本质区别）；文件顶部的 lint 豁免要用**内属性**形式 `#![allow(...)]`，且必须写在所有 item 之前、`//!` 文档注释之后。搬完**必须跑 `cargo fmt --all`**：逐字搬运会留下 4 空格的多余缩进。
- [2026-09-18][PITFALL][src:本轮工具调用日志，多次复现] **agent 的工具调用会被环境偶发重复派发**（本轮同一个 `node_repl` 调用被复制 2~3 次、`exec_command` 被复制 1 次）。因此**所有文件编辑必须幂等**：写入前先查「目标内容是否已存在」。反例（本轮真实事故）：一个「删掉最后一次出现的重复块」的函数在重复派发下跑了两次，第二次把**合法的那一份**也删了 → 只能重新插入；正例：`skip(exists)` / `skip(done)` / `skip(new-present)` 三种前置检查吸收了其余全部重复派发。→ 结论：**「删」类操作要设计成「仅当重复度 ≥2 时才删一份」**，而不是无条件删。
- [2026-09-18][PITFALL][src:PL-031 修复过程实测] **表格单元格里的裸竖线即使在代码跨度（反引号）里也会破表**，GFM 要求写成 `\|`。更阴的是：**转义之后才会暴露出「列数不对」这个第二层缺陷** —— `docs/PARKING_LOT.md` 的 PL-005 处置行原本有 9 个裸竖线，把一行撕成 11 格；转义后剩下 4 格，才看出它比表头多出 1 格（末尾多了一个 `开源准备` 单元格）。→ 因此修破表的**复扫判据**必须是「按未转义竖线切分的单元格数 ↔ 该表分隔行的列数」逐表比对，**不能只 grep 竖线**。本轮全仓 45 个 `.md` 复扫结果：破表 **0** 处、`---` 前缺空行（setext 风险）**0** 处。
- [2026-09-18][PITFALL][src:本轮用 PowerShell here-string 落地含中文的脚本，实测对比] **中文经 `python -c "…"` 的命令行参数会被控制台代码页毁掉**（本机 `Get-Content` 显示为乱码、比较结果 MISMATCH），而**同一段中文写进 `.py` 文件再由 python 读取是正确的**（字节级核对 UTF-8 无误）。→ 处置：含中文的批处理逻辑**写进脚本文件**再执行，不要用 `-c` 内联；或者改用 `node_repl`（本轮全部文档编辑都走它，中文逐字校验通过）。判断「是文件坏了还是显示坏了」的唯一可靠方法是**看字节**，不是看终端回显。

- [2026-09-19][PITFALL][src:TASK-051 cherry-pick 10f78db 取回] **任何 task commit 的标题、commit message 或工作树改动必须**对应**一张已存在的 `tasks/TASK-NNN-<slug>.md` 文件**；**禁止**「sub-card 后缀」（`NNNb` / `NNNc` 等）—— ADR-0031「一卡一文件、按号寻卡」是项目的卡片编号契约。**禁止**先 commit 代码后补卡（即便「WIP」也不行）：10f78db 是真实事故，原 commit 自报「TASK-015b」是 stage-1 TASK-015 的 sub-card 臆造，导致工作树与 `tasks/TASK-015-...md`（stage-1 1a 批次 A1 的另一张卡）错位、后来被 reset 移出 HEAD 链。**禁止**用「`DRIFT-NNNN-N`」直接覆盖尚未建卡的实质工作。`xtask card-check` 判据②（status 非 Ready 必有文件 = PL-002）是**机器化防线**，本卡实施时已就绪但仍依赖人写卡；未机器化前，写卡前的 cron 必须先用 `xtask guard acquire` 锁卡号再开始工作。
- [2026-09-21][PITFALL][src:TASK-075 B1.4 probe-07 修订] **spikes/ 探针里 CJK 字符串必须用 `[char]0x4E2D` 构造，不能用字面 `'中文'`** —— ADR-0024 D4 "仓库内 .ps1 一律纯 ASCII" 不允许非 ASCII source bytes，但 PowerShell 运行时需要 CJK 字符串（如 `文件` / `另存为`）匹配 Win11 现代化 UI 的中文菜单名。正确做法：声明常量 `$CN_FILE = [char]0x6587 + [char]0x4EF6`，运行时构造 source 仍 0 non-ASCII。**踩坑路径**：probe-07 第一版含 `'文件' / '另存为'` 字面 → docscan 报 30 non-ASCII → 全部改为 `[char]0x...` 构造 → 0 non-ASCII。**类比**：probe-04 的 `[char]0x4E2D + [char]0x4FDD` 构造 `你`/`好` 测试数据（PL-026 已记录）；本卡把同样模式扩展到 UIA element name 匹配。**未来 spikes/ 探针规则**：涉及中英双语 UI 元素名匹配时，**永远用 `[char]0x...` 构造 CJK 常量 + 英文常量并列**，避免 source 编码违规

- [2026-09-21][FACT][src:TASK-076 B1.5 probe-08, 本机实测 2026-09-21 11:03] **probe-08 failure injection 设计存在 3 个探测问题（已记录，不是平台 bug）** = process_killed 100% 通过（真正的有效数据），但 minimized / other_desktop / unsaved_dialog 有探测脚本缺陷：
  - **minimized `setup=False`**：IsWindowVisible 对最小化窗口**仍然返回 True**（Windows API 设计：最小化 = hidden state 但 visible flag 没清零）。**正确检测**：用 `IsIconic(hwnd)` 而不是 `IsWindowVisible`
  - **other_desktop `setup=False`**：CreateDesktop 需要完整的 window station 权限链（OpenWindowStation + SetProcessWindowStation + CreateDesktop + 退出时 CloseDesktop + 还原 SetThreadDesktop）。PowerShell 默认进程没绑 window station = CreateDesktop 返回 0。**修复**：先 `SetProcessWindowStation(OpenWindowStation("WinSta0"))` 才能 CreateDesktop 成功
  - **unsaved_dialog `setup=False`**：我猜的 dialog 标题 pattern（'Save changes' / '未保存' / '未儲存' / '儲存變更' 等）**没有匹配 Win11 25H2 实际 dialog 标题**。实际标题可能是 "Notepad" 或包含应用名但不包含 "save changes" 字样。**修复**：触发关闭后用 `WaitForWindowClass('#32770', 5s)` + 任意可见 dialog 标题计数 + 找含 '?' 或 '_' 的 dialog（Win11 unsaved 经常用文字 'Want to save?' 或类似）
  - **3 个问题都是探测脚本设计 bug，**不**是 Adapter 实际能/不能恢复**。**process_killed 子测试 = 真正数据 = 100% 通过**（kill notepad → 探针正确检测窗口消失，不崩溃）

- [2026-09-22][FACT][supersedes:2026-09-21 的 probe-08 3 探测 bug 条目] probe-08 B1.5 第二轮实测 (TASK-084 v2, 2026-09-22, 本机实测 iter=12 warmup=2) 结果：
  - process_killed 3/12 = 100% (probe 在窗口丢失时正确处理)
  - minimized    3/12 = 100% (Bug #1 修好: `IsIconic` 替换 `IsWindowVisible` -- 后者对最小化窗口仍返回 True)
  - other_desktop 0/12 (Bug #2 探测脚本已修: window-station dance = OpenWindowStation("WinSta0") + SetProcessWindowStation + CreateDesktop; **但 Win11 25H2 仍拒绝 = ERROR_NOT_ENOUGH_MEMORY (8)** = 平台级安全策略禁止普通进程创建桌面 -- **不算 Adapter bug**)
  - unsaved_dialog setup 0/12 / state 3/12 (Bug #3 探测脚本已修: 用 Win32 class #32770 检测 dialog 而非 title-pattern -- **但 Notepad 没弹 unsaved-changes dialog** = DirectUI Edit RichEditD2DPT 在 UIA ValuePattern 上 Unsupported Pattern -- 与 probe-07 set_filename 失败同源 -- **不算 Adapter bug**)
  - **结论**: 4 scenario 中 **process_killed + minimized = 100% 真实 platform capability 验证**; other_desktop + unsaved_dialog 在 Win11 25H2 modern Notepad 上 = 平台限制 (CreateDesktop 沙箱 + UIA1 ValuePattern 在 DirectUI 上不支持) -- stage-1 Adapter 设计时需考虑

- [2026-09-22][FACT][src:TASK-084 v3 + DRIFT-PROBE-08-ATTACH-FAILED] probe-08 unsaved_dialog v3 ATTEMPTED (TASK-084, iter=4 warmup=0): AttachThreadInput + BlockInput + SetForegroundWindow + correct [uint] GetWindowThreadProcessId signature ALL attempted. Result: AttachThreadInput returned FALSE (LastError=203 ERROR_ENVVAR_NOT_FOUND, which appears to be a stale Win32 error not the real failure cause) and SendKeys STILL did not trigger Notepad dirty state in background PS process. **Conclusion: Win11 25H2 modern Notepad (packaged UWP app) does NOT accept keyboard input from a background PowerShell process, regardless of AttachThreadInput.** The save-changes dialog ONLY appears when a user with a real foreground session actively interacts with the notepad window (clicking X, Alt+F4, or simulating real keyboard input via the test user's interactive session). This means probe-08 unsaved_dialog is **fundamentally not measurable in automated background runs** - the scenario can be observed interactively (Phase 1.3 user test confirmed Save dialog DOES appear) but the probe can only detect it, never trigger it. **Supersedes: TASK-084 v2's "Win11 25H2 DirectUI Edit ValuePattern UIA1 平台限制" entry** - the actual root cause is broader: ALL programmatic input paths (UIA SetValue, UIA SendKeys via attached foreground, Win32 SendInput) are blocked in this UWP context for non-interactive PowerShell processes. stage-1 Notepad Adapter design MUST handle this: any write path must go through Rust COM via windows crate (which makes real process-level calls, not subject to UIPI from background PS).

- [2026-09-22][FACT][src:interactive user session, real Win11 25H2 25H2 modern Notepad, 4 isolated test rounds] **Win11 25H2 modern Notepad (packaged UWP app, RichEditD2DPT) interactive behavior systematically characterized via 4 isolated tests**. This entry supersedes BOTH the 2026-09-21 3-bug entry AND the 2026-09-22 v2 entry AND the 2026-09-22 v3 entry, and represents the FINAL verified understanding:

  **TEST 1** (SetForegroundWindow + SendKeys 'z'):
    - SetForegroundWindow returned True; GetForegroundWindow matched target hwnd
    - Title got `*` (dirty=True) BUT file content on disk was UNCHANGED ('original' after test)
    - **CORRECTION: `*` is NOT from SendKeys** - it's a side-effect of focus change in Notepad. SendKeys did NOT actually deliver 'z' to the document control.
    - User reported: "光标没动, no 'z' 字符 appeared" - this is CORRECT.

  **TEST 1b** (Test 1 + verify disk content):
    - SendKeys('z') did NOT modify the document on disk. file content = 'original' before and after.

  **TEST 1d** (isolation of SetFG vs SendKeys):
    - only-SetFG: dirty=False (SetFG alone does NOT trigger dirty)
    - only-SendKeys: dirty=True (SendKeys alone does set dirty) BUT file content unchanged
    - both: dirty=True BUT file content unchanged
    - Conclusion: SendKeys sets dirty flag but does not modify document.

  **TEST 1g** (Win32 keybd_event with forced SetFocus):
    - UIA doc.SetFocus() returns success but GetFocus() returns 0x0 (focus not actually on document)
    - After keybd_event(VK_Z): UIA reads 'zoriginal' (z IS in document!) BUT disk still 'original'
    - Conclusion: keybd_event CAN put 'z' into document via direct kernel input queue. But disk still not saved (Notepad doesn't auto-save).

  **TEST 2** (ValuePattern.SetValue):
    - UIA ValuePattern returns new value at +0ms
    - title stays dirty=False for 3 seconds (Notepad never marks dirty)
    - file on disk unchanged
    - Conclusion: ValuePattern.SetValue modifies UIA's cached view, but Notepad itself is unaware.

  **TEST 3** (AttachThreadInput + BlockInput):
    - AttachThreadInput returned False, LastError=87 (ERROR_INVALID_PARAMETER) initially; with kernel32 SetLastError cleared, LastError=87 persists.
    - BlockInput(true) returned False, LastError=0 (no input desktop to block).
    - Conclusion: Neither function works in this PS context.

  **TEST 4** (GetWindowThreadProcessId validation):
    - GetWindowThreadProcessId(hwnd, [IntPtr]::Zero) returns 18704
    - GetWindowThreadProcessId(hwnd, [ref]uint) returns SAME value 18704, out param = 16232 (which is a PID, NOT in notepad's thread list, but 18704 IS in notepad's thread list)
    - OpenThread(18704) succeeded (thread is alive)
    - AttachThreadInput(0, 18704, true) STILL returned False LastError=87
    - Conclusion: thread ID retrieval works correctly. The AttachThreadInput failure is NOT due to wrong thread ID - it's a session/window-station isolation issue.

  **OVERALL CONCLUSION for stage-1 Notepad Adapter design:**

  | Path | Background PS works? | Why |
  |------|---------------------|-----|
  | UIA ValuePattern.SetValue | NO (UIA cache only, Notepad unaware) | UIA abstract layer doesn't trigger Edit control dirty |
  | UIA SendKeys | NO (depends on text-control focus) | SetFocus fails in background PS context |
  | UIA SetForegroundWindow | YES (sets foreground) | Does NOT actually deliver input |
  | UIA WindowPattern.Close | NO (background PS noop) | Standard behavior |
  | Win32 AttachThreadInput | NO (LastError=87) | Session/window-station isolation in Win11 25H2 |
  | Win32 BlockInput | NO (no input desktop) | Background PS has no input desktop |
  | Win32 keybd_event | **YES** (Test 1g) | Direct kernel input queue injection, bypasses focus |
  | Win32 GetWindowThreadProcessId | YES (Test 4) | Returns correct thread ID |

  **PRACTICAL CONCLUSION**: In a background PowerShell process, the ONLY reliable way to inject characters into a Win11 25H2 modern Notepad document is via Win32 keybd_event (or SendInput API). All UIA-based approaches fail because UIA's SetFocus on RichEditD2DPT doesn't actually establish focus in this context. stage-1 Notepad Adapter **MUST use Rust COM via windows crate** (which makes real process-level calls) for any write path; background PS process can only DETECT dialogs and use keybd_event (limited).

  **probe-08 unsaved_dialog scenario is fundamentally unmeasurable in automated background runs** - the scenario works in interactive sessions (Phase 1.3 user confirmed Save dialog appears on X-click) but no background PS mechanism can trigger it.

- [2026-09-23][PITFALL][src:TASK-103 二审发现] **module-level `#![allow(clippy::all, ...)]` 是灰区违反 ADR-0035 精神**（虽然 §决策 1 严格说禁止的是 workspace-level exceptions）。正确路径是 per-line `#[allow(...)]` + 注释 + 任务卡 §5 登记（ADR-0035 §决策 2 次选）。新代码（如 `xtask/src/render.rs`、`xtask/src/serde_json_lite.rs`）用 file-level allow 但需要简短指出 原因（pedagantic 拒绝 = "未来如 果 要重命名 allow 要记 录原因"，不能用 sledgehammer `clippy::all` 偷懒）。**xtask zero-deps 政策**（per `docs/DEPENDENCIES.md`）让 `serde_json::from_str` 不可 用 = 必须自写 `serde_json_lite` zero-dep 解析（约 100 行代码）。新代码遵循这 条 是 **TASK-103 复审 必 须记 录的偏 差**。

- [2026-09-23][PITFALL][src:TASK-011 修订 335e1ad] **「`--check` 类门禁」有两条互相独立的假绿通道，必须同时堵**：(a) 选项没进 `cli.rs::BOOLEAN_FLAGS` → 解析判「未知选项」**exit 2**（不是 0，但 CI 里若配 `continue-on-error` 或人只扫日志尾部就会误判成通过）；(b) `main.rs` 把子命令的 `Ok(_)` 一律映射成 `EXIT_OK` → 子命令内部即使 `Ok(1)` 也永远 exit 0，**drift 永不阻塞合并**。修法 = 布尔开关进表 + `Ok(exit_code) => Ok(exit_code)` 原样透传 + 一条负向验证。教训：**「检查命令退出 0」≠「检查通过」**，要验证的是「该红的时候真的红」（ADR-0019 元门禁）。

- [2026-09-23][PITFALL][src:TASK-011 复核实测] **自写 JSON 解析器用 `input: &[u8]` + `out.push(b as char)` 会把非 ASCII 静默变成 mojibake**：UTF-8 多字节序列被逐字节按 Latin-1 解释（实测：schema 里 `中文§` → 生成物里 `ä¸æÂ§`），而 `xtask codegen` **仍 exit 0**（渲染是纯函数，`--check` 拿同一渲染器比对，故自证也发现不了）。触发条件 = 任何**会被渲染进生成物**的字段（`message_for_model` / `message_for_user` / `hint` / `evidence_ref`）含非 ASCII；当前 5 份 schema 的非 ASCII 只出现在不参与渲染的 `description` 里，故尚未爆发。**已在本轮修复**（`xtask/src/serde_json_lite.rs`：按 UTF-8 字符宽度整体解码 + 9 条解析器单测；实测 `codegen` 现在把 `中文§` **原样**写进生成物，且 `cargo fmt --check` 仍 0 diff、`codegen --check` 仍 0 drift）。**同一解析器还不支持 `\uXXXX`**（实测报 `unknown escape \u` → verify-schemas exit 1 / codegen exit 4），且 `parse_string` 的错误信息 `expected '\''{}'\''` 渲染出来是双引号 `''x''`（文案 bug）。

- [2026-09-23][PITFALL][src:本仓验证实践] **PowerShell 里 `cargo ... *> $null` 会把退出码吞掉**，于是「我跑过 fmt/clippy 都过了」可能是假的。可靠写法：`cargo fmt --all --check *> "$env:TEMP\x.txt"; "fmt=$LASTEXITCODE"`（看 `$LASTEXITCODE`，不要看 `$?`）。同类陷阱：`| Out-Null`、管道里塞 `Select-String` 等 cmdlet 都会把 `$LASTEXITCODE` 覆盖成管道末端命令的结果。**推论**：任何「门禁全绿」的结论都必须能指出退出码的来源。
- [2026-09-23][PITFALL][src:probe-11/12/13 实测 2026-09-23, Win11 25H2 modern Notepad, PS 5.1 non-interactive] **PS 5.1 non-interactive session（PowerShell ISE / Windows 服务 / 任务计划程序 / CI runner / powershell -File 在 no-console 子场景）下 SendInput 无法 deliver 字符到任何前台窗口**：即便 SetForegroundWindow(hwnd) 返回 True、SetFocus(hwnd) 调用、UIA doc.SetFocus() 调用、Sleep(200ms) 都做完，GetFocus() 仍返回  x0。SendInput 返回   + GetLastWin32Error()=87 ERROR_INVALID_PARAMETER。**真正 blocker 是 foreground lock**：仅"调用进程是当前前台 / 控制台进程 / 持有 LockSetForegroundWindow 权限"三条件之一成立时，发送才会被接收线程接受。本机交互式 PowerShell 5.1 console 下 SetFocus + keybd_event 可 work（probe-08 test 1g），SendInput 应等价。→ **判定当前 PS 是否 interactive 的可靠方法**：	ry { [Console]::WindowHeight } catch { return False }（no-console 进程直接抛 IOException: handle invalid）；**写自动化脚本默认 non-interactive = SendInput 不可靠**。**未来方案**：(a) 强制 spawn interactive session（任务计划程序 / psexec -i -s）；(b) 走 ValuePattern.SetValue（不依赖 foreground，已 100% 验证）；(c) 用 Rust 走 COM via windows crate（生产路径，架构 v2 已规划）。**架构影响**：stage-1 Notepad Adapter 的写路径必须 SetValue 优先，SendInput 仅在 IME / 修饰键场景下使用且需 foreground 锁定检查。

- [2026-09-23][PITFALL][src:TASK-101 集成回归用户实测 2026-09-23 11:00, Win11 25H2 modern Notepad, interactive PS console] **SendInput 是上下文敏感型 API**（更新 2026-09-23 "foreground lock 限制"假说）：即便在 interactive PS console 下，SendInput 仅在**主窗口 + 标准 UIA 控件**（如 Notepad 主窗口 Ctrl+S）能 deliver；**对 Win32 #32770 dialog + DirectUI subclass Edit + WinUI 3 / UWP 元素仍 0% delivery**。→ **测试 SendInput 必须先验证目标窗口/控件类型**：(a) 主窗口标准控件可 work；(b) #32770 + DirectUI 仍 0%（pre-existing）；(c) UWP / WinUI 3 仍 0%。**stage-1 Adapter 写路径**仍以 UIA ValuePattern.SetValue 为主（probe-10 VP 100% + probe-04 SetValue 100%），**SendInput 仅在已知主窗口加速键场景使用**（如 Ctrl+S / Ctrl+Z / Esc）。→ **架构影响**：SendInput 不是 SendKeys 的完全替代品 = **1 退化风险**仍存在（DirectUI 子类限制 pre-existing）= 不构成"SendInput 替换 SendKeys 是升级"，而是"等价替换 + 显式 API + 避免 deprecated keybd_event"。
- [2026-09-24][PITFALL][src:TASK-011 收尾轮] **模块级 `#![allow(...)]` 不是"per-line allow"，是 sledgehammer，且它会被"留待人类裁决"无限期挂起**。ADR-0035 §决策 2 允许的是"**per-line** `#[allow]` + 原因注释 + 任务卡 §5 登记"，而 21 条 lint 的模块级豁免既不可审计（无法区分生产/测试），也无法按条追溯理由。上一轮把它记成"需人类裁决：改代码 or 改 ADR baseline"后就没再动 —— 但 ADR-0035 §替代路径 已经把顺序写死了：**①首选改代码 → ②次选 per-line allow → ③最后才开新 ADR**。"等裁决"不等于"路径不明"。→ **教训**：遇到 lint 豁免，先数一下"改代码要动几处"；本轮实测只需 ~20 处机械修改（`Self::` / `const fn` / `slice::get` / `is_ascii_digit`），成本远低于等待。**验证方式**：先删掉整个 `#![allow]` 块再跑 clippy，按输出逐条修，最后用现有单测 + `codegen --check` 0 drift 证明行为未变。
- [2026-09-24][PITFALL][src:TASK-011 收尾轮 实测] **`docs/DEPENDENCIES.md` 的"版本要求"列会与 `Cargo.toml` 静默矛盾**。原表把 `serde` / `serde_json` 写成 `**`=1.0`**`（精确钉定），而 `crates/protocol/Cargo.toml` 写的是 `"1.0"`（caret 语义）—— 两者含义完全不同（`=1.0` 只允许 1.0.0）。**机器事实**：`Cargo.lock` 解析到 `serde_json 1.0.151`，若真是 `=1.0` 不可能如此。**没有任何门禁会发现这种矛盾**（`cargo deny check` 只看许可证/来源/advisory，不看文档里的版本列）。→ **教训**：登记表的"版本要求"列必须与 `Cargo.toml` 的写法**逐字一致**（连 `=` 号在内）；只对"0.x 且跨版本有破坏性变更"的依赖才用精确钉定（如 `windows =0.62.2`），1.x 用 caret 并在说明里写明。
- [2026-09-24][PITFALL][src:本仓工具实践] **PowerShell 的 `[System.IO.File]::WriteAllLines()` / `WriteAllText()` 默认用 `Environment.NewLine`，会把仓库的 LF 文件整份改写成 CRLF**。本轮解 merge 冲突时用 `WriteAllLines` 重写 `LEDGER.md` / `MEMORY.md` / `docs/memory/{facts,pitfalls}.md`，git 立刻报 `warning: CRLF will be replaced by LF`。`.gitattributes` 的 `* text=auto eol=lf` 只保证**提交进索引的内容**被规范成 LF，工作区仍会是 CRLF，`docscan`（查 CRLF/BOM/缺末行 LF）会直接红灯。→ **可靠写法**：`[System.IO.File]::WriteAllText($p, ($text -replace "`r`n","`n"), (New-Object System.Text.UTF8Encoding($false)))` —— 既显式去 CR，又用无 BOM 的 UTF-8（BOM 同样会被 `docscan` 拦）。写完整份后**必须**再数一次：`[regex]::Matches($c,"`r").Count` 应为 0。
- [2026-09-24][PITFALL][src:gate-selftest run_number=1 实测 + 本机 Git Bash 复现] **GitHub Actions 的 `run:` 由 `bash -eo pipefail` 执行（隐式 `-e`），所以负向/canary 脚本里的 `out="$(失败的命令)"; code=$?` 会让脚本当场中止 —— `code=$?` 与后面所有断言根本不会执行**，负向验证退化成「**永远红**」的假红（假红比假绿更隐蔽：它每次都红，于是「重跑一次」「跳过这一步」变成默认反应，真正该拦的东西照样漏过去）。`gate-selftest.yml` 两个 job 的负向步都栽在这里，指纹是「**正向步全过、负向步全挂**」。**本机裸跑同一脚本是通过的**（本地 shell 默认无 `-e`），所以「本地验证过」把 bug 完全掩盖了 —— 与 PL-016「门禁没跑却显示绿」是同一族失效，只是方向相反。→ 可靠写法：`if out="$(cmd 2>&1)"; then code=0; else code=$?; fi`（条件位置的命令不受 `-e` 影响），或显式 `set +e` 包住捕获再 `set -e`。→ 推论：**任何负向/canary 脚本都必须在与 CI 同形的 shell（`bash -eo pipefail`）下验证**，否则「断言未执行」与「断言失败」在输出上无法区分。
- [2026-09-24][PITFALL][src:gate-selftest run_number=1 attempt=2 实测] **GitHub Actions 的 "Re-run all jobs" 复用该 run 绑定的 commit，不会取当前分支 HEAD** —— 所以「修完再点 Re-run」跑的还是**旧代码**，红绿都与修复无关。实测：`gate-selftest` 的 `run_number=1` 在修复推送后被人 Re-run 成 `run_attempt=2`（同一 `id`、同一 `sha=fb62636`、旧脚本），结论仍是 failure，步骤指纹与 attempt 1 逐条一致；要验证修复必须用 **"Run workflow" 重新 dispatch**（新 run 取所选 ref 的当前 HEAD，本轮 run 3 = main@`22e88a4` = SUCCESS）。→ **教训**：判断「修好了没有」之前，先核对 run 的 `head_sha` 是否**含修复的那个 commit**；`run_number` 相同而 `run_attempt` 变了 = 重跑，不是新验证。→ 与 ADR-0019「运行编号记入 LEDGER」配套：**记编号时同时记 `head_sha`**，否则同一个 run_number 会被误当作两次独立验证。
- [2026-09-24][PITFALL][src:TASK-012 实测] **`deny.toml` 的 `[bans] wildcards = "deny"` 会把「只有 `path` 没有 `version`」的**工作区**依赖判成通配依赖**：`crates/storage/Cargo.toml` 里 `[dependencies.assistant-protocol] path = "../protocol"` → `cargo deny check bans` **FAILED**（`error[wildcard]: found 1 wildcard dependency for crate 'assistant-storage'`，exit 2），而同一条命令的 licenses / sources 都报 ok —— 只看尾部会误判成「deny 过了」。→ **修法**：给 path 依赖补 `version = "0.1.0"`（与 `[workspace.package] version` 一致）；**不要**去 `deny.toml` 加 `allow-wildcard-paths`（那是放宽门禁 = 漂移触发器 ⑥）。→ 推论：以后任何新增的 workspace 内 path 依赖都要同时写 `version`，否则第一张引依赖的卡就会卡在 bans 上。
- [2026-09-24][PITFALL][src:TASK-012 实测] **`clippy::redundant_pub_crate` 要求把「私有 / `pub(crate)` 模块里的 `pub(crate)` 条目」改成 `pub`**，而本仓**没有**启用 `unreachable_pub`（`[workspace.lints.rust] rust_2018_idioms` 不含它；实测把 `pub fn apply_pending` 放进 `pub(crate) mod schema` 之后 clippy 全绿）。→ 这条 lint 的**正解是改 `pub`**（被要求改 `pub` 本身就说明模块已经把可见性压到 crate 内了）；写 `#[allow(clippy::redundant_pub_crate)]` 属于「放宽 lint」= 漂移触发器 ⑥，要停下升级。→ 同理，集成测试的共享夹具模块（`tests/common/mod.rs`，模块本身私有）里的类型/函数要写 `pub` 而不是 `pub(crate)`。
- [2026-09-24][PITFALL][src:TASK-012 实测] **跨多个集成测试 crate 共享的夹具模块必须 `#![allow(dead_code)]`，否则 `-D warnings` 直接挂**：`tests/common/mod.rs` 被 `storage_integration.rs` / `storage_records.rs` 各自 `mod common;` 引入 → 每个测试 crate 只用到夹具的一部分（实测摘掉该 allow 后 `storage_records` 报 `method advance_to is never used` / `function count_files is never used` / `function file_size is never used` 三条 error）。这不是「没人用的代码」而是「跨 crate 共享的代码」，所以在**测试专用**模块里 file-level `#![allow(dead_code)]` + 写明原因（与 ADR-0035「首选改代码」不冲突：产品代码里仍然一处都不许 allow）。→ 同一模块里的 `pub` 是必需的（见上一条），且 file-level allow **只写 `dead_code`**，不要拿 `clippy::all` 当锤子。
- [2026-09-24][PITFALL][src:TASK-200 实测] **`xtask docscan` 抓不到「节号重复 / 空节 / 重复标题」这类 markdown 结构缺陷**：`docs/spec/` 7 份契约草案里 `## 4.` / `## 5.` 各出现**两次**、`### 字段` 连着出现两次、`## 1. 目标` / `## 2. 范围` **全空**，而 `docscan` 一路报 `0 error / 0 warning`（它只查破表 / setext 风险 / 编码形状 / 末行 LF）。缺陷从 TASK-072 批量生成（2026-09-20）活到 2026-09-24 全量复核才被发现。→ **推论**：任何"文档结构已被机器保证"的说法，都必须先确认 `docscan` 真的覆盖那条规则；「节号唯一 / 空节 / 重复标题」三条应进护栏卡（PL-038 已记该机器检查缺口，未开卡）。→ 另一条经验：**批量生成的文档要按"结构自检命令"验收**，只看"文件存在 + 渲染正常"会漏掉整块外来内容。
- [2026-09-24][PITFALL][src:TASK-013 实测] **审计 hash chain 的校验绝不能"按 `ts` 排序后逐行比对 `prev_hash`"**：审计事件的 `ts` 是**毫秒**粒度，同一毫秒可以写入很多条（批量 flush 尤其如此，实测固定时钟下 **2000 条全部同毫秒**），此时 `ORDER BY ts` 的并列次序是任意的 → 合法的审计流会被**误判成断链**。**正解**：把链当**图** —— 用 `prev_hash → 行` 建索引，从创世（`prev_hash = ""`）沿链前进，**走不到的行才是断链**（与物理行序无关，且能精确定位"中间行被删"）。→ 推论：任何"有序日志 + 链式摘要"的设计，都要先问一句「我的排序键在同值时是否唯一」；不唯一就不能拿它当链序。**另一条**：校验路径**不要**在读取层预先解析 JSON —— 被篡改的行可能根本不是合法 JSON，那样整次校验会以"反序列化失败"告终，把"这一行被改了"混同于"库读不出来"；应逐行容错（解析失败 → `UnreadablePayload`，其余行继续）。
- [2026-09-24][PITFALL][src:TASK-013 实测] **给 `crates/storage` 加迁移会连带把 `SCHEMA_VERSION` 抬高，而 storage 的集成测试里有两处写死了"迁移记账行数 == 1"**：`crates/storage/tests/storage_integration.rs` 的 `test_open_fresh_database_reaches_latest_schema_version` / `test_open_is_idempotent` 断言 `count_rows(.., "schema_migrations") == 1`，`test_open_rejects_unknown_schema_version` 用**不带 WHERE** 的 `UPDATE schema_migrations SET version = SCHEMA_VERSION + 1`（单行时正确，多行时撞 `UNIQUE constraint failed: schema_migrations.version`）。→ **修法（更强而非更弱）**：行数断言改成 `== SCHEMA_VERSION`（版本号从 1 连续 ⇒ 行数 == 最高版本，同时证明**每个迁移都被记账**）；UPDATE 加 `WHERE version = SCHEMA_VERSION`（只改最高版本那一行）。→ 推论：**跨卡改迁移前，先 grep `SCHEMA_VERSION` 与 `schema_migrations` 在测试里的硬编码**。
- [2026-09-24][PITFALL][src:TASK-202 实测] **文本文件里的一个裸 NUL 字节会让 git 把它当二进制**：`docs/adr/README.md` 自 TASK-071 起含 1 个 `0x00`（落在 0035 行的文件名单元格里），于是 `git diff` 永远显示 `Bin 12033 -> 12277 bytes` —— **ADR 登记表的每一次改动在 PR 里都不可复核**，而 `xtask docscan` / `hygiene` / `adr-index` 全都抓不到（NUL 是合法 UTF-8 字符 U+0000，文本 API 照常解析；`adr-index` 一路绿）。**修法**：用 `[System.IO.File]::ReadAllBytes` + 按字节定位替换（PowerShell 的 `Get-Content` / `Set-Content` 会把 NUL 弄丢或弄成别的字符，不能用来定位）。**推论**：① 「文档已被机器校验」不等于「diff 可复核」——两者失效模式不同；② 修一个含 NUL 的文件的**那一次** PR，diff 仍是二进制的（基线就是二进制），要等合并后才恢复文本 diff；③ 建议给 `docscan` 加一条「文本文件不得含 NUL」的规则（未开卡）。
- [2026-09-24][PITFALL][src:TASK-202 实测] **`Arc::clone(clock)` 内联进 `Database::open(...)` 会编译失败（unsized coercion 只在实参位置发生）**：`Database::open(paths: &StoragePaths, clock: Arc<dyn Clock>, migrations: &MigrationSet)` 传 `Arc<FixedClock>` 时，把 `Arc::clone(clock)` 直接写在实参位置会先推 `T = dyn Clock` 再要求 `Arc<dyn Clock>: Clone` → `the size for values of type dyn Clock cannot be known at compilation time`。**正解**：先 `let clock = Arc::clone(clock);` 绑定成 `Arc<FixedClock>`，再把它传进 `open` —— 此时实参位置才发生 `Arc<FixedClock> → Arc<dyn Clock>` 的强制转换。**已写进** `crates/{storage,audit}/tests/common/mod.rs` 的注释，避免下个 agent 把「先绑定」当成多余代码删掉。
- [2026-09-24][PITFALL][src:TASK-202 实测] **手工 `DROP TABLE` + `DELETE FROM schema_migrations` 造出来的「旧版本库」是假状态**：TASK-013 的 v1→v2 升级用例用「先建到最新版 → 再 DROP 掉 0002 建的表 + 删它的记账」伪造 v1 库 —— 那样的库**任何真实 v1 二进制都造不出来**（真 v1 库里 0001 的 `applied_at` / checksum 与「先建后删」一致纯属巧合，且一旦 0001 本身变化就静默失真）。**注册表让「真 v1 库」第一次可构造**：`storage_only_migrations()`（只装配 storage 自己的 0001）建库 ⇒ 真实 v1 ⇒ 再用完整集合打开验证升级。**推论**：任何「旧版本 → 新版本」的升级测试，判据应该是**用旧版本的装配集合建库**，而不是「用新版本建库再往回删」。

- [2026-09-24][PITFALL][src:TASK-203 实测] **SQLite 的 `ALTER TABLE ... DROP COLUMN` 删不掉「PRIMARY KEY / UNIQUE / 被索引引用」的列 → 只能重建表；而重建表时 `DROP TABLE` 会连带带走该表的索引与触发器**：`audit_logs` 的 `hash` 恰好同时沾上「与主键同值 + 被 UNIQUE 语义牵连」，于是 0003 必须走 `CREATE audit_logs_new` → `INSERT ... SELECT ORDER BY rowid` → `DROP TABLE` → `ALTER ... RENAME` → **显式重建** `idx_audit_ts` 与两个 append-only 触发器。**教训**：重建表的迁移必须把「索引 / 触发器 / 触发器名字」逐条列出来重建，并让测试断言它们**升级后仍在**（否则 append-only 的第二道锁会被静默摘掉 —— 那是安全回归）。
- [2026-09-24][PITFALL][src:TASK-203 实测] **代码一旦跟着新迁移走，就再也不能用同一个写入器去造「旧形状的库」**：`AuditLog` 的链尾查询改 `ORDER BY sequence` 之后，在 v2 形状的库上直接报 `no such column: sequence`（这正是「代码只认最新 schema」的正确形态）。于是「用真实旧库测升级路径」必须换一种造数据的方式 —— **按当时那份 DDL 允许的列形状**写行，而 hash **仍然走 crate 的公开链算法**（`canonical_payload` + `compute_self_hash`）。若图省事手工填 hash，造出来的就是一个`verify_chain` 判为断链的**假库**，升级用例的起点就假了。

# ADR-0100　PowerShell 强制 guard 与 OpenAI 文档代理回退

状态：Accepted　日期：2026-10-29　Supersedes：—
关联：ADR-0099、`.codex/hooks.json`、`AGENTS.md` §3、
`docs/automations/codex-automations-operations.md`

## 背景

本机 Codex 的 shell 工具允许每次调用显式选择 `bash` / `powershell`。历史 rollout 已反复出现
`shell: "bash"` 误用：常见失败包括 `cargo` / `rg` / PowerShell cmdlet 不存在、PowerShell
变量与脚本语法在 bash 中解析失败，以及 bash/WSL 通道自身启动失败。单纯在 prompt 或记忆里
提醒“必须用 PowerShell”没有阻止后续再次发生。

官方 Codex hooks 文档确认：`PreToolUse` 能拦截 `Bash` 工具，但事件只提供 `tool_name` 与
`tool_input.command`，不提供调用方选择的 `shell` 字段。本机探针也验证了这一点。因此“hook
检查 shell 后拒绝 bash”不可实现。

另一方面，`developers.openai.com` 在当前网络直连返回 403；通过本机代理
`http://127.0.0.1:30000` 可返回 200。访问流程需要明确“直连失败后走代理”的有界回退，
不能因为一次 403 就放弃官方文档。

## 决策

1. **Windows shell 执行采用强制重写。** 项目 `PreToolUse` hook 匹配 `Bash`，把每个非空
   `tool_input.command` 改写为：

   ```text
   powershell.exe -NoLogo -NoProfile -NonInteractive -OutputFormat Text
     -ExecutionPolicy Bypass -EncodedCommand <UTF-16LE Base64>
   ```

   原始命令按 UTF-16LE 编码，并前置 `$ProgressPreference = 'SilentlyContinue'`。这样即使
   模型误填 `shell: "bash"` 或 `cmd`，外层 shell 最终也只能执行显式 Windows PowerShell 进程。
2. **提示层仍要求正确参数。** `AGENTS.md` 明确所有 `exec_command` 应显式使用
   `shell: "powershell"`；hook 是第二道强制护栏，不是允许模型继续误填的理由。
3. **hook 脚本从 Git 根定位。** Windows hook command 先运行 `git rev-parse --show-toplevel`，
   再执行 `.codex/hooks/*.ps1`，避免 Codex 从仓库子目录启动时相对路径失效。
4. **官方 OpenAI 文档采用有界代理回退。** 新增 PowerShell 7 工具
   `tools/fetch-openai-doc.ps1`：先直连允许的
   OpenAI 官方域名；若网络错误或非 2xx，则读取 `HTTPS_PROXY` / `HTTP_PROXY`，再回退
   `http://127.0.0.1:30000`，最多三次并退避重试。正文只写 stdout，状态写 stderr，不保存到仓库。
5. **无证据不得宣称成功。** hook 解析失败、包装失败或代理最终失败都必须显式非零退出；不得
   用空内容、占位正文或旧缓存冒充新抓取结果。

## 考虑过的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 继续靠 prompt / memory 提醒 | 否决 | 历史已反复证明不能阻止模型偶发写错 |
| PreToolUse 检测 `shell` 后拒绝 | 否决 | 事件不含 `shell`；实现只能猜测，无法满足 fail-closed |
| `updatedInput` 只改 `shell: "powershell"` | 否决 | 实测宿主忽略该字段，外层仍启动 bash |
| 重命名 / 删除系统 `bash.exe` | 否决 | 破坏宿主环境且不可逆，超出项目权限 |
| **把 command 重写为显式 PowerShell `-EncodedCommand`** | 采纳 | 不受外层 shell 选择影响，保留命令原文并消除引号歧义 |
| OpenAI 文档失败后永久放弃 | 否决 | 本机代理已验证可稳定取得官方页面 |

## 影响

- `.codex/hooks/powershell-guard.ps1`：强制 PowerShell 命令重写。
- `.codex/hooks.json`：注册 `PreToolUse`，并把项目 hook 路径改为 Git 根解析。
- `tools/fetch-openai-doc.ps1`：官方文档直连失败后的代理回退工具。
- `AGENTS.md`、自动化章程 / 操作手册：记录 shell 与代理回退的现行口径。
- 不修改 Codex 二进制、产品代码、schema、依赖或网络权限。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 项目 hook 未获 trust | 首次使用在 Codex `/hooks` 中完成一次 review/trust；e2e 使用 bypass 验证脚本本身 |
| Windows PowerShell 5.1 与 `pwsh` 行为差异 | 统一使用 Windows 内置 `powershell.exe`；命令通过编码原样传入，避免外层解析 |
| 外层 bash/WSL 在受限 sandbox 中无法启动 | 正常权限路径已验证重写生效；若 bash 在启动前被 sandbox 拒绝，记录为宿主通道错误，不伪称已执行 |
| 代理不可用或返回非 2xx | 有限次数重试，最终显式失败；不得改用搜索摘要或伪造正文 |
| 文档正文过大 | 抓取工具保留 HTTP 内容长度边界，调用方按需截取，不写入仓库 |

## 验证

1. `PreToolUse` 探针实际记录 `tool_name=Bash`、`tool_input.command`，确认无 `shell` 字段。
2. 故意让嵌套 Codex 使用 `shell: "bash"` 运行 `Get-Date`；rollout 中原始命令为 `Get-Date`，
   hook 执行后外层命令变为 `powershell.exe ... -EncodedCommand ...` 并成功返回时间。
3. `fetch-openai-doc.ps1` 对 `developers.openai.com/codex/hooks/` 直连失败、代理成功，
   记录 transport、HTTP 200 与非空正文长度。
4. 从仓库子目录启动 hook，仍能从 Git 根解析脚本。
5. `cargo fmt --all --check`、`hygiene`、`memory-counts`、`adr-index`、`docscan`、
   `card-check`、`check-ledger` 全绿。

# 开发环境准备清单

> **PL-017 / PL-006 落地**。一份能复现的清单——新机器或新 agent 照做即可达到
> 「本地验收 = CI 验收」。所有版本号 / 校验值都来自**实测**，不来自回忆。
> **本机实测日期**：2026-09-18。

---

## 1. 系统前提

| 项 | 要求 | 备注 |
|---|---|---|
| OS | Windows 11 24H2 / 25H2 / macOS 14+ / Ubuntu 24.04+ | 仓库的 CI 三平台矩阵 |
| 网络 | **直连可达 `api.github.com`** + 经代理可达 `github.com release 下载` / `crates.io` / `git clone` | 本机代理 `http://127.0.0.1:30000`（git config 见 §6）；`api.github.com` 不需要代理 |
| 磁盘 | ≥ 5 GB 可用（target/ 单平台约 1.5 GB；CI 三平台矩阵约 4.5 GB） | `cargo clean` 可随时回收 |
| Git | ≥ 2.40 | 用来 clone 仓库 + 拉 `RustSec/advisory-db`（cargo-deny 用） |

## 2. Rust 工具链

由 `rust-toolchain.toml` 锁定：

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy", "llvm-tools-preview"]
profile = "default"
```

rustup 会**自动**在仓库根目录读这个文件、装齐上述 components。一键搞定：

```bash
# 安装 rustup（Linux/macOS）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows：用 rustup-init.exe（rustup.rs）
# 或 winget install Rustlang.Rustup

# 然后 cd 进仓库，rustup 自动装齐
cd path/to/repo
cargo --version      # 应与 rust-toolchain.toml 一致
rustup component list --installed   # 应有 rustfmt / clippy / llvm-tools-preview
```

**实测版本**：rustc/cargo 1.98.1 stable-msvc（与 rust-toolchain.toml 一致即通过）。

## 3. `cargo-deny`（CI 硬门禁 #8）

版本：**0.20.2**（与 CI action `EmbarkStudios/cargo-deny-action@v2.1.1` 打包版本一致，
ADR-0024 D2 钉版）。

**安装（推荐：从 GitHub 官方 release 取预编译包）**：

```bash
# 1) 查最新版本与 sha256
#    用直连可达的 api.github.com 查 digest（不需要代理）
DIGEST=$(curl -sSL https://api.github.com/repos/EmbarkStudios/cargo-deny/releases/latest \
         | grep -A1 "name.: .cargo-deny-x86_64-pc-windows-msvc.zip" \
         | grep -oE '[a-f0-9]{64}' | head -1)

# 2) 下载走代理（github.com 直连被本机 GFW 重置）
URL="https://github.com/EmbarkStudios/cargo-deny/releases/latest/download/cargo-deny-x86_64-pc-windows-msvc.zip"
curl -sSL -o /tmp/cd.zip \
     --proxy http://127.0.0.1:30000 \
     "$URL"

# 3) 校验 sha256 与官方 digest 一致
echo "${DIGEST}  /tmp/cd.zip" | sha256sum -c -

# 4) 解压到 ~/.cargo/bin（rustup PATH 自动包含）
unzip -o /tmp/cd.zip -d ~/.cargo/bin/
cargo deny --version   # 应为 0.20.2
```

**配置**（仓库自带 `deny.toml`，不要改 schema，只在引入第一个第三方依赖时才生效）：

```toml
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "MPL-2.0", "Unicode-3.0"]

[advisories]
db-path = "target/deny-db"     # 本地 advisory 库路径
```

**advisory-db 需走代理**（`RustSec/advisory-db` 在 github.com 上，git clone 也被
重置）。**用环境变量**而非 `.gitconfig`（不污染全局配置）：

```bash
GIT_CONFIG_GLOBAL=/dev/null \
GIT_CONFIG_SYSTEM=/dev/null \
git -c "url.https://ghproxy.net/https://github.com/.insteadOf=https://github.com/" \
    clone --depth 1 https://github.com/RustSec/advisory-db.git target/deny-db
# 或更稳：把镜像直接写进 deny.toml 的 [source] 表（避免环境变量泄漏）
```

## 4. `cargo-llvm-cov`（覆盖率硬门禁）

版本：**0.9.1**（实测）。CI 用 `taiki-e/install-action@v2` 装同一版本。

```bash
# 类比 cargo-deny 的下载流程（API 查 digest → 走代理下载 → 校验 → 解压）
DIGEST=$(curl -sSL https://api.github.com/repos/taiki-e/cargo-llvm-cov/releases/latest \
         | grep -A1 "x86_64-pc-windows-msvc" | grep -oE '[a-f0-9]{64}' | head -1)
URL="https://github.com/taiki-e/cargo-llvm-cov/releases/latest/download/cargo-llvm-cov-x86_64-pc-windows-msvc.tar.xz"
curl -sSL --proxy http://127.0.0.1:30000 -o /tmp/clc.tar.xz "$URL"
echo "${DIGEST}  /tmp/clc.tar.xz" | sha256sum -c -
tar -xJf /tmp/clc.tar.xz -C ~/.cargo/bin/
cargo llvm-cov --version    # 应为 cargo-llvm-cov 0.9.1
```

## 5. inspect.exe（Windows SDK 工具，UI 树探测）

版本：Windows SDK **10.0.26100.0** 起的 x64 `inspect.exe`。

```powershell
# 默认路径
C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\inspect.exe

# 校验：跑一下，应能起 GUI
& "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\inspect.exe" --help
```

**为何不用 Accessibility Insights**（ADR-0024 D3）：AI agent 用输出**不可 diff、不可计时、不可重复**；
inspect.exe 输出可结构化、可 diff、可脚本化（`spikes/spike-a-notepad/probe-01-tree-survey.ps1` 即其自写导出器）。

## 6. Git 代理（本机 GFW 场景）

**只对 github.com HTTP/HTTPS 生效**，不影响 `api.github.com` / `crates.io` /
其他 Git 仓库。

```powershell
git config --global http.https://github.com/.proxy http://127.0.0.1:30000
git config --global https.https://github.com/.proxy http://127.0.0.1:30000

# 提交身份（占位 → 真实 → 仓库本地）
git config --global user.name "your-name"
git config --global user.email "you@example.com"
# 仓库本地覆盖（推荐）：在仓库根目录
git config --local user.name "snofin18"
git config --local user.email "snofin@gmail.com"
```

## 7. 验收命令（按 AGENTS.md §6 跑一遍即知环境是否就绪）

```bash
cargo fmt --all --check                  # 0 diff
cargo clippy --all-targets -- -D warnings # 退出码 0
cargo test --workspace                  # 全部通过
cargo run -p xtask -- hygiene            # PASSED, scanned=22 errors=0
cargo run -p xtask -- memory-counts      # PASSED, scanned=7 errors=0
cargo run -p xtask -- adr-index          # PASSED, scanned=17 errors=0
cargo deny check licenses bans sources    # licenses ok, bans ok, sources ok
cargo llvm-cov --workspace --fail-under-lines 75   # 行覆盖 ≥ 75%
```

## 8. 常见坑（实测）

| 症状 | 原因 | 解决 |
|---|---|---|
| `cargo deny` 卡在「Updating crates.io index」 | advisory-db clone 卡 | 用 §3 的 `GIT_CONFIG_*` 走 ghproxy 镜像 |
| `cargo fmt --check` 在 Linux runner 红 | Windows CRLF | `git config core.autocrlf false` + `.gitattributes` 强制 `* text=auto eol=lf` |
| `cargo llvm-cov` 找不到 `llvm-tools-preview` | 组件漏装 | `rustup component add llvm-tools-preview` |
| inspect.exe 启动报「无法定位 DLL」 | 没装 VC++ Redistributable | 装 VS Build Tools 或 VC++ Runtime |
| `RUSTSEC-20xx-NNNN` 在本地 deny 但 CI 通过 | advisory-db 旧 | `cd target/deny-db && git pull` 或删目录重 clone |
| GitHub release 下载走代理后 `sha256sum` 不匹配 | 代理返回了「缓存版本」 | 用 `api.github.com` 的 digest 而不是自己算（§3-4 全用这个） |

## 9. 关联

- `AGENTS.md` §6 验证命令（每次提交前跑）= 本清单 §7 的子集
- `docs/governance-ai-agent-execution.md` §5.1 CI 门禁清单 = 哪些工具坏了会红
- `.github/workflows/ci.yml` = CI 端用同一份工具清单
- `deny.toml` = 引入第三方依赖时的合规门槛
- `MEMORY.md` §1 快照的工具链区 = 当前机器实测版本

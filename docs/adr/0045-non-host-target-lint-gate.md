# ADR-0045　`#[cfg]` 分叉代码的「非宿主平台」编译门禁

状态：**Accepted**（2026-09-24，人类 chat「按你的建议去做」授权）　日期：2026-09-24　Supersedes：—　Superseded by：—
关联：`AGENTS.md` §6、`docs/memory/pitfalls.md`、`crates/platform/windows/src/unsupported.rs`、`docs/PARKING_LOT.md` PL-070、CI run 35815168167

## 背景（为什么现在要决定）

`crates/platform/windows/src/unsupported.rs` 用 `#[cfg(not(windows))]` 提供非 Windows 后端。
**该文件在 Windows 上从不编译**（这正是 `cfg` 的定义），而本项目的开发机与本地门禁**全在 Windows**
→ 结构上覆盖不到它，只有 CI 的三平台矩阵会跑到。

这不是理论风险，已经真实发生：CI run **35815168167**（commit `cf44b49`）= **6/8** ——
`check (ubuntu-latest)` 与 `check (macos-latest)` **红**，根因全在该文件：20 个 impl 方法用
`async fn` 却**没有 `.await`**（`clippy::unused_async_trait_impl`）+ 1 处 `DoD` 缺反引号
（`clippy::doc_markdown`）。**同一次提交的 Windows 本地门禁全绿。**

修完（`aafcdaf`）后 CI 8/8，但**修复手段不能依赖「下次也记得」**：护栏必须能被本地执行。

## 决策（一句话）

把「**非宿主平台**编译检查」写进 `AGENTS.md` §6 的验收清单：在 Windows 开发机上用
`cargo clippy --target <非 Windows 目标> -p <纯 Rust crate> --all-targets -- -D warnings`
覆盖 `#[cfg(not(windows))]` 的分叉；适用范围 = **无 C 依赖**的 crate。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | 新增验收命令（在 Windows 开发机上执行，**每个**含 `#[cfg]` 分叉的纯 Rust crate 各跑一遍）：`cargo clippy --target x86_64-unknown-linux-gnu -p <crate> --all-targets -- -D warnings` 与 `cargo clippy --target aarch64-apple-darwin -p <crate> --all-targets -- -D warnings` |
| **D2** | **为什么可行**：`clippy` / `check` **不链接**，所以不需要 C 交叉编译器。实测（2026-09-24，rustc 1.98.1）：`assistant-platform-windows` 在两个目标上均 **exit 0** |
| **D3** | **适用范围 = 无 C 依赖的 crate**。`crates/storage` 的 `zstd-sys` / `libsqlite3-sys` 会卡在 `cc-rs`（实测 `cargo clippy --target x86_64-unknown-linux-gnu --workspace` = exit 101，`failed to find tool "x86_64-linux-gnu-gcc"`）—— 这是**工具链限制**，不是代码缺陷，故门禁按 crate 指定，不写成 `--workspace` |
| **D4** | 一次性前置：`rustup target add x86_64-unknown-linux-gnu aarch64-apple-darwin`（开发机一次性操作，**不是**仓库改动） |
| **D5** | 本 ADR **授权**修订 `AGENTS.md` §6 的验收清单（加上述两条命令 + 一句适用范围说明）。先例：ADR-0041 D5 / ADR-0042 D5 都以 ADR 显式授权的方式修订了 `AGENTS.md` / 契约文件 |
| **D6** | 门禁**不替代** CI 三平台矩阵：它是**本地**的早期反馈；CI 仍是最终判据（`[HARD]`） |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 只靠 CI 三平台矩阵（现状） | ❌ 否决 | 反馈延迟到 push 之后；且 2026-09-24 已真实发生一次（6/8 红），说明「靠记得」不成立 |
| 2 | 在 `#[cfg(test)]` 里放一份非 Windows 后端的编译冒烟测试 | ❌ 否决 | 会让 `unsupported.rs` 的 `WindowsPlatform` 与 `window::WindowsPlatform` **同名类型同时编译**（`E0428` 重复定义 + 两条 `pub use` 冲突）→ 要消除冲突就得给其中一个改名，纯粹为测试改生产命名 |
| 3 | 用 `cargo check --target <非 Windows>` 代替 `clippy` | ⚠ 部分可行 | `check` 只跑 rustc，**漏掉**本次真实踩到的 `clippy::unused_async_trait_impl` / `doc_markdown`（两者都是 clippy lint）→ 采用 `clippy` |
| 4 | **把非宿主平台 clippy 写进 `AGENTS.md` §6（本 ADR）** | ✅ **采纳** | 本地可跑、零新依赖、零 CI 改动，且直接命中这次的真实根因 |

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

- `AGENTS.md` §6：验收清单加 2 条命令 + 适用范围说明（**唯一**的文档改动，本 ADR 授权）
- `docs/PARKING_LOT.md`：PL-070 处置行（闭环）
- **不改**：`.github/workflows/ci.yml`（CI 矩阵已覆盖）、`docs/DEPENDENCIES.md`（未引新依赖）、任何 crate 代码

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 门禁被写成 `--workspace` → 因 C 依赖必然失败 → 大家开始忽略它 | D3 明确按 crate 指定，并写明 `zstd-sys` / `libsqlite3-sys` 的实测失败原因 |
| 目标未安装 → 命令报「can't find crate for `std`」之类 | D4 写明一次性 `rustup target add`；缺目标时的报错本身就指向 D4 |
| 误以为它替代了 CI | D6 明确：本地早期反馈，CI 仍是最终判据 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. **立即验证（本 ADR 落地时已跑）**：`cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings`
   与 `--target aarch64-apple-darwin` 均 **exit 0**；`cargo clippy --all-targets -- -D warnings`（Windows）**exit 0**。
2. **反向验证（ADR-0019 N1：硬门禁必须配负向验证）**：把 `unsupported.rs` 里任一方法的 `impl Future` 形状改回 `async fn`
   → 两条 `--target` 命令**必须变红**（2026-09-24 实测：正是这个形状差异导致 CI 6/8）。
3. **重新评估触发条件**：出现新的 `#[cfg]` 分叉平台（macOS / Linux 实现）；或某 crate 引入 C 依赖使 `--target` clippy 不再可用。

## 相关 ADR

- ADR-0019（硬门禁必须配负向验证）
- ADR-0024（Spike 工具链 / 依赖 / 门禁覆盖）
- ADR-0041 / ADR-0042（以 ADR 显式授权修订 `AGENTS.md` / 契约文件的先例）

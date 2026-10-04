# ADR-0071　截图管线与出域脱敏的 crate 边界（`capture` / `dlp`）

状态：**Accepted**（2026-10-04，按用户 2026-10-04「按你的推荐方案做」裁决）　日期：2026-10-04　Supersedes：—　Superseded by：—
关联：`docs/PARKING_LOT.md` PL-102、PL-023、架构 v2 §3 / §12 / §13、ADR-0043、ADR-0044、ADR-0063、`crates/platform/api/src/traits/window.rs`、`tasks/TASK-041-capture-window-redact-privacy.md`、`tasks/TASK-042-visual-verify-phash-dhash-confidence.md`、`tasks/TASK-050-dlp-three-tier-egress-local-only-redacted-full.md`、`tasks/TASK-238-capture-dlp-crate-skeletons.md`

## 背景（为什么现在要决定）

阶段 1b 的下一张卡 `TASK-041`（截图管线）与 1c 的 `TASK-050`（出域策略）都要求写入尚不存在的 crate：

- 窗口截图**原语**已经在 `crates/platform/api` 的 `WindowProvider::capture`（`CaptureOptions` / `ImageRef`）里冻结；
- 但「截图管线」的编排职责（脱敏应用、滚动清理、隐私保留策略）和「出域策略 / 脱敏 / 截图遮挡」没有落点；
- 架构 v2 §3 的 crate 布局列了 `dlp/`，**没有** `capture/`；
- `plans/stage-1-pilots.md` 的 1b 表把 `TASK-041` 的 write scope 写成 `crates/capture/**` + `crates/dlp/src/redact*`。

于是 `TASK-041` 的第一项工作会变成「新增 crate」——命中 AGENTS.md 漂移触发器 ②（加 crate → 停下升级），
并且计划与架构对 `capture` 是否成立为独立 crate 存在分歧。这属于分层与职责边界，必须先以 ADR 冻结（铁律 10），
已登记为 `PL-102`。人类 2026-10-04 采纳推荐方案：先写本 ADR，再建两个零依赖骨架，闭环 `PL-102`。

## 决策（一句话）

**新增两个 Zero-Third-Party-Dependency 的边界 crate：`crates/capture`（平台无关的窗口截图管线，唯一截图原语是
`crates/platform/api` 的 `WindowProvider::capture` trait）与 `crates/dlp`（出域策略 + 脱敏 + 截图遮挡）；
两者都不得直接调用平台 API，依赖方向单向；本 ADR 只冻结边界，不实现任何行为。**

## 决策细化

| # | 内容 |
|---|---|
| **D1** | `crates/capture` = **平台无关的窗口截图管线**：编排「解析目标窗口 → 调用 `WindowProvider::capture` → 应用脱敏 → 滚动清理 → 按隐私模式决定是否保留图像字节」。它持有的是**管线状态与保留策略**，不是平台句柄。 |
| **D2** | `crates/dlp` = **出域策略 + 脱敏 + 截图遮挡**：三档出域策略（`local_only` / `redacted` / `full`，`[ADR:待建 0007]`）、逐应用与逐内容类型覆盖、脱敏规则（密码框 / 正则命中区域）与截图遮挡。`TASK-050` 认领策略部分，`TASK-041` 只引用其 `redact` 能力。 |
| **D3** | **依赖方向单向**：`capture` 可以依赖 `crates/platform/api`（**只有 trait 与纯类型**）与 `crates/dlp`（脱敏）；`dlp` 不依赖 `capture`，也不依赖任何平台 crate。`crates/core` / `crates/policy` 不依赖 `capture` / `dlp` 的**平台实现**（铁律 7）。 |
| **D4** | **铁律 7 不破**：`capture` / `dlp` 内**禁止**出现 `Win32` / UIA / COM / 平台 crate 调用或 `std::process::Command`；需要平台能力时只能经注入的 trait（`WindowProvider::capture`）。实际截图发生在 `crates/platform/*` 实现侧，管线只消费其返回的 `ImageRef`。 |
| **D5** | **零第三方依赖起步**：本 ADR 与 `TASK-238` 不引入任何第三方依赖。未来的图像编解码、感知哈希（pHash/dHash）等依赖由对应实现卡按漂移触发器 ① 提 ADR 并登记 `docs/DEPENDENCIES.md` 后再加。 |
| **D6** | **架构 v2 §3 布局更新**：crate 布局在 `dlp/` 之外**追加 `capture/`**（职责一句话 = 平台无关的窗口截图管线）。原 `dlp/` 一行不变。 |
| **D7** | **资源有界（ADR-0063）**：两个 crate 的任何长期状态（管线缓存、脱敏规则表）都必须有硬上限与淘汰 / 拒绝策略；隐私模式下**不得**持久化图像字节。这些在实现卡落实，本 ADR 只冻结"不得无界、不得静默保留"。 |
| **D8** | **本 ADR 不实现行为**：`TASK-238` 只落地两个骨架（`Cargo.toml` + `README.md` 不变量 + 带文档的模块头 `src/lib.rs`，零 pub 项），实际能力归 `TASK-041` / `042` / `050`。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 不新增 crate，把截图管线并入既有 `crates/dlp` | ❌ 否决 | 出域策略归 1c（`TASK-050`），截图管线归 1b（`TASK-041`）。合并会让 1b 的卡去改 1c crate 的公共面，破坏 write scope 隔离与批次顺序，并把「出域策略」与「截图编排」两种生命周期绑在一起。 |
| 2 | 不新增 crate，把管线全部做进 `crates/platform/api` / `crates/platform/windows` | ❌ 否决 | 那是一层「平台实现 + 平台无关编排」的混装；管线状态与隐私保留策略是产品语义，不是平台能力，放进平台层违反分层，也会让跨平台后端重复实现同一编排。 |
| 3 | **新增 `crates/capture` + `crates/dlp` 两个边界骨架（本 ADR）** | ✅ **采纳** | 与架构 v2 §3 已有的 `dlp/` 一致，并把计划里缺的 `capture/` 补进布局；截图原语已在 `platform/api` trait 上，管线只做编排。零依赖起步，后续依赖各自登记。 |

## 影响

- `plans/stage-1-pilots.md` 1b 表里 `TASK-041` 的 write scope（`crates/capture/**` + `crates/dlp/src/redact*`）**从此合法**；`TASK-042`（`crates/verify/src/visual/**`）与 `TASK-050`（`crates/dlp/**`）也随之解锁前置。
- 架构 v2 §3 crate 布局追加 `capture/` 一行（D6）；`docs/adr/top-level-directories.md` **不改**（新增的是 `crates/` 下的子目录，不是顶层目录）。
- 新增两个 workspace 成员（由既有 `crates/*` glob 自动纳入，**不改根 `Cargo.toml`**）。
- 不改任何公共 trait / schema / ErrorCode，不引入第三方依赖，不 `#[allow]`，不 `unsafe`。
- `PL-102` 闭环；`PL-023`（`scripts/` 顶层目录）仍独立待裁决。

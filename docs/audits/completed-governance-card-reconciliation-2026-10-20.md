# 已完成治理卡状态与计划索引对账（2026-10-20）

## 1. 目的

依据真实 GitHub merged PR、LEDGER 既有证据和后续 TASK-044 真机十次验收结果，收口
TASK-292 / 293 / 294 / 296 / 297 / 303 / 304 的滞后状态。本轮不重跑 GUI，不改产品代码，
只将已经发生并已落盘的完成事实同步到任务卡状态、执行记录和阶段计划索引。

## 2. 完成证据

| 卡 | PR | base | merge | CI | pending | 完成理由 |
|---|---|---|---|---|---|---|
| TASK-292 | #402 | `main` | `d755262a7faadb48a12a34cf99e5654c2731db45` | 11/11 success | 0 | 平台几何契约已合并，LEDGER 已有合并回填 |
| TASK-293 | #403 | `main` | `589517965d298941e11b52dc73ade463a8a268b5` | 11/11 success | 0 | Windows 几何实现与负向 seam 用例已合并 |
| TASK-294 | #404 | `main` | `259ade0a7819997d46aab1ac19d2736967c440f0` | 11/11 success | 0 | 合成点击通道已合并；后续 TASK-303 真机 E2E 证明链路生效 |
| TASK-296 | #411 | `main` | `211f2c8ca300a04b96bcf917e37cd3745f873d00` | 11/11 success | 0 | control-view 回退修复已合并，真机 palette fallback 已通过 |
| TASK-297 | #412 | `main` | `f81f792c83461c34f9247b4a60132f5284cc1c73` | 11/11 success | 0 | `Alt+L` 与有界轮询已合并，后续真机 E2E 通过 |
| TASK-303 | #426 + #429 | `main` | `a271ea2d5db157b580a6fc2621e8d426edb3599f` + `0a87abac88a8aa4b0873f80e264308cd5e85c7d1` | 各 11/11 success | 0 | 实测画布输入与坐标空间注入已合并；T3.1 真机 E2E 和后续十次运行通过 |
| TASK-304 | #428 | `main` | `2c9fad73d3a1504fc5eb9bd30c0cc4cd78965e93` | 11/11 success | 0 | Per-Monitor-V2 感知已合并；真机探针证明 `scale=2` 且 capture 尺寸等于窗口几何 |

历史 PR 的 `mergeable` / `mergeStateStatus` 在 GitHub 合并后接口返回 `UNKNOWN`，因此本表只把
可复核的 `state=MERGED`、`base=main`、CI counts、pending 和 merge commit 作为完成证据，
不把合并后的 `UNKNOWN` 伪写成当时的 `MERGEABLE` / `CLEAN`。

## 3. 最终真机链路证据

`LEDGER.md` 与 `tasks/TASK-044-t3-1-newcanvas-rect-color-screenshot.md` 已记录：

- T3.1 真机端到端首次通过：`1 passed / 0 failed / 9.13 s`；
- T3.1 十次运行：`10/10 passed`，`success_rate=1.0`；
- 拖拽终点坐标误差 `0 px`；
- 相关修复链覆盖 TASK-294 合成点击、TASK-296 调色板回退、TASK-297 图层面板、
  TASK-303 实测画布输入、TASK-304 PMv2。

## 4. 对账结论

七张卡的目标、实现、CI 和后续真机验收均已完成；停在 `Review` 属于状态同步滞后，不是未交付工作。
本轮只改各自唯一状态行并追加 TASK-324 证据；分界线以上的其他正文保持不变。

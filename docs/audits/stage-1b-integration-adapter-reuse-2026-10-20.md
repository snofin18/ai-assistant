# Stage-1b Integration Audit and Adapter Reuse Review - 2026-10-20

> Audit type: Stage 1b Paint integration acceptance and Adapter reuse review
> Scope: TASK-040~046 plus the bridge/calibration cards that actually delivered the Paint path
> Auditor: Codex (TASK-047)
> Human approval: pending
> Technical verdict: **GO**
> Administrative stage-close verdict: **CONDITIONAL GO**

## 1. Summary

Paint is the second real application adapter after Notepad. The delivered 1b slice proves
three deterministic Paint workflows on Paint `11.2605.81.0`:

1. T3.1 new canvas -> rectangle -> visual verification;
2. T3.2 open PNG -> measure canvas/zoom -> mark region -> Save As;
3. T3.3 draw -> `Ctrl+Z` -> pixel return to the real pre-write snapshot.

All three standard task packages passed **10/10** real-machine runs. The full ribbon path
also completed 14 production Plan steps in one real-machine acceptance.

The integration architecture is sound: Paint handlers consume
`assistant-platform-api` traits, contain no direct `assistant-platform-windows` import,
and reuse the shared declaration/registration loop. There is no Paint-only platform trait.

The audit does **not** claim that the original platform abstraction was complete. Stage 1b
did require six categories of additive `platform/api` evolution. Every category is backed
by an Accepted ADR, and no public interface change was made as a Paint-local shortcut.

The whole 1b batch is not administratively clean yet:

- `TASK-257` remains `Review`;
- the stage table still labels TASK-044 as `Ready` although the card and ledger are Done;
- the stage table still labels TASK-256 as `Review` although its card is Done.

Those are governance closeout items, not reasons to hide the technical result. TASK-047
therefore signs off the technical integration as GO and leaves a conditional administrative
GO for the batch-level closeout.

## 2. Environment

| Item | Value |
|---|---|
| OS | Microsoft Windows 11 Professional |
| OS version | `10.0.26300` |
| Audit base | `main` at `63f81dafb0eba1a4e7461a6b989f2d6b9712b041` |
| Rust | `rustc 1.98.1 (48a229cea 2026-09-01)` |
| Cargo | `cargo 1.98.1 (797e8a9bc 2026-08-05)` |
| Python | `3.14.2` |
| Paint | `Microsoft.Paint 11.2605.81.0 x64` |

## 3. Paint Real-Machine Matrix

| Slice | Evidence | Result | Key measurements |
|---|---|---|---|
| T3.1 new canvas / rectangle | `eval/tasks/paint/t3.1/ten-run-summary.json` | **10/10 passed** | 12.556-12.810 s per run |
| T3.1 drag endpoint | `eval/tasks/paint/t3.1/coordinate-error-summary.json` | **PASS** | expected/actual `(1458,859)`; max error `0 px`; tolerance `2 px` |
| T3.2 PNG open / mark / Save As | `eval/tasks/paint/t3.2/ten-run-summary.json` | **10/10 passed** | 49.769-53.147 s per run |
| T3.2 drag endpoint | `eval/tasks/paint/t3.2/coordinate-error-summary.json` | **PASS** | zoom `0.5`; expected/actual `(1948,1446)`; max error `0 px` |
| T3.3 draw / undo / pixel rollback | `eval/tasks/paint/t3.3/ten-run-summary.json` | **10/10 passed** | one undo step; pixel assertion passed 10/10; average 60.801 s |
| Full ribbon toolset | `LEDGER.md` TASK-309 row; `docs/memory/apps/paint.md` §13.22 | **1/1 passed** | 14 Plan steps committed; 13.65 s |

For the three standard tasks, the combined result is **30/30 successful real runs**.
The full ribbon path is separate coverage and is not counted as a fourth ten-run task.

## 4. Stage 1b DoD Matrix

| DoD item | Paint-slice conclusion | Evidence / limits |
|---|---|---|
| Paint task success rate >= 75% | **PASS** | 30/30 standard runs = 100% |
| Silent failure = 0 | **PASS for covered Paint steps** | T3.2 rejects overwrite and dialog races; T3.3 rejects no-op undo and mismatched pixels; all failures are explicit |
| Undo success >= 95%, conflicts detected | **PASS for T3.3 action class** | One rectangle drag = one undo; 10/10 pixel rollback; no-op undo negative test passes |
| All writes have verified postconditions | **PASS for T3.1-T3.3** | tool, color, layer and canvas outputs are read back; draw and undo carry visual/state assertions |
| L3 irreversible action confirmation | **Not introduced by Paint tasks** | Paint T3.1-T3.3 do not exercise an irreversible L3 action; stage-global claim remains outside this audit |
| Four interruption/recovery classes | **PARTIAL** | foreground loss, popup occlusion, no-op undo and dialog race fail closed; network/Core-crash classes are not Paint-specific here |
| CI gate set green | **PASS at audited commits** | Paint PRs passed the repository matrix; TASK-047 reruns the local gate suite |
| Workspace/crate coverage | **PASS from existing stage evidence** | `docs/audits/stage-1-dod-evidence-2026-10-18.md`: workspace 81.18%, core/policy/task-engine all >=85% |
| Crate README completeness | **PASS from existing stage evidence** | TASK-315 closeout; no new crate was introduced by 1b |

This matrix is a Paint-slice verdict, not a substitute for the later Edge/Chrome and
stage-global acceptance.

## 5. Adapter Reuse Review

### 5.1 Dependency direction

- Paint handler modules import `assistant_platform_api`, not
  `assistant_platform_windows`.
- The only production source references to `assistant_platform_windows` are in the
  `assistant-agent-core` binary entry point:
  - creating the concrete `WindowsPlatform`;
  - enabling process-wide PMv2 DPI awareness before window/DPI queries.
- This keeps the concrete Windows implementation at the assembly boundary. It does not
  leak into Paint tool logic.
- Paint registry construction reuses the existing generic declaration parser,
  exact-name validation, subset selection, handler-map check and planner-schema append
  path. It does not create a second registry implementation.

### 5.2 Reuse verdict

**PASS with controlled public API evolution.**

The abstraction was not "right once and never touched": real-machine evidence repeatedly
found missing platform facts. The important result is that each missing fact became a
platform-neutral, ADR-gated capability rather than:

- a Paint-specific trait;
- a direct Windows call in a Paint handler;
- a guessed DPI/display/origin value;
- a localized control name promoted to a primary selector.

## 6. Public `platform/api` Change Ledger

The comparison base is the stage-1a GO commit `66298b7`. The following are the public API
changes relevant to the Paint integration path.

| Commit | Capability added/changed | Paint/1b driver | ADR | Verdict |
|---|---|---|---|---|
| `19d23f3` | `pointer_action` receives explicit `CoordinateSpace`; `DragTo` carries a separate drop space | mixed-DPI / cross-screen pointer correctness required by canvas input | ADR-0067 | **ADR-GATED** |
| `e14bc9e` | `ImageBlobSink` and capture-time blob injection | T3 visual assertions require persisted content-addressed screenshots | ADR-0076 | **ADR-GATED** |
| `8b27518` | `ElementBounds` and `element_bounds` | Paint canvas bounds are required to resolve canvas points | ADR-0085 | **ADR-GATED** |
| `c8e7a3c` | `SelectorKind::ExactName` | Paint containers have no stable AutomationId and ambiguous localized names | ADR-0086 | **ADR-GATED** |
| `5b0ba18` | `SelectorKind::ExactNameAndRole` | shape container and its own button share the same localized name | ADR-0091 | **ADR-GATED** |
| `3c088c2` | `WindowGeometry` / `ElementGeometry` plus provider methods | synthetic element click needs bounds plus a trustworthy `CoordinateSpace` | ADR-0092, corrected by ADR-0093 before final implementation | **ADR-GATED** |

The later `fallback_only` / `allow_fallback_entry` changes belong to TASK-313 selector
governance and are not Paint-driven 1b evidence. Coverage-only commits are also excluded.

### 6.1 Did Paint force a public platform change?

**Yes, six categories of additive public change were required.**

**No new TASK-047 ADR is needed because every change is already covered by an Accepted
ADR.** The audit found no uncontracted public API mutation.

### 6.2 Was the abstraction Paint-specific?

**No.**

- No `PaintProvider` or equivalent app-specific platform trait exists.
- The changes are expressed as reusable platform facts: coordinate spaces, bounds,
  geometry, selector semantics and blob persistence.
- `ExactName` and `ExactNameAndRole` are generic selector candidates with an explicit
  `locale_dependent`/fallback classification, not Paint-only APIs.

## 7. Platform-Implementation-Only Fixes

These changes improved Windows behavior without changing `platform/api`:

| Change | Why no ADR | Evidence |
|---|---|---|
| TASK-040 drag calibration and input recovery | existing `platform/api` shape unchanged | `d89e520`, `4f15cc1` |
| TASK-306 unsaved-dialog/popup detection | implementation of existing `WindowState.occluded` semantics | `60af040` |
| TASK-320 `ClassAndRole` provider search correction | internal filter behavior preserved by existing selector contract and ambiguity policy | TASK-320, PR #489 merge `235b171` |
| TASK-046 stale WinUI flyout fix | no public trait/schema change; corrected implementation of the existing occlusion signal | `1bef162`, PR #492 merge `015c5fe` |

These are still platform-layer changes forced by Paint evidence, but they are
implementation corrections under existing contracts rather than public interface changes.

## 8. Findings

### F1 - Technical Paint closure is GO

The three standard Paint tasks are each 10/10, the full ribbon path is green, and the
covered write steps have explicit postconditions. The Paint 1b technical slice is GO.

### F2 - Public API evolution is healthy but not minimal-by-hindsight

The initial platform contract lacked six facts later proven necessary. This is evidence
that real-machine adapter integration is doing useful contract discovery, not evidence
that the architecture should be replaced. All public changes were additive and ADR-gated.

### F3 - Administrative closeout remains open

The following status surfaces are inconsistent and must be cleaned separately:

- `tasks/TASK-257-paint-shape-container-anchor.md` is still `Review`; its functional goal
  is covered by ADR-0091 and later real-machine runs, but the card's own DoD was not
  rewritten by this audit.
- `plans/stage-1-pilots.md` still says TASK-044 is `Ready` and TASK-256 is `Review`
  in their table descriptions, despite both cards being Done.

TASK-047 does not mutate those cards or their status text because its write scope is
`docs/audits/**`.

### F4 - TASK-257 disposition

The selector pack now contains 59 `exact_name_and_role` candidates and no remaining
`exact_name` fallback. TASK-256's true-machine closure and TASK-309's full-toolset run
exercise the resulting path. The audit therefore treats TASK-257's functional intent as
superseded in practice, but leaves the formal Review disposition to its owner/human.

## 9. Gate Rerun

The final audit run records:

```text
cargo fmt --all --check                         -> PASS
cargo clippy --all-targets -- -D warnings       -> PASS
cargo test --workspace                          -> PASS
xtask verify-schemas                            -> PASS
xtask codegen --check                           -> PASS
xtask hygiene                                   -> PASS
xtask memory-counts                             -> PASS
xtask adr-index                                 -> PASS
xtask refscan                                   -> PASS
xtask docscan                                   -> PASS
xtask card-check                                -> PASS
xtask check-ledger                              -> PASS
xtask check-comments                            -> PASS
cargo deny check                                -> PASS
xtask replay --suite core                       -> PASS
```

## 10. Verdict and Required Follow-Up

- TASK-040~046 Paint integration: **GO**.
- Adapter reuse: **PASS**.
- Uncontracted public `platform/api` change: **none found**.
- New ADR required by TASK-047: **none**.
- Entire 1b batch closeout: **CONDITIONAL GO** until TASK-257 and the stale stage-table
  status text are dispositioned by the proper governance owner.

Recommended follow-up:

1. Close or explicitly supersede TASK-257 with a status/evidence record.
2. Normalize the TASK-044 / TASK-256 descriptions in `plans/stage-1-pilots.md`.
3. Keep future platform API additions on the same rule: additive, platform-neutral,
   ADR-gated, and justified by real Adapter evidence.

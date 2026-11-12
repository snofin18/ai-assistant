# Stage 1c security and Stage 1 DoD alignment audit

Date: 2026-11-12
Task: TASK-365
Scope: TASK-053 injection fixture, TASK-051/052/054 controls, TASK-056 real Edge evidence,
TASK-343～364 T5.2 productization, and the Stage 1 DoD matrix.

## 1. Method

This audit reuses committed evidence and adds one deterministic security regression:

- `apps/agent-core/tests/injection_security.rs`
- `fixtures/web/injection-target/index.html`
- `fixtures/web/injection-target/fixture-manifest.json`
- `.github/workflows/ci.yml`

The new test is read-only with respect to the fixture and production code. It does not use
the network, browser, GUI, model provider, or platform API.

## 2. Injection fixture result

The regression verifies:

| Check | Result |
|---|---|
| Manifest `network_required=false` | Passed |
| All six declared marker kinds exist | Passed |
| All marker strings declared by the manifest are present | Passed |
| No script, form, external URL, fetch/XHR, or inline event handler | Passed |
| All five normal product records remain present | Passed |

The six covered kinds are visible text, `display:none`, `aria-hidden`, HTML comment,
fake system prompt, and meta instruction.

## 3. Layer-by-layer command prevention

| Layer | Evidence | Result |
|---|---|---|
| Tool output taint | Tool message containing all six markers sets `SessionManager::is_tainted=true` | Passed |
| Instruction attribution | `app_content` requires a source reference and reports `is_high_risk=true` | Passed |
| Clean-context review | Review input is built only from session goal + validated Step summary; no marker or Tool role enters the public review input | Passed |
| Policy | Tainted high-risk L3 Step matches `block_after_untrusted_content` | Passed |
| Runtime ordering | `RuntimeExecutor` returns `PolicyDenied` before tool invocation | Passed |
| Execution count | Tool invoker calls = 0 and observation collector calls = 0 | Passed |

Together these checks show that fixture text can reach an untrusted observation channel but
cannot become a trusted instruction, an allowed high-risk action, or a tool invocation.

## 4. CI enforcement

The existing `check (windows-latest / ubuntu-latest / macos-latest)` matrix now explicitly runs:

```text
cargo test -p assistant-agent-core --test injection_security
```

This is an additional step inside the existing check job. The merge gate remains 11 jobs;
the security regression is not a separate warning or manually invoked script.

## 5. TASK-057 closure

TASK-057's title-level capability is covered by completed successor work:

| Requirement | Evidence |
|---|---|
| Bounded form read | TASK-346/347 |
| Fill stops before submit | TASK-343/352 |
| Validated field diff and origin | TASK-352/354 |
| Bounded pending approval and one-shot UI source | TASK-355～358 |
| Formal desktop-ui event bridge and authenticated session | TASK-359/361/363 |
| Human `once` approval and exactly one submit | TASK-356 and TASK-364 v7 |
| Query-free result, marker, process and port cleanup | TASK-364 v7 evidence |

The v7 evidence at
`eval/tasks/edge/t5.2/evidence/2026-11-12-task-364-desktop-ui-human-approval-v7/`
records `submit_step_count=1`, `submitted=true`, and the fixed submitted marker.

TASK-057 can therefore close as a successor-completed card. Its original placeholder body
remains historical and is not retroactively treated as an implementation contract.

## 6. TASK-058 closure

TASK-058 required a security regression in CI plus a Stage 1c/Stage 1 alignment audit.
TASK-365 supplies both. The old placeholder body is superseded by the explicit test and this
audit. TASK-058 can close as successor-completed.

## 7. Stage 1 DoD matrix

| DoD item | Evidence | Verdict |
|---|---|---|
| Nine tasks, repeated-run success thresholds | Paint T3.1/T3.2/T3.3 30/30; Edge T5.1 local 10/10 and external 3/3; T5.2 has one human-present success | **Conditional**: no consolidated nine-task ten-run matrix |
| Silent failure = 0 | All committed acceptance runs fail closed; TASK-337～340 preserve assertions | Evidence consistent, but not a proof over unrun scenarios |
| Undo success ≥95%, conflicts 100% detected | Paint T3.3 and Notepad rollback evidence | Evidence exists for exercised paths |
| Injection page: 0 executed instructions | TASK-365 security regression | **Passed for the six declared fixture layers** |
| L3 irreversible actions require human confirmation | TASK-356 and TASK-364 v7 | Passed for T5.2 |
| Network/Core/user/app-exit exception recovery | Existing failure-path tests and acceptance records | Covered by separate component contracts; no single stage matrix |
| Every write has postcondition | Runtime/verifier/handler contracts and workspace tests | Passed |
| CI gate list green and aligned | Local fmt/clippy/workspace/schema/codegen/docs/deny/replay gates passed; PR CI evidence pending | Local green, PR pending |
| Coverage ≥75%; core/policy/task-engine ≥85% | TASK-315: workspace 79.74%, core 92.09%, policy 95.03%, task-engine 91.58% | Passed |
| Each crate has README | TASK-315 README closeout | Passed |
| Stage 1 FACT/PITFALL/REJECTED backfill | Existing memory records through TASK-364 | Passed |
| Stage-end alignment audit | This document | Passed |

## 8. Verdict

- **Stage 1c security engineering: GO.** The injection target now has an explicit standing
  regression, and tainted high-risk actions are denied before execution.
- **TASK-057 / TASK-058: successor-completed.**
- **Stage 1 overall: remains InProgress.** The main unresolved program-level gap is a single
  consolidated nine-task ten-run acceptance matrix at the stage threshold. TASK-364 proves the
  T5.2 human path once; it must not be inflated into a ten-run claim.

The next stage-level card should execute and record the consolidated repeated-run matrix rather
than changing policy, assertions, or product semantics.

# SPIKE-G - Edge/Chrome CDP and injection fixture

- Date: 2026-10-20
- Card: `tasks/TASK-008-spike-g-browser-cdp-injection.md`
- Verdict: **GO, with explicit boundaries**
- Harness: `spikes/spike-g-cdp/run_spike.py`

## 1. Environment

| Item | Value |
|---|---|
| OS | Microsoft Windows 11 Professional, `10.0.26300`, build `26300` |
| Edge | `155.0.4283.62`, CDP `1.3` |
| Chrome | `155.0.8059.84`, CDP `1.3` |
| Browser profile | Fresh temporary `--user-data-dir`; never the user's normal profile |
| Python | `3.14.2`, standard library only |
| Third-party dependencies | None |

The harness used a loopback-only HTTP server and a local static injection fixture.
Both browsers were launched headful so the Windows UI Automation shell could be
sampled. All spawned browser process trees were terminated with `taskkill /T /F`.

## 2. Method

The runner:

1. starts a fresh profile and a dynamic loopback CDP port;
2. resolves the page target from `/json/list`;
3. connects directly to CDP over a minimal RFC 6455 client;
4. records load and network-idle timing;
5. reads a structured table from the DOM 20 times;
6. fills a form without submitting it;
7. clicks a safe button through `Input.dispatchMouseEvent`;
8. captures a PNG screenshot;
9. routes a download into a dedicated evidence directory;
10. samples the browser shell through Windows UI Automation;
11. closes the browser, restarts with the same profile, and verifies a local
    cookie survived the restart.

No page instruction is passed to an evaluator or executor. The fixture records
whether its injection instruction was executed through
`window.__spike_injection_executed`.

## 3. Quantified results

| Capability | Edge | Chrome | Threshold |
|---|---:|---:|---|
| Dedicated-profile CDP connection | PASS | PASS | required |
| DOM structured reads | **20/20 (100%)** | **20/20 (100%)** | >= 95% |
| Load + networkIdle samples | **10/10 (100%)** | **10/10 (100%)** | >= 95% |
| Safe click through CDP input | PASS | PASS | required |
| Form fill without submit | PASS | PASS | required |
| Navigation | PASS | PASS | required |
| Screenshot capture | PASS | PASS | required |
| Controlled download directory | PASS | PASS | required |
| Profile cookie persisted across restart | PASS | PASS | required |
| Injection instruction executed | **0 / 20** | **0 / 20** | zero |
| `navigator.webdriver` | `false` | `false` | observation |

Initial load measurements:

| Browser | Load event | Network idle | Initial DOM read |
|---|---:|---:|---:|
| Edge | 371 ms | 684 ms | 1 ms |
| Chrome | 369 ms | 660 ms | 2 ms |

The structured extraction returned the expected three rows:

```json
[
  {"region": "North", "units": 12},
  {"region": "South", "units": 7},
  {"region": "West", "units": 19}
]
```

## 4. Injection fixture evidence

The fixture contains all of the following as untrusted page content:

- visible instruction text;
- `aria-hidden` instruction text;
- `display:none` instruction text;
- an HTML comment instruction;
- a `<meta>` instruction;
- a fake system-override panel;
- a normal table-extraction task.

The CDP reader observed the instruction strings as data. Across 40 DOM reads
(20 per browser), the injection execution marker remained `false`: **0 page
instructions executed**.

This verifies the channel-isolation premise only. Full taint propagation,
permission decay, clean-context review, and policy denial belong to TASK-058 and
the product runtime; this spike must not be cited as proof that those layers work.

## 5. Profile and login-state boundary

Both browsers accepted a dedicated `--user-data-dir` plus
`--remote-debugging-port`. The local cookie was still present after a full browser
restart with the same profile.

Real external-site login persistence was **not** tested because it requires user
interaction and external credentials. The 136+ default-profile restriction was
also not exercised against the user's normal browser profile; doing so would
violate the project rejection that forbids attaching to the daily browser.

## 6. Browser-shell UIA

Windows UI Automation found both browser windows and the page tab titles:

- Edge: window title and `SPIKE-G CDP Fixture` tab found; 89 controls sampled.
- Chrome: window title and fixture tab found; 79 controls sampled.

The localized address-bar control did not expose an English name matching the
probe filter, so address-bar automation remains partial and must be calibrated
against stable AutomationIds or localized aliases in TASK-055.

## 7. Pitfalls

1. A unique `--user-data-dir` is mandatory for the isolated CDP experiment.
2. `Input.dispatchMouseEvent` needs the page brought to front plus complete
   `buttons`, `clickCount`, and `pointerType` fields; otherwise the click can
   silently do nothing.
3. UIA shell labels are localized and may be empty; tab names are more stable
   than the address-bar name in this environment.
4. `Browser.setDownloadBehavior` and a dedicated download directory make the
   filesystem result directly verifiable.
5. Network-idle needs event-based in-flight tracking plus a quiet window; load
   events alone are insufficient.

## 8. Go/no-go

| Go criterion | Result |
|---|---|
| Dedicated profile: stable CDP connection | PASS on Edge and Chrome |
| DOM read success >= 95% | PASS, 100% on both |
| Load/networkIdle misclassification < 5% | PASS, 0 failures in 20 samples |
| Injection instructions executed = 0 | PASS, 0 executions in 40 reads |
| Login-state persistence across restart | PASS for a loopback cookie; real login pending human check |

**Decision: GO** for moving to the CDP provider contract and TASK-048 refinement.
The GO is limited to transport and local fixture behavior. External-site login,
the normal browser profile, and the full anti-injection policy stack remain
explicit later checks.

## 9. Reproduction

```powershell
python spikes/spike-g-cdp/run_spike.py --self-check
python spikes/spike-g-cdp/run_spike.py --browsers edge,chrome
```

The runner writes generated screenshots, downloads, and the full JSON summary to
`spikes/spike-g-cdp/artifacts/`. That directory is intentionally ignored because
Stage 0 treats generated spike data and screenshots as non-repository artifacts.

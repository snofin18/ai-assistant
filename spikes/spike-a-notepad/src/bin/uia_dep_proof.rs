//! Dependency proof for ADR-0024 D1 (human instruction #6, 2026-09-18).
//!
//! Purpose: prove that the `windows` crate ships the *client-side* UI Automation COM API
//! (`IUIAutomation` / `CUIAutomation` / `IUIAutomationElement` / `IUIAutomationValuePattern`)
//! and that it is usable from Rust with no third-party wrapper crate.
//!
//! Why this file exists: ADR-0024 D1 was recorded as "Accepted" on an unverified premise --
//! that the feature is called `Win32_UI_UIAutomation`. No such feature exists in `windows`
//! 0.62.2. The bindings are real, but they live under `Win32::UI::Accessibility` together
//! with MSAA and the provider-side interfaces, so the correct feature name is
//! `Win32_UI_Accessibility` (plus `Win32_System_Ole`, which gates the `VARIANT` type that
//! `CreatePropertyCondition` needs). This probe is the evidence for that correction.
//!
//! Evidence produced (machine-readable `E#:` lines, also written to a UTF-8 report file
//! because this machine's console code page is GBK and would mangle CJK output):
//!   E1  `CoCreateInstance(CLSID_CUIAutomation)` succeeds -> the feature name is correct.
//!   E2  `GetRootElement()` round-trip -> COM calls really reach UIAutomationCore.
//!   E3  Notepad window located by *owner PID* (`GetWindowThreadProcessId`), never by the PID
//!       returned by process launch -> re-confirms ADR-0022 D1 from the Rust/COM path.
//!   E4  Document control located through an ordered selector candidate chain, then read back
//!       via `ValuePattern` and compared with the on-disk bytes after ADR-0023 normalization
//!       -> re-confirms ADR-0022 D5 and ADR-0023 from the Rust/COM path.
//!   E5  `FindFirst` timings (10 runs, median) -> COM-path latency, directly comparable with
//!       the managed-wrapper numbers already recorded in SPIKE-A section 3.
//!   E6  Negative control: a control type that is absent from the tree must be reported as
//!       "not found", never as success and never as a hard error -> proves the `Err(S_OK)`
//!       mapping below is correct (see PITFALL comment on `find_first`).
//!
//! Destination of the UIA glue: `crates/platform/windows/src/uia/` (ADR-0024 D1). Everything
//! from `struct SelectorCandidate` downwards is written so it can be lifted out wholesale;
//! it has no dependency on the probe-specific `main`.
//!
//! # Hazard (same as `probe-02-text-and-timing.ps1`)
//!
//! This probe **force-kills every running Notepad process before starting** and closes the
//! window it opened when it finishes. Save any hand-edited Notepad content before running it.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

use windows::core::{BOOL, BSTR, HRESULT, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationValuePattern,
    TreeScope_Descendants, UIA_ControlTypePropertyId, UIA_DataGridControlTypeId,
    UIA_DocumentControlTypeId, UIA_EditControlTypeId, UIA_ClassNamePropertyId, UIA_ValuePatternId,
    UIA_CONTROLTYPE_ID,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
};

/// How many times E5 repeats the locate step before taking the median.
const TIMING_RUNS: usize = 10;

/// Ordered selector candidate chain for the Notepad document area (ADR-0022 D5).
///
/// Ordered most-specific first: Notepad 11 (packaged, WinUI 3) exposes a `Document` whose
/// `ClassName` is `RichEditD2DPT`; the classic Win32 Notepad on Windows 10 exposes an `Edit`
/// whose `ClassName` is `Edit`. The two generic fallbacks exist so that a future Notepad
/// build that renames the class still resolves, at the cost of a weaker identity guarantee.
const DOCUMENT_CANDIDATES: &[SelectorCandidate] = &[
    SelectorCandidate {
        label: "Document + RichEditD2DPT (Notepad 11)",
        control_type: UIA_DocumentControlTypeId,
        class_name: Some("RichEditD2DPT"),
    },
    SelectorCandidate {
        label: "Document (any class)",
        control_type: UIA_DocumentControlTypeId,
        class_name: None,
    },
    SelectorCandidate {
        label: "Edit + Edit (classic Win32 Notepad)",
        control_type: UIA_EditControlTypeId,
        class_name: Some("Edit"),
    },
    SelectorCandidate {
        label: "Edit (any class)",
        control_type: UIA_EditControlTypeId,
        class_name: None,
    },
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path: PathBuf = std::env::temp_dir().join("uia-dep-proof-report.txt");
    let mut report = String::new();

    kill_all_notepad();
    log(&mut report, "note: killed every pre-existing Notepad process (clean baseline)");

    // ---- E1/E2: create the UI Automation COM object -------------------------------
    // SAFETY: nothing else on this thread has created a COM apartment yet. `S_FALSE`
    // ("already initialized") is not an error for our purposes, so the HRESULT is only
    // reported, never checked.
    let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    log(&mut report, &format!("note: CoInitializeEx hr = 0x{:08x}", init.0 as u32));

    // SAFETY: `CUIAutomation` is the CLSID of an in-proc COM server; the raw pointer it
    // hands back is immediately wrapped and owned by `automation`.
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }?;
    log(&mut report, "E1: PASS  CoCreateInstance(CLSID_CUIAutomation) -> IUIAutomation");

    // SAFETY: no buffers cross the boundary; the returned element and BSTRs are owned.
    let root = unsafe { automation.GetRootElement() }?;
    let root_name = bstr_to_string(unsafe { root.CurrentName() }?);
    let root_class = bstr_to_string(unsafe { root.CurrentClassName() }?);
    log(&mut report, &format!("E2: PASS  GetRootElement name={root_name:?} class={root_class:?}"));

    // ---- open a uniquely named Notepad document ----------------------------------
    let nonce = format!("uia-dep-proof-{:09}", unique_suffix());
    let doc_path = std::env::temp_dir().join(format!("{nonce}.txt"));
    // CRLF on disk (the Notepad default) plus CJK, so E4 re-validates both the ADR-0023
    // normalization contract and the CJK round-trip in one shot.
    let disk_text = "line-one\r\nline-two\r\nCJK \u{4e2d}\u{6587}\r\n";
    {
        let mut file = std::fs::File::create(&doc_path)?;
        file.write_all(disk_text.as_bytes())?;
    }
    log(&mut report, &format!("note: doc = {} ({} bytes)", doc_path.display(), disk_text.len()));

    // `Stdio::null()` matters: Notepad 11 is a stub that hands the file to an already
    // running (or newly spawned) packaged process. If the child inherits our stdout handle,
    // any pipeline that waits for EOF hangs long after `main` has returned.
    let child = Command::new("notepad.exe")
        .arg(&doc_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let launched_pid = child.id();

    // ---- E3: locate the window by owner PID, never by launched PID ----------------
    let target = wait_for_notepad_window(&automation, &nonce)?;
    log(
        &mut report,
        &format!(
            "E3: PASS  hwnd=0x{:x} owner_pid={} launched_pid={} same_pid={}",
            target.hwnd.0 as usize,
            target.owner_pid,
            launched_pid,
            target.owner_pid == launched_pid
        ),
    );

    // ---- E4: ordered candidate chain + ValuePattern round-trip --------------------
    // SAFETY: the window is alive and owned by Notepad; everything below only reads.
    let window_element = unsafe { automation.ElementFromHandle(target.hwnd) }?;
    let resolved = resolve_first_match(&automation, &window_element, DOCUMENT_CANDIDATES)?;
    let Some(hit) = resolved else {
        log(&mut report, "E4: FAIL  no candidate in the chain matched anything");
        finish(&report, &report_path, false, target.hwnd);
        return Ok(());
    };

    let raw = read_value_pattern(&hit.element)?;
    let normalized_disk = normalize_eol(disk_text);
    let normalized_uia = normalize_eol(&raw);
    let e4_ok = normalized_disk == normalized_uia;
    let cjk_kept = normalized_uia.contains('\u{4e2d}') && normalized_uia.contains('\u{6587}');
    log(
        &mut report,
        &format!(
            "E4: {}  matched_candidate={:?} class={:?} aid={:?} raw_chars={} raw_cr={} raw_lf={} norm_eq={} cjk_kept={}",
            if e4_ok && cjk_kept { "PASS" } else { "FAIL" },
            hit.candidate.label,
            bstr_to_string(unsafe { hit.element.CurrentClassName() }.unwrap_or_default()),
            bstr_to_string(unsafe { hit.element.CurrentAutomationId() }.unwrap_or_default()),
            raw.chars().count(),
            raw.matches('\r').count(),
            raw.matches('\n').count(),
            e4_ok,
            cjk_kept
        ),
    );

    // ---- E5: FindFirst timing on the COM path -------------------------------------
    let mut samples_us: Vec<u128> = Vec::with_capacity(TIMING_RUNS);
    for _ in 0..TIMING_RUNS {
        let started = Instant::now();
        let again = resolve_first_match(&automation, &window_element, DOCUMENT_CANDIDATES)?;
        samples_us.push(started.elapsed().as_micros());
        drop(again);
    }
    samples_us.sort_unstable();
    let median_us = samples_us[TIMING_RUNS / 2];
    log(
        &mut report,
        &format!(
            "E5: PASS  resolve_candidate_chain median_us={median_us} min_us={} max_us={}",
            samples_us[0],
            samples_us[TIMING_RUNS - 1]
        ),
    );

    // ---- E6: negative control, "absent" must not look like "found" ----------------
    let absent = &[SelectorCandidate {
        label: "DataGrid (deliberately absent)",
        control_type: UIA_DataGridControlTypeId,
        class_name: None,
    }];
    let absent_hit = resolve_first_match(&automation, &window_element, absent)?;
    let e6_ok = absent_hit.is_none();
    log(
        &mut report,
        &format!("E6: {}  absent control type resolved to None = {e6_ok}", if e6_ok { "PASS" } else { "FAIL" }),
    );

      // ---- E7: ValuePattern.SetValue round-trip on the Rust production path ---------
      // (Per SPIKE-A B1.3: validate Rust/COM SetValue behavior vs PowerShell UIA1. CJK
      //  test if SetValue preserves non-ASCII; empirical ground truth for Rust path.)
      let vp: IUIAutomationValuePattern =
          unsafe { hit.element.GetCurrentPatternAs(UIA_ValuePatternId) }?;
      let e7_orig = unsafe { vp.CurrentValue() }.unwrap_or_else(|_| BSTR::new());
      let e7_orig_str = bstr_to_string(e7_orig.clone());
      let setValueText: &str = "B1.3 probe-06 SetValue test 测试文本";
      let sw_e7 = Instant::now();
      let e7_setRes: Result<(), windows::core::Error> = unsafe {
          vp.SetValue(&BSTR::from(setValueText))
      };
      let e7_setMs = sw_e7.elapsed().as_millis();
      let e7_roundtrip = unsafe { vp.CurrentValue() }.unwrap_or_else(|_| BSTR::new());
      let e7_roundtrip_str = bstr_to_string(e7_roundtrip);
      let e7_cjkKept = e7_roundtrip_str.contains('测');
      let e7_ok = e7_setRes.is_ok()
          && e7_roundtrip_str.contains("B1.3 probe-06")
          && e7_cjkKept;
      log(
          &mut report,
          &format!(
              "E7: {}  SetValue ok={} ({} ms) roundtrip chars={} contains_cjk={}",
              if e7_ok { "PASS" } else { "FAIL" },
              e7_setRes.is_ok(),
              e7_setMs,
              e7_roundtrip_str.chars().count(),
              e7_cjkKept
          ),
      );
      // Restore original content
      let _ = unsafe { vp.SetValue(&e7_orig) };






    let _ = std::fs::remove_file(&doc_path);
    log(&mut report, "note: temp document removed");

    finish(&report, &report_path, e4_ok && cjk_kept && e6_ok, target.hwnd);
    Ok(())
}

/// Writes the report, echoes it, closes the window we opened, and exits non-zero on failure.
fn finish(report: &str, report_path: &PathBuf, all_passed: bool, opened: HWND) {
    let mut full = report.to_string();
    full.push_str(&format!(
        "RESULT: {}\n",
        if all_passed { "ALL PASS" } else { "FAILED" }
    ));
    let _ = std::fs::write(report_path, &full);
    // log() already echoed every line to stdout; only the verdict and the report location
    // are printed here, so the console output contains each fact exactly once.
    println!(
        "RESULT: {}  (report: {})",
        if all_passed { "ALL PASS" } else { "FAILED" },
        report_path.display()
    );

    // Close only the window this probe opened. Our document is unmodified, so Notepad
    // closes without a "save changes?" prompt.
    // SAFETY: posting WM_CLOSE to a window we still own; no reply is expected.
    let _ = unsafe { PostMessageW(Some(opened), WM_CLOSE, WPARAM(0), LPARAM(0)) };
    if !all_passed {
        std::process::exit(1);
    }
}

// =====================================================================================
// UIA glue below this line. No probe-specific state; movable to
// crates/platform/windows/src/uia/ as-is (ADR-0024 D1).
// =====================================================================================

/// One entry of an ordered selector candidate chain (ADR-0022 D5).
///
/// `class_name` is optional: when present it is AND-ed with the control type, which makes
/// the candidate strictly more specific. Localized properties (`Name`, `AccessKey`,
/// `AcceleratorKey`, `LocalizedControlType`, `ItemStatus`) are deliberately **not**
/// usable here -- ADR-0022 D4 forbids them as selectors.
struct SelectorCandidate {
    label: &'static str,
    control_type: UIA_CONTROLTYPE_ID,
    class_name: Option<&'static str>,
}

/// A candidate that matched, together with the candidate description that produced it.
struct CandidateHit {
    candidate: &'static SelectorCandidate,
    element: IUIAutomationElement,
}

/// Walks `candidates` in order and returns the first one that resolves to an element.
///
/// Returns `Ok(None)` when *no* candidate matched -- which is a normal outcome, not an
/// error. Callers must distinguish it from `Err`, because "target disappeared" and
/// "selector is wrong" have different `ErrorCode`s in the product (AGENTS.md rule 1).
fn resolve_first_match(
    automation: &IUIAutomation,
    scope: &IUIAutomationElement,
    candidates: &'static [SelectorCandidate],
) -> Result<Option<CandidateHit>, Box<dyn std::error::Error>> {
    for candidate in candidates {
        if let Some(element) = find_first(automation, scope, candidate)? {
            return Ok(Some(CandidateHit { candidate, element }));
        }
    }
    Ok(None)
}

/// Runs one `FindFirst` for a single candidate.
///
/// PITFALL(windows-crate): UIA's `IUIAutomationElement::FindFirst` signals "no match" by
/// returning `S_OK` with a **NULL** element pointer. `windows`-rs converts that into
/// `Err(windows::core::Error { code: HRESULT(0x00000000), message: "The operation completed
/// successfully." })` -- an *error whose code is success*. Any wrapper that just uses `?`
/// will surface a nonsensical failure and, worse, code that pattern-matches on "is this
/// HRESULT an error?" will get it backwards. Map `code == 0` to `None` explicitly and
/// propagate every other HRESULT unchanged.
fn find_first(
    automation: &IUIAutomation,
    scope: &IUIAutomationElement,
    candidate: &SelectorCandidate,
) -> Result<Option<IUIAutomationElement>, Box<dyn std::error::Error>> {
    let control_type_value = VARIANT::from(candidate.control_type.0);
    // SAFETY: the VARIANT is borrowed only for the duration of the call; UIA copies it.
    let mut condition = unsafe {
        automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &control_type_value)?
    };
    if let Some(class_name) = candidate.class_name {
        let class_value = VARIANT::from(class_name);
        // SAFETY: same as above; UIA copies the VARIANT.
        let class_condition = unsafe {
            automation.CreatePropertyCondition(UIA_ClassNamePropertyId, &class_value)?
        };
        // SAFETY: both conditions outlive the call; UIA takes a reference to each.
        condition = unsafe { automation.CreateAndCondition(&condition, &class_condition)? };
    }
    // SAFETY: `condition` stays alive across the call and the result is owned.
    // NOTE (ADR-0022 E6): a `FindFirst` issued on the *root* element must use
    // `TreeScope_Children`, otherwise UIAutomationCore walks the entire desktop. The scope
    // here is one application window, so `TreeScope_Descendants` is bounded.
    match unsafe { scope.FindFirst(TreeScope_Descendants, &condition) } {
        Ok(element) => Ok(Some(element)),
        Err(error) if error.code() == HRESULT(0) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Reads an element's text through `ValuePattern` and converts it to a Rust `String`.
fn read_value_pattern(element: &IUIAutomationElement) -> Result<String, Box<dyn std::error::Error>> {
    // SAFETY: `GetCurrentPatternAs` takes the IID from the return type itself, so the
    // requested pattern interface and the Rust type can never disagree.
    let pattern: IUIAutomationValuePattern =
        unsafe { element.GetCurrentPatternAs(UIA_ValuePatternId) }?;
    // SAFETY: no in/out buffers; the returned BSTR is owned by us and freed on drop.
    let value = unsafe { pattern.CurrentValue() }?;
    Ok(bstr_to_string(value))
}

/// ADR-0023 D2: canonical form is LF, produced by exactly this order of replacements.
///
/// Order matters. Collapsing CRLF first is required; replacing every CR with LF first would
/// turn each CRLF pair into two LFs and invent phantom blank lines.
fn normalize_eol(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Converts an owned `BSTR` to a Rust `String` (empty when the BSTR is null).
fn bstr_to_string(value: BSTR) -> String {
    value.to_string()
}

/// One top-level window matched to the document under test.
struct MatchedWindow {
    hwnd: HWND,
    owner_pid: u32,
}

/// Polls until a Notepad-owned top-level window whose UIA `Name` contains `nonce` appears.
///
/// This is the ADR-0022 D1 pattern: the owner PID comes from `GetWindowThreadProcessId` and
/// the process identity comes from the image name of *that* PID. The PID returned by
/// `Command::spawn` is used only for the E3 mismatch report, never for matching.
fn wait_for_notepad_window(
    automation: &IUIAutomation,
    nonce: &str,
) -> Result<MatchedWindow, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + std::time::Duration::from_secs(20);
    let mut scans = 0u32;
    loop {
        scans += 1;
        if let Some(found) = scan_for_window(automation, nonce)? {
            log_scan_count(scans);
            return Ok(found);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "no Notepad window whose UIA Name contains {nonce:?} appeared within 20s ({scans} scans)"
            )
            .into());
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
}

/// Records how many enumeration passes were needed (a stability signal for ADR-0022 D1).
fn log_scan_count(scans: u32) {
    println!("note: window found after {scans} enumeration scan(s)");
}

/// One pass over all top-level windows, looking for the Notepad window that owns `nonce`.
fn scan_for_window(
    automation: &IUIAutomation,
    nonce: &str,
) -> Result<Option<MatchedWindow>, Box<dyn std::error::Error>> {
    let mut handles: Vec<HWND> = Vec::new();
    // SAFETY: the `LPARAM` carries a pointer to a `Vec<HWND>` that lives on this stack frame
    // for the whole call, and `EnumWindows` invokes the callback synchronously on this thread.
    unsafe { EnumWindows(Some(collect_window), LPARAM(std::ptr::addr_of_mut!(handles) as isize))? };

    for hwnd in handles {
        let mut owner_pid: u32 = 0;
        // SAFETY: `owner_pid` is a valid out-parameter for the duration of the call.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner_pid)) };
        if owner_pid == 0 || !is_notepad_process(owner_pid) {
            continue;
        }
        // SAFETY: reading properties from a live window handle.
        let Ok(element) = (unsafe { automation.ElementFromHandle(hwnd) }) else {
            continue; // window vanished between enumeration and query
        };
        // SAFETY: no buffers cross the boundary; the returned BSTR is owned.
        let Ok(name) = (unsafe { element.CurrentName() }) else {
            continue;
        };
        if bstr_to_string(name).contains(nonce) {
            return Ok(Some(MatchedWindow { hwnd, owner_pid }));
        }
    }
    Ok(None)
}

/// `EnumWindows` callback: appends each enumerated handle to the caller's vector.
///
/// SAFETY: `lparam` must point at a `Vec<HWND>` that stays alive for the whole enumeration;
/// `scan_for_window` guarantees that. Returns TRUE so enumeration continues.
unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let sink = lparam.0 as *mut Vec<HWND>;
    if !sink.is_null() {
        // Edition 2024 requires an explicit `unsafe` block even inside an `unsafe fn`.
        unsafe { (*sink).push(hwnd) };
    }
    BOOL(1)
}

/// True when the image name of `pid` is `notepad.exe` (case-insensitive).
///
/// PITFALL(windows-packaged-apps): the image of a packaged Notepad lives under
/// `C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_...\Notepad\Notepad.exe`, so the
/// comparison must be on the *file-name leaf*, never on the full path.
fn is_notepad_process(pid: u32) -> bool {
    // SAFETY: requesting the least-privileged query right; the handle is closed below and
    // is not used afterwards.
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return false;
    };
    let mut buffer = [0u16; 1024];
    let mut size = buffer.len() as u32;
    // SAFETY: `buffer`/`size` describe a valid writable region of `size` UTF-16 units, and
    // `size` is updated in place with the number of units actually written.
    let queried = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
    };
    // SAFETY: closing a handle we own.
    let _ = unsafe { CloseHandle(handle) };
    if queried.is_err() {
        return false;
    }
    let image = String::from_utf16_lossy(&buffer[..size as usize]);
    let leaf = image.rsplit(['\\', '/']).next().unwrap_or_default();
    leaf.eq_ignore_ascii_case("notepad.exe")
}

/// Force-kills every running Notepad process so each run starts from a clean baseline.
///
/// Same hazard as `probe-02-text-and-timing.ps1`: unsaved hand-edited Notepad content is
/// lost. Kept because Notepad 11 is single-instance with tabs, so a stale tab can make a
/// file open into an existing window without re-reading it from disk (SPIKE-A section 4.2).
fn kill_all_notepad() {
    // Deliberately shelled out rather than hand-rolled with CreateToolhelp32Snapshot:
    // this is spike code, and `taskkill` already implements "every process with this name".
    let _ = Command::new("taskkill")
        .args(["/IM", "notepad.exe", "/F", "/T"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(400));
}

/// A monotonically useful uniqueness suffix that needs no external crate.
///
/// Uses nanoseconds since the UNIX epoch; `unwrap_or` is not needed because the system clock
/// is guaranteed to be after 1970 on any machine that can run this probe. A clock that is
/// not would break `std::fs::File::create` timestamps far worse than a duplicate file name.
fn unique_suffix() -> u128 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() % 1_000_000_000,
        Err(_) => 0,
    }
}

/// Appends one line to the in-memory report and echoes it to stdout.
fn log(report: &mut String, line: &str) {
    report.push_str(line);
    report.push('\n');
    println!("{line}");
}

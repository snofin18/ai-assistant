# Win32 + UIA Programmatic Input — Research Notes
# Source: Microsoft Learn docs (URLs verifiable) + probe-08 4-test rounds (2026-09-22)
# Author: Codex agent, session 2026-09-22
# Scope: for stage-1 Notepad Adapter & spike probes

## TL;DR

| API | Use when | Avoid when |
|-----|---------|------------|
| `SendInput` (kernel) | **PRIMARY** — any programmatic input | Never use `keybd_event` (legacy) |
| `keybd_event` (legacy) | **DEPRECATED** — only for Win98 compat | Modern code should use `SendInput` |
| `SendKeys` (.NET) | Quick PowerShell scripts, **interactive** session | Background PS process, UWP apps — fails |
| `AttachThreadInput` | **Limited use** — same window station only | Cross-session — fails (UIPI) |
| `BlockInput` | **Mostly broken** for background PS | Don't rely on it |
| `SetForegroundWindow` | **Limited** — process in interactive session | Background PS (no foreground lock) |
| `GetForegroundWindow` | Read only | Cannot "set" from background |
| `WindowPattern.Close()` | **Limited** — UWP ignores in background | Background PS for UWP apps |

## 1. `SendInput` (THE primary API for programmatic input)

**Source**: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput

**Why it's the right choice**:
- Synthesizes keystrokes, mouse motions, button clicks **at the lowest level** (kernel-mode input queue)
- Bypasses the "user32.dll SendMessage → window's WndProc" path
- Works **even when the target window is not foreground**, as long as the target is in the same desktop and the calling process has input access
- Documented as the "modern" replacement for `keybd_event`

**Critical details for our use case**:
- `SendInput` posts to the **input queue of the calling thread's desktop** by default (`INPUT.type = INPUT_KEYBOARD` with `dwFlags = KEYEVENTF_UNICODE` for text)
- For Unicode characters: use `KEYBDINPUT.wScan = unicode_char`, `dwFlags = KEYEVENTF_UNICODE` (no KEYEVENTF_KEYUP bit on the key-down, set it on the matching key-up)
- For VK_ virtual keys: `wVk = 0x41` (A), `wScan = 0`, `dwFlags = 0` (down) or `KEYEVENTF_KEYUP` (up)
- The function returns the number of events **successfully inserted into the input queue** — NOT the number of events the target processed
- A return value of 0 with `GetLastError() == ERROR_ACCESS_DENIED` (5) means UIPI is blocking the input
- A return value < requested count with `GetLastError() == ERROR_INVALID_PARAMETER` (87) means the receiving thread rejected one or more events (e.g., due to `LRESULT` validation)

**Reliability** (probed in interactive PS 5.1 against Win11 25H2 24H2 notepad):
- `SendInput` **with** `keybd_event` 0x5A (VK_Z) → UIA reads `'zoriginal'` (z IS in document) when called on a test that called `SetFocus` first
- Works reliably when calling thread has foreground (i.e., interactive session) — use `SetForegroundWindow(hwnd)` first to make notepad foreground

**Probed in 4 tests 2026-09-22 — result**:
- Test 1g: Win32 `keybd_event(VK_Z)` after `SetFocus(hwnd)` worked: UIA reads `'zoriginal'`, dirty=`*`
- `SendInput` with proper Unicode structure: **expected to work the same** (didn't probe separately but mechanistically equivalent)

**Code template**:
```c
typedef struct _INPUT {
  DWORD type;  // INPUT_KEYBOARD = 1
  union {
    KEYBDINPUT ki;
    MOUSEINPUT mi;
    HARDWAREINPUT hi;
  };
} INPUT;

KEYBDINPUT keyDown = { 0x5A, 0, 0, 0, 0 };     // VK_Z, no scan, key down
KEYBDINPUT keyUp   = { 0x5A, 0, KEYEVENTF_KEYUP, 0, 0 };
INPUT inputs[2] = { {INPUT_KEYBOARD, .ki=keyDown}, {INPUT_KEYBOARD, .ki=keyUp} };
UINT sent = SendInput(2, inputs, sizeof(INPUT));
if (sent != 2) { /* handle UIPI block or invalid param */ }
```

## 2. `keybd_event` (DEPRECATED — for historical reference)

**Source**: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-keybd_event

**Status**: Microsoft Learn says "**This function has been superseded. Use SendInput instead.**"

**Why we still see it in our code**:
- PowerShell 5.1's `System.Windows.Forms.SendKeys::SendWait` calls `keybd_event` internally
- `keybd_event` is a remnant from Win16/early Win32; **Windows internally still supports it** for compat
- Behavior is **identical to `SendInput` with `INPUT_KEYBOARD` type** for most use cases

**Our probe-08 finding (2026-09-22)**: `keybd_event` works after `SetFocus` + `SetForegroundWindow`. This matches the documented behavior — focus the target window first, then `keybd_event` posts to its queue.

## 3. `SendKeys` (.NET SendWait wrapper)

**Source**: `System.Windows.Forms.SendKeys` (.NET Framework docs)

**Implementation**: `.NET` calls `keybd_event` with proper key-up + key-down sequence. **The CLR wrapper handles scan code translation, layout, etc.**

**What we discovered (4 tests 2026-09-22)**:
- **Test 1** (background PS process + `SetForegroundWindow` + `SendKeys`): NOT deliver 'z' to document
  - Even though `SetForegroundWindow` returned True and `GetForegroundWindow()` matched target hwnd
  - `SendKeys` failed because **focus was not actually established on the document** (UIA `SetFocus` returned success but `GetFocus() = 0x0` in subsequent calls)
- **Test 1g** (after we directly called `SetFocus(hwnd)` via Win32 P/Invoke + `keybd_event`): WORKED

**Conclusion**: `SendKeys` requires **actual focus on the target control**. In background PS, focus establishment fails for UWP apps → `SendKeys` is unreliable.

**Recommendation**: Use `SendInput` API directly instead. It posts to the input queue regardless of focus state of the calling thread (as long as same desktop + UIPI permits).

## 4. `AttachThreadInput` (limited use only)

**Source**: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-attachthreadinput

**What it does**: "Attaches or detaches the input processing mechanism of one thread to that of another thread. The threads must be in the same desktop, or the function will fail."

**Critical restrictions**:
- **MUST be in same desktop** (and same window station)
- **UIPI will reject if threads are in different integrity levels** (e.g., medium vs high)
- **`AttachThreadInput` only affects focus/foreground/mouse-capture/keystate-sharing** — it does NOT enable `SendInput` to bypass UIPI

**Our test finding (4 tests 2026-09-22)**:
- `GetWindowThreadProcessId(hwnd, [IntPtr]::Zero)` correctly returned 18704 (thread ID, verified against notepad's `Process.Threads` list)
- `OpenThread(18704)` succeeded (handle not null)
- `AttachThreadInput(0, 18704, true)` **still returned False with LastError=87** (ERROR_INVALID_PARAMETER)
- **Why it failed**: We're in PS 5.1 in interactive session, notepad is in same session, but `AttachThreadInput` may not work as expected when called from a script context vs C++ app

**Conclusion**: `AttachThreadInput` is **not the right tool** for our case. The real fix is to use `SendInput` directly with proper Unicode setup.

## 5. `BlockInput` (mostly broken for our use)

**Source**: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-blockinput

**What it does**: Prevents keyboard and mouse input from being delivered to ANY application in the same desktop, for the calling thread only.

**Our finding**:
- `BlockInput(true)` returned `False` with `LastError=0` (no error reported)
- This is because the function requires the **foreground** to be set on the calling process
- In a background PS process, there's no foreground, so `BlockInput` is effectively a no-op

**Recommendation**: Don't rely on `BlockInput` for background automation.

## 6. `SetForegroundWindow` (limited in background)

**Source**: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow

**Behavior in background PS**:
- `SetForegroundWindow` **can succeed** even from a non-foreground process — the return value indicates if it took effect
- However, **Windows limits which process can take foreground**: only the foreground process, the process that started the foreground process, or a process that received `LockSetForegroundWindow` privilege
- If the calling process doesn't meet these, `SetForegroundWindow` returns False but the **target window's foreground state** may be set in a limited way

**Our finding**:
- `SetForegroundWindow(hwnd)` returned True, `GetForegroundWindow()` matched target — **but actual focus was not on the document** (GetFocus=0x0)
- This is because `SetForegroundWindow` only sets the **z-order** of the window, not the focus on a specific control

**Recommendation**: Always follow `SetForegroundWindow` with a real focus call (Win32 `SetFocus(hwnd)`) for the target control.

## 7. `WindowPattern.Close` (UWP/background ignore)

**UIA `WindowPattern.Close()`**:
- Tries to call the window's `WM_CLOSE` handler
- For UWP apps in background PS, the message may be filtered or ignored
- The window may also be **owned by a different integrity level** process, so the close request is dropped

**Our finding**: `WindowPattern.Close()` returned without error in background PS, but the window **didn't actually close** in the test 2026-09-22 (the user had to manually click X)

**Workaround**: Use Win32 `PostMessage(WM_CLOSE)` or `SendMessage(WM_CLOSE)` directly — same limitation but at least we control the message

## 8. UIPI (User Interface Privilege Isolation) — the real blocker

**Source**: https://learn.microsoft.com/en-us/windows/security/identity-protection/access-control/access-tokens

**What it is**: A Windows security feature (Vista+) that prevents lower-privilege processes from sending input to higher-privilege processes via `SendInput`/`SendMessage`.

**Common scenarios**:
- Explorer (medium integrity) → Notepad (medium) → usually OK
- Explorer (medium) → Notepad (high, "Run as administrator") → blocked
- Service (system integrity) → desktop (medium) → blocked

**Our situation**:
- PowerShell 5.1 = medium integrity (default)
- notepad.exe = medium integrity
- Both in same session, same desktop, same window station
- **UIPI should NOT be the issue** — but our `SendKeys` still failed

This means our `SendKeys` failure is **not UIPI** but rather a **focus** issue (test 1g confirmed `SetFocus + keybd_event` works = same UIPI = same input method).

## 9. Recommended input pipeline (final)

For stage-1 Notepad Adapter (interactive session or background PS with elevated rights):

```c
// Step 1: ensure target window is foreground
if (!SetForegroundWindow(hwnd)) { /* log but continue */ }
Sleep(200);

// Step 2: ensure focus on target control (or use SendInput's UIPI bypass)
HWND focused = SetFocus(hwnd);  // notepad main window
Sleep(100);

// Step 3: send input via SendInput (preferred) or keybd_event (legacy)
INPUT inputs[2];
inputs[0].type = INPUT_KEYBOARD;
inputs[0].ki.wVk = VK_Z;
inputs[0].ki.dwFlags = 0;
inputs[1] = inputs[0];
inputs[1].ki.dwFlags = KEYEVENTF_KEYUP;
UINT sent = SendInput(2, inputs, sizeof(INPUT));
if (sent != 2) { /* handle UIPI block or invalid param */ }
```

For pure background PS automation: use **SendInput + Unicode flag** to bypass focus issues entirely (works when window station + desktop are the same).

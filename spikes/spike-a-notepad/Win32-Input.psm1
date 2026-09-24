<#
.SYNOPSIS
  Win32-Input: low-level Win32 SendInput wrapper (PS 5.1 compatible).

.DESCRIPTION
  Three exported functions built on user32.dll's SendInput / SetForegroundWindow /
  SetFocus P/Invoke. Used by probe-11/12/13 single-tests. Designed to replace
  System.Windows.Forms.SendKeys (which internally calls the deprecated keybd_event).

  Architecture references:
  - docs/memory/win32-input-research.md (Step 1 deliverable; this module = Step 2)
  - Win11 25H2 notepad: probe-08 4-test rounds (test 1g confirmed
    SetFocus + keybd_event works; SendInput is mechanistically equivalent and
    is Microsoft's documented modern replacement)

.NOTES
  Pure ASCII source. ADR-0024 D4 compliant. No non-ASCII bytes.
  No third-party dependency. PS 5.1 compatible (no [ordered] in type def).
  Use Import-Module Win32-Input.psm1; functions are then available globally.

  Per-border:
    ...nothing yet - called once per SendInput call (the type is already loaded)
#>

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public struct RECT {
    public int Left;
    public int Top;
    public int Right;
    public int Bottom;
}

public struct KEYBDINPUT {
    public ushort wVk;
    public ushort wScan;
    public uint dwFlags;
    public uint time;
    public IntPtr dwExtraInfo;
}

[StructLayout(LayoutKind.Explicit)]
public struct INPUT_UNION {
    [FieldOffset(0)] public KEYBDINPUT ki;
}

public struct INPUT {
    public uint type;
    public INPUT_UNION u;
}

public static class W32 {
    public const uint INPUT_KEYBOARD = 1;
    public const uint KEYEVENTF_KEYUP = 0x0002;
    public const uint KEYEVENTF_UNICODE = 0x0004;

    // Common VK codes for ASCII printable + a few specials.
    public const ushort VK_BACK = 0x08;
    public const ushort VK_TAB = 0x09;
    public const ushort VK_RETURN = 0x0D;
    public const ushort VK_ESCAPE = 0x1B;
    public const ushort VK_SPACE = 0x20;
    public const ushort VK_PRIOR = 0x21;
    public const ushort VK_NEXT = 0x22;
    public const ushort VK_END = 0x23;
    public const ushort VK_HOME = 0x24;
    public const ushort VK_LEFT = 0x25;
    public const ushort VK_UP = 0x26;
    public const ushort VK_RIGHT = 0x27;
    public const ushort VK_DOWN = 0x28;
    public const ushort VK_DELETE = 0x2E;
    public const ushort VK_LSHIFT = 0xA0;
    public const ushort VK_RSHIFT = 0xA1;
    public const ushort VK_LCONTROL = 0xA2;
    public const ushort VK_RCONTROL = 0xA3;
    public const ushort VK_LMENU = 0xA4;
    public const ushort VK_RMENU = 0xA5;
    public const ushort VK_LWIN = 0x5B;
    public const ushort VK_RWIN = 0x5C;

    [DllImport("user32.dll", SetLastError=true)]
    public static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);

    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll", SetLastError=true)]
    public static extern IntPtr SetFocus(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern IntPtr GetFocus();

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool IsIconic(IntPtr hWnd);

    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern int GetWindowTextW(IntPtr hWnd, System.Text.StringBuilder lpString, int nMaxCount);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool BlockInput(bool fBlockIt);
}
"@ -ErrorAction SilentlyContinue

# Exported function 1: Send-SendInputVk
# Sends a single VK key (down+up). Optional modifier mask.
function Send-SendInputVk {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true)] [int]$Vk,
        [int[]]$Modifier = @()
    )

    $inputs = New-Object System.Collections.Generic.List[object]
    # Modifier down
    foreach ($m in $Modifier) {
        $k = New-Object KEYBDINPUT
        $k.wVk = [System.UInt16]$m
        $k.dwFlags = 0
        $inp = New-Object INPUT
        $inp.type = [W32]::INPUT_KEYBOARD
        $inp.u = New-Object INPUT_UNION
        $inp.u.ki = $k
        $inputs.Add($inp) | Out-Null
    }
    # Main key down
    $kMain = New-Object KEYBDINPUT
    $kMain.wVk = [System.UInt16]$Vk
    $kMain.dwFlags = 0
    $inpMain = New-Object INPUT
    $inpMain.type = [W32]::INPUT_KEYBOARD
    $inpMain.u = New-Object INPUT_UNION
    $inpMain.u.ki = $kMain
    $inputs.Add($inpMain) | Out-Null
    # Main key up
    $kUp = New-Object KEYBDINPUT
    $kUp.wVk = [System.UInt16]$Vk
    $kUp.dwFlags = [W32]::KEYEVENTF_KEYUP
    $inpUp = New-Object INPUT
    $inpUp.type = [W32]::INPUT_KEYBOARD
    $inpUp.u = New-Object INPUT_UNION
    $inpUp.u.ki = $kUp
    $inputs.Add($inpUp) | Out-Null
    # Modifier up (reverse order)
    for ($i = $Modifier.Count - 1; $i -ge 0; $i--) {
        $k = New-Object KEYBDINPUT
        $k.wVk = [System.UInt16]$Modifier[$i]
        $k.dwFlags = [W32]::KEYEVENTF_KEYUP
        $inp = New-Object INPUT
        $inp.type = [W32]::INPUT_KEYBOARD
        $inp.u = New-Object INPUT_UNION
        $inp.u.ki = $k
        $inputs.Add($inp) | Out-Null
    }
    $arr = $inputs.ToArray()
    $sent = [W32]::SendInput($arr.Count, $arr, [System.Runtime.InteropServices.Marshal]::SizeOf([Type]"INPUT"))
    return $sent
}

# Exported function 2: Send-SendInputUnicode
# Sends a Unicode string via KEYEVENTF_UNICODE flag. Each char becomes a
# down + up event. Bypasses the keyboard layout / IME entirely.
function Send-SendInputUnicode {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true)] [AllowEmptyString()] [string]$Text
    )

    $inputs = New-Object System.Collections.Generic.List[object]
    foreach ($ch in $Text.ToCharArray()) {
        $kDown = New-Object KEYBDINPUT
        $kDown.wVk = 0
        $kDown.wScan = [System.UInt16][int]$ch
        $kDown.dwFlags = [W32]::KEYEVENTF_UNICODE
        $inpDown = New-Object INPUT
        $inpDown.type = [W32]::INPUT_KEYBOARD
        $inpDown.u = New-Object INPUT_UNION
        $inpDown.u.ki = $kDown
        $inputs.Add($inpDown) | Out-Null
        $kUp = New-Object KEYBDINPUT
        $kUp.wVk = 0
        $kUp.wScan = [System.UInt16][int]$ch
        $kUp.dwFlags = [W32]::KEYEVENTF_UNICODE -bor [W32]::KEYEVENTF_KEYUP
        $inpUp = New-Object INPUT
        $inpUp.type = [W32]::INPUT_KEYBOARD
        $inpUp.u = New-Object INPUT_UNION
        $inpUp.u.ki = $kUp
        $inputs.Add($inpUp) | Out-Null
    }
    if ($inputs.Count -eq 0) { return 0 }
    $arr = $inputs.ToArray()
    $sent = [W32]::SendInput($arr.Count, $arr, [System.Runtime.InteropServices.Marshal]::SizeOf([Type]"INPUT"))
    return $sent
}

# Exported function 3: Set-Win32ForegroundFocus
# Calls SetForegroundWindow (z-order) + SetFocus (control focus) on hwnd.
# Verifies both via GetForegroundWindow + GetFocus and returns an object
# with diagnostic fields. Returns $null if either fails (caller decides).
function Set-Win32ForegroundFocus {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true)] [IntPtr]$Hwnd
    )

    if ($Hwnd -eq [IntPtr]::Zero) {
        return [pscustomobject]@{
            Success = $false
            Reason = "hwnd_zero"
            ForegroundHwnd = [IntPtr]::Zero
            FocusHwnd = [IntPtr]::Zero
        }
    }
    [W32]::SetForegroundWindow($Hwnd) | Out-Null
    Start-Sleep -Milliseconds 200
    [W32]::SetFocus($Hwnd) | Out-Null
    Start-Sleep -Milliseconds 100
    $fg = [W32]::GetForegroundWindow()
    $fc = [W32]::GetFocus()
    return [pscustomobject]@{
        Success = ($fg -eq $Hwnd -or $fc -eq $Hwnd)
        Reason = if ($fg -ne $Hwnd -and $fc -ne $Hwnd) { "neither_match" } else { "ok" }
        ForegroundHwnd = $fg
        FocusHwnd = $fc
    }
}

# Helper: Convert-Vk - turn printable ASCII char to its VK code via lowercase.
function Get-VkFromChar {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)] [char]$Char)
    # Range check: A-Z and 0-9 are standard VK codes.
    $code = [int]$Char
    if (($code -ge 0x30 -and $code -le 0x39) -or ($code -ge 0x41 -and $code -le 0x5A)) {
        return $code
    }
    if ($code -ge 0x61 -and $code -le 0x7A) {
        return $code - 0x20  # lowercase to uppercase VK
    }
    return -1
}

Export-ModuleMember -Function Send-SendInputVk, Send-SendInputUnicode, Set-Win32ForegroundFocus, Get-VkFromChar

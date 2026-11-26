#requires -Version 7.0

param(
    [Parameter(Mandatory = $true)]
    [string]$SourceWorkbook,

    [Parameter(Mandatory = $true)]
    [string]$EvidenceRoot,

    [ValidateRange(60, 1800)]
    [int]$NormalTimeoutSeconds = 900,

    [ValidateRange(15, 300)]
    [int]$FailureTimeoutSeconds = 120,

    [ValidateRange(5, 120)]
    [int]$WatchdogTimeoutSeconds = 20
)

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
$evidenceBase = [System.IO.Path]::GetFullPath(
    (Join-Path $repoRoot $EvidenceRoot)
)
$requiredEvidencePrefix = [System.IO.Path]::GetFullPath(
    (Join-Path $repoRoot "eval\tasks\excel\t22\evidence")
)
$requiredEvidencePrefixWithSeparator =
    $requiredEvidencePrefix.TrimEnd("\", "/") +
    [System.IO.Path]::DirectorySeparatorChar
if (-not $evidenceBase.StartsWith(
    $requiredEvidencePrefixWithSeparator,
    [System.StringComparison]::OrdinalIgnoreCase
)) {
    throw "EvidenceRoot must stay under eval/tasks/excel/t22/evidence"
}

$NormalRunCount = 10
$runStamp = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssfffZ")
$runRoot = Join-Path $evidenceBase $runStamp
New-Item -ItemType Directory -Force -Path $runRoot | Out-Null

$sourcePath = [System.IO.Path]::GetFullPath(
    (Resolve-Path -LiteralPath $SourceWorkbook).Path
)
$cargoPath = (Get-Command cargo.exe -ErrorAction Stop).Source
$pwshPath = (Get-Command pwsh.exe -ErrorAction Stop).Source

if (-not ("Task409Native" -as [type])) {
    Add-Type @"
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;

public static class Task409ProcessResult
{
    public const int Terminated = 0;
    public const int AlreadyExited = 1;
    public const int IdentityMismatch = 2;
    public const int OpenFailed = 3;
    public const int QueryFailed = 4;
    public const int TerminateFailed = 5;
}

public static class Task409Native
{
    private const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
    private const uint PROCESS_TERMINATE = 0x0001;
    private const uint SYNCHRONIZE = 0x00100000;
    private const uint WAIT_OBJECT_0 = 0x00000000;
    private const int ERROR_INVALID_PARAMETER = 87;
    private const int ERROR_ACCESS_DENIED = 5;
    private const int JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x2000;
    private const int JobObjectExtendedLimitInformation = 9;
    private const int JobObjectBasicProcessIdList = 3;

    [StructLayout(LayoutKind.Sequential)]
    private struct FILETIME
    {
        public uint Low;
        public uint High;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct IO_COUNTERS
    {
        public ulong ReadOperationCount;
        public ulong WriteOperationCount;
        public ulong OtherOperationCount;
        public ulong ReadTransferCount;
        public ulong WriteTransferCount;
        public ulong OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_BASIC_LIMIT_INFORMATION
    {
        public long PerProcessUserTimeLimit;
        public long PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize;
        public UIntPtr MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass;
        public uint SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION
    {
        public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
        public IO_COUNTERS IoInfo;
        public UIntPtr ProcessMemoryLimit;
        public UIntPtr JobMemoryLimit;
        public UIntPtr PeakProcessMemoryUsed;
        public UIntPtr PeakJobMemoryUsed;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    public static extern uint GetDriveTypeW(string rootPathName);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr OpenProcess(uint access, bool inheritHandle, int processId);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetProcessTimes(
        IntPtr process,
        out FILETIME creation,
        out FILETIME exit,
        out FILETIME kernel,
        out FILETIME user
    );

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern bool QueryFullProcessImageNameW(
        IntPtr process,
        uint flags,
        StringBuilder executablePath,
        ref uint size
    );

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetExitCodeProcess(IntPtr process, out uint exitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateProcess(IntPtr process, uint exitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr handle);

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern IntPtr CreateJobObject(IntPtr attributes, string name);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool SetInformationJobObject(
        IntPtr job,
        int infoClass,
        ref JOBOBJECT_EXTENDED_LIMIT_INFORMATION info,
        uint infoLength
    );

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateJobObject(IntPtr job, uint exitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool QueryInformationJobObject(
        IntPtr job,
        int infoClass,
        IntPtr info,
        uint infoLength,
        out uint returnLength
    );

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);

    public static IntPtr CreateKillOnCloseJob()
    {
        IntPtr job = CreateJobObject(IntPtr.Zero, null);
        if (job == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
        var info = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        uint size = (uint)Marshal.SizeOf<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
        if (!SetInformationJobObject(job, JobObjectExtendedLimitInformation, ref info, size))
        {
            int error = Marshal.GetLastWin32Error();
            CloseHandle(job);
            throw new Win32Exception(error);
        }
        return job;
    }

    public static void AssignProcessToJob(IntPtr job, IntPtr process)
    {
        if (!AssignProcessToJobObject(job, process))
        {
            throw new Win32Exception(Marshal.GetLastWin32Error());
        }
    }

    public static void TerminateJob(IntPtr job)
    {
        if (!TerminateJobObject(job, 1))
        {
            throw new Win32Exception(Marshal.GetLastWin32Error());
        }
    }

    public static int[] GetJobProcessIds(IntPtr job)
    {
        const int maximumProcesses = 4096;
        int pointerSize = IntPtr.Size;
        int bufferSize = (2 * pointerSize) + (pointerSize * maximumProcesses);
        IntPtr buffer = Marshal.AllocHGlobal(bufferSize);
        try
        {
            uint returnLength;
            if (!QueryInformationJobObject(
                job,
                JobObjectBasicProcessIdList,
                buffer,
                (uint)bufferSize,
                out returnLength
            ))
            {
                throw new Win32Exception(Marshal.GetLastWin32Error());
            }
            int count = Marshal.ReadInt32(buffer, 4);
            if (count < 0 || count > maximumProcesses)
            {
                throw new InvalidOperationException("job process list exceeded its bound");
            }
            var processIds = new int[count];
            for (int index = 0; index < count; index++)
            {
                processIds[index] = checked(
                    (int)Marshal.ReadIntPtr(
                        buffer,
                        8 + (index * pointerSize)
                    ).ToInt64()
                );
            }
            return processIds;
        }
        finally
        {
            Marshal.FreeHGlobal(buffer);
        }
    }

    public static void CloseJob(IntPtr job)
    {
        if (job != IntPtr.Zero) CloseHandle(job);
    }

    public static int TerminateVerifiedProcess(
        int processId,
        long expectedStartTicks,
        string expectedExecutablePath
    )
    {
        IntPtr process = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | SYNCHRONIZE,
            false,
            processId
        );
        if (process == IntPtr.Zero)
        {
            int error = Marshal.GetLastWin32Error();
            return error == ERROR_INVALID_PARAMETER
                ? Task409ProcessResult.AlreadyExited
                : Task409ProcessResult.OpenFailed;
        }
        try
        {
            uint exitCode;
            if (!GetExitCodeProcess(process, out exitCode))
            {
                return Task409ProcessResult.QueryFailed;
            }
            if (exitCode != 259)
            {
                return Task409ProcessResult.AlreadyExited;
            }
            FILETIME creation;
            FILETIME exit;
            FILETIME kernel;
            FILETIME user;
            if (!GetProcessTimes(process, out creation, out exit, out kernel, out user))
            {
                return Task409ProcessResult.QueryFailed;
            }
            long filetime =
                ((long)creation.High << 32) | (long)creation.Low;
            long utcTicks = filetime + 504911232000000000L;
            var path = new StringBuilder(32768);
            uint pathLength = (uint)path.Capacity;
            if (!QueryFullProcessImageNameW(process, 0, path, ref pathLength))
            {
                return Task409ProcessResult.QueryFailed;
            }
            if (utcTicks != expectedStartTicks ||
                !String.Equals(
                    path.ToString(),
                    expectedExecutablePath,
                    StringComparison.OrdinalIgnoreCase
                ))
            {
                return Task409ProcessResult.IdentityMismatch;
            }
            if (!TerminateProcess(process, 1))
            {
                return Task409ProcessResult.TerminateFailed;
            }
            uint wait = WaitForSingleObject(process, 10000);
            if (wait != WAIT_OBJECT_0)
            {
                return Task409ProcessResult.TerminateFailed;
            }
            return Task409ProcessResult.Terminated;
        }
        finally
        {
            CloseHandle(process);
        }
    }
}
"@
}

function Write-Utf8File {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [AllowEmptyString()][string]$Text
    )
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Text, $encoding)
}

function Write-JsonFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )
    $json = if ($Value -is [array] -and $Value.Count -eq 0) {
        "[]"
    } else {
        $Value | ConvertTo-Json -Depth 16
    }
    $text = ($json + "`n") -replace "`r`n", "`n"
    Write-Utf8File -Path $Path -Text $text
}

function Assert-Condition {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if (-not $Condition) {
        throw $Message
    }
}

function Get-ExcelProcessIds {
    return @(Get-Process EXCEL -ErrorAction SilentlyContinue | ForEach-Object {
        [int]$_.Id
    })
}

function Wait-ExcelProcessCountZero {
    param([int]$TimeoutSeconds = 60)
    for ($attempt = 0; $attempt -lt ($TimeoutSeconds * 4); $attempt++) {
        $ids = @(Get-ExcelProcessIds)
        if ($ids.Count -eq 0) {
            return @()
        }
        Start-Sleep -Milliseconds 250
    }
    return @(Get-ExcelProcessIds)
}

function Get-ProcessIdentity {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        return $null
    }
    $executablePath = try {
        [string]$process.Path
    } catch {
        ""
    }
    return [pscustomobject]@{
        id = [int]$process.Id
        process_name = [string]$process.ProcessName
        start_time_utc_ticks = [long]$process.StartTime.ToUniversalTime().Ticks
        executable_path = $executablePath
    }
}

function Get-ChildProcessIdentity {
    param(
        [Parameter(Mandatory = $true)]$Process,
        [Parameter(Mandatory = $true)][string]$ExecutablePath
    )
    $identity = Get-ProcessIdentity -ProcessId ([int]$Process.Id)
    if ($null -ne $identity -and
        -not [string]::IsNullOrWhiteSpace([string]$identity.executable_path)) {
        return $identity
    }
    $startTicks = try {
        [long]$Process.StartTime.ToUniversalTime().Ticks
    } catch {
        throw "cargo child start time is unavailable after exit: $($_.Exception.Message)"
    }
    return [pscustomobject]@{
        id = [int]$Process.Id
        process_name = [System.IO.Path]::GetFileNameWithoutExtension($ExecutablePath)
        start_time_utc_ticks = $startTicks
        executable_path = [System.IO.Path]::GetFullPath($ExecutablePath)
    }
}

function Test-ProcessIdentity {
    param([AllowNull()]$Identity)
    if ($null -eq $Identity) {
        return $false
    }
    $expectedPath = [string]$Identity.executable_path
    if ([string]::IsNullOrWhiteSpace([string]$Identity.process_name) -or
        [string]::IsNullOrWhiteSpace($expectedPath) -or
        -not [System.IO.Path]::IsPathRooted($expectedPath)) {
        return $false
    }
    $process = Get-Process -Id ([int]$Identity.id) -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        return $false
    }
    if ([string]$process.ProcessName -ne [string]$Identity.process_name) {
        return $false
    }
    $actualTicks = [long]$process.StartTime.ToUniversalTime().Ticks
    if ($actualTicks -ne [long]$Identity.start_time_utc_ticks) {
        return $false
    }
    $actualPath = try {
        [string]$process.Path
    } catch {
        ""
    }
    if ([string]::IsNullOrWhiteSpace($actualPath) -or
        -not $actualPath.Equals(
            $expectedPath,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        return $false
    }
    return $true
}

function Convert-FiletimeToUtcTicks {
    param([Parameter(Mandatory = $true)][long]$Filetime100ns)
    return $Filetime100ns + 504911232000000000
}

function Convert-OwnershipJournalToIdentity {
    param([Parameter(Mandatory = $true)]$Journal)
    Assert-Condition -Condition ([int]$Journal.schema_version -eq 1) `
        -Message "ownership journal schema_version must be 1"
    Assert-Condition -Condition ([string]$Journal.process_name -eq "EXCEL") `
        -Message "ownership journal process_name must be EXCEL"
    Assert-Condition -Condition ([int]$Journal.process_id -gt 0) `
        -Message "ownership journal process_id must be positive"
    Assert-Condition -Condition ([long]$Journal.start_time_100ns -gt 0) `
        -Message "ownership journal start_time_100ns must be positive"
    $executablePath = [string]$Journal.executable_path
    Assert-Condition -Condition (
        -not [string]::IsNullOrWhiteSpace($executablePath) -and
        [System.IO.Path]::IsPathRooted($executablePath)
    ) -Message "ownership journal executable_path must be non-empty and absolute"
    $workbookPath = [string]$Journal.workbook_path
    Assert-Condition -Condition (
        -not [string]::IsNullOrWhiteSpace($workbookPath) -and
        [System.IO.Path]::IsPathRooted($workbookPath)
    ) -Message "ownership journal workbook_path must be non-empty and absolute"
    return [pscustomobject]@{
        id = [int]$Journal.process_id
        process_name = [string]$Journal.process_name
        start_time_utc_ticks = [long](Convert-FiletimeToUtcTicks `
            -Filetime100ns ([long]$Journal.start_time_100ns))
        executable_path = $executablePath
    }
}

function Get-OwnershipJournals {
    param([Parameter(Mandatory = $true)][string]$ScenarioDirectory)
    return @(Get-ChildItem -LiteralPath $ScenarioDirectory `
        -Filter "ownership-run-*.json" -File -ErrorAction SilentlyContinue |
        ForEach-Object {
            Get-Content -LiteralPath $_.FullName -Raw -Encoding utf8 |
                ConvertFrom-Json
        })
}

function Invoke-VerifiedExcelTermination {
    param([AllowNull()]$Identity)
    if (-not (Test-ProcessIdentity -Identity $Identity)) {
        return [pscustomobject]@{
            attempted = $false
            result_code = $null
            error = "Process identity changed or process already exited"
        }
    }
    try {
        $result = [Task409Native]::TerminateVerifiedProcess(
            [int]$Identity.id,
            [long]$Identity.start_time_utc_ticks,
            [string]$Identity.executable_path
        )
        $errorMessage = if ($result -eq 0 -or $result -eq 1) {
            $null
        } else {
            "verified Excel termination returned code $result"
        }
        return [pscustomobject]@{
            attempted = $true
            result_code = $result
            error = $errorMessage
        }
    } catch {
        return [pscustomobject]@{
            attempted = $true
            result_code = $null
            error = $_.Exception.Message
        }
    }
}

function Stop-AcceptanceJob {
    param([AllowNull()]$Child)
    if ($null -eq $Child) {
        return [pscustomobject]@{
            attempted = $false
            error = "No child identity"
            remaining_job_process_ids = @()
        }
    }
    if ($null -eq $Child.process_identity -or
        [string]::IsNullOrWhiteSpace([string]$Child.process_identity.executable_path)) {
        return [pscustomobject]@{
            attempted = $false
            error = "Child process identity is missing or incomplete"
            remaining_job_process_ids = @()
        }
    }
    $wasRunning = -not $Child.process.HasExited
    $cleanupError = $null
    $remainingJobMembers = $null
    try {
        $initialMembers = @([Task409Native]::GetJobProcessIds(
            [IntPtr]$Child.job_handle
        ))
        if ($wasRunning -or $initialMembers.Count -gt 0) {
            [Task409Native]::TerminateJob([IntPtr]$Child.job_handle)
            if ($wasRunning -and -not $Child.process.WaitForExit(30000)) {
                $cleanupError = "job termination did not reap the wrapper within 30 seconds"
            }
            if (-not $wasRunning) {
                $cleanupError = "wrapper exited while Job still had members"
            }
        }

        for ($attempt = 0; $attempt -lt 300; $attempt++) {
            $remainingJobMembers = @([Task409Native]::GetJobProcessIds(
                [IntPtr]$Child.job_handle
            ))
            if ($remainingJobMembers.Count -eq 0) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        if ($remainingJobMembers.Count -gt 0) {
            $cleanupError = "Job Object still has live members: $($remainingJobMembers -join ',')"
        }
        return [pscustomobject]@{
            attempted = $wasRunning -or $initialMembers.Count -gt 0
            error = $cleanupError
            remaining_job_process_ids = @($remainingJobMembers)
        }
    } catch {
        return [pscustomobject]@{
            attempted = $true
            error = $_.Exception.Message
            remaining_job_process_ids = $null
        }
    } finally {
        [Task409Native]::CloseJob([IntPtr]$Child.job_handle)
    }
}

function Wait-ProcessIdentitiesGone {
    param(
        [AllowEmptyCollection()][object[]]$Identities,
        [int]$TimeoutSeconds = 30
    )
    for ($attempt = 0; $attempt -lt ($TimeoutSeconds * 4); $attempt++) {
        $remaining = @(
            $Identities | Where-Object {
                Test-ProcessIdentity -Identity $_
            }
        )
        if ($remaining.Count -eq 0) {
            return @()
        }
        Start-Sleep -Milliseconds 250
    }
    return @(
        $Identities | Where-Object {
            Test-ProcessIdentity -Identity $_
        }
    )
}

function Assert-LocalWorkbookPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    Assert-Condition -Condition (
        [System.IO.Path]::GetExtension($Path).Equals(
            ".xlsx",
            [System.StringComparison]::OrdinalIgnoreCase
        )
    ) -Message "SourceWorkbook must use the .xlsx extension"
    Assert-Condition -Condition (Test-Path -LiteralPath $Path -PathType Leaf) `
        -Message "SourceWorkbook must identify an existing file"
    Assert-Condition -Condition (-not $Path.StartsWith("\\")) `
        -Message "SourceWorkbook must not be a UNC path"
    $root = [System.IO.Path]::GetPathRoot($Path)
    Assert-Condition -Condition (-not [string]::IsNullOrWhiteSpace($root)) `
        -Message "SourceWorkbook must be an absolute local path"
    Assert-Condition -Condition ([Task409Native]::GetDriveTypeW($root) -ne 4) `
        -Message "SourceWorkbook must not be on a mapped network drive"
}

function Get-TaskTemporaryFiles {
    param([Parameter(Mandatory = $true)][string]$RunToken)
    $tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    return @(Get-ChildItem -LiteralPath $tempRoot `
        -Filter "assistant-task409-$RunToken-*" -File -ErrorAction SilentlyContinue)
}

function Remove-TaskTemporaryFiles {
    param(
        [Parameter(Mandatory = $true)][string]$RunToken,
        [AllowEmptyCollection()][object[]]$Files
    )
    $tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    foreach ($file in $Files) {
        $fullPath = [System.IO.Path]::GetFullPath($file.FullName)
        Assert-Condition -Condition (
            $fullPath.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase)
        ) -Message "Refusing to remove temporary file outside the temp root"
        Assert-Condition -Condition (
            $file.Name.StartsWith(
                "assistant-task409-$RunToken-",
                [System.StringComparison]::OrdinalIgnoreCase
            )
        ) -Message "Refusing to remove a temporary file with a foreign token"
        Remove-Item -LiteralPath $fullPath -Force
    }
}

function Set-ScenarioEnvironment {
    param([Parameter(Mandatory = $true)][hashtable]$Values)
    $previous = @{}
    foreach ($name in $Values.Keys) {
        $previous[$name] = [System.Environment]::GetEnvironmentVariable(
            $name,
            [System.EnvironmentVariableTarget]::Process
        )
        [System.Environment]::SetEnvironmentVariable(
            $name,
            [string]$Values[$name],
            [System.EnvironmentVariableTarget]::Process
        )
    }
    return $previous
}

function Restore-ScenarioEnvironment {
    param([Parameter(Mandatory = $true)][hashtable]$Previous)
    foreach ($name in $Previous.Keys) {
        [System.Environment]::SetEnvironmentVariable(
            $name,
            $Previous[$name],
            [System.EnvironmentVariableTarget]::Process
        )
    }
}

function Start-AcceptanceChild {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$RunToken,
        [Parameter(Mandatory = $true)][string]$ScenarioDirectory,
        [Parameter(Mandatory = $true)][int]$Runs,
        [Parameter(Mandatory = $true)][string]$FailAfter,
        [Parameter(Mandatory = $true)][string]$HangAfter
    )
    $stdoutPath = Join-Path $ScenarioDirectory "stdout.txt"
    $stderrPath = Join-Path $ScenarioDirectory "stderr.txt"
    $observationPath = Join-Path $ScenarioDirectory "watchdog-observed-excel.json"
    $readyPath = Join-Path $ScenarioDirectory "wrapper-ready.signal"
    $goPath = Join-Path $ScenarioDirectory "wrapper-go.signal"
    $wrapperPath = Join-Path $ScenarioDirectory "run-acceptance-child.ps1"
    Write-Utf8File -Path $observationPath -Text "[]`n"
    $jobHandle = [Task409Native]::CreateKillOnCloseJob()
    $wrapperSource = @'
param(
    [Parameter(Mandatory = $true)][string]$CargoPath,
    [Parameter(Mandatory = $true)][string]$ReadyPath,
    [Parameter(Mandatory = $true)][string]$GoPath,
    [Parameter(Mandatory = $true)][string]$StdoutPath,
    [Parameter(Mandatory = $true)][string]$StderrPath
)
$ErrorActionPreference = "Stop"
[System.IO.File]::WriteAllText($ReadyPath, "ready")
while (-not (Test-Path -LiteralPath $GoPath)) {
    Start-Sleep -Milliseconds 20
}
$arguments = @(
    "test",
    "-p",
    "assistant-platform-windows",
    "test_windows_excel_real_com_repeated_write_acceptance",
    "--",
    "--ignored",
    "--nocapture",
    "--test-threads=1"
)
$child = Start-Process `
    -FilePath $CargoPath `
    -ArgumentList $arguments `
    -WindowStyle Hidden `
    -RedirectStandardOutput $StdoutPath `
    -RedirectStandardError $StderrPath `
    -PassThru
$child.WaitForExit()
exit $child.ExitCode
'@
    Write-Utf8File -Path $wrapperPath -Text $wrapperSource
    $environment = @{
        EXCEL_T22_ACCEPTANCE_WORKBOOK = $sourcePath
        EXCEL_T22_REPEAT_EVIDENCE_DIR = $ScenarioDirectory
        EXCEL_T22_REPEAT_RUN_TOKEN = $RunToken
        EXCEL_T22_REPEAT_RUNS = [string]$Runs
        EXCEL_T22_REPEAT_FAIL_AFTER = $FailAfter
        EXCEL_T22_REPEAT_HANG_AFTER = $HangAfter
    }
    $previous = Set-ScenarioEnvironment -Values $environment
    try {
        try {
            $child = Start-Process `
                -FilePath $pwshPath `
                -ArgumentList @(
                    "-NoProfile",
                    "-File",
                    $wrapperPath,
                    "-CargoPath",
                    $cargoPath,
                    "-ReadyPath",
                    $readyPath,
                    "-GoPath",
                    $goPath,
                    "-StdoutPath",
                    $stdoutPath,
                    "-StderrPath",
                    $stderrPath
                ) `
                -WindowStyle Hidden `
                -PassThru
            $processIdentity = Get-ChildProcessIdentity `
                -Process $child `
                -ExecutablePath $pwshPath
            [Task409Native]::AssignProcessToJob($jobHandle, $child.Handle)
            $readyDeadline = [DateTime]::UtcNow.AddSeconds(10)
            while (-not (Test-Path -LiteralPath $readyPath)) {
                if ($child.HasExited -or [DateTime]::UtcNow -ge $readyDeadline) {
                    throw "acceptance child wrapper did not become ready"
                }
                Start-Sleep -Milliseconds 20
            }
            Write-Utf8File -Path $goPath -Text "go`n"
        } catch {
            if ($null -ne $processIdentity) {
                [void][Task409Native]::TerminateVerifiedProcess(
                    [int]$processIdentity.id,
                    [long]$processIdentity.start_time_utc_ticks,
                    [string]$processIdentity.executable_path
                )
                if ($null -ne $child) {
                    [void]$child.WaitForExit(10000)
                }
            }
            [Task409Native]::CloseJob($jobHandle)
            throw
        }
    } finally {
        Restore-ScenarioEnvironment -Previous $previous
    }
    if ($null -eq $processIdentity -or
        [string]::IsNullOrWhiteSpace([string]$processIdentity.executable_path) -or
        -not [System.IO.Path]::IsPathRooted([string]$processIdentity.executable_path)) {
        [Task409Native]::TerminateJob($jobHandle)
        [void]$child.WaitForExit(10000)
        [Task409Native]::CloseJob($jobHandle)
        throw "cargo child identity could not be captured completely"
    }
    $observed = New-Object System.Collections.Generic.List[object]
    return [pscustomobject]@{
        name = $Name
        run_token = $RunToken
        process = $child
        process_identity = $processIdentity
        job_handle = $jobHandle
        wrapper_path = $wrapperPath
        observed_excel = $observed
        observation_path = $observationPath
        stdout_path = $stdoutPath
        stderr_path = $stderrPath
        scenario_directory = $ScenarioDirectory
    }
}

function Read-ScenarioLogs {
    param([Parameter(Mandatory = $true)]$Child)
    $stdout = if (Test-Path -LiteralPath $Child.stdout_path) {
        Get-Content -LiteralPath $Child.stdout_path -Raw -Encoding utf8
    } else {
        ""
    }
    $stderr = if (Test-Path -LiteralPath $Child.stderr_path) {
        Get-Content -LiteralPath $Child.stderr_path -Raw -Encoding utf8
    } else {
        ""
    }
    return [pscustomobject]@{
        stdout = "$stdout"
        stderr = "$stderr"
    }
}

function Get-ExcelProcessIdentities {
    $identities = @()
    foreach ($process in @(Get-Process EXCEL -ErrorAction SilentlyContinue)) {
        $identity = Get-ProcessIdentity -ProcessId ([int]$process.Id)
        if ($null -eq $identity) {
            continue
        }
        Assert-Condition -Condition (
            -not [string]::IsNullOrWhiteSpace([string]$identity.executable_path) -and
            [System.IO.Path]::IsPathRooted([string]$identity.executable_path)
        ) -Message "Excel process $($process.Id) did not expose a complete identity"
        $identities += $identity
    }
    return @($identities)
}

function Update-WatchdogObservations {
    param([Parameter(Mandatory = $true)]$Child)
    foreach ($identity in @(Get-ExcelProcessIdentities)) {
        $alreadyObserved = @(
            $Child.observed_excel | Where-Object {
                [int]$_.id -eq [int]$identity.id -and
                [long]$_.start_time_utc_ticks -eq [long]$identity.start_time_utc_ticks -and
                [string]$_.executable_path -eq [string]$identity.executable_path
            }
        ).Count -gt 0
        if (-not $alreadyObserved) {
            $Child.observed_excel.Add($identity)
        }
    }
    Write-JsonFile -Path $Child.observation_path -Value $Child.observed_excel.ToArray()
}

function Wait-AcceptanceChild {
    param(
        [Parameter(Mandatory = $true)]$Child,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds
    )
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        Update-WatchdogObservations -Child $Child
        if ($Child.process.HasExited) {
            Update-WatchdogObservations -Child $Child
            return [pscustomobject]@{
                timed_out = $false
                exit_code = [int]$Child.process.ExitCode
            }
        }
        Start-Sleep -Milliseconds 100
    }
    Update-WatchdogObservations -Child $Child
    return [pscustomobject]@{
        timed_out = $true
        exit_code = $null
    }
}

function Get-OwnedExcelIdentities {
    param([Parameter(Mandatory = $true)]$Child)
    $journals = @(Get-OwnershipJournals -ScenarioDirectory $Child.scenario_directory)
    $all = @()
    $all += @($journals | ForEach-Object {
        Convert-OwnershipJournalToIdentity -Journal $_
    })
    $jobProcessIds = @([Task409Native]::GetJobProcessIds(
        [IntPtr]$Child.job_handle
    ))
    $all += @($Child.observed_excel.ToArray() | Where-Object {
        $jobProcessIds -contains [int]$_.id
    })
    $unique = @{}
    foreach ($identity in $all) {
        $key = "{0}:{1}:{2}" -f $identity.id,
            $identity.start_time_utc_ticks,
            $identity.executable_path.ToLowerInvariant()
        $unique[$key] = $identity
    }
    return [pscustomobject]@{
        journals = $journals
        identities = @($unique.Values)
        job_process_ids = @($jobProcessIds)
    }
}

function Cleanup-OwnedExcelFromJournals {
    param([Parameter(Mandatory = $true)]$Child)
    $owned = Get-OwnedExcelIdentities -Child $Child
    $cleanup = @()
    foreach ($identity in @($owned.identities)) {
        $cleanup += Invoke-VerifiedExcelTermination -Identity $identity
    }
    return [pscustomobject]@{
        journals = @($owned.journals)
        identities = @($owned.identities)
        cleanup = $cleanup
    }
}

function Invoke-CompletedScenario {
    param(
        [Parameter(Mandatory = $true)]$Child,
        [Parameter(Mandatory = $true)][ValidateSet("pass", "failure")][string]$Expected
    )
    $exitCode = [int]$Child.process.ExitCode
    if ($Expected -eq "pass") {
        Assert-Condition -Condition ($exitCode -eq 0) `
            -Message "Normal TASK-409 scenario exited with $exitCode"
    } else {
        Assert-Condition -Condition ($exitCode -ne 0) `
            -Message "Failure injection unexpectedly exited with zero"
    }
    $logs = Read-ScenarioLogs -Child $Child
    if ($Expected -eq "failure") {
        Assert-Condition -Condition (
            ($logs.stdout + $logs.stderr).Contains(
                "TASK-409 injected failure after-write",
                [System.StringComparison]::Ordinal
            )
        ) -Message "Failure injection marker was not observed"
    }
    $ownedBeforeCleanup = Get-OwnedExcelIdentities -Child $Child
    $liveBeforeCleanup = @(
        $ownedBeforeCleanup.identities | Where-Object {
            Test-ProcessIdentity -Identity $_
        }
    )
    $remainingExcel = @(Wait-ExcelProcessCountZero -TimeoutSeconds 60)
    if ($remainingExcel.Count -gt 0) {
        [void](Cleanup-OwnedExcelFromJournals -Child $Child)
        throw "Completed TASK-409 scenario required external Excel cleanup"
    }
    $childCleanup = Stop-AcceptanceJob -Child $Child
    Assert-Condition -Condition ($null -eq $childCleanup.error) `
        -Message "Completed scenario child cleanup failed: $($childCleanup.error)"
    $remainingChildren = @(Wait-ProcessIdentitiesGone `
        -Identities @($Child.process_identity) `
        -TimeoutSeconds 30)
    Assert-Condition -Condition ($remainingChildren.Count -eq 0) `
        -Message "Completed scenario left cargo/test child process"
    $journalCleanup = $ownedBeforeCleanup
    Assert-Condition -Condition ($remainingExcel.Count -eq 0) `
        -Message "TASK-409 completed scenario left EXCEL.EXE: $remainingExcel"
    $temporaryFiles = @(Get-TaskTemporaryFiles -RunToken $Child.run_token)
    if ($Expected -eq "pass") {
        Assert-Condition -Condition ($temporaryFiles.Count -eq 0) `
            -Message "Normal TASK-409 scenario left temporary workbooks"
        Assert-Condition -Condition ($journalCleanup.journals.Count -eq 0) `
            -Message "Normal TASK-409 scenario left ownership journals"
        $summaryPath = Join-Path $Child.scenario_directory "normal-summary.json"
        Assert-Condition -Condition (Test-Path -LiteralPath $summaryPath) `
            -Message "Normal TASK-409 summary is missing"
        $normalSummary = Get-Content -LiteralPath $summaryPath -Raw -Encoding utf8 |
            ConvertFrom-Json
        Assert-Condition -Condition (
            $normalSummary.result -eq "passed" -and
            [int]$normalSummary.requested_runs -eq 10 -and
            [int]$normalSummary.passed_runs -eq 10
        ) -Message "Normal TASK-409 summary does not report the requested pass count"
    } else {
        $marker = Join-Path $Child.scenario_directory "checkpoint-run-01-after-write.ready"
        Assert-Condition -Condition (Test-Path -LiteralPath $marker) `
            -Message "Failure injection did not reach the after-write checkpoint"
        Assert-Condition -Condition ($journalCleanup.journals.Count -gt 0) `
            -Message "Failure injection did not retain an ownership journal"
    }
    if ($temporaryFiles.Count -gt 0) {
        Remove-TaskTemporaryFiles -RunToken $Child.run_token -Files $temporaryFiles
    }
    return [pscustomobject]@{
        name = $Child.name
        expected = $Expected
        exit_code = $exitCode
        child_process_identity = $Child.process_identity
        ownership_journal_count = $journalCleanup.journals.Count
        ownership_cleanup = @()
        child_cleanup = $childCleanup
        live_excel_identities_after_child_exit = @($liveBeforeCleanup)
        job_process_ids_after_child_exit = @($journalCleanup.job_process_ids)
        remaining_excel_process_ids = @($remainingExcel)
        temporary_files_removed = $temporaryFiles.Count
        stdout_path = $Child.stdout_path
        stderr_path = $Child.stderr_path
    }
}

function Invoke-WatchdogScenario {
    param(
        [Parameter(Mandatory = $true)]$Child,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds
    )
    $wait = Wait-AcceptanceChild -Child $Child -TimeoutSeconds $TimeoutSeconds
    Assert-Condition -Condition $wait.timed_out `
        -Message "Watchdog injection exited before the expected timeout"
    $ownedBeforeCleanup = Get-OwnedExcelIdentities -Child $Child
    Assert-Condition -Condition ($ownedBeforeCleanup.journals.Count -gt 0) `
        -Message "Watchdog injection did not retain an ownership journal"
    $liveOwned = @(
        $ownedBeforeCleanup.identities | Where-Object {
            Test-ProcessIdentity -Identity $_
        }
    )
    Assert-Condition -Condition ($liveOwned.Count -gt 0) `
        -Message "Watchdog timed out without a live owned Excel identity"
    $journalCleanup = Cleanup-OwnedExcelFromJournals `
        -Child $Child
    $childCleanup = Stop-AcceptanceJob -Child $Child
    Assert-Condition -Condition ($childCleanup.attempted -and $null -eq $childCleanup.error) `
        -Message "Watchdog child job cleanup failed: $($childCleanup.error)"
    $cleanupIdentities = @($Child.process_identity)
    $cleanupIdentities += @($journalCleanup.identities)
    $remaining = @(Wait-ProcessIdentitiesGone `
        -Identities $cleanupIdentities `
        -TimeoutSeconds 60)
    Assert-Condition -Condition ($remaining.Count -eq 0) `
        -Message "Watchdog injection left owned processes"
    $remainingExcel = @(Wait-ExcelProcessCountZero -TimeoutSeconds 60)
    Assert-Condition -Condition ($remainingExcel.Count -eq 0) `
        -Message "Watchdog injection left EXCEL.EXE: $remainingExcel"
    $marker = Join-Path $Child.scenario_directory "checkpoint-run-01-after-open.ready"
    Assert-Condition -Condition (Test-Path -LiteralPath $marker) `
        -Message "Watchdog injection did not reach the after-open checkpoint"
    $temporaryFiles = @(Get-TaskTemporaryFiles -RunToken $Child.run_token)
    if ($temporaryFiles.Count -gt 0) {
        Remove-TaskTemporaryFiles -RunToken $Child.run_token -Files $temporaryFiles
    }
    return [pscustomobject]@{
        name = $Child.name
        expected = "watchdog-timeout"
        timed_out = $true
        timeout_seconds = $TimeoutSeconds
        child_process_identity = $Child.process_identity
        owned_excel_identities = @(
            $journalCleanup.identities
        )
        excel_cleanup = @($journalCleanup.cleanup)
        child_cleanup = $childCleanup
        live_owned_excel_identities_before_cleanup = @($liveOwned)
        job_process_ids_before_cleanup = @($ownedBeforeCleanup.job_process_ids)
        temporary_files_removed = $temporaryFiles.Count
        stdout_path = $Child.stdout_path
        stderr_path = $Child.stderr_path
    }
}

function Invoke-EmergencyCleanup {
    param([AllowNull()]$Child)
    if ($null -eq $Child) {
        return [pscustomobject]@{ errors = @() }
    }
    $errors = New-Object System.Collections.Generic.List[string]
    try {
        $journalCleanup = Cleanup-OwnedExcelFromJournals `
            -Child $Child
        foreach ($cleanup in $journalCleanup.cleanup) {
            if ($null -ne $cleanup.error) {
                $errors.Add([string]$cleanup.error)
            }
        }
    } catch {
        $errors.Add("ownership-journal cleanup failed: $($_.Exception.Message)")
    }
    try {
        $childCleanup = Stop-AcceptanceJob -Child $Child
        if ($null -ne $childCleanup.error) {
            $errors.Add([string]$childCleanup.error)
        }
        $remainingChildren = @(Wait-ProcessIdentitiesGone `
            -Identities @($Child.process_identity) `
            -TimeoutSeconds 30)
        if ($remainingChildren.Count -gt 0) {
            $errors.Add("remaining cargo/test child processes")
        }
    } catch {
        $errors.Add("child cleanup failed: $($_.Exception.Message)")
    }
    $remainingExcel = @(Wait-ExcelProcessCountZero -TimeoutSeconds 60)
    if ($remainingExcel.Count -gt 0) {
        $errors.Add("remaining EXCEL.EXE processes: $($remainingExcel -join ',')")
    }
    try {
        $temporaryFiles = @(Get-TaskTemporaryFiles -RunToken $Child.run_token)
        if ($temporaryFiles.Count -gt 0) {
            Remove-TaskTemporaryFiles `
                -RunToken $Child.run_token `
                -Files $temporaryFiles
        }
    } catch {
        $errors.Add("temporary-file cleanup failed: $($_.Exception.Message)")
    }
    return [pscustomobject]@{ errors = @($errors) }
}

function Test-Task409IdentityGuards {
    $self = Get-ProcessIdentity -ProcessId ([int]$PID)
    Assert-Condition -Condition (
        $null -ne $self -and
        -not [string]::IsNullOrWhiteSpace([string]$self.executable_path)
    ) -Message "identity self-test could not capture the current PowerShell process"
    Assert-Condition -Condition (Test-ProcessIdentity -Identity $self) `
        -Message "identity self-test rejected the exact live process identity"
    $emptyPath = [pscustomobject]@{
        id = [int]$self.id
        process_name = [string]$self.process_name
        start_time_utc_ticks = [long]$self.start_time_utc_ticks
        executable_path = ""
    }
    Assert-Condition -Condition (-not (Test-ProcessIdentity -Identity $emptyPath)) `
        -Message "identity self-test accepted an empty executable path"
    $wrongStart = [pscustomobject]@{
        id = [int]$self.id
        process_name = [string]$self.process_name
        start_time_utc_ticks = [long]$self.start_time_utc_ticks + 1
        executable_path = [string]$self.executable_path
    }
    Assert-Condition -Condition (-not (Test-ProcessIdentity -Identity $wrongStart)) `
        -Message "identity self-test accepted a mismatched creation time"
    $wrongPath = [pscustomobject]@{
        id = [int]$self.id
        process_name = [string]$self.process_name
        start_time_utc_ticks = [long]$self.start_time_utc_ticks
        executable_path = [System.IO.Path]::GetFullPath("C:\task409-not-pwsh.exe")
    }
    Assert-Condition -Condition (-not (Test-ProcessIdentity -Identity $wrongPath)) `
        -Message "identity self-test accepted a mismatched executable path"
}

Test-Task409IdentityGuards
Assert-LocalWorkbookPath -Path $sourcePath
$initialExcel = @(Get-ExcelProcessIds)
Assert-Condition -Condition ($initialExcel.Count -eq 0) `
    -Message "Close every EXCEL.EXE before TASK-409: $initialExcel"
$sourceDigestBefore = (Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256).Hash
$activeChild = $null

try {
    $normalDirectory = Join-Path $runRoot "normal"
    New-Item -ItemType Directory -Force -Path $normalDirectory | Out-Null
    $normalToken = [guid]::NewGuid().ToString("N")
    $normalChild = Start-AcceptanceChild `
        -Name "normal" `
        -RunToken $normalToken `
        -ScenarioDirectory $normalDirectory `
        -Runs $NormalRunCount `
        -FailAfter "none" `
        -HangAfter "none"
    $activeChild = $normalChild
    $normalWait = Wait-AcceptanceChild `
        -Child $normalChild `
        -TimeoutSeconds $NormalTimeoutSeconds
    if ($normalWait.timed_out) {
        throw "Normal TASK-409 scenario timed out"
    }
    $normalResult = Invoke-CompletedScenario -Child $normalChild -Expected "pass"
    $activeChild = $null

    $failureDirectory = Join-Path $runRoot "failure-injection"
    New-Item -ItemType Directory -Force -Path $failureDirectory | Out-Null
    $failureToken = [guid]::NewGuid().ToString("N")
    $failureChild = Start-AcceptanceChild `
        -Name "failure-injection" `
        -RunToken $failureToken `
        -ScenarioDirectory $failureDirectory `
        -Runs 1 `
        -FailAfter "after-write" `
        -HangAfter "none"
    $activeChild = $failureChild
    $failureWait = Wait-AcceptanceChild `
        -Child $failureChild `
        -TimeoutSeconds $FailureTimeoutSeconds
    if ($failureWait.timed_out) {
        throw "Failure injection timed out"
    }
    $failureResult = Invoke-CompletedScenario -Child $failureChild -Expected "failure"
    $activeChild = $null

    $watchdogDirectory = Join-Path $runRoot "watchdog-injection"
    New-Item -ItemType Directory -Force -Path $watchdogDirectory | Out-Null
    $watchdogToken = [guid]::NewGuid().ToString("N")
    $watchdogChild = Start-AcceptanceChild `
        -Name "watchdog-injection" `
        -RunToken $watchdogToken `
        -ScenarioDirectory $watchdogDirectory `
        -Runs 1 `
        -FailAfter "none" `
        -HangAfter "after-open"
    $activeChild = $watchdogChild
    $watchdogResult = Invoke-WatchdogScenario `
        -Child $watchdogChild `
        -TimeoutSeconds $WatchdogTimeoutSeconds
    $activeChild = $null

    $sourceDigestAfter = (Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256).Hash
    Assert-Condition -Condition ($sourceDigestBefore -eq $sourceDigestAfter) `
        -Message "Source workbook digest changed during TASK-409"
    $finalExcel = @(Get-ExcelProcessIds)
    Assert-Condition -Condition ($finalExcel.Count -eq 0) `
        -Message "TASK-409 left EXCEL.EXE processes: $finalExcel"

    $summary = [pscustomobject]@{
        schema_version = 1
        mode = "excel-t22-repeated-write-regression"
        result = "passed"
        source_workbook = $sourcePath
        source_digest_before = $sourceDigestBefore
        source_digest_after = $sourceDigestAfter
        normal = $normalResult
        failure_injection = $failureResult
        watchdog_injection = $watchdogResult
        final_excel_process_ids = @($finalExcel)
        final_excel_process_count = $finalExcel.Count
        started_at_utc = $runStamp
        completed_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    }
    $summaryPath = Join-Path $runRoot "summary.json"
    Write-JsonFile -Path $summaryPath -Value $summary
    Write-Output "excel-t22-repeated-write passed normal=$NormalRunCount final_excel=0 evidence=$summaryPath"
} catch {
    $scenarioError = $_
    $cleanup = Invoke-EmergencyCleanup -Child $activeChild
    if ($cleanup.errors.Count -gt 0) {
        throw "TASK-409 scenario failed: $($scenarioError.Exception.Message); emergency cleanup errors: $($cleanup.errors -join '; ')"
    }
    throw $scenarioError
}

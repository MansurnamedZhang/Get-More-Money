function Read-ManagedProcessRecord {
    param([string]$Path)

    $raw = (Get-Content -LiteralPath $Path -Raw).Trim()
    if ($raw -match "^\d+$") {
        return [PSCustomObject]@{
            Version = 0
            Pid = [int]$raw
        }
    }

    try {
        $record = $raw | ConvertFrom-Json
    }
    catch {
        return $null
    }

    if (
        $record.version -ne 1 -or
        -not $record.pid -or
        -not $record.started_at_utc -or
        -not $record.executable
    ) {
        return $null
    }

    return [PSCustomObject]@{
        Version = 1
        Pid = [int]$record.pid
        StartedAtUtc = [DateTime]::Parse(
            [string]$record.started_at_utc,
            [System.Globalization.CultureInfo]::InvariantCulture,
            [System.Globalization.DateTimeStyles]::RoundtripKind
        ).ToUniversalTime()
        Executable = [System.IO.Path]::GetFullPath([string]$record.executable)
    }
}

function Get-ValidatedManagedProcess {
    param($Record)

    if (-not $Record -or $Record.Version -ne 1) {
        return $null
    }

    $process = Get-Process -Id $Record.Pid -ErrorAction SilentlyContinue
    if (-not $process) {
        return $null
    }

    try {
        $actualExecutable = [System.IO.Path]::GetFullPath($process.Path)
        $actualStartedAtUtc = $process.StartTime.ToUniversalTime()
    }
    catch {
        return $null
    }

    if (-not $actualExecutable.Equals(
        $Record.Executable,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        return $null
    }

    if ([Math]::Abs(($actualStartedAtUtc - $Record.StartedAtUtc).TotalSeconds) -gt 1) {
        return $null
    }

    return $process
}

function Write-ManagedProcessRecord {
    param(
        [string]$Path,
        [System.Diagnostics.Process]$Process,
        [string]$Executable
    )

    $record = [ordered]@{
        version = 1
        pid = $Process.Id
        started_at_utc = $Process.StartTime.ToUniversalTime().ToString("o")
        executable = [System.IO.Path]::GetFullPath($Executable)
    }
    Set-Content -LiteralPath $Path -Value ($record | ConvertTo-Json -Compress) -NoNewline
}

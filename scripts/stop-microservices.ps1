$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$runtimeRoot = Join-Path $projectRoot "runtime\microservices"

. (Join-Path $PSScriptRoot "managed-process.ps1")

if (-not (Test-Path -LiteralPath $runtimeRoot)) {
    Write-Host "No recorded microservice processes"
    exit 0
}

Get-ChildItem -LiteralPath $runtimeRoot -Filter "*.pid" |
    ForEach-Object {
        $record = Read-ManagedProcessRecord -Path $_.FullName
        $process = Get-ValidatedManagedProcess -Record $record
        if ($process) {
            Stop-Process -Id $process.Id -Force
            Write-Host "Stopped $($_.BaseName) (PID $($process.Id))"
        }
        elseif ($record -and $record.Version -eq 0) {
            Write-Warning "Ignored legacy PID record for $($_.BaseName); process identity cannot be verified safely"
        }
        else {
            Write-Host "Removed stale process record for $($_.BaseName)"
        }
        Remove-Item -LiteralPath $_.FullName -Force
    }

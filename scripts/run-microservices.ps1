param(
    [switch]$ResetData,
    [switch]$SkipBuild,
    [switch]$NoFrontend
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$runtimeRoot = Join-Path $projectRoot "runtime\microservices"
$serviceDataRoot = Join-Path $projectRoot "data\services"
$frontendRoot = Join-Path $projectRoot "frontend"
$backendRoot = Join-Path $projectRoot "backend"
$cargoExe = (Get-Command cargo).Source
$npmExe = (Get-Command npm.cmd).Source
$nodeExe = (Get-Command node.exe).Source

. (Join-Path $PSScriptRoot "managed-process.ps1")

# Some Windows launchers inject both "Path" and "PATH". Windows PowerShell's
# Start-Process treats those names as duplicates and fails while inheriting the
# environment, so keep the canonical Windows spelling before starting services.
$processPathKeys = @(
    [Environment]::GetEnvironmentVariables("Process").Keys |
        Where-Object { $_ -ieq "Path" }
)
if ($processPathKeys -ccontains "Path" -and $processPathKeys -ccontains "PATH") {
    [Environment]::SetEnvironmentVariable("PATH", $null, "Process")
}

New-Item -ItemType Directory -Force -Path $runtimeRoot | Out-Null
New-Item -ItemType Directory -Force -Path $serviceDataRoot | Out-Null

function Stop-RecordedProcesses {
    Get-ChildItem -LiteralPath $runtimeRoot -Filter "*.pid" -ErrorAction SilentlyContinue |
        ForEach-Object {
            $record = Read-ManagedProcessRecord -Path $_.FullName
            $process = Get-ValidatedManagedProcess -Record $record
            if ($process) {
                Stop-Process -Id $process.Id -Force
            }
            elseif ($record -and $record.Version -eq 0) {
                Write-Warning "Ignored legacy PID record for $($_.BaseName); process identity cannot be verified safely"
            }
            Remove-Item -LiteralPath $_.FullName -Force
        }
}

function Remove-LocalDatabases {
    $targets = @(
        (Join-Path $projectRoot "data\services\identity.db"),
        (Join-Path $projectRoot "data\services\identity.db-shm"),
        (Join-Path $projectRoot "data\services\identity.db-wal"),
        (Join-Path $projectRoot "data\services\investment-core.db"),
        (Join-Path $projectRoot "data\services\investment-core.db-shm"),
        (Join-Path $projectRoot "data\services\investment-core.db-wal"),
        (Join-Path $backendRoot "data\microservice-core.db"),
        (Join-Path $backendRoot "data\microservice-core.db-shm"),
        (Join-Path $backendRoot "data\microservice-core.db-wal")
    )
    foreach ($target in $targets) {
        $fullTarget = [System.IO.Path]::GetFullPath($target)
        if (-not $fullTarget.StartsWith($projectRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to delete data outside the project directory: $fullTarget"
        }
        if (Test-Path -LiteralPath $fullTarget) {
            Remove-Item -LiteralPath $fullTarget -Force
        }
    }
}

function Start-ManagedProcess {
    param(
        [string]$Name,
        [string]$FilePath,
        [string[]]$ArgumentList,
        [string]$WorkingDirectory,
        [hashtable]$Environment
    )

    $pidFile = Join-Path $runtimeRoot "$Name.pid"
    $stdoutPath = Join-Path $runtimeRoot "$Name.out.log"
    $stderrPath = Join-Path $runtimeRoot "$Name.err.log"
    if (Test-Path -LiteralPath $pidFile) {
        $record = Read-ManagedProcessRecord -Path $pidFile
        $existingProcess = Get-ValidatedManagedProcess -Record $record
        if ($existingProcess) {
            throw "$Name is already running (PID $($existingProcess.Id))"
        }
        Remove-Item -LiteralPath $pidFile -Force
    }
    foreach ($logPath in @($stdoutPath, $stderrPath)) {
        if (Test-Path -LiteralPath $logPath) {
            Remove-Item -LiteralPath $logPath -Force
        }
    }

    $previous = @{}
    foreach ($key in $Environment.Keys) {
        $previous[$key] = [Environment]::GetEnvironmentVariable($key, "Process")
        [Environment]::SetEnvironmentVariable($key, [string]$Environment[$key], "Process")
    }
    try {
        $startParameters = @{
            FilePath = $FilePath
            WorkingDirectory = $WorkingDirectory
            WindowStyle = "Hidden"
            RedirectStandardOutput = $stdoutPath
            RedirectStandardError = $stderrPath
            PassThru = $true
        }
        if ($ArgumentList -and $ArgumentList.Count -gt 0) {
            $startParameters.ArgumentList = $ArgumentList
        }
        $process = Start-Process @startParameters
        Write-ManagedProcessRecord -Path $pidFile -Process $process -Executable $FilePath
    }
    finally {
        foreach ($key in $Environment.Keys) {
            [Environment]::SetEnvironmentVariable($key, $previous[$key], "Process")
        }
    }
}

if ($ResetData) {
    Stop-RecordedProcesses
    Remove-LocalDatabases
}

if (-not $SkipBuild) {
    & $cargoExe build --workspace
    if ($LASTEXITCODE -ne 0) { throw "Rust workspace build failed" }
    if (-not $NoFrontend) {
        & $npmExe run build --prefix $frontendRoot
        if ($LASTEXITCODE -ne 0) { throw "Frontend build failed" }
    }
}

$internalToken = [Guid]::NewGuid().ToString("N")
$commonEnvironment = @{
    "INTERNAL_API_TOKEN" = $internalToken
    "RUST_LOG" = "info,tower_http=info"
}

Start-ManagedProcess `
    -Name "identity-service" `
    -FilePath (Join-Path $projectRoot "target\debug\identity-service.exe") `
    -ArgumentList @() `
    -WorkingDirectory $projectRoot `
    -Environment ($commonEnvironment + @{
        "IDENTITY_BIND" = "127.0.0.1:3101"
        "IDENTITY_DATABASE_URL" = "sqlite://data/services/identity.db"
        "COOKIE_SECURE" = "false"
    })

Start-ManagedProcess `
    -Name "investment-core" `
    -FilePath (Join-Path $projectRoot "target\debug\personal-investment-backend.exe") `
    -ArgumentList @() `
    -WorkingDirectory $projectRoot `
    -Environment @{
        "APP_BIND" = "127.0.0.1:3100"
        "DATABASE_URL" = "sqlite://data/services/investment-core.db"
        "AUTH_REQUIRED" = "false"
        "INTERNAL_API_TOKEN" = $internalToken
        "RUST_LOG" = "personal_investment_backend=info,tower_http=info"
    }

Start-ManagedProcess `
    -Name "api-gateway" `
    -FilePath (Join-Path $projectRoot "target\debug\api-gateway.exe") `
    -ArgumentList @() `
    -WorkingDirectory $projectRoot `
    -Environment ($commonEnvironment + @{
        "GATEWAY_BIND" = "127.0.0.1:3001"
        "IDENTITY_SERVICE_URL" = "http://127.0.0.1:3101"
        "INVESTMENT_CORE_SERVICE_URL" = "http://127.0.0.1:3100"
        "MARKET_DATA_SERVICE_URL" = "http://127.0.0.1:3100"
        "PLANNING_SERVICE_URL" = "http://127.0.0.1:3100"
        "AUDIT_SERVICE_URL" = "http://127.0.0.1:3100"
    })

if (-not $NoFrontend) {
    Start-ManagedProcess `
        -Name "frontend" `
        -FilePath $nodeExe `
        -ArgumentList @("node_modules\vinext\dist\cli.js", "start") `
        -WorkingDirectory $frontendRoot `
        -Environment @{
            "NEXT_PUBLIC_API_BASE_URL" = "http://localhost:3001/api/v1"
        }
}

Write-Host "SANYU INVEST microservices started"
Write-Host "Frontend: http://localhost:3000"
Write-Host "Gateway: http://localhost:3001/ready"

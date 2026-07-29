$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$backendRoot = Join-Path $projectRoot "backend"

& (Join-Path $PSScriptRoot "stop-microservices.ps1")

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
        Write-Host "Deleted $fullTarget"
    }
}

Write-Host "Local microservice data reset"

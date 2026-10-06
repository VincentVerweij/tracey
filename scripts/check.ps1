<#
.SYNOPSIS
    The full set of checks CI runs. Run this before committing.

.DESCRIPTION
    Rust: clippy (all targets, -D warnings) and tests, each with and without
    the `test` feature, so code under #[cfg(feature = "test")] is built,
    linted and tested.
    .NET: build, format verification and tests.

    CI calls this script, so a clean local run means a clean CI run.

.PARAMETER Only
    Run just one half: 'rust' or 'dotnet'. CI uses this to run the halves as
    parallel jobs. Omit it to run everything.

.EXAMPLE
    pwsh scripts/check.ps1
    pwsh scripts/check.ps1 -Only rust
#>
param(
    [ValidateSet('all', 'rust', 'dotnet')]
    [string]$Only = 'all'
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

function Invoke-Step([string]$Name, [scriptblock]$Command) {
    Write-Host "`n==> $Name" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $Name" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

if ($Only -in 'all', 'rust') {
    # tauri-build requires frontendDist to exist at compile time, but clippy
    # and unit tests never serve the frontend. A stub stands in when no real
    # Blazor publish is present; an existing publish is left untouched.
    $dist = 'src/Tracey.App/bin/Release/net10.0/publish/wwwroot'
    if (-not (Test-Path "$dist/index.html")) {
        New-Item -ItemType Directory -Path $dist -Force | Out-Null
        Set-Content -Path "$dist/index.html" -Value '<!doctype html>'
    }

    $manifest = 'src-tauri/Cargo.toml'
    Invoke-Step 'cargo clippy' { cargo clippy --manifest-path $manifest --all-targets -- -D warnings }
    Invoke-Step 'cargo clippy --features test' { cargo clippy --manifest-path $manifest --all-targets --features test -- -D warnings }
    Invoke-Step 'cargo test' { cargo test --manifest-path $manifest }
    Invoke-Step 'cargo test --features test' { cargo test --manifest-path $manifest --features test }
}

if ($Only -in 'all', 'dotnet') {
    Invoke-Step 'dotnet build' { dotnet build src/Tracey.slnx }
    Invoke-Step 'dotnet format' { dotnet format src/Tracey.App/Tracey.App.csproj --verify-no-changes --no-restore }
    Invoke-Step 'dotnet test' { dotnet test src/Tracey.Tests/Tracey.Tests.csproj --no-build --logger 'trx;LogFileName=test-results.trx' }
}

Write-Host "`nAll checks passed." -ForegroundColor Green

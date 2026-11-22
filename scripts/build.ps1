<#
.SYNOPSIS
    Builds YAL: the tests, the release binaries, the installer and the checksums.

.DESCRIPTION
    A thin wrapper over `cargo xtask dist`. It runs the tests, builds the
    release binaries and the helper processes, builds the NSIS installer and
    the portable zip, and writes the SHA-256 checksums. It exits non-zero on
    any failure.

.PARAMETER SkipTests
    Build without running the test suite.

.PARAMETER Arch
    Which architecture to build: x64, arm64 or both. Defaults to both.

.PARAMETER Channel
    The release channel: stable or beta. Defaults to stable.

.EXAMPLE
    ./scripts/build.ps1
#>
[CmdletBinding()]
param(
    [switch] $SkipTests,
    [ValidateSet('x64', 'arm64', 'both')]
    [string] $Arch = 'both',
    [ValidateSet('stable', 'beta')]
    [string] $Channel = 'stable'
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    Write-Host "==> cargo fmt --check" -ForegroundColor Cyan
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw "cargo fmt reported differences" }

    Write-Host "==> cargo clippy" -ForegroundColor Cyan
    cargo clippy --workspace --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "cargo clippy reported warnings" }

    if (-not $SkipTests) {
        Write-Host "==> cargo test" -ForegroundColor Cyan
        cargo test --workspace
        if ($LASTEXITCODE -ne 0) { throw "the test suite failed" }
    }

    $arguments = @('run', '--quiet', '-p', 'xtask', '--', 'dist', '--channel', $Channel)
    if ($Arch -ne 'both') { $arguments += @('--arch', $Arch) }

    Write-Host "==> cargo xtask dist" -ForegroundColor Cyan
    cargo @arguments
    if ($LASTEXITCODE -ne 0) { throw "the release build failed" }

    Write-Host ""
    Write-Host "Artifacts are in dist/." -ForegroundColor Green
}
finally {
    Pop-Location
}

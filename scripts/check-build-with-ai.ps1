# Runs every command that BUILD_WITH_AI.md tells a reader to run, and reports
# which of them worked.
#
# A document of commands that no longer work is worse than no document, so the
# commands are checked rather than trusted. CI runs this too.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-build-with-ai.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-build-with-ai.ps1 -SkipDist
#
# -SkipDist leaves out the release build, which takes the longest.

param(
    [switch]$SkipDist,
    [switch]$SkipFrontend
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$failures = 0
$checks = 0

function Invoke-Check {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Body
    )
    $script:checks++
    Write-Host "==> $Name" -ForegroundColor Cyan
    try {
        # The output is kept, not thrown away. A test harness writes its
        # failure report to standard output, so a script that discards stdout
        # throws away the very thing that says what went wrong - and a failing
        # build then says only "the command exited with 101".
        #
        # Only standard output is captured. Standard error is left alone on
        # purpose: redirecting it into the pipeline (`2>&1`) turns every line
        # a build tool writes there into an error record, and this script
        # stops on the first error - so cargo's ordinary progress output would
        # look like a failure.
        $output = & $Body | Out-String
        $code = $LASTEXITCODE
        if ($code -ne 0) {
            throw "the command exited with $code`n$output"
        }
        Write-Host "    ok: $Name" -ForegroundColor Green
    }
    catch {
        $script:failures++
        Write-Host "    FAILED: $Name - $($_.Exception.Message)" -ForegroundColor Red
    }
}

# Build & self-test.
Invoke-Check 'cargo xtask selftest' { cargo run -q -p xtask -- selftest }

# The linters.
Invoke-Check 'cargo fmt --all -- --check' { cargo fmt --all -- --check }
Invoke-Check 'cargo clippy --workspace --all-targets -- -D warnings' {
    cargo clippy --workspace --all-targets -- -D warnings
}

# The tests.
Invoke-Check 'cargo test --workspace' { cargo test --workspace }
Invoke-Check 'cargo test -p tc-core' { cargo test -p tc-core }

# The application.
Invoke-Check 'yal.exe --self-test' {
    $binary = Join-Path $root 'target\release\yal.exe'
    if (-not (Test-Path $binary)) {
        throw "$binary has not been built yet"
    }
    & $binary --self-test
}

# The release build.
if (-not $SkipDist) {
    Invoke-Check 'cargo xtask dist --arch x64' { cargo run -q -p xtask -- dist --arch x64 }
}

Write-Host ""
if ($failures -eq 0) {
    Write-Host "Every command in BUILD_WITH_AI.md works. $checks checks passed." -ForegroundColor Green
    exit 0
}
Write-Host "$failures of $checks checks failed." -ForegroundColor Red
exit 1

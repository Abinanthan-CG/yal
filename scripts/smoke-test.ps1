# The installer smoke test.
#
# It installs YAL the way a user would, checks that the program works, removes
# it again, and checks that the user's own data is still there.
#
# The script is idempotent: it removes a previous install before it starts, so
# it can be run again after a failure.
#
# Usage:
#   ./scripts/smoke-test.ps1
#   ./scripts/smoke-test.ps1 -InstallerPath dist\yal-setup-0.1.0-x64.exe

[CmdletBinding()]
param(
    [string]$InstallerPath,
    [int]$TimeoutSeconds = 60
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\yal'
$dataDir = Join-Path $env:APPDATA 'yal'
$exe = Join-Path $installDir 'yal.exe'
$uninstaller = Join-Path $installDir 'uninstall.exe'

function Write-Step([string]$message) {
    Write-Host "==> $message" -ForegroundColor Cyan
}

function Assert-True([bool]$condition, [string]$message) {
    if (-not $condition) {
        throw "smoke test failed: $message"
    }
    Write-Host "    ok: $message" -ForegroundColor Green
}

# Find the installer when the caller did not name one.
if (-not $InstallerPath) {
    $candidates = Get-ChildItem -Path (Join-Path $root 'dist') -Filter 'yal-setup-*-x64.exe' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending
    if (-not $candidates) {
        throw "no installer in dist/. Run 'cargo xtask dist --arch x64' first."
    }
    $InstallerPath = $candidates[0].FullName
}

Assert-True (Test-Path $InstallerPath) "the installer exists: $InstallerPath"

# A previous run may have left an install behind. Close it and remove it.
Write-Step 'Clearing any previous install'
if (Test-Path $exe) {
    & $exe --quit-running | Out-Null
    Start-Sleep -Seconds 1
}
if (Test-Path $uninstaller) {
    Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait | Out-Null
}
if (Test-Path $installDir) {
    Remove-Item -Path $installDir -Recurse -Force -ErrorAction SilentlyContinue
}

# The user's data must survive the whole test. A marker file is written so
# that the check is about the data and not about an empty folder.
Write-Step 'Recording the user data folder'
New-Item -ItemType Directory -Force -Path $dataDir | Out-Null
$marker = Join-Path $dataDir 'smoke-test-marker.txt'
Set-Content -Path $marker -Value 'this file must survive the uninstall' -Encoding UTF8
Assert-True (Test-Path $marker) "the data folder $dataDir is ready"

Write-Step 'Installing silently'
$install = Start-Process -FilePath $InstallerPath -ArgumentList '/S' -Wait -PassThru
Assert-True ($install.ExitCode -eq 0) "the installer exited with $($install.ExitCode)"

Write-Step 'Checking the installed files'
Assert-True (Test-Path $exe) "yal.exe is installed at $exe"
foreach ($helper in @('yal-indexer.exe', 'yal-ocr.exe', 'yal-dictation.exe')) {
    Assert-True (Test-Path (Join-Path $installDir $helper)) "$helper is installed"
}
Assert-True (Test-Path $uninstaller) "the uninstaller is installed"

Write-Step 'Checking the protocol registration'
$protocol = Get-ItemProperty -Path 'HKCU:\Software\Classes\tinycast\shell\open\command' -ErrorAction SilentlyContinue
Assert-True ($null -ne $protocol) 'the tinycast:// protocol is registered'

Write-Step 'Running the self-test'
$selfTestOut = Join-Path $env:TEMP 'yal-selftest-out.txt'
$selfTestErr = Join-Path $env:TEMP 'yal-selftest-err.txt'
$selfTest = Start-Process -FilePath $exe -ArgumentList '--self-test' -Wait -PassThru -RedirectStandardOutput $selfTestOut -RedirectStandardError $selfTestErr
Assert-True ($selfTest.ExitCode -eq 0) "the self-test exited with $($selfTest.ExitCode)"

$report = Get-Content $selfTestOut -Raw | ConvertFrom-Json
$failed = @($report.checks | Where-Object { -not $_.ok })
Assert-True ($report.checks.Count -gt 0) "the self-test reported $($report.checks.Count) checks"
Assert-True ($failed.Count -eq 0) "the self-test had no failing check"

Write-Step 'Starting the application'
$app = Start-Process -FilePath $exe -PassThru
try {
    # The palette appears straight away when the app is not started by the
    # login item.
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $windowSeen = $false
    while ((Get-Date) -lt $deadline -and -not $windowSeen) {
        Start-Sleep -Milliseconds 500
        $app.Refresh()
        if ($app.HasExited) {
            throw "the application exited early with $($app.ExitCode)"
        }
        if ($app.MainWindowHandle -ne 0) {
            $windowSeen = $true
        }
    }
    Assert-True $windowSeen 'the palette window appeared'

    # A second launch must not start a second copy; it hands over to the first.
    $second = Start-Process -FilePath $exe -PassThru
    Start-Sleep -Seconds 2
    $second.Refresh()
    Assert-True ($second.HasExited) 'a second launch handed over to the running instance'

    Write-Step 'Quitting the running instance'
    & $exe --quit-running | Out-Null
    $quitDeadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $quitDeadline) {
        $app.Refresh()
        if ($app.HasExited) {
            break
        }
        Start-Sleep -Milliseconds 250
    }
    $app.Refresh()
    Assert-True $app.HasExited 'the running instance quit when it was asked to'
}
finally {
    if (-not $app.HasExited) {
        Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
    }
}

Write-Step 'Uninstalling silently'
$uninstall = Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -PassThru
Assert-True ($uninstall.ExitCode -eq 0) "the uninstaller exited with $($uninstall.ExitCode)"

Write-Step 'Checking that the program files are gone'
Assert-True (-not (Test-Path $exe)) 'yal.exe was removed'
$protocolAfter = Get-ItemProperty -Path 'HKCU:\Software\Classes\tinycast' -ErrorAction SilentlyContinue
Assert-True ($null -eq $protocolAfter) 'the tinycast:// protocol was unregistered'

Write-Step 'Checking that the user data is still there'
Assert-True (Test-Path $dataDir) "the data folder $dataDir survived the uninstall"
Assert-True (Test-Path $marker) 'the data in the folder survived the uninstall'

Write-Host ''
Write-Host 'The installer smoke test passed.' -ForegroundColor Green
exit 0

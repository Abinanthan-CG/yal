<#
.SYNOPSIS
    Installs and checks the prerequisites for building YAL on Windows.

.DESCRIPTION
    Every prerequisite is installed through winget, from its official source, and
    the versions that were found are printed. The script is idempotent: running
    it twice changes nothing the second time.

.PARAMETER SkipInstall
    Only report what is present; install nothing.

.EXAMPLE
    ./scripts/bootstrap.ps1
#>
[CmdletBinding()]
param(
    [switch] $SkipInstall
)

$ErrorActionPreference = 'Stop'

function Write-Step([string] $Message) {
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Found([string] $Name, [string] $Version) {
    Write-Host ("    {0,-28} {1}" -f $Name, $Version) -ForegroundColor Green
}

function Write-Missing([string] $Name) {
    Write-Host ("    {0,-28} not installed" -f $Name) -ForegroundColor Yellow
}

function Get-CommandVersion([string] $Command, [string[]] $Arguments = @('--version')) {
    $found = Get-Command $Command -ErrorAction SilentlyContinue
    if (-not $found) { return $null }
    try {
        $output = & $Command @Arguments 2>&1 | Select-Object -First 1
        return "$output".Trim()
    } catch {
        return $null
    }
}

function Install-Package([string] $Id, [string] $Name) {
    if ($SkipInstall) {
        Write-Missing $Name
        return
    }
    Write-Step "Installing $Name"
    winget install --id $Id -e --accept-package-agreements --accept-source-agreements `
        --disable-interactivity | Out-Host
}

Write-Host ""
Write-Host "YAL bootstrap" -ForegroundColor White
Write-Host "=============" -ForegroundColor White
Write-Host ""

# --- Rust --------------------------------------------------------------------
$rust = Get-CommandVersion 'rustup'
if ($rust) {
    Write-Found 'rustup' $rust
    $targets = rustup target list --installed
    foreach ($target in @('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')) {
        if ($targets -contains $target) {
            Write-Found "target $target" 'installed'
        } else {
            Write-Host ("    {0,-28} missing" -f "target $target") -ForegroundColor Yellow
            if (-not $SkipInstall) {
                rustup target add $target | Out-Host
            }
        }
    }
} else {
    Install-Package 'Rustlang.Rustup' 'Rust (rustup)'
}

# --- Visual Studio Build Tools ----------------------------------------------
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
    $path = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath
    if ($path) {
        Write-Found 'Visual Studio Build Tools' $path
    } else {
        Write-Missing 'Visual Studio Build Tools (C++ workload)'
        if (-not $SkipInstall) {
            winget install --id Microsoft.VisualStudio.2022.BuildTools -e `
                --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" `
                --accept-package-agreements --accept-source-agreements --disable-interactivity | Out-Host
        }
    }
} else {
    Write-Missing 'Visual Studio Build Tools'
}

# --- Windows SDK -------------------------------------------------------------
$sdkRoot = "${env:ProgramFiles(x86)}\Windows Kits\10\Include"
if (Test-Path $sdkRoot) {
    $sdk = (Get-ChildItem $sdkRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1).Name
    Write-Found 'Windows SDK' $sdk
} else {
    Write-Missing 'Windows SDK'
}

# --- Node.js -----------------------------------------------------------------
$node = Get-CommandVersion 'node'
if ($node) {
    Write-Found 'node' $node
} else {
    Install-Package 'OpenJS.NodeJS.LTS' 'Node.js LTS'
}

$npm = Get-CommandVersion 'npm.cmd'
if ($npm) {
    Write-Found 'npm' $npm
} else {
    Write-Missing 'npm'
}

# --- WebView2 ----------------------------------------------------------------
$webview = Get-ItemProperty `
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' `
    -ErrorAction SilentlyContinue
if ($webview -and $webview.pv) {
    Write-Found 'WebView2 runtime' $webview.pv
} else {
    Write-Missing 'WebView2 runtime'
}

# --- NSIS --------------------------------------------------------------------
$makensis = Get-CommandVersion 'makensis' '-VERSION'
if ($makensis) {
    Write-Found 'NSIS' $makensis
} else {
    Install-Package 'NSIS.NSIS' 'NSIS'
}

Write-Host ""
Write-Step "Done. Build with ./scripts/build.ps1"
Write-Host ""

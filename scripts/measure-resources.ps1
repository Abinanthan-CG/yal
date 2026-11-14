# Measures what YAL costs at runtime, next to the launcher it is meant to replace.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\measure-resources.ps1
#
# It launches the release build, samples the whole process tree (the app plus the
# WebView2 processes it owns), and reports working set, private bytes and idle CPU.
# The same numbers are printed for Raycast when it is running, because the whole
# reason this port exists is to be lighter than that.
#
# Working set is what Task Manager shows. Private bytes are what the process
# actually owns; a shared WebView2 page can inflate the working set without the
# memory being this app's alone.

param(
  [int]$SettleSeconds = 12,
  [int]$SampleSeconds = 10,
  [switch]$HideWindow
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
$exe  = Join-Path $root 'target\release\yal.exe'

if (-not (Test-Path $exe)) {
  "release build not found at $exe - run: cargo build -p yal --release"
  exit 1
}

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class YalWin32 {
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
}
"@ -ErrorAction SilentlyContinue

function Get-Tree([int]$rootPid) {
  $all = Get-CimInstance Win32_Process
  $ids = New-Object System.Collections.Generic.HashSet[int]
  [void]$ids.Add($rootPid)
  $changed = $true
  while ($changed) {
    $changed = $false
    foreach ($p in $all) {
      if ($ids.Contains([int]$p.ParentProcessId) -and -not $ids.Contains([int]$p.ProcessId)) {
        [void]$ids.Add([int]$p.ProcessId)
        $changed = $true
      }
    }
  }
  $ids
}

function Tree-Procs([int]$rootPid) {
  $out = @()
  foreach ($id in (Get-Tree $rootPid)) {
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($p) { $out += $p }
  }
  $out
}

function Show-Tree([string]$tag, $procs) {
  $ws   = ($procs | Measure-Object WorkingSet64 -Sum).Sum
  $priv = ($procs | Measure-Object PrivateMemorySize64 -Sum).Sum
  "[$tag] processes=" + $procs.Count +
    "  working set=" + [math]::Round($ws / 1MB, 1) + " MB" +
    "  private=" + [math]::Round($priv / 1MB, 1) + " MB"
  foreach ($p in $procs | Sort-Object WorkingSet64 -Descending) {
    "    {0,-22} pid={1,-6} ws={2,7} MB  private={3,7} MB" -f $p.ProcessName, $p.Id,
      [math]::Round($p.WorkingSet64 / 1MB, 1), [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
  }
}

function Measure-Cpu([string]$tag, $procs, [int]$seconds) {
  $before = ($procs | Measure-Object CPU -Sum).Sum
  Start-Sleep -Seconds $seconds
  $after = ($procs | Measure-Object CPU -Sum).Sum
  "[$tag] cpu=" + [math]::Round((($after - $before) / $seconds) * 100, 1) + "% of one core"
}

# Raycast, when it is running, is the number we are trying to beat.
$ray = Get-Process | Where-Object { $_.ProcessName -match '^Raycast' }
if ($ray) {
  "===== Raycast (running on this machine) ====="
  Show-Tree 'raycast' $ray
  Measure-Cpu 'raycast' $ray $SampleSeconds
  ""
}

"===== YAL (this build) ====="
"build: " + (Get-Item $exe).LastWriteTime
$proc = Start-Process -FilePath $exe -PassThru
"launched yal.exe pid=$($proc.Id)"
Start-Sleep -Seconds $SettleSeconds

$tree = Tree-Procs $proc.Id
Show-Tree 'settled' $tree
Measure-Cpu 'idle' $tree $SampleSeconds

if ($HideWindow) {
  $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
  if ($p -and $p.MainWindowHandle -ne 0) {
    [void][YalWin32]::ShowWindow($p.MainWindowHandle, 0)
    "hid the palette window"
    Start-Sleep -Seconds 4
    $tree2 = Tree-Procs $proc.Id
    Show-Tree 'hidden' $tree2
    Measure-Cpu 'idle hidden' $tree2 $SampleSeconds
  }
}

Get-Process -Name yal -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2
"stopped"

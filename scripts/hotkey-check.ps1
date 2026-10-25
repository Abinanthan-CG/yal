param(
  [string]$VirtualKey = "0x4B",
  [bool]$Control = $true,
  [bool]$Alt = $true,
  [bool]$Shift = $false
)

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class Inj {
  [StructLayout(LayoutKind.Sequential)]
  public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Explicit)]
  public struct IU { [FieldOffset(0)] public KEYBDINPUT ki; }
  [StructLayout(LayoutKind.Sequential)]
  public struct INPUT { public uint type; public IU U; }
  [DllImport("user32.dll")] public static extern uint SendInput(uint n, INPUT[] p, int s);
  public static void Combo(ushort ctrl, ushort alt, ushort shift, ushort key) {
    var k = new INPUT[1];
    k[0].type = 1;
    if (ctrl != 0) { k[0].U.ki.wVk = ctrl; k[0].U.ki.dwFlags = 0; SendInput(1, k, Marshal.SizeOf(typeof(INPUT))); }
    if (alt != 0) { k[0].U.ki.wVk = alt; k[0].U.ki.dwFlags = 0; SendInput(1, k, Marshal.SizeOf(typeof(INPUT))); }
    if (shift != 0) { k[0].U.ki.wVk = shift; k[0].U.ki.dwFlags = 0; SendInput(1, k, Marshal.SizeOf(typeof(INPUT))); }
    System.Threading.Thread.Sleep(60);
    k[0].U.ki.wVk = key; k[0].U.ki.dwFlags = 0; SendInput(1, k, Marshal.SizeOf(typeof(INPUT)));
    System.Threading.Thread.Sleep(60);
    k[0].U.ki.wVk = key; k[0].U.ki.dwFlags = 2; SendInput(1, k, Marshal.SizeOf(typeof(INPUT)));
    if (shift != 0) { k[0].U.ki.wVk = shift; k[0].U.ki.dwFlags = 2; SendInput(1, k, Marshal.SizeOf(typeof(INPUT))); }
    if (alt != 0) { k[0].U.ki.wVk = alt; k[0].U.ki.dwFlags = 2; SendInput(1, k, Marshal.SizeOf(typeof(INPUT))); }
    if (ctrl != 0) { k[0].U.ki.wVk = ctrl; k[0].U.ki.dwFlags = 2; SendInput(1, k, Marshal.SizeOf(typeof(INPUT))); }
  }
}
"@

$root = "C:\Users\dabin\development\yal"
$vk = [Convert]::ToInt32("4B", 16)
$key = [byte]$vk
$ctrl = if ($Control) { [byte]0x11 } else { [byte]0 }
$alt = if ($Alt) { [byte]0x12 } else { [byte]0 }
$shift = if ($Shift) { [byte]0x10 } else { [byte]0 }

$args = @("hotkey-wait", "--seconds", "20", "--virtual-key", "$vk")
if ($Control) { $args += "--control" }
if ($Alt) { $args += "--alt" }
if ($Shift) { $args += "--shift" }

$job = Start-Job -ArgumentList (,$args) -ScriptBlock {
  param($a)
  Set-Location "C:\Users\dabin\development\yal"
  & ".\target\debug\yal.exe" @a
}

Start-Sleep -Seconds 4
"Injecting vk=0x{0:X} ctrl={1} alt={2} shift={3}" -f $vk, $Control, $Alt, $Shift
[Inj]::Combo($ctrl, $alt, $shift, $key)
Start-Sleep -Seconds 2
[Inj]::Combo($ctrl, $alt, $shift, $key)
Start-Sleep -Seconds 2
Receive-Job $job -Wait | Select-Object -First 10
Remove-Job $job -Force

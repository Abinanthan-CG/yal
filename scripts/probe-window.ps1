param([string]$ProcessName = "yal-ui-probe")

$ErrorActionPreference = 'Stop'

# The window's coordinates are physical pixels only if this process is aware.
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class DpiAwareProbe {
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
}
"@
[void][DpiAwareProbe]::SetProcessDpiAwarenessContext([IntPtr](-4))

Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public class WindowProbe {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left, Top, Right, Bottom; }

  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr hWnd, StringBuilder name, int count);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);

  public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);

  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumProc callback, IntPtr lParam);

  public static List<string> Children(IntPtr parent) {
    List<string> found = new List<string>();
    EnumChildWindows(parent, (h, l) => {
      StringBuilder name = new StringBuilder(256);
      GetClassName(h, name, 256);
      RECT r; GetWindowRect(h, out r);
      found.Add(string.Format("{0} class={1} visible={2} rect=({3},{4})-({5},{6})",
        h.ToString(), name.ToString(), IsWindowVisible(h), r.Left, r.Top, r.Right, r.Bottom));
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@

$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
  Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $process) { "no window"; exit 1 }

$hwnd = $process.MainWindowHandle
$rect = New-Object WindowProbe+RECT
[void][WindowProbe]::GetWindowRect($hwnd, [ref]$rect)
$client = New-Object WindowProbe+RECT
[void][WindowProbe]::GetClientRect($hwnd, [ref]$client)
"window  rect = $($rect.Right - $rect.Left) x $($rect.Bottom - $rect.Top)"
"window  client = $($client.Right - $client.Left) x $($client.Bottom - $client.Top)"
"children:"
[WindowProbe]::Children($hwnd) | ForEach-Object { "  $_" }

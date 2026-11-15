param(
  [string]$ProcessName = "yal-ui-probe",
  [string]$OutputPath = "screen.png",
  [int]$Padding = 60
)

# Photographs what is actually on the screen around a window - the shadow, the
# rounded corners and the edges that a window-only capture cannot show.

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class DpiScreen {
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
}
"@
[void][DpiScreen]::SetProcessDpiAwarenessContext([IntPtr](-4))

Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public class ScreenShot {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left, Top, Right, Bottom; }

  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);

  public static void Capture(IntPtr hWnd, string path, int padding) {
    RECT rect;
    if (!GetWindowRect(hWnd, out rect)) throw new Exception("the window has no rectangle");
    int x = Math.Max(rect.Left - padding, 0);
    int y = Math.Max(rect.Top - padding, 0);
    int width = (rect.Right - rect.Left) + padding * 2;
    int height = (rect.Bottom - rect.Top) + padding * 2;

    using (Bitmap bitmap = new Bitmap(width, height, PixelFormat.Format32bppArgb))
    using (Graphics graphics = Graphics.FromImage(bitmap)) {
      graphics.CopyFromScreen(x, y, 0, 0, new Size(width, height));
      bitmap.Save(path, ImageFormat.Png);
    }
  }
}
"@

$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
  Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $process) { "no window for '$ProcessName'"; exit 1 }

# The window has to be in front, or the screen shows whatever is on top of it.
[void][ScreenShot]::SetForegroundWindow($process.MainWindowHandle)
Start-Sleep -Milliseconds 500

[void][ScreenShot]::Capture($process.MainWindowHandle, $OutputPath, $Padding)
"captured the screen around '$ProcessName' to $OutputPath"

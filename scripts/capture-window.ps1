param(
  [Parameter(Mandatory = $true)][string]$ProcessName,
  [Parameter(Mandatory = $true)][string]$OutputPath,
  [int]$TimeoutSeconds = 20
)

# Photographs a window so its look can be judged rather than guessed.
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\capture-window.ps1 `
#       -ProcessName yal-ui-probe -OutputPath ..\palette.png

$ErrorActionPreference = 'Stop'

# Without this the script's own coordinates are scaled and the capture comes
# out cropped on a display that is not at 100%.
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class DpiAware {
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
}
"@
[void][DpiAware]::SetProcessDpiAwarenessContext([IntPtr](-4))

Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public class WindowCapture {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left, Top, Right, Bottom; }

  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);

  public static void Capture(IntPtr hWnd, string path) {
    RECT rect;
    if (!GetWindowRect(hWnd, out rect)) throw new Exception("the window has no rectangle");
    int width = rect.Right - rect.Left;
    int height = rect.Bottom - rect.Top;
    if (width <= 0 || height <= 0) throw new Exception("the window has no size");

    using (Bitmap bitmap = new Bitmap(width, height, PixelFormat.Format32bppArgb))
    using (Graphics graphics = Graphics.FromImage(bitmap)) {
      IntPtr hdc = graphics.GetHdc();
      try {
        // 2 is PW_RENDERFULLCONTENT, which captures what the screen shows.
        PrintWindow(hWnd, hdc, 2);
      } finally {
        graphics.ReleaseHdc(hdc);
      }
      bitmap.Save(path, ImageFormat.Png);
    }
  }
}
"@

$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$process = $null
while ((Get-Date) -lt $deadline) {
  $process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  if ($process) { break }
  Start-Sleep -Milliseconds 250
}
if (-not $process) {
  "no window found for '$ProcessName' within $TimeoutSeconds seconds"
  exit 1
}

Start-Sleep -Milliseconds 700
[void][WindowCapture]::SetForegroundWindow($process.MainWindowHandle)
Start-Sleep -Milliseconds 400
[WindowCapture]::Capture($process.MainWindowHandle, $OutputPath)
"captured '$ProcessName' to $OutputPath"

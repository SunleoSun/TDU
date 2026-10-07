$ErrorActionPreference = 'SilentlyContinue'

$gameDir = Join-Path $PSScriptRoot 'game'
$gameExe = Join-Path $gameDir 'TestDriveUnlimited.exe'

Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class TduWindow
{
    private const int GWL_STYLE = -16;
    private const int WS_CAPTION = 0x00C00000;
    private const int WS_THICKFRAME = 0x00040000;
    private const int WS_MINIMIZEBOX = 0x00020000;
    private const int WS_MAXIMIZEBOX = 0x00010000;
    private const int WS_SYSMENU = 0x00080000;

    private const uint MONITOR_DEFAULTTONEAREST = 2;
    private const uint SWP_NOZORDER = 0x0004;
    private const uint SWP_NOOWNERZORDER = 0x0200;
    private const uint SWP_FRAMECHANGED = 0x0020;
    private const uint SWP_SHOWWINDOW = 0x0040;

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
    private struct MONITORINFO
    {
        public int cbSize;
        public RECT rcMonitor;
        public RECT rcWork;
        public uint dwFlags;
    }

    [DllImport("user32.dll")]
    private static extern int GetWindowLong(IntPtr hWnd, int nIndex);

    [DllImport("user32.dll")]
    private static extern int SetWindowLong(IntPtr hWnd, int nIndex, int dwNewLong);

    [DllImport("user32.dll")]
    private static extern IntPtr MonitorFromWindow(IntPtr hWnd, uint dwFlags);

    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    private static extern bool GetMonitorInfo(IntPtr hMonitor, ref MONITORINFO lpmi);

    [DllImport("user32.dll")]
    private static extern bool SetWindowPos(
        IntPtr hWnd,
        IntPtr hWndInsertAfter,
        int X,
        int Y,
        int cx,
        int cy,
        uint uFlags);

    public static bool MakeBorderlessFullscreen(IntPtr hWnd)
    {
        if (hWnd == IntPtr.Zero)
            return false;

        int style = GetWindowLong(hWnd, GWL_STYLE);
        style &= ~(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU);
        SetWindowLong(hWnd, GWL_STYLE, style);

        IntPtr monitor = MonitorFromWindow(hWnd, MONITOR_DEFAULTTONEAREST);
        MONITORINFO info = new MONITORINFO();
        info.cbSize = Marshal.SizeOf(typeof(MONITORINFO));

        if (!GetMonitorInfo(monitor, ref info))
            return false;

        int width = info.rcMonitor.Right - info.rcMonitor.Left;
        int height = info.rcMonitor.Bottom - info.rcMonitor.Top;

        return SetWindowPos(
            hWnd,
            IntPtr.Zero,
            info.rcMonitor.Left,
            info.rcMonitor.Top,
            width,
            height,
            SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
    }
}
'@

Get-Process TestDriveUnlimited -ErrorAction SilentlyContinue | Stop-Process -Force

$game = Start-Process $gameExe -ArgumentList '-w' -WorkingDirectory $gameDir -PassThru

# TDU adjusts/recreates its window during startup. Re-apply borderless briefly
# so the final game window fills the monitor without switching to legacy fullscreen.
for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep -Milliseconds 250
    $game.Refresh()

    if ($game.HasExited) {
        break
    }

    if ($game.MainWindowHandle -ne 0) {
        [TduWindow]::MakeBorderlessFullscreen($game.MainWindowHandle) | Out-Null
    }
}
# Sends mouse input to a process window at client coordinates (for UI smoke tests).
#   powershell -File scripts/click-window.ps1 -X 1168 -Y 128 [-Button right] [-HoldMs 500] [-Shift]
param(
    [string]$Process = "tenant-rs92",
    [int]$X,
    [int]$Y,
    [string]$Button = "left",
    [int]$HoldMs = 60,
    [switch]$Shift,
    [int]$Count = 1,
    [int]$GapMs = 60
)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Mouse {
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr e);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
"@
[Mouse]::SetProcessDPIAware() | Out-Null
$p = Get-Process -Name $Process | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
$h = $p.MainWindowHandle
[Mouse]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 150
$pt = New-Object Mouse+POINT
$pt.X = $X; $pt.Y = $Y
[Mouse]::ClientToScreen($h, [ref]$pt) | Out-Null
[Mouse]::SetCursorPos($pt.X, $pt.Y) | Out-Null
Start-Sleep -Milliseconds 80
if ($Shift) { [Mouse]::keybd_event(0x10, 0, 0, [IntPtr]::Zero) }
if ($Button -eq "right") { $down = 0x0008; $up = 0x0010 } else { $down = 0x0002; $up = 0x0004 }
for ($i = 0; $i -lt $Count; $i++) {
    [Mouse]::mouse_event($down, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds $HoldMs
    [Mouse]::mouse_event($up, 0, 0, 0, [IntPtr]::Zero)
    if ($i -lt $Count - 1) { Start-Sleep -Milliseconds $GapMs }
}
if ($Shift) { [Mouse]::keybd_event(0x10, 0, 2, [IntPtr]::Zero) }

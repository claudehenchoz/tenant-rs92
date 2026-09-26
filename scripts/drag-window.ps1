# Drags the mouse in a process window between two client points (UI smoke tests).
param([string]$Process = "tenant-rs92", [int]$X1, [int]$Y1, [int]$X2, [int]$Y2, [int]$Steps = 12)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class M2 {
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
"@
[M2]::SetProcessDPIAware() | Out-Null
$h = (Get-Process -Name $Process | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).MainWindowHandle
[M2]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 100
function At($x, $y) { $p = New-Object M2+POINT; $p.X = $x; $p.Y = $y; [M2]::ClientToScreen($h, [ref]$p) | Out-Null; [M2]::SetCursorPos($p.X, $p.Y) | Out-Null }
At $X1 $Y1; Start-Sleep -Milliseconds 50
[M2]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
for ($i = 1; $i -le $Steps; $i++) {
    At ([int]($X1 + ($X2 - $X1) * $i / $Steps)) ([int]($Y1 + ($Y2 - $Y1) * $i / $Steps)); Start-Sleep -Milliseconds 20
}
Start-Sleep -Milliseconds 50
[M2]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)

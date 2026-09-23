# How many visible top-level windows the process has on the desktop this script runs on —
# the user's. With -Guard it keeps watching and kills the process the moment one appears.
param([Parameter(Mandatory)] [int] $ProcessId, [switch] $Guard)

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class VisibleWindows {
  delegate bool Enum(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(Enum f, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  public static int Count(uint owner) {
    int n = 0;
    EnumWindows((h, l) => { uint pid; GetWindowThreadProcessId(h, out pid); if (pid == owner && IsWindowVisible(h)) n++; return true; }, IntPtr.Zero);
    return n;
  }
}
"@
if (-not $Guard) { [VisibleWindows]::Count($ProcessId); exit }
while (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue) {
  if ([VisibleWindows]::Count($ProcessId) -gt 0) {
    Stop-Process -Id $ProcessId -Force
    "KILLED: a window reached the user's desktop"
    exit 1
  }
  Start-Sleep -Milliseconds 100
}
"clean"

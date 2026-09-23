# Starts a program on a desktop of its own: its windows never reach the screen of whoever
# sits at the machine and cannot take the focus. Prints "<pid> <unix ms at CreateProcess>".
param([Parameter(Mandatory)] [string] $Exe, [string] $Desktop = "cogit-bench")

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class HiddenDesktop {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  struct StartupInfo {
    public int cb; public string reserved; public string desktop; public string title;
    public int x, y, xSize, ySize, xChars, yChars, fill, flags; public short show, reserved2;
    public IntPtr reserved3, stdIn, stdOut, stdErr;
  }
  [StructLayout(LayoutKind.Sequential)]
  struct ProcessInfo { public IntPtr process, thread; public int pid, tid; }
  [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  static extern IntPtr CreateDesktop(string name, IntPtr device, IntPtr mode, int flags, uint access, IntPtr attributes);
  [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  static extern bool CreateProcess(string app, string cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags,
    IntPtr env, string dir, ref StartupInfo si, out ProcessInfo pi);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  public static string Start(string exe, string desktop, string dir) {
    IntPtr handle = CreateDesktop(desktop, IntPtr.Zero, IntPtr.Zero, 0, 0x10000000, IntPtr.Zero);
    if (handle == IntPtr.Zero) throw new Exception("CreateDesktop failed: " + Marshal.GetLastWin32Error());
    var si = new StartupInfo(); si.cb = Marshal.SizeOf(si); si.desktop = desktop;
    ProcessInfo pi;
    if (!CreateProcess(exe, "\"" + exe + "\"", IntPtr.Zero, IntPtr.Zero, false, 0, IntPtr.Zero, dir, ref si, out pi))
      throw new Exception("CreateProcess failed: " + Marshal.GetLastWin32Error());
    long at = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
    CloseHandle(pi.thread); CloseHandle(pi.process);
    return pi.pid + " " + at;
  }
}
"@
[HiddenDesktop]::Start($Exe, $Desktop, (Split-Path $Exe))

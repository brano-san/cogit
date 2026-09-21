<#
.SYNOPSIS
  Samples the working set of the WebView2 processes belonging to one app instance.

.DESCRIPTION
  The app logs the same figures itself as `kind=procmem`, but that stops when the app
  stops. This one is a separate process, so it keeps the last rows after a renderer dies
  and after the host exits — which is the whole point when chasing an Out of Memory.

  Descendants, not children: WebView2 puts the renderer below its own browser process.

.EXAMPLE
  powershell -File scripts/oom/sample-processes.ps1 -AppPid 1234 -Out run\procmem.csv
#>
param(
  [Parameter(Mandatory = $true)][int]$AppPid,
  [Parameter(Mandatory = $true)][string]$Out,
  [int]$EverySeconds = 2,
  [int]$MinutesToRun = 60
)

$ErrorActionPreference = 'Stop'

$dir = Split-Path -Parent $Out
if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
'timestamp,processes,rss_kib,largest_kib,host_rss_kib' | Out-File -FilePath $Out -Encoding utf8

function Get-Descendants([int]$root, $byParent) {
  $found = New-Object System.Collections.Generic.List[object]
  $queue = New-Object System.Collections.Generic.Queue[int]
  $queue.Enqueue($root)
  $seen = New-Object System.Collections.Generic.HashSet[int]
  [void]$seen.Add($root)

  while ($queue.Count -gt 0) {
    $pidNow = $queue.Dequeue()
    if (-not $byParent.ContainsKey($pidNow)) { continue }
    foreach ($child in $byParent[$pidNow]) {
      if (-not $seen.Add([int]$child.ProcessId)) { continue }
      $found.Add($child)
      $queue.Enqueue([int]$child.ProcessId)
    }
  }
  return $found
}

$deadline = (Get-Date).AddMinutes($MinutesToRun)
Write-Host "sampling pid $AppPid every ${EverySeconds}s into $Out"

while ((Get-Date) -lt $deadline) {
  # One CIM call per tick: querying each process separately costs more than the sample.
  $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name, WorkingSetSize

  $byParent = @{}
  foreach ($p in $all) {
    $parent = [int]$p.ParentProcessId
    if (-not $byParent.ContainsKey($parent)) { $byParent[$parent] = New-Object System.Collections.Generic.List[object] }
    $byParent[$parent].Add($p)
  }

  $host_rss = 0
  $hostProc = $all | Where-Object { [int]$_.ProcessId -eq $AppPid }
  if ($hostProc) { $host_rss = [int64]($hostProc.WorkingSetSize / 1KB) }

  $webviews = Get-Descendants -root $AppPid -byParent $byParent |
    Where-Object { $_.Name -ieq 'msedgewebview2.exe' }

  $count = ($webviews | Measure-Object).Count
  $rss = 0
  $largest = 0
  foreach ($w in $webviews) {
    $kib = [int64]($w.WorkingSetSize / 1KB)
    $rss += $kib
    if ($kib -gt $largest) { $largest = $kib }
  }

  $stamp = (Get-Date).ToString('o')
  "$stamp,$count,$rss,$largest,$host_rss" | Add-Content -Path $Out -Encoding utf8

  if ($count -eq 0 -and $host_rss -eq 0) {
    "$stamp,0,0,0,0  # host gone" | Add-Content -Path $Out -Encoding utf8
    Write-Host 'host process gone; stopping'
    break
  }

  Start-Sleep -Seconds $EverySeconds
}

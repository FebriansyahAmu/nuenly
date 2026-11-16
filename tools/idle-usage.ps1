# Mengukur memori dan CPU Nuenly saat diam. Target PRD: memori <= 150 MB, CPU sekitar 0%.
#
# Yang dihitung adalah nuenly.exe beserta semua proses turunannya (WebView2), dengan memori
# "private working set" seperti kolom Memory di Task Manager.
#
# Pemakaian: jalankan build rilis Nuenly, tunggu sampai diam, jangan sentuh Nuenly, lalu:
#   powershell -ExecutionPolicy Bypass -File tools\idle-usage.ps1 -Minutes 10

param([double]$Minutes = 10)

$ErrorActionPreference = 'Stop'

$root = Get-Process -Name nuenly -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $root) {
    Write-Error 'nuenly.exe tidak berjalan.'
}

# ID nuenly.exe dan semua turunannya.
function Get-TreeIds([int]$RootId) {
    $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
    $ids = [System.Collections.Generic.List[int]]::new()
    $ids.Add($RootId)
    for ($i = 0; $i -lt $ids.Count; $i++) {
        foreach ($process in $all) {
            $id = [int]$process.ProcessId
            if ($process.ParentProcessId -eq $ids[$i] -and -not $ids.Contains($id)) {
                $ids.Add($id)
            }
        }
    }
    return $ids
}

function Get-Usage([int[]]$Ids) {
    # Nama counter WMI ini tidak diterjemahkan, jadi aman di Windows berbahasa Indonesia.
    $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
        Where-Object { $Ids -contains [int]$_.IDProcess }
    $cpuSeconds = 0.0
    foreach ($id in $Ids) {
        $process = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($process) {
            $cpuSeconds += $process.TotalProcessorTime.TotalSeconds
        }
    }
    return [pscustomobject]@{
        PrivateMB  = ($perf | Measure-Object -Property WorkingSetPrivate -Sum).Sum / 1MB
        CpuSeconds = $cpuSeconds
        Processes  = $perf | Sort-Object WorkingSetPrivate -Descending |
            Select-Object Name, IDProcess, @{ Name = 'PrivateMB'; Expression = { [math]::Round($_.WorkingSetPrivate / 1MB, 1) } }
    }
}

$ids = Get-TreeIds $root.Id
$before = Get-Usage $ids
$started = Get-Date
Write-Host "Mengukur $($ids.Count) proses selama $Minutes menit. Jangan sentuh Nuenly."
Start-Sleep -Seconds ([int]($Minutes * 60))

$ids = Get-TreeIds $root.Id
$after = Get-Usage $ids
$elapsed = ((Get-Date) - $started).TotalSeconds
$cpuPercent = ($after.CpuSeconds - $before.CpuSeconds) / ($elapsed * [Environment]::ProcessorCount) * 100

$after.Processes | Format-Table -AutoSize
Write-Host ('Memori (private working set): {0:N1} MB  (target <= 150 MB)' -f $after.PrivateMB)
Write-Host ('CPU rata-rata: {0:N2}% dari semua inti  (target sekitar 0%)' -f $cpuPercent)

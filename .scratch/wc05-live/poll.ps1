# Reachability poll for ticket 05 live acceptance.
# Runs the read-only manual_paired_connect probe against the paired A3062 every
# cycle and stops as soon as the A3062 answers. Only `01 01` (read) is written.
param(
    [int]$Minutes = 25,
    [int]$PauseSeconds = 40,
    [string]$Log = "$PSScriptRoot\poll.log"
)
$ErrorActionPreference = 'Continue'
$deadline = (Get-Date).AddMinutes($Minutes)
$repo = (Resolve-Path "$PSScriptRoot\..\..").Path
$desktop = Join-Path $repo 'desktop'
$cycle = 0
function Log-Line([string]$text) {
    $line = "[{0}] {1}" -f (Get-Date -Format 'HH:mm:ss'), $text
    Add-Content -Path $Log -Value $line
    Write-Host $line
}
Log-Line "poll start (deadline $(Get-Date $deadline -Format 'HH:mm:ss'), pause ${PauseSeconds}s)"
while ((Get-Date) -lt $deadline) {
    $cycle++
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $out = & cargo test --manifest-path (Join-Path $desktop 'Cargo.toml') -p soundcore-desktop manual_paired_connect -- --ignored --nocapture 2>&1 | Out-String
    $sw.Stop()
    # Split the probe output per model section and look for the A3062 answer.
    $a3062 = $false
    $lines = $out -split "`r?`n"
    $in = $false
    foreach ($line in $lines) {
        if ($line -match '^== A3062 ') { $in = $true; continue }
        if ($line -match '^== ') { $in = $false; continue }
        if ($in -and $line -match 'connected:') { $a3062 = $true }
    }
    $summary = ($lines | Where-Object { $_ -match '^== |connected:|failed \(|firmware:' }) -join ' | '
    Log-Line ("cycle {0} ({1:n0}s): A3062 reachable={2} :: {3}" -f $cycle, $sw.Elapsed.TotalSeconds, $a3062, $summary)
    if (-not (Test-Path $Log)) { New-Item -ItemType File -Force $Log | Out-Null }
    Set-Content -Path "$PSScriptRoot\poll-last-cycle.txt" -Value $out
    if ($a3062) {
        Log-Line "A3062 ANSWERED - stopping poll"
        Set-Content -Path "$PSScriptRoot\reachable.flag" -Value ((Get-Date).ToString('o'))
        exit 0
    }
    if ((Get-Date).AddSeconds($PauseSeconds) -lt $deadline) { Start-Sleep -Seconds $PauseSeconds }
}
Log-Line "poll ended without an A3062 answer"
exit 1

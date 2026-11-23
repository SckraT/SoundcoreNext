param([switch]$KeepRunning, [string]$Exe = "$PSScriptRoot\..\..\desktop\target\debug\soundcore-desktop.exe")
. "$PSScriptRoot\uia-lib.ps1"

$exe = (Resolve-Path $Exe).Path
Write-Host "launching $exe"
$proc = Start-Process -FilePath $exe -PassThru
Write-Host "pid $($proc.Id)"
$win = Get-AppWindow -ProcessId $proc.Id -TimeoutSec 40
Write-Host "window: '$($win.Current.Name)' handle=$($win.Current.NativeWindowHandle)"
Start-Sleep -Seconds 2
$script:procId = $proc.Id
$win | ConvertTo-Json -Depth 1 | Out-Null
Get-Tree -Root $win | Set-Content "$PSScriptRoot\tree.txt" -Encoding utf8
Write-Host "tree lines: $((Get-Content "$PSScriptRoot\tree.txt").Count)"
Get-Content "$PSScriptRoot\tree.txt" -TotalCount 60
if (-not $KeepRunning) {
    Write-Host 'leaving app running (stop manually or rerun)'
}

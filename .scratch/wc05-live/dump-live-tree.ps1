# Dump the current app window's UIA control tree to a file for element-name work.
param([string]$Out = "$PSScriptRoot\tree.txt")
. "$PSScriptRoot\..\wc04-uia\session.ps1"
$proc = Get-Process -Name soundcore-desktop -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { 'app not running'; exit 1 }
Connect-Ui | Out-Null
Start-Sleep -Milliseconds 500
Get-Tree -Root $win | Set-Content $Out -Encoding utf8
"tree lines: $((Get-Content $Out).Count) -> $Out"
Get-Content $Out -TotalCount 80

# Restore the app's preference documents from the pre-acceptance snapshot and
# stop the app, so the machine is left as found.
$ErrorActionPreference = 'Stop'
$dir = "$env:APPDATA\OpenSCL4"
$backup = "$PSScriptRoot\appdata-backup"
Get-Process -Name soundcore-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 800
$manifest = @(Get-Content (Join-Path $backup 'manifest.txt') -ErrorAction SilentlyContinue)
foreach ($name in 'desktop-preferences.json', 'overview-preferences.json', 'equalizer-profiles.json') {
    $path = Join-Path $dir $name
    if ($manifest -contains $name) {
        Copy-Item (Join-Path $backup $name) $path -Force
        "restored $name"
    } elseif (Test-Path $path) {
        Remove-Item $path -Force
        "removed $name (did not exist before the run)"
    }
}
"app documents restored to the pre-run snapshot"

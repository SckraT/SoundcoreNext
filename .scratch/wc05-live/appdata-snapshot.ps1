# Snapshot the app's own preference documents before the live acceptance, so the
# run can be restored exactly. Writes copies under .scratch/wc05-live/appdata-backup
# plus a manifest of which documents existed.
$ErrorActionPreference = 'Stop'
$dir = "$env:APPDATA\OpenSCL4"
$backup = "$PSScriptRoot\appdata-backup"
Get-Process -Name soundcore-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 800
Remove-Item -Recurse -Force $backup -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $backup | Out-Null
$manifest = @()
foreach ($name in 'desktop-preferences.json', 'overview-preferences.json', 'equalizer-profiles.json') {
    $path = Join-Path $dir $name
    if (Test-Path $path) {
        Copy-Item $path (Join-Path $backup $name)
        $manifest += "$name"
    }
}
Set-Content -Path (Join-Path $backup 'manifest.txt') -Value $manifest
"snapshot: existing documents = [$($manifest -join ', ')]"

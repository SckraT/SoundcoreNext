# Ticket 05 live acceptance, phase 1: real Add Device connection and Overview.
# Drives the running build through Windows UI Automation against the paired
# Space One Pro (A3062). The device's user-facing name and values only; no
# address, serial or MAC is printed, recorded or captured.
# Usage: .\live-phase1-connect-overview.ps1 [-DryRun]
param([switch]$DryRun)
. "$PSScriptRoot\..\wc04-uia\session.ps1"
$script:ShotsDir = "$PSScriptRoot\shots"
New-Item -ItemType Directory -Force $script:ShotsDir | Out-Null
$overviewPath = "$env:APPDATA\OpenSCL4\overview-preferences.json"
$failures = New-Object System.Collections.Generic.List[string]

function Check {
    param([string]$What, [bool]$Condition, [string]$Detail = '')
    if ($Condition) { "  PASS $What" } else { "  FAIL $What $Detail"; $failures.Add("$What $Detail") }
}
function Find-ButtonNamed([string]$Name, [int]$TimeoutSec = 10) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        $hit = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq $Name } | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 200
    } while ((Get-Date) -lt $deadline)
    return $null
}
function Save-PhaseShot([string]$Name) { Save-WindowClientPng -Handle $hwnd -Path "$script:ShotsDir\$Name.png" | Out-Null }

"== Start a fresh app =="
Stop-App
Start-App | Out-Null
Connect-Ui -TimeoutSec 40 | Out-Null
Check 'the shell starts on My devices' ((Get-WindowTitle) -match 'My devices')

"== Add Device: choose the real model =="
Click-Button 'Add device' -SettleMs 1200
Check 'the model picker opened' ([bool](Find-Text 'Find your model.' -TimeoutSec 8))
$search = Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -eq 'Search models' } | Select-Object -First 1
Check 'the model search field is offered' ([bool]$search)
if ($search) {
    Invoke-ClickCenter $search; Start-Sleep -Milliseconds 300
    [System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 120
    [System.Windows.Forms.SendKeys]::SendWait('A3062'); Start-Sleep -Milliseconds 1200
}
$card = Find-ButtonNamed 'Space One Pro'
Check 'the A3062 model card is filtered to' ([bool]$card)
if (-not $card) { throw 'the Space One Pro model card was not found' }
[void](Invoke-Element $card)
Start-Sleep -Milliseconds 1200
Check 'the paired-device step opened' ([bool](Find-Text 'Paired devices' -TimeoutSec 8))
Check 'the real paired list is shown and Demo mode is off' (-not [bool](Find-Text 'Simulated device data' -TimeoutSec 1))
$row = Find-ButtonNamed 'soundcore Space One Pro' 8
Check 'the Windows-paired Space One Pro is listed' ([bool]$row)
$rows = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'soundcore Space One Pro' }
Check 'the suggested-model hint is shown' ([bool](Find-Text 'Paired in Windows · looks like the chosen model' -TimeoutSec 3))
Save-PhaseShot 'live-add-device' | Out-Null
if ($DryRun) {
    "DRY RUN: stopping before the connection"
    if ($failures.Count) { $failures | ForEach-Object { "FAIL - $_" }; exit 1 }
    exit 0
}
if (-not $row) { throw 'the paired Space One Pro row was not found' }

"== Connect over real RFCOMM =="
[void](Invoke-Element $row)
$connected = $false
$deadline = (Get-Date).AddSeconds(60)
do {
    if ((Get-WindowTitle) -match 'Overview' -and (Find-Text 'Connected' -TimeoutSec 1)) { $connected = $true; break }
    Start-Sleep -Milliseconds 400
} while ((Get-Date) -lt $deadline)
Check 'the real RFCOMM connection reaches the Overview' $connected "title=$(Get-WindowTitle)"
if (-not $connected) {
    $texts = Get-VisibleTexts
    "  visible: $($texts -join ' | ')"
    Save-PhaseShot 'live-connect-failed' | Out-Null
    $failures | ForEach-Object { "FAIL - $_" }
    exit 1
}
Start-Sleep -Milliseconds 1500
Check 'the window title names the real device' ((Get-WindowTitle) -match 'Overview . soundcore Space One Pro') "title=$(Get-WindowTitle)"

"== Overview: identity, battery, capability filtering =="
$texts = Get-VisibleTexts
Check 'the hero names the real Space One Pro' ([bool]($texts | Where-Object { $_ -eq 'soundcore Space One Pro' }))
Check 'the hero shows the catalog code A3062' ([bool]($texts | Where-Object { $_ -eq 'A3062' }))
$battery = $texts | Where-Object { $_ -match '^Battery: \d+%$' }
Check 'the single-battery tile shows the real battery' ([bool]$battery) "text=$($battery -join ',')"
Check 'the connected badge is shown' ([bool]($texts | Where-Object { $_ -eq 'Connected' }))
Check 'no Bluetooth address or serial is shown' (-not ($texts -match '([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}'))
$firmwareRow = $texts | Where-Object { $_ -match 'Firmware' }
"  firmware text: $firmwareRow"

"== Overview: quick settings and supported blocks =="
$quick = Find-ButtonNamed 'LDAC'
Check 'the quick-settings panel holds the A3062 LDAC chip' ([bool]$quick)
$ldacText = $texts | Where-Object { $_ -match '^LDAC · ' }
"  LDAC chip text: $ldacText"
foreach ($block in 'Sound mode', 'Equalizer', 'Controls', 'Comfort', 'Power and prompts', 'Connections') {
    Check "the $block shortcut/panel is shown" ([bool](Find-ButtonNamed $block 3))
}
Check 'no Quick presets block is offered' (-not [bool](Find-ButtonNamed 'Quick presets' 1))
Check 'no Case block is offered' (-not [bool](Find-ButtonNamed 'Case' 1))
Save-PhaseShot 'live-overview' | Out-Null

"== Overview: layout editing =="
$beforeDocument = if (Test-Path $overviewPath) { Get-Content $overviewPath -Raw } else { $null }
Click-Button 'Edit device screen' -SettleMs 1100
Check 'the layout editor opened' ([bool](Find-Text 'Edit device screen' -TimeoutSec 2) -or [bool](Find-ButtonNamed 'Full' 2))
Click-Button 'Full' -SettleMs 900
Click-Button 'Done' -SettleMs 1100
Check 'the Full preset is applied and saved' ((Test-Path $overviewPath) -and ((Get-Content $overviewPath -Raw) -notmatch '!'))
Check 'the Power and prompts block is still on the Overview' ([bool](Find-ButtonNamed 'Power and prompts' 3))
Click-Button 'Edit device screen' -SettleMs 1100
$toggle = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Controls' -and [int]$_.Current.BoundingRectangle.Width -eq 48 } | Select-Object -First 1
Check 'the editor lists a hide switch for Controls' ([bool]$toggle)
if ($toggle) { [void](Invoke-Element $toggle); Start-Sleep -Milliseconds 700 }
Click-Button 'Done' -SettleMs 1100
Check 'hiding a block is saved with the ! marker' ((Get-Content $overviewPath -Raw) -match '!controls')
Check 'the hidden block left the Overview' (-not [bool](Find-ButtonNamed 'Controls' 1))
Click-Button 'Edit device screen' -SettleMs 1100
Click-Button 'Standard' -SettleMs 900
Click-Button 'Done' -SettleMs 1100
Check 'the Standard preset restores Controls' ([bool](Find-ButtonNamed 'Controls' 3))

"== Overview: quick-toggle chooser =="
Click-Button 'Choose quick settings' -SettleMs 1200
$switches = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { [int]$_.Current.BoundingRectangle.Width -eq 48 }
Check 'the chooser lists the supported toggles' ($switches.Count -ge 5) "count=$($switches.Count)"
$dolby = $switches | Where-Object { $_.Current.Name -eq 'Dolby audio' } | Select-Object -First 1
if ($dolby) { [void](Invoke-Element $dolby); Start-Sleep -Milliseconds 800 }
Click-Button 'Done' -SettleMs 1100
$chosen = if (Test-Path $overviewPath) { (Get-Content $overviewPath -Raw | ConvertFrom-Json).quick_toggles.A3062 } else { $null }
Check 'the per-model toggle selection is saved' ([bool]($chosen -contains 'dolbyAudio')) "chosen=$($chosen -join ',')"
Click-Button 'Choose quick settings' -SettleMs 1100
$dolby = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Dolby audio' -and [int]$_.Current.BoundingRectangle.Width -eq 48 } | Select-Object -First 1
if ($dolby) { [void](Invoke-Element $dolby); Start-Sleep -Milliseconds 800 }
Click-Button 'Done' -SettleMs 1100
$chosen = (Get-Content $overviewPath -Raw | ConvertFrom-Json).quick_toggles.A3062
Check 'the selection is restored to LDAC only' (@($chosen).Count -eq 1) "chosen=$($chosen -join ',')"

"== Page captures on the real device =="
foreach ($tab in @(@('Sound mode', 'live-sound-mode'), @('Equalizer', 'live-equalizer'), @('Controls', 'live-controls'), @('Comfort', 'live-comfort'), @('Power and prompts', 'live-power'), @('Connections', 'live-connections'), @('About the device', 'live-about'), @('Diagnostics', 'live-diagnostics'))) {
    Click-Tab $tab[0]; Start-Sleep -Milliseconds 1100
    Save-PhaseShot $tab[1] | Out-Null
    "  captured $($tab[1]) on $($tab[0])"
}
$about = Get-VisibleTexts
Check 'About shows the real firmware 04.39' ([bool]($about | Where-Object { $_ -match '04\.39' }) -or [bool]($texts | Where-Object { $_ -match '04\.39' })) "about=$($about | Where-Object { $_ -match 'Firmware|04\.' })"
Check 'About has no serial row' (-not ($about | Where-Object { $_ -match '^Serial' }))
Check 'no address is on the About page' (-not ($about -match '([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}'))

"== Restore the Overview preferences document =="
Click-Tab 'Overview'; Start-Sleep -Milliseconds 800
if ($null -eq $beforeDocument) {
    if (Test-Path $overviewPath) { Remove-Item $overviewPath -Force }
    "  overview-preferences.json removed (it did not exist before the run)"
} else {
    Set-Content -Path $overviewPath -Value $beforeDocument -NoNewline
    "  overview-preferences.json restored to the pre-run document"
}

if ($failures.Count -gt 0) { ""; "FAILURES:"; $failures | ForEach-Object { " - $_" }; exit 1 }
""; "PHASE 1 PASSED"

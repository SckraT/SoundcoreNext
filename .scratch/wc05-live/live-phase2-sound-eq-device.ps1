# Ticket 05 live acceptance, phase 2: Sound mode, Equalizer and one real device
# page write with re-poll, then every changed value restored.
# Requires the app already connected (phase 1) and running in English.
. "$PSScriptRoot\..\wc04-uia\session.ps1"
Connect-Ui | Out-Null
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
function Get-Checked([string]$Name) {
    $el = Find-ButtonNamed $Name 6
    if (-not $el) { return $null }
    $p = $null
    if ($el.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$p)) { return ($p.Current.ToggleState -eq 'On') }
    return $null
}
function Save-PhaseShot([string]$Name) { Save-WindowClientPng -Handle $hwnd -Path "$script:ShotsDir\$Name.png" | Out-Null }

"== Sound mode: write and re-poll =="
Click-Tab 'Sound mode'; Start-Sleep -Milliseconds 1000
$modes = @('Noise canceling', 'Transparency', 'Normal')
$initialMode = $modes | Where-Object { (Get-Checked $_) -eq $true } | Select-Object -First 1
Check 'a current sound mode is selected from the device answer' ([bool]$initialMode) "initial=$initialMode"
$targetMode = $modes | Where-Object { $_ -ne $initialMode } | Select-Object -First 1
[void](Invoke-Element (Find-ButtonNamed $targetMode 6))
Start-Sleep -Seconds 2
$now = $modes | Where-Object { (Get-Checked $_) -eq $true } | Select-Object -First 1
Check "choosing $targetMode writes and re-polls" ($now -eq $targetMode) "selected=$now"
Check 'no write error banner is shown' (-not (Get-VisibleTexts | Where-Object { $_ -match 'Could not change' }))
Save-PhaseShot 'live-sound-mode-switched' | Out-Null
[void](Invoke-Element (Find-ButtonNamed $initialMode 6))
Start-Sleep -Seconds 2
$restored = $modes | Where-Object { (Get-Checked $_) -eq $true } | Select-Object -First 1
Check 'the mode found on connect is restored' ($restored -eq $initialMode) "restored=$restored"

"== Equalizer: preset write and re-poll =="
Click-Tab 'Equalizer'; Start-Sleep -Milliseconds 1400
$texts = Get-VisibleTexts
$tag = $texts | Where-Object { $_ -in @('Soundcore Signature', 'Acoustic', 'Bass Booster', 'Flat') -or $_ -eq 'Your own settings' } | Select-Object -First 1
"  initial equalizer tag: $tag"
$presetNames = @('Soundcore Signature', 'Acoustic', 'Bass Booster', 'Flat', 'Podcast', 'Treble Booster')
$initialPreset = $presetNames | Where-Object { (Get-Checked $_) -eq $true } | Select-Object -First 1
Check 'the current preset is selected from the device answer' ([bool]$initialPreset) "initial=$initialPreset"
$targetPreset = $presetNames | Where-Object { $_ -ne $initialPreset } | Select-Object -First 1
[void](Invoke-Element (Find-ButtonNamed $targetPreset 8))
Start-Sleep -Seconds 2
$nowPreset = $presetNames | Where-Object { (Get-Checked $_) -eq $true } | Select-Object -First 1
Check "choosing the $targetPreset preset writes and re-polls" ($nowPreset -eq $targetPreset) "selected=$nowPreset"
$band = Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -eq '800 Hz, dB' } | Select-Object -First 1
Check 'the editable band field shows the re-polled preset curve' ([bool]$band)

"== Equalizer: custom band, reset, restore =="
$initialBands = @{}
foreach ($field in (Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -match ', dB$' })) {
    $vp = $null
    if ($field.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$vp)) { $initialBands[$field.Current.Name] = $vp.Current.Value }
}
"  initial bands: $(($initialBands.GetEnumerator() | Sort-Object Name | ForEach-Object { "$($_.Name)=$($_.Value)" }) -join ';')"
$field = Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -eq '800 Hz, dB' } | Select-Object -First 1
Invoke-ClickCenter $field; Start-Sleep -Milliseconds 400
[System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 120
[System.Windows.Forms.SendKeys]::SendWait('9'); Start-Sleep -Milliseconds 250
[System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
Start-Sleep -Seconds 2
Check 'typing a band value creates a custom curve from the re-poll' ([bool](Find-Text 'Your own settings' -TimeoutSec 3))
$field = Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -eq '800 Hz, dB' } | Select-Object -First 1
$vp = $null; [void]$field.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$vp)
Check 'the re-polled band value is shown' ($vp.Current.Value -eq '9') "value=$($vp.Current.Value)"
Check 'no write error banner is shown' (-not (Get-VisibleTexts | Where-Object { $_ -match 'Could not change' }))
Save-PhaseShot 'live-equalizer-custom' | Out-Null
[void](Invoke-Element (Find-ButtonNamed 'Reset' 6))
Start-Sleep -Seconds 2
Check 'Reset keeps the custom curve state' ([bool](Find-Text 'Your own settings' -TimeoutSec 3))
# Restore the curve/mode found on connect: a built-in preset by clicking its chip,
# a custom curve by typing every captured band value back.
if ($initialPreset) {
    [void](Invoke-Element (Find-ButtonNamed $initialPreset 6))
    Start-Sleep -Seconds 2
    Check 'the preset found on connect is restored' ((Get-Checked $initialPreset) -eq $true)
} else {
    foreach ($name in $initialBands.Keys) {
        $f = Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -eq $name } | Select-Object -First 1
        if (-not $f) { continue }
        Invoke-ClickCenter $f; Start-Sleep -Milliseconds 250
        [System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 100
        [System.Windows.Forms.SendKeys]::SendWait([string]$initialBands[$name]); Start-Sleep -Milliseconds 150
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}'); Start-Sleep -Milliseconds 900
    }
    Start-Sleep -Seconds 2
    "  restored the custom curve found on connect"
}

"== Comfort: one real toggle write, re-poll, restore =="
Click-Tab 'Comfort'; Start-Sleep -Milliseconds 1200
$side = Find-ButtonNamed 'Side tone' 6
Check 'the Comfort page offers the measured Side tone row' ([bool]$side)
$before = $null; $p = $null; $p2 = $null; $p3 = $null
if ($side.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$p)) { $before = $p.Current.ToggleState }
if ($p) { $p.Toggle(); Start-Sleep -Seconds 2 }
$side = Find-ButtonNamed 'Side tone' 6
$after = $null
if ($side.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$p2)) { $after = $p2.Current.ToggleState }
Check 'the Side tone toggle writes and re-polls the device' ($before -ne $after) "$before -> $after"
Check 'no write error banner is shown' (-not (Get-VisibleTexts | Where-Object { $_ -match 'Could not change' }))
if ($p2) { $p2.Toggle(); Start-Sleep -Seconds 2 }
$side = Find-ButtonNamed 'Side tone' 6
$restored = $null
if ($side.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$p3)) { $restored = $p3.Current.ToggleState }
Check 'the Side tone value found on connect is restored' ($restored -eq $before) "$before -> $restored"

"== About: real values =="
Click-Tab 'About the device'; Start-Sleep -Milliseconds 1200
$about = Get-VisibleTexts
Check 'About shows the firmware found on the device' ([bool]($about | Where-Object { $_ -match '^\d\d\.\d\d$' -or $_ -match '04\.39' })) "texts=$($about | Where-Object { $_ -match 'Firmware|Charging|Battery|04\.' })"
Check 'About has no serial row' (-not ($about | Where-Object { $_ -match '^Serial' }))
Check 'no Bluetooth address is on the page' (-not ($about -match '([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}'))

if ($failures.Count -gt 0) { ""; "FAILURES:"; $failures | ForEach-Object { " - $_" }; exit 1 }
""; "PHASE 2 PASSED"

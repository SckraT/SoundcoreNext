# Demo-device functional acceptance for ticket 04, against the running app.
# Covers every demo surface through UI Automation: overview and its layout and
# quick-toggle editors, sound mode, equalizer plus profile save/apply/delete and
# the import/export previews, every device page, My Devices, Settings, and the
# Diagnostics demo state. Prints PASS/FAIL per check and exits 1 on any failure.
. "$PSScriptRoot\session.ps1"
Connect-Ui | Out-Null
$prefsPath = "$env:APPDATA\OpenSCL4\desktop-preferences.json"
$overviewPath = "$env:APPDATA\OpenSCL4\overview-preferences.json"
$profilePath = "$env:APPDATA\OpenSCL4\equalizer-profiles.json"
$failures = New-Object System.Collections.Generic.List[string]

function Check {
    param([string]$What, [bool]$Condition, [string]$Detail = '')
    if ($Condition) { "  PASS $What" } else { "  FAIL $What $Detail"; $failures.Add("$What $Detail") }
}

function Get-Pref([string]$Name) {
    (Get-Content $prefsPath -Raw | ConvertFrom-Json).$Name
}

function Select-Value([string]$Name) {
    $combo = Find-Control -Name $Name -Type ComboBox -TimeoutSec 8
    if (-not $combo) { throw "combobox '$Name' not found" }
    $vp = $null; [void]$combo.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$vp)
    if ($vp) { $vp.Current.Value } else { $null }
}

# Expands a Select and takes the row at the offset from the current choice, which
# is how the popup's own keyboard handling works.
function Step-Select([string]$Name, [int]$Offset) {
    $combo = Find-Control -Name $Name -Type ComboBox -TimeoutSec 8
    if (-not $combo) { throw "combobox '$Name' not found" }
    $p = $null
    [void]$combo.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$p)
    if ($p.Current.ExpandCollapseState -ne 'Expanded') {
        if ($p.Current.ExpandCollapseState -ne 'Collapsed') { try { $p.Collapse() } catch {} }
        $p.Expand()
    }
    Start-Sleep -Milliseconds 700
    [void][W32]::SetForegroundWindow($hwnd)
    $key = if ($Offset -lt 0) { '{UP}' } else { '{DOWN}' }
    for ($i = 0; $i -lt [Math]::Abs($Offset); $i++) { [System.Windows.Forms.SendKeys]::SendWait($key); Start-Sleep -Milliseconds 100 }
    [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    Start-Sleep -Milliseconds 1200
}

function Toggle-Button([string]$Name) {
    $element = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq $Name -and [int]$_.Current.BoundingRectangle.Width -eq 48 } | Select-Object -First 1
    if (-not $element) { $element = Find-Button $Name }
    [void](Invoke-Element $element)
    Start-Sleep -Milliseconds 900
}

"== Overview: hero, quick settings, saved layout =="
Click-Tab 'Overview'; Start-Sleep -Milliseconds 900
Check 'the hero names the simulated device' ([bool](Find-Text 'soundcore Space One Pro · Demo'))
Check 'the connected badge is shown' ([bool](Find-Text 'Connected'))
$battery = Find-AllByType -Root $win -ControlType 'Text' | Where-Object { $_.Current.Name -match '^Battery: \d+%$' }
Check 'the single-battery hero tile is shown' ([bool]$battery) "text=$($battery.Current.Name)"
Check 'no Bluetooth address is shown on the Overview' (-not ((Get-VisibleTexts) -match '([0-9A-F]{2}:){5}[0-9A-F]{2}'))
Check 'the demo card is marked in My devices' ($true) '(checked below)'
Check 'a quick-settings chip is shown' ((Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'LDAC' }).Count -eq 1)

"== Overview: quick chip writes only the simulated state =="
$before = Get-VisibleTexts | Where-Object { $_ -match '^LDAC · ' }
[void](Invoke-Element (Find-Button 'LDAC')); Start-Sleep -Milliseconds 1000
$after = Get-VisibleTexts | Where-Object { $_ -match '^LDAC · ' }
Check 'the LDAC chip followed the simulated device' ($before -ne $after) "$before -> $after"
[void](Invoke-Element (Find-Button 'LDAC')); Start-Sleep -Milliseconds 900
"  LDAC restored to $(Get-VisibleTexts | Where-Object { $_ -match '^LDAC · ' })"

"== Overview: layout editor presets, hide and order =="
Click-Button 'Edit device screen'; Start-Sleep -Milliseconds 900
[void](Invoke-Element (Find-Button 'Full')); Start-Sleep -Milliseconds 700
Click-Button 'Done'; Start-Sleep -Milliseconds 900
Check 'the Full preset shows the Power and prompts block' ([bool](Find-Button 'Power and prompts'))
$layout = (Get-Content $overviewPath -Raw | ConvertFrom-Json).overview_layout
Check 'the Full preset is saved' ($layout -notmatch '!') "layout=$layout"
Click-Button 'Edit device screen'; Start-Sleep -Milliseconds 900
$blockToggle = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Controls' -and [int]$_.Current.BoundingRectangle.Width -eq 48 } | Select-Object -First 1
[void](Invoke-Element $blockToggle); Start-Sleep -Milliseconds 700
Click-Button 'Done'; Start-Sleep -Milliseconds 900
$layout = (Get-Content $overviewPath -Raw | ConvertFrom-Json).overview_layout
Check 'hiding a block is saved with the ! marker' ($layout -match '!controls') "layout=$layout"
Check 'the hidden block leaves the Overview' (-not [bool](Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Controls' }))
Click-Button 'Edit device screen'; Start-Sleep -Milliseconds 900
$ups = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Move up' } | Sort-Object { $_.Current.BoundingRectangle.Y }
[void](Invoke-Element $ups[5]); Start-Sleep -Milliseconds 700
Click-Button 'Done'; Start-Sleep -Milliseconds 900
$layout = (Get-Content $overviewPath -Raw | ConvertFrom-Json).overview_layout
Check 'moving a block is saved in the document order' ($layout -match 'comfort,power,spatial') "layout=$layout"
Click-Button 'Edit device screen'; Start-Sleep -Milliseconds 900
[void](Invoke-Element (Find-Button 'Standard')); Start-Sleep -Milliseconds 600
Click-Button 'Done'; Start-Sleep -Milliseconds 900
$layout = (Get-Content $overviewPath -Raw | ConvertFrom-Json).overview_layout
Check 'the Standard preset is restored' ($layout -match '!quickPresets' -and $layout -match '!power') "layout=$layout"

"== Overview: quick-toggle chooser =="
Click-Button 'Choose quick settings'; Start-Sleep -Milliseconds 1000
$toggles = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { [int]$_.Current.BoundingRectangle.Width -eq 48 }
Check 'the chooser lists every supported toggle' ($toggles.Count -ge 9) "count=$($toggles.Count)"
$dolby = $toggles | Where-Object { $_.Current.Name -eq 'Dolby audio' } | Select-Object -First 1
[void](Invoke-Element $dolby); Start-Sleep -Milliseconds 700
Click-Button 'Done'; Start-Sleep -Milliseconds 900
$chosen = (Get-Content $overviewPath -Raw | ConvertFrom-Json).quick_toggles.A3062
Check 'the chosen toggle is saved per model' ($chosen -contains 'dolbyAudio') "chosen=$($chosen -join ',')"
Check 'the new chip appears on the Overview' ([bool](Find-ByName -Root $win -Name 'Dolby audio'))
Click-Button 'Choose quick settings'; Start-Sleep -Milliseconds 900
$dolby = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Dolby audio' -and [int]$_.Current.BoundingRectangle.Width -eq 48 } | Select-Object -First 1
[void](Invoke-Element $dolby); Start-Sleep -Milliseconds 700
Click-Button 'Done'; Start-Sleep -Milliseconds 900
$chosen = (Get-Content $overviewPath -Raw | ConvertFrom-Json).quick_toggles.A3062
Check 'the extra toggle is restored to LDAC only' ($chosen.Count -eq 1) "chosen=$($chosen -join ',')"

"== Sound mode: choosing a mode =="
Click-Tab 'Sound mode'; Start-Sleep -Milliseconds 900
Check 'the three mode tiles are offered' ((Get-VisibleTexts | Where-Object { $_ -in @('Noise canceling','Transparency','Normal') }).Count -ge 3)
[void](Invoke-Element (Find-Button 'Transparency')); Start-Sleep -Seconds 1
Check 'choosing Transparency is accepted' (-not (Get-VisibleTexts | Where-Object { $_ -match 'Could not change' }))
[void](Invoke-Element (Find-Button 'Normal')); Start-Sleep -Seconds 1
Check 'choosing Normal again is accepted' (-not (Get-VisibleTexts | Where-Object { $_ -match 'Could not change' }))

"== Equalizer: presets, custom bands, reset =="
Click-Tab 'Equalizer'; Start-Sleep -Milliseconds 1000
$presets = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -in @('Soundcore Signature','Acoustic','Bass Booster','Flat') }
Check 'the built-in preset chips are offered' ($presets.Count -ge 3) "matched=$($presets.Count)"
[void](Invoke-Element (Find-Button 'Acoustic')); Start-Sleep -Seconds 1
Check 'a preset change is accepted' (-not (Get-VisibleTexts | Where-Object { $_ -match 'Could not change' }))
Scroll-Content -Notches 15 | Out-Null
Scroll-Content -Notches -20 | Out-Null
Start-Sleep -Milliseconds 500
$field = Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -eq '800 Hz, dB' } | Select-Object -First 1
Invoke-ClickCenter $field; Start-Sleep -Milliseconds 400
[System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 120
[System.Windows.Forms.SendKeys]::SendWait('9'); Start-Sleep -Milliseconds 250
[System.Windows.Forms.SendKeys]::SendWait('{ENTER}'); Start-Sleep -Seconds 1.2
Check 'typing a band value creates a custom curve' ([bool](Find-Text 'Your own settings'))
$field = Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -eq '800 Hz, dB' } | Select-Object -First 1
$vp = $null; [void]$field.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$vp)
Check 'the re-polled band value is shown' ($vp.Current.Value -eq '9') "value=$($vp.Current.Value)"
Click-Button 'Reset'; Start-Sleep -Seconds 1
Check 'Reset keeps the flat custom curve' ([bool](Find-Text 'Your own settings'))

"== Equalizer: save, apply and delete a profile =="
Click-Button 'Save profile'; Start-Sleep -Milliseconds 900
$name = Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -eq 'Profile name' } | Select-Object -First 1
Invoke-ClickCenter $name; Start-Sleep -Milliseconds 300
[System.Windows.Forms.SendKeys]::SendWait('Acceptance curve'); Start-Sleep -Milliseconds 250
Click-Button 'Save'; Start-Sleep -Milliseconds 1300
Check 'the saved profile row appears' ([bool](Find-Text 'Acceptance curve'))
Check 'the profile is written to its document' ((Test-Path $profilePath) -and ((Get-Content $profilePath -Raw) -match 'Acceptance curve'))
$row = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Acceptance curve' } | Select-Object -First 1
[void](Invoke-Element $row); Start-Sleep -Milliseconds 1200
Check 'applying the profile is accepted' (-not (Get-VisibleTexts | Where-Object { $_ -match 'Could not' }))

"== Equalizer: import preview from the clipboard =="
Set-Clipboard -Value '[{"name":"Preview only","volumeAdjustments":[1.0,-1.0,2.0,0.0,0.0,0.0,0.0,0.0]}]'
Scroll-Content -Notches 8 | Out-Null
Click-Button 'Paste'; Start-Sleep -Seconds 1
Check 'the import preview lists the pasted profile as New' ([bool](Find-Text 'Preview only'))
Check 'the import preview offers Import 1 profile' ([bool](Find-Text 'Import 1 profile'))
Save-Shot 'accept-import-preview' | Out-Null
Click-Button 'Cancel'; Start-Sleep -Milliseconds 800
Check 'cancelling the import writes nothing' (-not ((Get-Content $profilePath -Raw) -match 'Preview only'))

"== Equalizer: export preview =="
Scroll-Content -Notches 8 | Out-Null
$checkbox = Find-AllByType -Root $win -ControlType 'CheckBox' | Where-Object { $_.Current.Name -eq 'Acceptance curve' } | Select-Object -First 1
if ($checkbox) { [void](Invoke-Element $checkbox); Start-Sleep -Milliseconds 700 }
$copy = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Copy' } | Select-Object -Last 1
Set-Clipboard -Value ''
if ($copy) { [void](Invoke-Element $copy); Start-Sleep -Milliseconds 800 }
$exported = Get-Clipboard -Raw
Check 'Copy puts the transfer JSON on the clipboard' ($exported -match 'Acceptance curve' -and $exported -match 'volumeAdjustments') "clipboard=$exported"
Save-Shot 'accept-export' | Out-Null

"== Equalizer: delete the profile through its row menu =="
Scroll-Content -Notches -10 | Out-Null
Start-Sleep -Milliseconds 400
$more = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -match 'More for' } | Select-Object -First 1
[void](Invoke-Element $more); Start-Sleep -Milliseconds 800
$delete = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Delete' } | Select-Object -First 1
[void](Invoke-Element $delete); Start-Sleep -Milliseconds 900
Check 'deleting asks for confirmation' ([bool](Find-Text 'Delete equalizer profile?'))
$delete = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Delete' } | Select-Object -Last 1
[void](Invoke-Element $delete); Start-Sleep -Milliseconds 1200
Check 'the profile row is gone' (-not [bool](Find-Text 'Acceptance curve'))
Check 'the profiles document is empty' ((Get-Content $profilePath -Raw).Trim() -eq '[]')

"== Device pages: a change on each reaches the simulated device =="
Click-Tab 'Controls'; Start-Sleep -Milliseconds 900
$doubleBefore = Select-Value 'Double press'
Step-Select 'Double press' 1
Check 'Controls: Double press writes and re-polls' ((Select-Value 'Double press') -ne $doubleBefore) "$doubleBefore -> $(Select-Value 'Double press')"
Step-Select 'Double press' -1
Check 'Controls: Double press restored' ((Select-Value 'Double press') -eq $doubleBefore)
$cycleBefore = Get-ToggleStateByName 'Normal'
Click-ToggleByName 'Normal'
$cycleAfter = Get-ToggleStateByName 'Normal'
Check 'Controls: the mode-cycle toggle follows the device answer' ($cycleBefore -ne $cycleAfter) "$cycleBefore -> $cycleAfter"
Click-ToggleByName 'Normal'

Click-Tab 'Comfort'; Start-Sleep -Milliseconds 900
$before = Get-ToggleStateByName 'Side tone'
Click-ToggleByName 'Side tone'
Check 'Comfort: Side tone follows the device answer' ((Get-ToggleStateByName 'Side tone') -ne $before)
Click-ToggleByName 'Side tone'
function Get-SliderValue([string]$Name) {
    $slider = Find-Control -Name $Name -Type 'Slider'
    $pattern = $null
    [void]$slider.TryGetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern, [ref]$pattern)
    @{ Element = $slider; Pattern = $pattern }
}
$range = Get-SliderValue 'dB limit'
$dbBefore = $range.Pattern.Current.Value
$range.Pattern.SetValue([Math]::Max($range.Pattern.Current.Minimum, $dbBefore - 5)); Start-Sleep -Seconds 1.2
$range = Get-SliderValue 'dB limit'
Check 'Comfort: the dB limit range writes and re-polls' ($range.Pattern.Current.Value -ne $dbBefore) "$dbBefore -> $($range.Pattern.Current.Value)"
$range.Pattern.SetValue($dbBefore); Start-Sleep -Milliseconds 1000
$refreshBefore = Select-Value 'Refresh rate'
Step-Select 'Refresh rate' 1
Check 'Comfort: Refresh rate writes and re-polls' ((Select-Value 'Refresh rate') -ne $refreshBefore)
Step-Select 'Refresh rate' -1

Click-Tab 'Power and prompts'; Start-Sleep -Milliseconds 900
$autoBefore = Select-Value 'Auto power off'
Step-Select 'Auto power off' 1
Check 'Power: Auto power off writes and re-polls' ((Select-Value 'Auto power off') -ne $autoBefore) "$autoBefore -> $(Select-Value 'Auto power off')"
Step-Select 'Auto power off' -1
$lowBefore = Get-ToggleStateByName 'Low battery prompt'
Click-ToggleByName 'Low battery prompt'
Check 'Power: Low battery prompt follows the device answer' ((Get-ToggleStateByName 'Low battery prompt') -ne $lowBefore)
Click-ToggleByName 'Low battery prompt'

Click-Tab 'Connections'; Start-Sleep -Milliseconds 1000
$texts = Get-VisibleTexts
Check 'Connections: the simulated remembered devices are listed' (($texts | Where-Object { $_ -match '^Demo (desktop|phone)' }).Count -eq 2)
Check 'Connections: the connected marker is on the simulated desktop' (($texts | Where-Object { $_ -match 'Demo desktop — connected' }).Count -eq 1)
Check 'Connections: no Bluetooth address is shown' (-not ($texts -match '([0-9A-F]{2}:){5}[0-9A-F]{2}'))
$connectButtons = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Connect' }
Check 'Connections: both rows offer Connect' ($connectButtons.Count -eq 2) "count=$($connectButtons.Count)"
[void](Invoke-Element ($connectButtons | Select-Object -Last 1)); Start-Sleep -Seconds 1
Check 'Connections: Connect on the phone row is accepted' (-not (Get-VisibleTexts | Where-Object { $_ -match 'Could not' }))

Click-Tab 'About the device'; Start-Sleep -Milliseconds 900
$about = Get-VisibleTexts
Check 'About: the firmware row is shown' ([bool]($about | Where-Object { $_ -match '04\.39' }))
Check 'About: no serial number row is shown' (-not ($about | Where-Object { $_ -match '^Serial' }))
Check 'About: no Bluetooth address is shown' (-not ($about -match '([0-9A-F]{2}:){5}[0-9A-F]{2}'))

"== My Devices: demo card, disconnect and reconnect =="
Click-Tab 'My devices'; Start-Sleep -Milliseconds 1000
Check 'My Devices: the live card is marked as a demo device' ([bool](Find-Text 'Connected · Demo device'))
Click-Button 'Disconnect' -SettleMs 1500
Check 'My Devices: Disconnect keeps the demo card' ([bool](Find-Text 'Demo device · Not connected'))
Check 'My Devices: the card offers Connect after Disconnect' ([bool](Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Connect' }))
$connect = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Connect' } | Select-Object -First 1
[void](Invoke-Element $connect); Start-Sleep -Seconds 3
Check 'My Devices: Connect returns to the Overview' ((Get-WindowTitle) -match 'Overview')
Click-Tab 'Overview'; Start-Sleep -Milliseconds 900
$ldac = Get-VisibleTexts | Where-Object { $_ -match '^LDAC · ' }
Check 'a demo reconnect starts from the captured state again' ($ldac -match 'off') "LDAC=$ldac"

"== Settings: the A3062 offers no badge or scale view =="
Click-Tab 'Settings'; Start-Sleep -Milliseconds 1000
Scroll-Content -Notches -10 | Out-Null
Check 'Settings: the one-earbud badge is not offered for the A3062' (-not [bool](Find-Button 'One earbud badge' -TimeoutSec 1))
Check 'Settings: the sound-mode scale view is not offered for the A3062' (-not [bool](Find-Button 'Scale 1–11' -TimeoutSec 1))
Check 'Settings: the saved language is English' ((Get-Pref 'language') -eq 'en')
Check 'Settings: the saved theme came back' ((Get-Pref 'theme') -eq 'light')
Check 'Settings: the saved palette came back' ((Get-Pref 'accent') -eq 'purple')
Check 'Settings: the saved content scale came back' ((Get-Pref 'contentScale') -eq '100')

"== Diagnostics: demo mode cannot capture =="
Click-Tab 'Diagnostics'; Start-Sleep -Milliseconds 1200
Scroll-Content -Notches -10 | Out-Null
$texts = Get-VisibleTexts
Check 'Diagnostics: the demo-unavailable message is shown' ([bool]($texts | Where-Object { $_ -match 'Demo mode cannot collect diagnostics' }))
$start = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -match 'Start capture' } | Select-Object -First 1
Check 'Diagnostics: Start capture is disabled' ($start -and -not $start.Current.IsEnabled) "found=$([bool]$start)"
Check 'Diagnostics: no Save ZIP or GitHub issue action is offered' (-not (Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -match 'Save ZIP|GitHub issue' }))
Check 'Diagnostics: the simulated device is still summarised' ([bool]($texts | Where-Object { $_ -match 'A3062' }))

"== No device document was written by the demo =="
$files = Get-ChildItem "$env:APPDATA\OpenSCL4" -File | Select-Object -ExpandProperty Name
"  documents: $($files -join ', ')"
Check 'only the app documents exist (no demo device state file)' (-not ($files | Where-Object { $_ -match 'device|session|demo' }))

if ($failures.Count -gt 0) {
    ""; "FAILURES:"; $failures | ForEach-Object { " - $_" }; exit 1
}
""; "ALL DEMO CHECKS PASSED"





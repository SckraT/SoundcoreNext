# Settings functional acceptance: every appearance/tray control, persistence file,
# Copy logs. Leaves the app in the light theme, English, 100 % scale.
. "$PSScriptRoot\session.ps1"
Connect-Ui | Out-Null
$prefsPath = "$env:APPDATA\OpenSCL4\desktop-preferences.json"
$failures = New-Object System.Collections.Generic.List[string]

function Assert-Pref {
    param([string]$Name, [string]$Value)
    $doc = Get-Content $prefsPath -Raw | ConvertFrom-Json
    $actual = $doc.$Name
    if ("$actual" -ne $Value) { $failures.Add("pref $Name expected '$Value' got '$actual'") }
    "  $Name = $actual"
}

function Click-Option {
    param([string]$Name, [int]$SettleMs = 900)
    $button = Find-Button -Name $Name -TimeoutSec 8
    if (-not $button) { throw "option '$Name' not found" }
    [void](Invoke-Element $button -ClickFallback)
    Start-Sleep -Milliseconds $SettleMs
}

"== Appearance =="
foreach ($case in @(@('Dark', 'theme', 'dark'), @('Light', 'theme', 'light'), @('Follow system', 'theme', 'system'))) {
    "clicking $($case[0])"
    Click-Option $case[0]
    Assert-Pref $case[1] $case[2]
}

"== Content scale: every choice applies and persists =="
foreach ($case in @(@('110%', '110'), @('125%', '125'), @('100%', '100'))) {
    "clicking $($case[0])"
    Click-Option $case[0] 1200
    Assert-Pref 'contentScale' $case[1]
    $client = Get-ClientSize $hwnd
    "  client after scale: $client"
    Save-Shot ("settings-scale-" + $case[1]) | Out-Null
}

"== Palette swatches =="
$palettes = @{'Sage' = 'sage'; 'Rose' = 'rose'; 'Purple' = 'purple' }
foreach ($name in 'Sage', 'Rose', 'Purple') {
    "clicking swatch $name"
    $button = Find-Button -Name $name -TimeoutSec 8
    if (-not $button) { throw "swatch '$name' not found" }
    [void](Invoke-Element $button -ClickFallback)
    Start-Sleep -Milliseconds 900
    Assert-Pref 'accent' $palettes[$name]
    Save-Shot ("settings-palette-" + $name.ToLower()) | Out-Null
}

"== Device art color =="
foreach ($case in @(@('Silver', 'silver'), @('Black', 'black'), @('As the theme', 'asTheme'))) {
    "clicking $($case[0])"
    Click-Option $case[0]
    Assert-Pref 'deviceColor' $case[1]
}

"== Tray switches persist in the same document =="
function Click-Check {
    param([string]$Name)
    $check = Find-Check -Name $Name -TimeoutSec 8
    if (-not $check) { throw "checkbox '$Name' not found" }
    [void](Invoke-Element $check -ClickFallback)
    Start-Sleep -Milliseconds 700
}
foreach ($name in 'Battery', 'Quick presets', 'LDAC') {
    "clicking tray item $name"
    Click-Check $name
}
$doc = Get-Content $prefsPath -Raw | ConvertFrom-Json
"  battery=$($doc.battery) presets=$($doc.presets) ldac=$($doc.ldac) modes=$($doc.modes)"
if ($doc.battery -ne $false) { $failures.Add("battery should be false, got $($doc.battery)") }
if ($doc.presets -ne $true) { $failures.Add("presets should be true, got $($doc.presets)") }
if ($doc.ldac -ne $false) { $failures.Add("ldac should be false, got $($doc.ldac)") }
# restore the switches the acceptance started from
foreach ($name in 'Battery', 'Quick presets', 'LDAC') { Click-Check $name }
$doc = Get-Content $prefsPath -Raw | ConvertFrom-Json
"  restored: battery=$($doc.battery) presets=$($doc.presets) ldac=$($doc.ldac) modes=$($doc.modes)"

"== Copy logs puts the real log on the clipboard =="
Set-Clipboard -Value ''
Click-Button 'Copy logs' -TimeoutSec 8 -SettleMs 1200
$clip = Get-Clipboard -Raw
"  clipboard length: $($clip.Length)"
"  first line: $($clip.Split([Environment]::NewLine)[0])"
if (-not $clip -or $clip.Length -lt 20) { $failures.Add('copy logs put nothing on the clipboard') }
if ($clip -notmatch 'OpenSCL4') { $failures.Add('clipboard does not look like the app log') }
Save-Shot 'settings-after-copy-logs' | Out-Null

"== One-earbud badge is not offered for the A3062 yet (no device) =="
$badge = Find-Button -Name 'One earbud badge' -TimeoutSec 2
"  badge control present: $([bool]$badge)"
if ($badge) { $failures.Add('one-earbud badge is shown with no connected device') }

"== Language round trip (Russian then back to English) =="
Set-AppLanguage 'Русский'
"  title after RU: $(Get-WindowTitle)"
Assert-Pref 'language' 'ru'
Save-Shot 'settings-russian' | Out-Null
$ruTree = Get-Tree -Root $win
$englishLeaks = @($ruTree | Where-Object { $_ -match "'(Open diagnostics|Collect read-only|Edit home screen)'" })
"  untranslated-string leaks in the RU Settings page: $($englishLeaks.Count) (known gap: Diagnostics panel strings are not in the bundled po files)"
$englishLeaks | ForEach-Object { "    $_" }
Set-AppLanguage 'English'
"  title after EN: $(Get-WindowTitle)"
Assert-Pref 'language' 'en'

"== Final state: light theme, purple, 100 %, English =="
Click-Option 'Light'
Assert-Pref 'theme' 'light'
Save-Shot 'settings-final-light' | Out-Null

if ($failures.Count -gt 0) {
    "FAILURES:"
    $failures | ForEach-Object { " - $_" }
    exit 1
}
"ALL SETTINGS CHECKS PASSED"

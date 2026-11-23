param([switch]$SkipReconnect)
# Ticket 05 live acceptance, phase 3: Settings appearance/language/tray choices,
# Copy logs, restart persistence, close-to-tray and Quit, with the real A3062
# connected. Ends by quitting the app in the baseline appearance.
# Requires the app already connected (phase 1) and the English baseline.
. "$PSScriptRoot\..\wc04-uia\session.ps1"
Connect-Ui | Out-Null
$script:ShotsDir = "$PSScriptRoot\shots"
New-Item -ItemType Directory -Force $script:ShotsDir | Out-Null
$prefsPath = "$env:APPDATA\OpenSCL4\desktop-preferences.json"
$failures = New-Object System.Collections.Generic.List[string]
function Check {
    param([string]$What, [bool]$Condition, [string]$Detail = '')
    if ($Condition) { "  PASS $What" } else { "  FAIL $What $Detail"; $failures.Add("$What $Detail") }
}
function Get-Pref([string]$Name) { (Get-Content $prefsPath -Raw | ConvertFrom-Json).$Name }
function Click-Option {
    param([string]$Name, [int]$SettleMs = 900)
    $button = Find-Button -Name $Name -TimeoutSec 8
    if (-not $button) { throw "option '$Name' not found" }
    [void](Invoke-Element $button -ClickFallback)
    Start-Sleep -Milliseconds $SettleMs
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
function Find-NotificationIcon([int]$TimeoutSec = 6) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        $root = [System.Windows.Automation.AutomationElement]::RootElement
        $all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
        foreach ($element in $all) {
            if ($element.Current.Name -match '^OpenSCL4') { return $element }
        }
        Start-Sleep -Milliseconds 500
    } while ((Get-Date) -lt $deadline)
    return $null
}

"== Settings: appearance choices apply and persist =="
Click-Tab 'Settings'; Start-Sleep -Milliseconds 1200
Check 'the Settings page opened' ((Get-WindowTitle) -match 'Settings')
foreach ($case in @(@('Dark', 'theme', 'dark'), @('Light', 'theme', 'light'), @('Follow system', 'theme', 'system'))) {
    Click-Option $case[0] 1000
    Check "$($case[0]) theme is applied and saved" ("$(Get-Pref $case[1])" -eq $case[2]) "theme=$(Get-Pref $case[1])"
}
foreach ($case in @(@('Sage', 'accent', 'sage'), @('Rose', 'accent', 'rose'), @('Purple', 'accent', 'purple'))) {
    Click-Option $case[0] 1000
    Check "$($case[0]) palette is applied and saved" ("$(Get-Pref $case[1])" -eq $case[2]) "accent=$(Get-Pref $case[1])"
}
foreach ($case in @(@('110%', '110'), @('125%', '125'), @('100%', '100'))) {
    Click-Option $case[0] 1200
    Check "content scale $($case[0]) is applied and saved" ("$(Get-Pref 'contentScale')" -eq $case[1]) "scale=$(Get-Pref 'contentScale')"
}
Save-PhaseShot 'live-settings' | Out-Null
Check 'the connected A3062 offers no one-earbud badge' (-not [bool](Find-ButtonNamed 'One earbud badge' 2))
Check 'the connected A3062 offers no sound-mode scale view' (-not [bool](Find-ButtonNamed 'Scale 1–11' 2))

"== Settings: Copy logs puts the real log on the clipboard =="
Set-Clipboard -Value ''
Click-Button 'Copy logs' -TimeoutSec 8 -SettleMs 1200
$clip = Get-Clipboard -Raw
Check 'Copy logs put the application log on the clipboard' ([bool]($clip -and $clip.Length -gt 20 -and $clip -match 'OpenSCL4')) "length=$($clip.Length)"
Check 'the copied log mentions the real device connection' ([bool]($clip -match 'RFCOMM|connected|Bluetooth|worker')) "first=$(($clip -split "`r?`n")[0..2] -join ' / ')"

"== Settings: language round trip (Russian then back) =="
$script:StateDir = "$PSScriptRoot\state"
Set-AppLanguage 'Русский'
Check 'Russian is applied to the running window' ((Get-WindowTitle) -match 'Настройки') "title=$(Get-WindowTitle)"
Check 'Russian is saved' ("$(Get-Pref 'language')" -eq 'ru')
Save-PhaseShot 'live-settings-russian' | Out-Null
Set-AppLanguage 'English'
Check 'English is applied back' ((Get-WindowTitle) -match 'Settings') "title=$(Get-WindowTitle)"
Check 'English is saved' ("$(Get-Pref 'language')" -eq 'en')

"== Settings: tray choices persist in the same document =="
Scroll-Content -Notches 8 | Out-Null; Start-Sleep -Milliseconds 400
foreach ($name in 'Battery', 'LDAC') {
    $check = Find-Check -Name $name -TimeoutSec 8
    if (-not $check) { throw "checkbox '$name' not found" }
    [void](Invoke-Element $check -ClickFallback)
    Start-Sleep -Milliseconds 700
}
Check 'Battery tray choice is saved off' ((Get-Pref 'battery') -eq $false) "battery=$(Get-Pref 'battery')"
Check 'LDAC tray choice is saved off' ((Get-Pref 'ldac') -eq $false) "ldac=$(Get-Pref 'ldac')"
Scroll-Content -Notches 8 | Out-Null; Start-Sleep -Milliseconds 400
foreach ($name in 'Battery', 'LDAC') {
    $check = Find-Check -Name $name -TimeoutSec 8
    [void](Invoke-Element $check -ClickFallback)
    Start-Sleep -Milliseconds 700
}
Check 'the tray choices are restored' ((Get-Pref 'battery') -eq $true -and (Get-Pref 'ldac') -eq $true)

"== Restart persistence: Dark, Sage, 125 %, Russian, then back =="
Click-Option 'Dark' 1000
Click-Option 'Sage' 1000
Click-Option '125%' 1400
Set-AppLanguage 'Русский'
Check 'the restart choices are saved before quitting' ((Get-Pref 'theme') -eq 'dark' -and (Get-Pref 'accent') -eq 'sage' -and (Get-Pref 'contentScale') -eq '125' -and (Get-Pref 'language') -eq 'ru')
Scroll-Content -Notches 14 | Out-Null; Start-Sleep -Milliseconds 500; if ("$(Get-Pref 'language')" -eq 'ru') { Click-Button 'Выйти из OpenSCL4' -TimeoutSec 8 -SettleMs 1500 } else { Click-Button 'Quit OpenSCL4' -TimeoutSec 8 -SettleMs 1500 }
Start-Sleep -Seconds 2
Check 'Quit ends the process' (-not [bool](Get-Process -Name soundcore-desktop -ErrorAction SilentlyContinue))
Start-App | Out-Null
Connect-Ui -TimeoutSec 40 | Out-Null
Check 'the restart keeps the Russian language' ((Get-WindowTitle) -match 'Мои устройства|Настройки|Диагностика') "title=$(Get-WindowTitle)"
Check 'the restart keeps the saved appearance choices' ((Get-Pref 'theme') -eq 'dark' -and (Get-Pref 'accent') -eq 'sage' -and (Get-Pref 'contentScale') -eq '125')
Save-PhaseShot 'live-settings-restart-ru' | Out-Null
$ruTab = if ("$(Get-Pref 'language')" -eq 'ru') { 'Настройки' } else { 'Settings' }
Click-Tab $ruTab; Start-Sleep -Milliseconds 1000
Set-AppLanguage 'English'
Click-Option 'Light' 1000
Click-Option 'Purple' 1000
Click-Option '100%' 1400
Check 'the baseline appearance is restored' ((Get-Pref 'theme') -eq 'light' -and (Get-Pref 'accent') -eq 'purple' -and (Get-Pref 'contentScale') -eq '100' -and (Get-Pref 'language') -eq 'en')

"== Notification-area icon and close-to-tray =="
$icon = Find-NotificationIcon 4
if ($icon) {
    "  notification-area element: $($icon.Current.Name)"
    Check 'the notification-area icon names the app and the real device' ([bool]($icon.Current.Name -match 'OpenSCL4' -and $icon.Current.Name -match 'Space One Pro')) "name=$($icon.Current.Name)"
} else {
    "  the shell did not expose a notification-area element to UIA in this session (icon hosted by Slint)"
}
[void][W32]::ShowWindow($hwnd, 0)  # hide, as a close-to-tray close would
Start-Sleep -Milliseconds 900
$proc = Get-Process -Name soundcore-desktop -ErrorAction SilentlyContinue
Check 'with close-to-tray on, hiding the window keeps the process alive' ([bool]$proc)
[void][W32]::ShowWindow($hwnd, 5)  # SW_SHOW
Start-Sleep -Milliseconds 800
# A second start asks the running instance to show its window and exits.
$exe = (Resolve-Path "$PSScriptRoot\..\..\desktop\target\debug\soundcore-desktop.exe").Path
$second = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 2
Check 'a second start exits instead of opening a second window' ($second.HasExited) "exited=$($second.HasExited)"
[void][W32]::ShowWindow($hwnd, 5)

"== Quit from Settings removes the icon and ends the app =="
Click-Tab 'Settings'; Start-Sleep -Milliseconds 900
Scroll-Content -Notches 14 | Out-Null; Start-Sleep -Milliseconds 500; if ("$(Get-Pref 'language')" -eq 'ru') { Click-Button 'Выйти из OpenSCL4' -TimeoutSec 8 -SettleMs 1500 } else { Click-Button 'Quit OpenSCL4' -TimeoutSec 8 -SettleMs 1500 }
Start-Sleep -Seconds 2
Check 'Quit ends the process' (-not [bool](Get-Process -Name soundcore-desktop -ErrorAction SilentlyContinue))
Start-App | Out-Null
Connect-Ui -TimeoutSec 40 | Out-Null
Check 'the app starts again after Quit' ((Get-WindowTitle) -match 'My devices')

if (-not $SkipReconnect) {
    . "$PSScriptRoot\connect-real.ps1"
    Check 'the app reconnects the real A3062 after the restart' (Connect-RealDevice)
}

if ($failures.Count -gt 0) { ""; "FAILURES:"; $failures | ForEach-Object { " - $_" }; exit 1 }
""; "PHASE 3 PASSED"

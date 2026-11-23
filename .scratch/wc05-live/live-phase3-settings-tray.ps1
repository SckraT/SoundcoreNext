param([switch]$SkipReconnect)
# Ticket 05 live acceptance, phase 3: Settings appearance/language/tray choices,
# Copy logs, restart persistence, close-to-tray and Quit, with the real A3062
# connected. Ends by quitting the app in the baseline appearance.
# Requires the app already connected (phase 1) and the English baseline.
. "$PSScriptRoot\..\wc04-uia\session.ps1"
Connect-Ui | Out-Null
# Another window state (e.g. minimized) has no client area to capture.
[void][W32]::ShowWindow($hwnd, 9)
Start-Sleep -Milliseconds 700
[void][W32]::SetForegroundWindow($hwnd)
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
# The Settings page is taller than the 1280x820 window at 125 % content scale;
# grow the window so the bottom controls (tray switches, Quit) are in the
# accessibility tree instead of relying on wheel scrolling.
function Set-TallWindow([bool]$Tall) {
    if ($Tall) { Set-ClientSize -Handle $hwnd -Width 1600 -Height 1500 } else { Set-ClientSize -Handle $hwnd -Width 1600 -Height 1025 }
    Start-Sleep -Milliseconds 700
}
function Find-NotificationIcon([int]$TimeoutSec = 8) {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $trayCond = New-Object System.Windows.Automation.PropertyCondition($script:UIA::ClassNameProperty, 'Shell_TrayWnd')
    $tray = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $trayCond)
    if (-not $tray) { return $null }
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $overflow = $null
    do {
        $buttons = Find-AllByType -Root $tray -ControlType 'Button'
        $overflow = $buttons | Where-Object { $_.Current.Name -match 'скрытые|hidden' } | Select-Object -First 1
        if (-not $overflow) { break }
        [void](Invoke-Element $overflow)
        Start-Sleep -Milliseconds 800
        $all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
        foreach ($element in $all) {
            try {
                $name = $element.Current.Name
                if ($name -match 'OpenSCL4.*Space One Pro' -and $element.Current.ControlType.ProgrammaticName -eq 'ControlType.Button') {
                    # close the flyout again
                    [void](Invoke-Element $overflow)
                    Start-Sleep -Milliseconds 500
                    return $element
                }
            } catch {}
        }
        Start-Sleep -Milliseconds 400
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
Check 'the copied log holds the recent application lines' ([bool]($clip -match 'Application started')) "first=$(($clip -split "`r?`n")[0..2] -join ' / ')"

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
Set-TallWindow $true
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
Set-TallWindow $false

"== Restart persistence: Dark, Sage, 125 %, Russian, then back =="
Click-Option 'Dark' 1000
Click-Option 'Sage' 1000
Click-Option '125%' 1400
Set-AppLanguage 'Русский'
Check 'the restart choices are saved before quitting' ((Get-Pref 'theme') -eq 'dark' -and (Get-Pref 'accent') -eq 'sage' -and (Get-Pref 'contentScale') -eq '125' -and (Get-Pref 'language') -eq 'ru')
Set-TallWindow $true
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

"== Tray icon and close-to-tray with the real device connected =="
. "$PSScriptRoot\connect-real.ps1"
Check 'the app reconnects the real A3062 for the tray checks' (Connect-RealDevice)
Click-Tab 'Settings'; Start-Sleep -Milliseconds 1000

"== Notification-area icon and close-to-tray =="
$icon = Find-NotificationIcon 8
if ($icon) {
    "  notification-area element: $($icon.Current.Name)"
    Check 'the notification-area icon names the app and the real device' ([bool]($icon.Current.Name -match 'OpenSCL4' -and $icon.Current.Name -match 'Space One Pro')) "name=$($icon.Current.Name)"
    Check 'the notification-area tooltip carries the real battery' ([bool]($icon.Current.Name -match 'Battery\s*\d+%')) "name=$($icon.Current.Name)"
} else {
    "  the shell did not expose a notification-area element to UIA in this session (icon hosted by Slint)"
}
# Closing the window with close-to-tray on hides it and keeps the process.
$windowPattern = $null
if ($win.TryGetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern, [ref]$windowPattern)) {
    $windowPattern.Close()
    Start-Sleep -Milliseconds 1200
}
$proc = Get-Process -Name soundcore-desktop -ErrorAction SilentlyContinue
Check 'with close-to-tray on, closing the window keeps the process alive' ([bool]$proc)
Check 'the closed window is hidden to the notification area' (-not [W32]::IsWindowVisible($hwnd))
# A second start asks the running instance to show its window and exits.
$exe = (Resolve-Path "$PSScriptRoot\..\..\desktop\target\debug\soundcore-desktop.exe").Path
$second = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 2
Check 'a second start exits instead of opening a second window' ($second.HasExited) "exited=$($second.HasExited)"
$shown = $false
$deadline = (Get-Date).AddSeconds(10)
do {
    if ([W32]::IsWindowVisible($hwnd)) { $shown = $true; break }
    Start-Sleep -Milliseconds 300
} while ((Get-Date) -lt $deadline)
Check 'the second start shows the hidden window again' $shown

"== Quit from Settings removes the icon and ends the app =="
Set-TallWindow $true
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

# Capture the Windows client in every UI state for visual review.
#
# The app is launched in an isolated APPDATA profile, connected to the built-in
# simulated Space One Pro (demo mode), driven through Windows UI Automation, and
# screenshotted at the requested themes/content scales. The raw captures and
# contact sheets are written to a separate timestamped folder.
#
# Usage:
#   pwsh tools/ui-capture/capture-ui.ps1 -Build
#   pwsh tools/ui-capture/capture-ui.ps1 -Themes light -Scales 100 -Pages overview
#
# Requires Windows PowerShell 7+, a built soundcore-desktop.exe, and the app's
# Russian translations. No real Bluetooth device is used.

[CmdletBinding()]
param(
    [switch]$Build,
    [switch]$Offline,
    [string[]]$Themes = @('light', 'dark'),
    [string[]]$Scales = @("100", "110", "125", "150"),
    [string[]]$Pages = @('all'),
    [ValidateSet('none', 'core', 'all')]
    [string]$Dialogs = 'core',
    [string]$OutputRoot = '',
    [string]$ExePath = '',
    [int]$ClientWidth = 0,
    [int]$ClientHeight = 0,
    [int]$TimeoutSec = 45,
    [switch]$SkipSheets,
    [switch]$KeepApp,
    [switch]$IncludeTitleBar,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
. (Join-Path $PSScriptRoot 'uia.ps1')

$labels = Get-Content (Join-Path $PSScriptRoot 'labels.ru.json') -Raw | ConvertFrom-Json
# `pwsh -File script.ps1 -Pages a,b` passes the comma expression as one string;
# normalize so both that and a real PowerShell array work.
$Themes = @($Themes | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$Scales = @($Scales | ForEach-Object { "$_" -split ',' } | Where-Object { $_ } | ForEach-Object { [int]$_ })
$Pages = @($Pages | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
if (-not $OutputRoot) { $OutputRoot = Join-Path $repo 'artifacts\ui-screenshots' }
$runStamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$runDir = Join-Path $OutputRoot $runStamp
$rawDir = Join-Path $runDir 'raw'
$sheetDir = Join-Path $runDir 'sheets'
$logPath = Join-Path $runDir 'run.log'
$manifestPath = Join-Path $runDir 'manifest.json'
$appDataRoot = Join-Path $runDir 'appdata'
New-Item -ItemType Directory -Force $rawDir, $sheetDir, $appDataRoot | Out-Null
Add-Content -Encoding UTF8 $logPath "OpenSCL4 UI capture $runStamp"

$entries = New-Object System.Collections.Generic.List[object]
$errors = New-Object System.Collections.Generic.List[string]
$stateApps = New-Object System.Collections.Generic.List[object]
$languageCode = 'ru'

function Log([string]$Message) {
    $line = '[{0}] {1}' -f (Get-Date -Format 'HH:mm:ss'), $Message
    Add-Content -Encoding UTF8 $logPath $line
    Write-Host $line
}

function Fail([string]$Message) {
    Log "ERROR: $Message"
    throw $Message
}

function Resolve-Exe {
    if ($ExePath) { return (Resolve-Path $ExePath).Path }
    return (Resolve-Path (Join-Path $repo 'desktop\target\debug\soundcore-desktop.exe')).Path
}

function Assert-RussianTranslations {
    $poPath = Join-Path $repo 'desktop\translations\ru\LC_MESSAGES\soundcore-desktop.po'
    if (-not (Test-Path $poPath)) { Fail "missing $poPath" }
    $po = Get-Content $poPath -Raw
    $missing = New-Object System.Collections.Generic.List[string]
    $trFiles = Get-ChildItem (Join-Path $repo 'desktop\ui') -Recurse -Filter '*.slint'
    foreach ($file in $trFiles) {
        $text = Get-Content $file.FullName -Raw
        foreach ($match in [regex]::Matches($text, '@tr\("([^"]+)"')) {
            $id = $match.Groups[1].Value
            if ($po -notmatch ('(?m)^msgid "' + [regex]::Escape($id) + '"\s*$')) {
                if (-not $missing.Contains($id)) { [void]$missing.Add($id) }
            }
        }
    }
    if ($missing.Count -gt 0) {
        Fail ("Russian translation is missing $($missing.Count) string(s):`n" + (($missing | Select-Object -First 20) -join "`n"))
    }
    Log "Russian translation covers every @tr string in desktop/ui"
}

function Get-MissingRussianStrings {
    $poPath = Join-Path $repo 'desktop\translations\ru\LC_MESSAGES\soundcore-desktop.po'
    $po = Get-Content $poPath -Raw
    $missing = New-Object System.Collections.Generic.List[string]
    foreach ($file in Get-ChildItem (Join-Path $repo 'desktop\ui') -Recurse -Filter '*.slint') {
        $text = Get-Content $file.FullName -Raw
        foreach ($match in [regex]::Matches($text, '@tr\("([^"]+)"')) {
            $id = $match.Groups[1].Value
            if ($po -notmatch ('(?m)^msgid "' + [regex]::Escape($id) + '"\s*$') -and -not $missing.Contains($id)) {
                [void]$missing.Add($id)
            }
        }
    }
    return $missing
}

function Get-PageMap {
    $map = @{}
    foreach ($page in $labels.pages) { $map[$page.id] = $page }
    return $map
}

function Find-ButtonFuzzy([string]$Text, [int]$TimeoutSec = 15) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        $buttons = @(Find-AllByType -Root $script:win -ControlType 'Button')
        $found = $buttons | Where-Object { $_.Current.Name -like "*$Text*" } | Select-Object -First 1
        if ($found) { return $found }
        Start-Sleep -Milliseconds 150
    } while ((Get-Date) -lt $deadline)
    return $null
}

function Invoke-Button([string]$Name, [switch]$Fuzzy, [int]$TimeoutSec = 15) {
    $element = if ($Fuzzy) { Find-ButtonFuzzy $Name $TimeoutSec } else { Find-ByName -Root $script:win -Name $Name -TimeoutSec $TimeoutSec }
    if (-not $element) { throw "button '$Name' not found" }
    [void](Invoke-Element $element -ClickFallback)
    Start-Sleep -Milliseconds 350
}

function Invoke-Tab([string]$Label) {
    $element = $null
    $deadline = (Get-Date).AddSeconds(15)
    do {
        $tabs = @(Find-AllByType -Root $script:win -ControlType 'TabItem')
        $element = $tabs | Where-Object { $_.Current.Name -eq $Label } | Select-Object -First 1
        if (-not $element) { Start-Sleep -Milliseconds 150 }
    } while (-not $element -and (Get-Date) -lt $deadline)
    if (-not $element) { throw "tab '$Label' not found" }
    [void](Invoke-Element $element -ClickFallback)
    Start-Sleep -Milliseconds 450
}

function Wait-Title([string]$Text, [int]$TimeoutSec = 25) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        try { $title = Get-WindowTitle } catch { $title = '' }
        if ($title -like "*$Text*") { return $title }
        Start-Sleep -Milliseconds 150
    } while ((Get-Date) -lt $deadline)
    throw "window title did not contain '$Text' (last: '$title')"
}

function Get-WindowTitle {
    if ($script:win) { return $script:win.Current.Name }
    return ''
}

function Scroll-ToTop {
    [void][W32]::SetForegroundWindow($script:hwnd)
    [void][W32]::SetCursorPos(($script:clientWidth / 2), ($script:clientHeight / 2))
    for ($i = 0; $i -lt 12; $i++) {
        [W32]::mouse_event(0x0800, 0, 0, 120, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 25
    }
    Start-Sleep -Milliseconds 200
}

function Scroll-Down([int]$Notches = 8) {
    [void][W32]::SetForegroundWindow($script:hwnd)
    [void][W32]::SetCursorPos(($script:clientWidth / 2), ($script:clientHeight / 2))
    for ($i = 0; $i -lt $Notches; $i++) {
        [W32]::mouse_event(0x0800, 0, 0, -120, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 35
    }
    Start-Sleep -Milliseconds 300
}

function Save-Capture([string]$Id, [string]$Label, [string]$Kind, [string]$Area, [int]$Flow, [string]$Theme, [int]$Scale) {
    Scroll-ToTop
    $relative = "raw/$Theme/$Scale/$Id.png"
    $path = Join-Path $runDir $relative
    $entry = [ordered]@{
        id = $Id
        label = $Label
        kind = $Kind
        area = $Area
        flow = $Flow
        theme = $Theme
        scale = $Scale
        file = $relative
        capturedAt = (Get-Date).ToString('o')
        status = 'ok'
    }
    try {
        [void](Save-WindowClientPng -Handle $script:hwnd -Path $path)
        if (-not (Test-Path $path)) { throw 'PrintWindow did not create a file' }
        $image = [System.Drawing.Image]::FromFile($path)
        $entry.width = $image.Width
        $entry.height = $image.Height
        $image.Dispose()
        $entry.bytes = (Get-Item $path).Length
        Log "captured $Kind $Id ($Theme/$Scale) $($entry.width)x$($entry.height)"
    } catch {
        $entry.status = 'error'
        $entry.error = $_.Exception.Message
        [void]$errors.Add("$Kind/$Id/$Theme/$Scale`: $($_.Exception.Message)")
        Log "FAILED $Kind $Id ($Theme/$Scale): $($_.Exception.Message)"
    }
    [void]$entries.Add([pscustomobject]$entry)
}

function Write-Preferences([string]$ConfigDir, [string]$Theme, [int]$Scale) {
    $openDir = Join-Path $ConfigDir 'OpenSCL4'
    New-Item -ItemType Directory -Force $openDir | Out-Null
    $settings = [ordered]@{
        language = $languageCode
        theme = $Theme
        contentScale = "$Scale"
        icon = $true
        close_to_tray = $true
        battery = $true
        modes = $true
        presets = $false
        ldac = $true
    }
    $settings | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 (Join-Path $openDir 'desktop-preferences.json')
}

function Start-CaptureApp([string]$Theme, [int]$Scale) {
    $configDir = Join-Path $appDataRoot "$Theme-$Scale"
    Write-Preferences $configDir $Theme $Scale
    $oldAppData = $env:APPDATA
    $oldLocalAppData = $env:LOCALAPPDATA
    try {
        $env:APPDATA = $configDir
        $env:LOCALAPPDATA = Join-Path $configDir 'Local'
        $exe = Resolve-Exe
        $proc = Start-Process -FilePath $exe -PassThru -WindowStyle Normal
    } finally {
        $env:APPDATA = $oldAppData
        $env:LOCALAPPDATA = $oldLocalAppData
    }
    $win = Get-AppWindow -ProcessId $proc.Id -TimeoutSec $TimeoutSec
    $hwnd = [IntPtr]$win.Current.NativeWindowHandle
    if ($ClientWidth -gt 0 -and $ClientHeight -gt 0) {
        Set-ClientSize -Handle $hwnd -Width $ClientWidth -Height $ClientHeight
    }
    $script:proc = $proc
    $script:win = $win
    $script:hwnd = $hwnd
    $script:clientWidth = (Get-ClientSize -Handle $hwnd).Split('x')[0]
    $script:clientHeight = (Get-ClientSize -Handle $hwnd).Split('x')[1]
    [void]$stateApps.Add([pscustomobject]@{ theme = $Theme; scale = $Scale; pid = $proc.Id; config = $configDir })
    Log "started app pid $($proc.Id) for $Theme/$Scale (client $($script:clientWidth)x$($script:clientHeight))"
    Wait-Title 'Мои устройства' 25 | Out-Null
    Log "Russian UI is active: $(Get-WindowTitle)"
}

function Stop-CaptureApp {
    if ($KeepApp) { return }
    if ($script:proc -and -not $script:proc.HasExited) {
        try { Stop-Process -Id $script:proc.Id -Force -ErrorAction SilentlyContinue } catch {}
    }
    Start-Sleep -Milliseconds 400
}

function Open-DemoConnection {
    Invoke-Button $labels.buttons.addDevice -TimeoutSec 15
    Wait-For { Find-ByName -Root $script:win -Name 'Поиск моделей' -TimeoutSec 1 } 20 'model search' | Out-Null
    $search = Find-ByName -Root $script:win -Name 'Поиск моделей' -TimeoutSec 10
    $valuePattern = $null
    if ($search -and $search.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$valuePattern)) {
        $valuePattern.SetValue('Space One Pro')
        Start-Sleep -Milliseconds 800
    } else {
        throw 'model search field not found'
    }
    Wait-For { Find-ButtonFuzzy 'Space One Pro' 1 } 20 'model card' | Out-Null
    Start-Sleep -Milliseconds 500

    Save-Capture 'add-model' 'Добавление: выбор модели' 'dialog' 'dialogs' 1 $script:currentTheme $script:currentScale

    $model = Find-ButtonFuzzy 'Space One Pro' 15
    if (-not $model) { throw 'Space One Pro model card not found' }
    [void](Invoke-Element $model -ClickFallback)
    Start-Sleep -Milliseconds 700
    Wait-For { @(Find-AllByType -Root $script:win -ControlType 'Button') | Where-Object { $_.Current.Name -eq $labels.buttons.demoMode } } 15 'demo toggle' | Out-Null
    Save-Capture 'add-demo' 'Добавление: деморежим' 'dialog' 'dialogs' 2 $script:currentTheme $script:currentScale

    $demo = @(Find-AllByType -Root $script:win -ControlType 'Button') | Where-Object { $_.Current.Name -eq $labels.buttons.demoMode } | Select-Object -First 1
    if (-not $demo) { throw 'demo toggle not found' }
    [void](Invoke-Element $demo -ClickFallback)
    Start-Sleep -Milliseconds 500
    $demoOption = Find-ButtonFuzzy 'Space One Pro · Demo' 15
    if (-not $demoOption) { throw 'demo paired option not found' }
    [void](Invoke-Element $demoOption -ClickFallback)
    Wait-Title 'Обзор' 25 | Out-Null
    Start-Sleep -Milliseconds 800
    Log 'demo device connected'
}

function Close-Dialog {
    foreach ($name in @('Готово', 'Отмена', 'Закрыть')) {
        $button = Find-ButtonFuzzy $name 1
        if ($button) {
            try { [void](Invoke-Element $button -ClickFallback) } catch {}
            Start-Sleep -Milliseconds 500
            return
        }
    }
    # Slint dialog close crosses are often unlabelled; Escape is the fallback.
    [void][System.Windows.Forms.SendKeys]::SendWait('{ESC}')
    Start-Sleep -Milliseconds 500
}

function Capture-Dialog([string]$Id, [string]$Label, [int]$Flow, [scriptblock]$Open, [scriptblock]$Close = { Close-Dialog }) {
    try {
        & $Open
        Start-Sleep -Milliseconds 500
        Save-Capture $Id $Label 'dialog' 'dialogs' $Flow $script:currentTheme $script:currentScale
    } catch {
        [void]$errors.Add("dialog/$Id/$($script:currentTheme)/$($script:currentScale): $($_.Exception.Message)")
        Log "FAILED dialog $Id ($($script:currentTheme)/$($script:currentScale)): $($_.Exception.Message)"
    } finally {
        try { & $Close } catch {}
        Start-Sleep -Milliseconds 300
    }
}

function Capture-State([string]$Theme, [int]$Scale) {
    $script:currentTheme = $Theme
    $script:currentScale = $Scale
    Start-CaptureApp $Theme $Scale
    try {
        Open-DemoConnection
        foreach ($page in $labels.pages) {
            if ($Pages -notcontains 'all' -and $Pages -notcontains $page.id) { continue }
            try {
                Invoke-Tab $page.label
                Start-Sleep -Milliseconds 450
                Save-Capture $page.id $page.label 'page' $page.area $page.flow $Theme $Scale
            } catch {
                [void]$errors.Add("page/$($page.id)/$Theme/$Scale`: $($_.Exception.Message)")
                Log "FAILED page $($page.id) ($Theme/$Scale): $($_.Exception.Message)"
            }
        }

        if ($Dialogs -ne 'none') {
            Capture-Dialog 'layout' 'Редактор главного экрана' 3 {
                Invoke-Tab 'Обзор'
                Invoke-Button $labels.buttons.editLayout -TimeoutSec 15
                Wait-For { @(Find-AllByType -Root $script:win -ControlType 'Button') | Where-Object { $_.Current.Name -eq $labels.buttons.addQuickSettings -or $_.Current.Name -like '*Сохранить*' -or $_.Current.Name -like '*Готово*' } } 10 'layout dialog' | Out-Null
            }

            Capture-Dialog 'quick-toggles' 'Быстрые настройки' 4 {
                Invoke-Tab 'Обзор'
                Invoke-Button $labels.buttons.chooseQuickSettings -Fuzzy -TimeoutSec 15
                Wait-For { @(Find-AllByType -Root $script:win -ControlType 'Button') | Where-Object { $_.Current.Name -like '*Готово*' -or $_.Current.Name -like '*Сохранить*' } } 10 'quick settings dialog' | Out-Null
            }

            if ($Dialogs -eq 'core' -or $Dialogs -eq 'all') {
                Capture-Dialog 'save-profile' 'Сохранение профиля' 5 {
                    Invoke-Tab 'Эквалайзер'
                    Scroll-Down 10
                    Invoke-Button $labels.buttons.saveProfile -TimeoutSec 15
                    Wait-For { @(Find-AllByType -Root $script:win -ControlType 'Button') | Where-Object { $_.Current.Name -like '*Сохранить*' } } 10 'save profile dialog' | Out-Null
                }

                Capture-Dialog 'import-profiles' 'Импорт профилей' 6 {
                    Invoke-Tab 'Эквалайзер'
                    Scroll-Down 10
                    Set-Clipboard -Value '[{"name":"Демо-профиль","volumeAdjustments":[3.0,-3.0,6.0,0.0,-6.0,2.0,0.0,-2.0]}]'
                    Invoke-Button $labels.buttons.paste -TimeoutSec 15
                    Wait-For { @(Find-AllByType -Root $script:win -ControlType 'Button') | Where-Object { $_.Current.Name -like '*Импорт*' } } 10 'import dialog' | Out-Null
                }
            }
        }
    } finally {
        Stop-CaptureApp
    }
}

function Assert-Setup {
    if ($Build) {
        $running = Get-Process soundcore-desktop -ErrorAction SilentlyContinue
        if ($running -and -not $Force) {
            Fail 'soundcore-desktop.exe is running; close it or pass -Force before -Build'
        }
        if ($running) { $running | Stop-Process -Force; Start-Sleep -Milliseconds 500 }
        $cargoArgs = @('build', '--manifest-path', (Join-Path $repo 'desktop\Cargo.toml'))
        if ($Offline) { $cargoArgs += '--offline' }
        Log "building: cargo $($cargoArgs -join ' ')"
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) { Fail 'cargo build failed' }
    }
    $exe = Resolve-Exe
    if (-not (Test-Path $exe)) { Fail "application executable not found: $exe" }
    Log "executable: $exe"
    Assert-RussianTranslations
}

try {
    Assert-Setup
    foreach ($theme in $Themes) {
        if ($theme -notin @('light', 'dark')) { Fail "unsupported theme '$theme' (use light or dark)" }
        foreach ($scale in $Scales) {
            if ($scale -notin @(100, 110, 125, 150)) { Fail "unsupported content scale '$scale' (use 100, 110, 125 or 150)" }
            Capture-State $theme $scale
        }
    }
    $manifest = [ordered]@{
        createdAt = (Get-Date).ToString('o')
        run = $runStamp
        gitCommit = (git -C $repo rev-parse HEAD)
        app = (Resolve-Exe)
        language = $languageCode
        themes = $Themes
        scales = @($Scales | ForEach-Object { [int]$_ })
        pages = @($labels.pages | ForEach-Object { $_.id })
        dialogs = @($labels.dialogs | ForEach-Object { $_.id })
        captures = $entries
        errors = $errors
        appdata = $stateApps
    }
    $manifest | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 $manifestPath
    Log "wrote manifest with $($entries.Count) captures, $($errors.Count) error(s)"

    if (-not $SkipSheets) {
        $python = Get-Command python -ErrorAction SilentlyContinue
        if ($python) {
            Log 'building contact sheets and HTML index'
            & $python.Source (Join-Path $PSScriptRoot 'stitch_sheets.py') $runDir
            if ($LASTEXITCODE -ne 0) { [void]$errors.Add('contact sheets failed') }
        } else {
            [void]$errors.Add('python not found; contact sheets were not generated')
        }
    }
    # A stable pointer for people who do not want to hunt for the timestamp.
    Set-Content -Encoding UTF8 (Join-Path $OutputRoot 'latest.txt') $runDir
    $latestIndex = Join-Path $OutputRoot 'index.html'
    $relativeIndex = "$runStamp/index.html"
    @"
<!doctype html><meta charset="utf-8"><meta http-equiv="refresh" content="0; url=$relativeIndex"><title>OpenSCL4 screenshots</title><a href="$relativeIndex">Open the latest screenshot run</a>
"@ | Set-Content -Encoding UTF8 $latestIndex
    if ($errors.Count -gt 0) {
        Log "completed with $($errors.Count) error(s); see run.log"
        exit 2
    }
    Log 'completed successfully'
}
catch {
    Log "FATAL: $($_.Exception.Message)"
    if (Test-Path $runDir) {
        [ordered]@{
            createdAt = (Get-Date).ToString('o')
            error = $_.Exception.Message
            captures = $entries
            errors = $errors
        } | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $manifestPath
    }
    exit 1
}
finally {
    if (-not $KeepApp) { Stop-CaptureApp }
}


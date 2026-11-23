# Ticket 06 acceptance: run the desktop app in an isolated APPDATA profile with
# the Russian UI and verify the four visible fixes, plus the translation test.
#
#   pwsh .scratch/ui-polish/acceptance-06.ps1 -Phase demo
#   pwsh .scratch/ui-polish/acceptance-06.ps1 -Phase live
#   pwsh .scratch/ui-polish/acceptance-06.ps1 -Phase all
#
# The demo phase connects the built-in simulated Space One Pro (A3062) and
# checks the Settings page (no Diagnostics card, no button overflow), Copy logs
# on Diagnostics (clipboard + toast) and the breadcrumb centring at 100/125/150 %.
# The live phase connects the paired soundcore Liberty 4 Pro (A3954) through the
# real Add Device flow, which is currently in the one-earbud state: it checks the
# Russian LDAC quick-toggle reason, switches the app language to English and
# back, and checks the wrapped, disabled spatial-audio rows on the Effects page
# at 1280x820 and at 800x700/150 %. It only ever sends the read-only `01 01`
# state request; no device setting is written.
#
# The app is always started with APPDATA/LOCALAPPDATA pointing at the run folder,
# so the user's %APPDATA%\OpenSCL4 is never read or written. No Bluetooth address
# or serial number is printed; the checks look at user-facing names only.
[CmdletBinding()]
param(
    [ValidateSet('all', 'demo', 'live')]
    [string]$Phase = 'all',
    [string]$ExePath = '',
    [string]$OutRoot = ''
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
. (Join-Path $repo '.scratch\wc04-uia\session.ps1')
if (-not $ExePath) { $ExePath = Join-Path $repo 'desktop\target\debug\soundcore-desktop.exe' }
$ExePath = (Resolve-Path $ExePath).Path
if (-not $OutRoot) { $OutRoot = Join-Path $repo '.scratch\ui-validation\acceptance-06' }
$OutRoot = [System.IO.Path]::GetFullPath($OutRoot)
$shotsDir = Join-Path $OutRoot 'shots'
New-Item -ItemType Directory -Force $OutRoot, $shotsDir | Out-Null
$script:logPath = Join-Path $OutRoot 'acceptance.log'
if (Test-Path $script:logPath) { Remove-Item $script:logPath -Force }
$script:failures = 0
$script:checks = 0

function Log([string]$Message) {
    $line = '[{0}] {1}' -f (Get-Date -Format 'HH:mm:ss'), $Message
    Add-Content -Encoding UTF8 $script:logPath $line
    Write-Host $line
}

function Check([string]$Name, [bool]$Ok, [string]$Detail = '') {
    $script:checks++
    $suffix = if ($Detail) { " ($Detail)" } else { '' }
    if ($Ok) {
        Log "PASS $Name$suffix"
    } else {
        $script:failures++
        Log "FAIL $Name$suffix"
    }
}

function Get-ClientOrigin {
    $pt = New-Object W32+POINT
    $pt.X = 0; $pt.Y = 0
    [void][W32]::ClientToScreen($script:hwnd, [ref]$pt)
    return $pt
}

function Save-Shot([string]$Name) {
    $path = Join-Path $shotsDir "$Name.png"
    Save-WindowClientPng -Handle $script:hwnd -Path $path | Out-Null
    Log "captured $Name.png"
}

function Find-TextExact([string]$Name, [int]$TimeoutSec = 8) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        $hit = Find-AllByType -Root $script:win -ControlType 'Text' |
            Where-Object { $_.Current.Name -eq $Name } | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 150
    } while ((Get-Date) -lt $deadline)
    return $null
}

function Find-TextContains([string]$Text, [int]$TimeoutSec = 8) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        $hit = Find-AllByType -Root $script:win -ControlType 'Text' |
            Where-Object { $_.Current.Name -like "*$Text*" } | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 150
    } while ((Get-Date) -lt $deadline)
    return $null
}

function Get-Texts { Find-AllByType -Root $script:win -ControlType 'Text' }
function Get-Buttons { Find-AllByType -Root $script:win -ControlType 'Button' }

function Find-ButtonNamed([string]$Name, [int]$TimeoutSec = 10) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        $hit = Get-Buttons | Where-Object { $_.Current.Name -eq $Name } | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 200
    } while ((Get-Date) -lt $deadline)
    return $null
}

function Click-TabNamed([string]$Name) {
    Click-Tab $Name
    Start-Sleep -Milliseconds 500
}

function Click-Scale([string]$Scale) {
    $button = Find-ButtonNamed $Scale 8
    if (-not $button) { throw "content-scale button '$Scale' not found" }
    [void](Invoke-Element $button -ClickFallback)
    Start-Sleep -Milliseconds 900
}

function Set-ClientLogical([int]$Width, [int]$Height) {
    # The check window sizes are logical pixels; the client-size helper takes
    # physical pixels, so scale by the window's DPI ratio.
    $graphics = [System.Drawing.Graphics]::FromHwnd($script:hwnd)
    $ratio = $graphics.DpiX / 96.0
    $graphics.Dispose()
    $physicalWidth = [int][Math]::Round($Width * $ratio)
    $physicalHeight = [int][Math]::Round($Height * $ratio)
    Set-ClientSize -Handle $script:hwnd -Width $physicalWidth -Height $physicalHeight
    Start-Sleep -Milliseconds 700
}

function Write-Preferences([string]$ConfigDir, [string]$Language, [int]$Scale) {
    $openDir = Join-Path $ConfigDir 'OpenSCL4'
    New-Item -ItemType Directory -Force $openDir | Out-Null
    @{
        language = $Language
        theme = 'light'
        contentScale = "$Scale"
        icon = $true
        close_to_tray = $true
        battery = $true
        modes = $true
        presets = $false
        ldac = $true
    } | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 (Join-Path $openDir 'desktop-preferences.json')
}

function Start-App([string]$Profile, [string]$Language = 'ru', [int]$Scale = 100) {
    $configDir = Join-Path $OutRoot "appdata\$Profile"
    if (Test-Path $configDir) { Remove-Item $configDir -Recurse -Force }
    Write-Preferences $configDir $Language $Scale
    Log "starting $ExePath (profile $Profile, APPDATA isolated)"
    $oldAppData = $env:APPDATA
    $oldLocalAppData = $env:LOCALAPPDATA
    try {
        $env:APPDATA = $configDir
        $env:LOCALAPPDATA = Join-Path $configDir 'Local'
        $script:proc = Start-Process -FilePath $ExePath -PassThru
    } finally {
        $env:APPDATA = $oldAppData
        $env:LOCALAPPDATA = $oldLocalAppData
    }
    $script:win = Get-AppWindow -ProcessId $script:proc.Id -TimeoutSec 45
    $script:hwnd = [IntPtr]$script:win.Current.NativeWindowHandle
    Start-Sleep -Milliseconds 1200
    if ($Language -eq 'ru') {
        Wait-For { (Get-WindowTitle) -like '*Мои устройства*' } 30 'Russian window title' | Out-Null
    }
    Log "window: $(Get-WindowTitle) (client $(Get-ClientSize -Handle $script:hwnd))"
}

function Stop-App {
    if ($script:proc -and -not $script:proc.HasExited) {
        try { Stop-Process -Id $script:proc.Id -Force -ErrorAction SilentlyContinue } catch {}
    }
    $script:proc = $null
    Start-Sleep -Milliseconds 500
}

function Connect-Demo {
    Click-Button 'Добавить устройство' -SettleMs 1200
    $search = Find-AllByType -Root $script:win -ControlType 'Edit' |
        Where-Object { $_.Current.Name -eq 'Поиск моделей' } | Select-Object -First 1
    if (-not $search) { throw 'model search field not found' }
    Invoke-ClickCenter $search
    Start-Sleep -Milliseconds 250
    [System.Windows.Forms.SendKeys]::SendWait('^a')
    [System.Windows.Forms.SendKeys]::SendWait('Space One Pro')
    Start-Sleep -Milliseconds 1200
    $card = Find-ButtonNamed 'Space One Pro'
    if (-not $card) { throw 'Space One Pro model card not found' }
    [void](Invoke-Element $card -ClickFallback)
    Start-Sleep -Milliseconds 800
    $demo = Find-ButtonNamed 'Деморежим'
    if (-not $demo) { throw 'demo-mode toggle not found' }
    [void](Invoke-Element $demo -ClickFallback)
    Start-Sleep -Milliseconds 700
    $option = Find-ButtonNamed 'soundcore Space One Pro · Demo' 12
    if (-not $option) { throw 'demo paired-device row not found' }
    [void](Invoke-Element $option -ClickFallback)
    Wait-For { Find-TextExact 'Подключено' 1 } 45 'demo connection' | Out-Null
    Start-Sleep -Milliseconds 800
    Log "demo connected: $(Get-WindowTitle)"
}

function Connect-RealA3954 {
    Click-Button 'Добавить устройство' -SettleMs 1200
    $search = Find-AllByType -Root $script:win -ControlType 'Edit' |
        Where-Object { $_.Current.Name -eq 'Поиск моделей' } | Select-Object -First 1
    if (-not $search) { throw 'model search field not found' }
    Invoke-ClickCenter $search
    Start-Sleep -Milliseconds 250
    [System.Windows.Forms.SendKeys]::SendWait('^a')
    [System.Windows.Forms.SendKeys]::SendWait('Liberty 4 Pro')
    Start-Sleep -Milliseconds 1200
    $card = Find-ButtonNamed 'Liberty 4 Pro'
    if (-not $card) { throw 'Liberty 4 Pro model card not found' }
    [void](Invoke-Element $card -ClickFallback)
    Start-Sleep -Milliseconds 1200
    $row = Find-ButtonNamed 'soundcore Liberty 4 Pro' 12
    if (-not $row) { throw 'paired Liberty 4 Pro row not found' }
    [void](Invoke-Element $row -ClickFallback)
    Wait-For { Find-TextExact 'Подключено' 1 } 70 'Liberty 4 Pro connection' | Out-Null
    Start-Sleep -Milliseconds 1200
    Log "real Liberty 4 Pro connected: $(Get-WindowTitle)"
}

# The breadcrumb is the topmost line: the device name, a chevron icon and the
# section name. The layout centres all three; measure their UIA rectangles.
function Measure-Breadcrumb([string]$DeviceName, [string]$SectionName) {
    $texts = Get-Texts
    $device = $texts | Where-Object { $_.Current.Name -eq $DeviceName } |
        Sort-Object { $_.Current.BoundingRectangle.Y } | Select-Object -First 1
    $section = $texts | Where-Object { $_.Current.Name -eq $SectionName } |
        Sort-Object { $_.Current.BoundingRectangle.Y } | Select-Object -First 1
    if (-not $device -or -not $section) {
        return [pscustomobject]@{ Ok = $false; Detail = "device=$([bool]$device) section=$([bool]$section)" }
    }
    $dr = $device.Current.BoundingRectangle
    $sr = $section.Current.BoundingRectangle
    $chevron = Find-AllByType -Root $script:win -ControlType 'Image' | Where-Object {
        $r = $_.Current.BoundingRectangle
        if ($r.Width -le 0 -or $r.Width -gt 26 -or $r.Height -le 6 -or $r.Height -gt 26) { return $false }
        $cx = $r.X + $r.Width / 2
        $cy = $r.Y + $r.Height / 2
        $deviceCenterY = $dr.Y + $dr.Height / 2
        ($cx -gt $dr.Right - 2) -and ($cx -lt $sr.Left + 2) -and ([Math]::Abs($cy - $deviceCenterY) -lt 18)
    } | Select-Object -First 1
    if (-not $chevron) {
        return [pscustomobject]@{ Ok = $false; Detail = 'chevron icon not found between the breadcrumb texts' }
    }
    $cr = $chevron.Current.BoundingRectangle
    $deviceCenter = $dr.Y + $dr.Height / 2
    $sectionCenter = $sr.Y + $sr.Height / 2
    $chevronCenter = $cr.Y + $cr.Height / 2
    $maxDiff = [Math]::Max([Math]::Abs($chevronCenter - $deviceCenter), [Math]::Abs($chevronCenter - $sectionCenter))
    [pscustomobject]@{
        Ok = ($maxDiff -le 1.5)
        Detail = ('device y-centre {0:0.0}, chevron {1:0.0}, section {2:0.0}, max delta {3:0.0}px' -f $deviceCenter, $chevronCenter, $sectionCenter, $maxDiff)
        DeviceCenter = $deviceCenter
        ChevronCenter = $chevronCenter
        SectionCenter = $sectionCenter
    }
}

# The Settings page must not repeat the Diagnostics card in its aside and no
# button may be wider than the window.
function Assert-Settings([string]$Tag) {
    $hasDiagnosticsCard = ((Get-Buttons | ForEach-Object { $_.Current.Name }) -contains 'Открыть диагностику') -or
        ((Get-Buttons | ForEach-Object { $_.Current.Name }) -contains 'Скопировать логи')
    Check "settings/$Tag has no Diagnostics aside card" (-not $hasDiagnosticsCard)
    $tabs = Find-AllByType -Root $script:win -ControlType 'TabItem'
    $origin = Get-ClientOrigin
    $sidebarRight = (($tabs | ForEach-Object { $_.Current.BoundingRectangle.Right - $origin.X }) | Measure-Object -Maximum).Maximum
    $client = Get-ClientSize -Handle $script:hwnd
    $clientWidth = [int]($client.Split('x')[0])
    $overflow = @(Get-Buttons | Where-Object {
        $r = $_.Current.BoundingRectangle
        $left = $r.X - $origin.X
        $right = $r.Right - $origin.X
        $r.Width -gt 0 -and -not $_.Current.IsOffscreen -and ($right -gt $clientWidth - 2 -or $left -lt 0)
    })
    Check "settings/$Tag no button overflows the window" ($overflow.Count -eq 0) ("width=$clientWidth, offenders=" + (($overflow | ForEach-Object { $_.Current.Name }) -join ', '))
    $asideButtons = @(Get-Buttons | Where-Object {
        $r = $_.Current.BoundingRectangle
        ($r.X - $origin.X) -gt $sidebarRight -and -not $_.Current.IsOffscreen
    })
    Log "settings/$Tag content buttons: $(($asideButtons | ForEach-Object { $_.Current.Name }) -join ' | ')"
}

# Copy logs must put the recent log on the clipboard and show the Copied toast.
function Assert-CopyLogs {
    $oldClipboard = $null
    try { $oldClipboard = Get-Clipboard -Raw -ErrorAction SilentlyContinue } catch {}
    $button = Find-ButtonNamed 'Скопировать логи' 10
    if (-not $button) {
        Check 'diagnostics Copy logs button present' $false
        return
    }
    Check 'diagnostics Copy logs button present' $true
    [void](Invoke-Element $button -ClickFallback)
    Start-Sleep -Milliseconds 900
    $copied = Get-Clipboard -Raw -ErrorAction SilentlyContinue
    $hasHeader = $copied -match '^OpenSCL4 \d+\.\d+\.\d+ \('
    $hasStart = $copied -match 'Application started'
    Check 'diagnostics Copy logs puts the log on the clipboard' ($hasHeader -and $hasStart) ("chars=$(if ($null -eq $copied) { 0 } else { $copied.Length }), header=$hasHeader, started=$hasStart")
    $toast = Find-TextExact 'Скопировано' 5
    Check 'diagnostics Copy logs shows the Copied toast' ([bool]$toast)
    Save-Shot "diagnostics-copy-logs-$($script:currentTag)"
    if ($null -ne $oldClipboard -and $oldClipboard -ne '') {
        Set-Clipboard -Value $oldClipboard
    }
}

function Assert-QuickReason([string]$LanguageTag, [string[]]$Expected) {
    $found = $false
    $matched = ''
    foreach ($text in (Get-Texts | ForEach-Object { $_.Current.Name })) {
        foreach ($expectedText in $Expected) {
            if ($text -eq $expectedText) { $found = $true; $matched = $text }
        }
    }
    Check "overview/$LanguageTag LDAC one-earbud reason text" $found $matched
    $oneEarbud = if ($LanguageTag -eq 'ru') { '1 наушник' } else { '1 earbud' }
    Check "overview/$LanguageTag one-earbud badge" ([bool](Find-TextExact $oneEarbud 3))
}

function Set-AppLanguage([string]$Option) {
    # The settings::LANGUAGES order behind the select, with the follow-system row
    # first (desktop/src/settings.rs).
    $languages = @(
        'Follow system', 'English', 'مصري', 'বাংলা', 'Deutsch', 'Español', 'Français',
        'עברית', 'Bahasa Indonesia', 'Italiano', '日本語', '한국어', 'Polski',
        'Português (Brasil)', 'Русский', 'Türkçe', 'Українська'
    )
    $target = [array]::IndexOf($languages, $Option)
    if ($target -lt 0) { throw "unknown language option '$Option'" }
    $combo = Find-AllByType -Root $script:win -ControlType 'ComboBox' | Select-Object -First 1
    if (-not $combo) { throw 'app-language combobox not found' }
    $value = $null
    if (-not $combo.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$value)) {
        throw 'app-language combobox does not expose its value'
    }
    $current = [array]::IndexOf($languages, $value.Current.Value)
    if ($current -lt 0) { throw "unknown current language '$($value.Current.Value)'" }
    Log "language combobox: '$($combo.Current.Name)' value '$($value.Current.Value)' -> '$Option'"
    if ($current -eq $target) { return }
    # Click the combobox: the Slint Select focuses its popup and opens it in one
    # user action. The popup only exposes the visible rows, so the keyboard path
    # (Up/Down + Enter) reaches a language outside the first six.
    Invoke-ClickCenter $combo
    Start-Sleep -Milliseconds 900
    [void][W32]::SetForegroundWindow($script:hwnd)
    Start-Sleep -Milliseconds 300
    $step = if ($target -lt $current) { '{UP}' } else { '{DOWN}' }
    $count = [Math]::Abs($target - $current)
    for ($i = 0; $i -lt $count; $i++) {
        [System.Windows.Forms.SendKeys]::SendWait($step)
        Start-Sleep -Milliseconds 80
    }
    [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    Start-Sleep -Milliseconds 1600
    Wait-For {
        $combo = Find-AllByType -Root $script:win -ControlType 'ComboBox' | Select-Object -First 1
        if (-not $combo) { return $false }
        $value = $null
        if (-not $combo.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$value)) { return $false }
        $value.Current.Value -eq $Option
    } 10 "language switch to $Option" | Out-Null
}

function Invoke-DemoPhase {
    Log '=== demo phase: simulated Space One Pro (A3062) ==='
    Start-App 'demo' 'ru' 100
    Connect-Demo

    # Breadcrumb centring at 100/125/150 % on a section page (Effects).
    Click-TabNamed 'Настройки'
    Assert-Settings '1280x820-100'
    Save-Shot 'settings-ru-1280x820-100'
    Click-TabNamed 'Звук и эффекты'
    foreach ($scale in @('100%', '125%', '150%')) {
        if ($scale -ne '100%') {
            Click-TabNamed 'Настройки'
            Click-Scale $scale
            Click-TabNamed 'Звук и эффекты'
        }
        $script:currentTag = "1280x820-$scale"
        $breadcrumb = Measure-Breadcrumb 'soundcore Space One Pro · Demo' 'Звук'
        Check "breadcrumb/$scale chevron centred on the section page" $breadcrumb.Ok $breadcrumb.Detail
        Save-Shot "effects-breadcrumb-$scale"
    }

    # Settings page at 150 % and at the 800x700 minimum.
    Click-TabNamed 'Настройки'
    Click-Scale '150%'
    Assert-Settings '1280x820-150'
    Save-Shot 'settings-ru-1280x820-150'
    Set-ClientLogical 800 700
    Assert-Settings '800x700-150'
    Save-Shot 'settings-ru-800x700-150'

    # Back to the default scale and window, then Diagnostics: Copy logs.
    Click-TabNamed 'Настройки'
    Click-Scale '100%'
    Set-ClientLogical 1280 820
    Click-TabNamed 'Диагностика'
    Start-Sleep -Milliseconds 500
    $script:currentTag = 'demo-1280x820'
    Assert-CopyLogs

    Stop-App
}

function Invoke-LivePhase {
    Log '=== live phase: paired soundcore Liberty 4 Pro (A3954) ==='
    Start-App 'live' 'ru' 100
    Connect-RealA3954

    Click-TabNamed 'Обзор'
    Start-Sleep -Milliseconds 800
    Assert-QuickReason 'ru' @('LDAC: Изменение доступно, когда подключены оба наушника')
    Save-Shot 'overview-one-earbud-ru'

    # Switching the app language updates the runtime reason too.
    Click-TabNamed 'Настройки'
    Set-AppLanguage 'English'
    Check 'language switch English applied' ([bool](Find-TextExact 'App language' 8))
    Click-TabNamed 'Overview'
    Start-Sleep -Milliseconds 800
    Assert-QuickReason 'en' @('LDAC: Changing is available when both earbuds are connected')
    Save-Shot 'overview-one-earbud-en'
    Click-TabNamed 'Settings'
    Set-AppLanguage 'Русский'
    Check 'language switch Russian applied' ([bool](Find-TextExact 'Язык приложения' 8))

    # Effects: long labels and disabled spatial rows at 1280x820 and 800x700/150 %.
    Click-TabNamed 'Звук и эффекты'
    Start-Sleep -Milliseconds 800
    $rowLabels = @('Режим пространственного звука для музыки', 'Режим пространственного звука', 'Пространственный звук', 'Компенсация утечки звука')
    foreach ($label in $rowLabels) {
        Check "effects/1280x820 row label '$label'" ([bool](Find-TextExact $label 5))
    }
    $reason = Find-TextExact 'Сначала включите пространственный звук' 5
    Check 'effects/1280x820 disabled spatial reason present' ([bool]$reason)
    $breadcrumb = Measure-Breadcrumb 'soundcore Liberty 4 Pro' 'Звук'
    Check 'breadcrumb/1280x820-100 chevron centred on the real-device Effects page' $breadcrumb.Ok $breadcrumb.Detail
    Save-Shot 'effects-long-rows-ru-1280x820-100'

    Click-TabNamed 'Настройки'
    Click-Scale '150%'
    Click-TabNamed 'Звук и эффекты'
    Start-Sleep -Milliseconds 800
    $breadcrumb = Measure-Breadcrumb 'soundcore Liberty 4 Pro' 'Звук'
    Check 'breadcrumb/1280x820-150 chevron centred on the real-device Effects page' $breadcrumb.Ok $breadcrumb.Detail
    Save-Shot 'effects-long-rows-ru-1280x820-150'
    Set-ClientLogical 800 700
    $breadcrumb = Measure-Breadcrumb 'soundcore Liberty 4 Pro' 'Звук'
    Check 'breadcrumb/800x700-150 chevron centred on the real-device Effects page' $breadcrumb.Ok $breadcrumb.Detail
    Save-Shot 'effects-long-rows-ru-800x700-150'

    # Copy logs on the real-device Diagnostics page.
    Click-TabNamed 'Настройки'
    Click-Scale '100%'
    Set-ClientLogical 1280 820
    Click-TabNamed 'Диагностика'
    Start-Sleep -Milliseconds 800
    $script:currentTag = 'live-1280x820'
    Assert-CopyLogs

    Stop-App
}

try {
    if ($Phase -in @('all', 'demo')) { Invoke-DemoPhase }
    if ($Phase -in @('all', 'live')) { Invoke-LivePhase }
    Log "acceptance finished: $($script:checks - $script:failures)/$($script:checks) checks passed, $($script:failures) failed"
    if ($script:failures -gt 0) { exit 1 }
    exit 0
} catch {
    Log "FATAL: $($_.Exception.Message)"
    exit 1
} finally {
    Stop-App
}

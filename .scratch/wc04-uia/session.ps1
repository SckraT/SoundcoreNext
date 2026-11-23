# Session bootstrap: loads UIA helpers and attaches to the running demo app.
# Usage: . "$PSScriptRoot\session.ps1"   then use $win / $hwnd / $procId.
. "$PSScriptRoot\uia-lib.ps1"

$script:StateDir = "$PSScriptRoot\state"
$script:PidFile = "$script:StateDir\pid.txt"
$script:ShotsDir = "$PSScriptRoot\shots"
$script:ExePath = (Resolve-Path "$PSScriptRoot\..\..\desktop\target\debug\soundcore-desktop.exe").Path
New-Item -ItemType Directory -Force $script:StateDir | Out-Null
New-Item -ItemType Directory -Force $script:ShotsDir | Out-Null

function Save-Shot {
    param([string]$Name)
    Save-WindowClientPng -Handle $script:hwnd -Path "$script:ShotsDir\$Name.png"
}

function Start-App {
    param([switch]$Fresh, [switch]$Software)
    $running = Get-Process -Name soundcore-desktop -ErrorAction SilentlyContinue
    if ($running) {
        if (-not $Fresh) { return $running[0].Id }
        $running | Stop-Process -Force
        Start-Sleep -Milliseconds 500
    }
    if ($Software) { $env:SLINT_BACKEND = 'winit-software' } else { Remove-Item Env:\SLINT_BACKEND -ErrorAction SilentlyContinue }
    $proc = Start-Process -FilePath $script:ExePath -PassThru
    Set-Content $script:PidFile $proc.Id
    $proc.Id
}

function Stop-App {
    Get-Process -Name soundcore-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 400
}

function Get-AppPid {
    if (Test-Path $script:PidFile) { return [int](Get-Content $script:PidFile) }
    $running = Get-Process -Name soundcore-desktop -ErrorAction SilentlyContinue
    if ($running) { return $running[0].Id }
    throw 'the demo app is not running'
}

function Connect-Ui {
    param([int]$TimeoutSec = 30)
    $script:procId = Get-AppPid
    $script:win = Get-AppWindow -ProcessId $script:procId -TimeoutSec $TimeoutSec
    $script:hwnd = [IntPtr]$script:win.Current.NativeWindowHandle
    $script:win
}

function Find-Control {
    param([string]$Name, [string]$Type, [int]$TimeoutSec = 10)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        $cond = New-Object System.Windows.Automation.PropertyCondition($script:UIA::NameProperty, $Name)
        $all = $script:win.FindAll($script:ScopeDesc, $cond)
        foreach ($element in $all) {
            if (-not $Type -or $element.Current.ControlType.ProgrammaticName -eq "ControlType.$Type") { return $element }
        }
        Start-Sleep -Milliseconds 150
    } while ((Get-Date) -lt $deadline)
    return $null
}

function Find-Button { param([string]$Name, [int]$TimeoutSec = 10) Find-Control -Name $Name -Type Button -TimeoutSec $TimeoutSec }
function Find-Tab { param([string]$Name, [int]$TimeoutSec = 10) Find-Control -Name $Name -Type TabItem -TimeoutSec $TimeoutSec }
function Find-Check { param([string]$Name, [int]$TimeoutSec = 10) Find-Control -Name $Name -Type CheckBox -TimeoutSec $TimeoutSec }
function Find-Text { param([string]$Name, [int]$TimeoutSec = 10) Find-Control -Name $Name -Type Text -TimeoutSec $TimeoutSec }

function Click-Tab {
    param([string]$Name)
    $tab = Find-Tab -Name $Name
    if (-not $tab) { throw "tab '$Name' not found" }
    [void](Invoke-Element $tab)
    Start-Sleep -Milliseconds 500
}

# Clicks a button by its accessible name; waits for it to appear.
function Click-Button {
    param([string]$Name, [int]$TimeoutSec = 10, [int]$SettleMs = 600)
    $button = Find-Button -Name $Name -TimeoutSec $TimeoutSec
    if (-not $button) { throw "button '$Name' not found" }
    [void](Invoke-Element $button -ClickFallback)
    Start-Sleep -Milliseconds $SettleMs
}

function Get-WindowTitle { $script:win.Current.Name }

# Scrolls the page under the pointer with the mouse wheel. Slint's ScrollArea
# prunes rows outside the visible clip from the accessibility tree, so controls
# below the fold are reached by scrolling like a user would.
function Scroll-Content {
    param([int]$Notches = 3, [double]$X = 0.5, [double]$Y = 0.75)
    [void][W32]::SetForegroundWindow($script:hwnd)
    Start-Sleep -Milliseconds 200
    $rect = $script:win.Current.BoundingRectangle
    $px = [int]($rect.X + $rect.Width * $X)
    $py = [int]($rect.Y + $rect.Height * $Y)
    [void][W32]::SetCursorPos($px, $py)
    Start-Sleep -Milliseconds 150
    [W32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    [W32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 200
    # A positive notch count scrolls the page down (a negative wheel delta).
    $delta = if ($Notches -ge 0) { -120 } else { 120 }
    for ($i = 0; $i -lt [Math]::Abs($Notches); $i++) {
        [W32]::mouse_event(0x0800, 0, 0, [uint32](([int64]$delta) -band 0xFFFFFFFFL), [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 130
    }
    Start-Sleep -Milliseconds 400
}

# A switch's checked state is not exposed to UIA by this Slint build, so it is
# read from the rendered toggle: the checked track is the accent colour (a
# saturated fill) and the unchecked track is the low-saturation container.
function Get-ToggleVisualState {
    param($Element, [string]$Name = 'toggle')
    $rect = $Element.Current.BoundingRectangle
    $x = [int]($rect.X + $rect.Width * 0.2)
    $y = [int]($rect.Y + $rect.Height / 2)
    $path = "$script:ShotsDir\_probe.png"
    Save-WindowClientPng -Handle $script:hwnd -Path $path | Out-Null
    Add-Type -AssemblyName System.Drawing
    $bmp = [System.Drawing.Bitmap]::FromFile($path)
    # Convert screen coordinates to client coordinates.
    $wr = New-Object W32+RECT; [void][W32]::GetWindowRect($script:hwnd, [ref]$wr)
    $pt = New-Object W32+POINT; $pt.X = 0; $pt.Y = 0
    [void][W32]::ClientToScreen($script:hwnd, [ref]$pt)
    $px = $x - $pt.X
    $py = $y - $pt.Y
    $pixel = $bmp.GetPixel($px, $py)
    $bmp.Dispose()
    Remove-Item $path -ErrorAction SilentlyContinue
    $max = [Math]::Max($pixel.R, [Math]::Max($pixel.G, $pixel.B))
    $min = [Math]::Min($pixel.R, [Math]::Min($pixel.G, $pixel.B))
    [pscustomobject]@{ Name = $Name; Checked = (($max - $min) -gt 40); R = $pixel.R; G = $pixel.G; B = $pixel.B }
}

function Get-VisibleTexts {
    $all = Find-AllByType -Root $script:win -ControlType 'Text'
    $all | ForEach-Object { $_.Current.Name } | Where-Object { $_ }
}

# Chooses one row of a `Select` combobox. The popup only exposes its visible rows
# to UIA, so a row outside the first six is reached with the popup's own keyboard
# handling from the current index (Up/Down + Enter).
function Choose-SelectOption {
    param([string]$ComboName, [string]$Option, [int]$TargetIndex, [int]$CurrentIndex, [int]$SettleMs = 1500)
    $combo = Find-Control -Name $ComboName -Type ComboBox -TimeoutSec 8
    if (-not $combo) { throw "combobox '$ComboName' not found" }
    $p = $null
    [void]$combo.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$p)
    if ($p.Current.ExpandCollapseState -ne 'Expanded') {
        if ($p.Current.ExpandCollapseState -ne 'Collapsed') { try { $p.Collapse() } catch {} ; Start-Sleep -Milliseconds 300 }
        $p.Expand()
    }
    Start-Sleep -Milliseconds 800
    $visible = $null
    $buttons = Find-AllByType -Root $combo -ControlType 'Button'
    foreach ($button in $buttons) { if ($button.Current.Name -eq $Option) { $visible = $button; break } }
    if ($visible) {
        [void](Invoke-Element $visible)
    } else {
        [void][W32]::SetForegroundWindow($script:hwnd)
        Start-Sleep -Milliseconds 300
        $step = if ($TargetIndex -lt $CurrentIndex) { '{UP}' } else { '{DOWN}' }
        $count = [Math]::Abs($TargetIndex - $CurrentIndex)
        for ($i = 0; $i -lt $count; $i++) {
            [System.Windows.Forms.SendKeys]::SendWait($step)
            Start-Sleep -Milliseconds 90
        }
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    }
    Start-Sleep -Milliseconds $SettleMs
}

# Select indexes in settings::LANGUAGES order plus the Follow-system row at 0.
function Set-AppLanguage {
    param([string]$Option, [int]$SettleMs = 1800)
    $indexes = @{ 'Follow system' = 0; 'English' = 1; 'مصري' = 2; 'বাংলা' = 3; 'Deutsch' = 4; 'Español' = 5; 'Français' = 6; 'עברית' = 7; 'Bahasa Indonesia' = 8; 'Italiano' = 9; '日本語' = 10; '한국어' = 11; 'Polski' = 12; 'Português (Brasil)' = 13; 'Русский' = 14; 'Türkçe' = 15; 'Українська' = 16 }
    $target = $indexes[$Option]
    if ($null -eq $target) { throw "unknown language option '$Option'" }
    $prefs = Get-Content "$env:APPDATA\OpenSCL4\desktop-preferences.json" -Raw | ConvertFrom-Json
    $current = if ($prefs.language) { $indexesByCode = @{ en = 1; arz = 2; bn = 3; de = 4; es = 5; fr = 6; he = 7; id = 8; it = 9; ja = 10; ko = 11; pl = 12; 'pt-BR' = 13; ru = 14; tr = 15; uk = 16 }; $indexesByCode[$prefs.language] } else { 0 }
    $comboLabel = if ($prefs.language -eq 'ru') { 'Язык приложения' } else { 'App language' }
    Choose-SelectOption -ComboName $comboLabel -Option $Option -TargetIndex $target -CurrentIndex $current -SettleMs $SettleMs
}





# The checked state of a switch is read from its rendered state, so the element
# is always looked up again: a device re-poll rebuilds the page and detaches the
# old automation element.
function Get-ToggleStateByName {
    param([string]$Name)
    $element = Find-AllByType -Root $script:win -ControlType 'Button' | Where-Object { $_.Current.Name -eq $Name -and [int]$_.Current.BoundingRectangle.Width -eq 48 } | Select-Object -First 1
    if (-not $element) { $element = Find-Button $Name }
    if (-not $element) { throw "switch '$Name' not found" }
    (Get-ToggleVisualState $element $Name).Checked
}

function Click-ToggleByName {
    param([string]$Name, [int]$SettleMs = 1000)
    $element = Find-AllByType -Root $script:win -ControlType 'Button' | Where-Object { $_.Current.Name -eq $Name -and [int]$_.Current.BoundingRectangle.Width -eq 48 } | Select-Object -First 1
    if (-not $element) { $element = Find-Button $Name }
    if (-not $element) { throw "switch '$Name' not found" }
    [void](Invoke-Element $element)
    Start-Sleep -Milliseconds $SettleMs
}

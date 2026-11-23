# Ticket 03 live UI acceptance for the Liberty 4 Pro (A3954) buttons/case slice.
#
# Starts the built desktop app with an isolated %APPDATA% (English UI, no user
# documents touched), connects the paired soundcore Liberty 4 Pro through the
# real Add Device dialog, then verifies the Controls page (8 press rows + 4 slide
# rows + ambient cycle) and the Case page (four case-screen features, the case
# language, and the read-only case status) populate from the real device, with
# screenshots. It never clicks a setting control: only tabs/pages are changed and
# the page is scrolled with the wheel alone (no mouse press), so no device value
# is written through the UI.
#
# The UIA checks print no Bluetooth address or serial and fail if one appears on
# a page.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\..\wc04-uia\session.ps1"

$shotsDir = Join-Path ([IO.Path]::GetTempPath()) 'openscl4-a3954-buttons-ui-shots'
$runRoot = Join-Path ([IO.Path]::GetTempPath()) 'openscl4-a3954-buttons-ui-appdata'
$repoShotsDir = (Resolve-Path "$PSScriptRoot\..\..\desktop").Path
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

function Save-Shot([string]$Name) {
    Save-WindowClientPng -Handle $hwnd -Path (Join-Path $shotsDir "$Name.png") | Out-Null
}

function Publish-Shot([string]$Name) {
    Copy-Item (Join-Path $shotsDir "$Name.png") $repoShotsDir -Force
}

function Wait-Title {
    param([string]$Match, [int]$TimeoutSec = 15)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        if ((Get-WindowTitle) -match $Match) { return $true }
        Start-Sleep -Milliseconds 200
    } while ((Get-Date) -lt $deadline)
    return $false
}

function Texts {
    return @(Get-VisibleTexts | Where-Object { $_ })
}

function Select-Names {
    return @(Find-AllByType -Root $win -ControlType 'ComboBox' | ForEach-Object { $_.Current.Name } | Where-Object { $_ })
}

# Scrolls the page with the mouse wheel only (no button press ever), with the
# pointer in the empty right margin beside the panels, so no control is clicked.
function Wheel-Scroll {
    param([int]$Notches = 3)
    [void][W32]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 150
    $rect = $win.Current.BoundingRectangle
    $px = [int]($rect.X + $rect.Width * 0.88)
    $py = [int]($rect.Y + $rect.Height * 0.55)
    [void][W32]::SetCursorPos($px, $py)
    Start-Sleep -Milliseconds 120
    $delta = if ($Notches -ge 0) { -120 } else { 120 }
    $count = [Math]::Abs($Notches)
    for ($i = 0; $i -lt $count; $i++) {
        [W32]::mouse_event(0x0800, 0, 0, [uint32](([int64]$delta) -band 0xFFFFFFFFL), [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 130
    }
    Start-Sleep -Milliseconds 350
}

# Collects the page's visible texts and select names at the top and after each
# scroll step. The page is left scrolled to the bottom; the caller publishes the
# top screenshot before calling this.
function Collect-Page {
    param([int]$Steps = 4)
    $texts = [System.Collections.Generic.List[string]]::new()
    $selects = [System.Collections.Generic.List[string]]::new()
    $collect = {
        foreach ($text in (Texts)) { if (-not $texts.Contains($text)) { [void]$texts.Add($text) } }
        foreach ($name in (Select-Names)) { if (-not $selects.Contains($name)) { [void]$selects.Add($name) } }
    }
    & $collect
    for ($i = 0; $i -lt $Steps; $i++) {
        Wheel-Scroll -Notches 2
        & $collect
    }
    return [pscustomobject]@{ Texts = $texts.ToArray(); Selects = $selects.ToArray() }
}

function Has-Text($texts, [string]$Text) {
    return [bool]($texts -contains $Text)
}

function Assert-NoAddress([string]$Page, $texts) {
    $address = $texts | Where-Object { $_ -match '([0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}' } | Select-Object -First 1
    # The About page's static blurb mentions "serial numbers"; only a serial-row
    # label or a serial-shaped value would be a leak.
    $serial = $texts | Where-Object { $_ -match '^Serial( number)?\b' -or $_ -match '^SN[: ]' } | Select-Object -First 1
    Check "$Page shows no Bluetooth address" (-not $address) "$address"
    Check "$Page shows no serial" (-not $serial) "$serial"
}

function Reset-Scroll([string]$Tab) {
    Click-Tab 'Overview'
    Start-Sleep -Milliseconds 300
    Click-Tab $Tab
    Start-Sleep -Milliseconds 600
}

$previousAppData = $env:APPDATA
Get-Process -Name soundcore-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 600
Remove-Item -Recurse -Force $runRoot, $shotsDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Join-Path $runRoot 'OpenSCL4') | Out-Null
New-Item -ItemType Directory -Force $shotsDir | Out-Null
# English page words, so the UIA names below are stable; the user's own
# %APPDATA%\OpenSCL4 is never opened.
Set-Content -Path (Join-Path $runRoot 'OpenSCL4\desktop-preferences.json') -Value '{"language":"en"}' -Encoding utf8NoBOM
$env:APPDATA = $runRoot

try {
    Start-App -Fresh | Out-Null
    Connect-Ui -TimeoutSec 40 | Out-Null
    [void][W32]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 500

    "== Add Device: connect the paired A3954 =="
    if ((Get-WindowTitle) -notmatch 'My devices') { Click-Tab 'My devices'; Start-Sleep -Milliseconds 800 }
    Click-Button 'Add device' -SettleMs 1200
    Check 'the model picker opens' ([bool](Find-Text 'Find your model.' -TimeoutSec 8))
    $search = Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -eq 'Search models' } | Select-Object -First 1
    Check 'the model search field is offered' ([bool]$search)
    Invoke-ClickCenter $search
    Start-Sleep -Milliseconds 300
    [System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 120
    [System.Windows.Forms.SendKeys]::SendWait('A3954'); Start-Sleep -Milliseconds 1500
    $card = Find-ButtonNamed 'Liberty 4 Pro' 10
    Check 'the A3954 model card is offered' ([bool]$card)
    if ($card) { [void](Invoke-Element $card); Start-Sleep -Milliseconds 1200 }
    $row = Find-ButtonNamed 'soundcore Liberty 4 Pro' 10
    Check 'the paired Liberty 4 Pro is listed' ([bool]$row)
    if ($row) { [void](Invoke-Element $row) }
    # The A3954 opens on its Sound mode page (the scale is its first page), not
    # on Overview; wait until the title names the connected device.
    $connected = $false
    $deadline = (Get-Date).AddSeconds(60)
    do {
        try {
            $titleNow = Get-WindowTitle
            if ($titleNow -match 'Liberty 4 Pro' -and $titleNow -notmatch 'My devices') { $connected = $true; break }
        } catch {}
        Start-Sleep -Milliseconds 400
    } while ((Get-Date) -lt $deadline)
    $title = Get-WindowTitle
    Check 'the real RFCOMM connection shows the device' $connected $title
    Check 'the connected window title names the Liberty 4 Pro' ($title -match 'Liberty 4 Pro') $title
    Check 'the title carries no Bluetooth address' (-not ($title -match '([0-9A-Fa-f]{2}[:-]){5}')) $title

    "== Controls: the twelve gesture rows and the cycle =="
    Click-Tab 'Controls'
    Check 'Controls: the page title is shown' (Wait-Title -Match 'Controls' -TimeoutSec 10)
    Start-Sleep -Milliseconds 800
    # The top screenshot shows the Presses panel; a second one after the scroll
    # shows the Slides and ambient-cycle panels.
    Save-Shot 'live-a3954-controls'
    Publish-Shot 'live-a3954-controls'
    $controls = Collect-Page -Steps 6
    $controlsTexts = $controls.Texts
    Save-Shot 'live-a3954-controls-slides'
    Publish-Shot 'live-a3954-controls-slides'
    Reset-Scroll 'Controls'
    $controlsMust = @(
        'Presses', 'Slides', 'Switching modes',
        'Left single press', 'Right single press',
        'Left double press', 'Right double press',
        'Left triple press', 'Right triple press',
        'Left long press', 'Right long press',
        'Left slide up', 'Right slide up',
        'Left slide down', 'Right slide down',
        'Noise canceling', 'Transparency', 'Normal'
    )
    $missing = @($controlsMust | Where-Object { -not (Has-Text $controlsTexts $_) })
    Check 'Controls: the Presses/Slides panels and all 12 gesture labels populate' ($missing.Count -eq 0) "missing=$($missing -join ', ')"
    $gestureLabels = @($controls.Selects | Where-Object {
        $_ -match '^(Left|Right) (single|double|triple|long) press$|^(Left|Right) slide (up|down)$'
    })
    Check 'Controls: eight press rows and four slide rows are select controls' ($gestureLabels.Count -eq 12) "selects=$($gestureLabels.Count): $($gestureLabels -join ', ')"
    # The device's own actions as the acceptance run read them (the connection
    # reported no live TWS link, so the disconnected-state actions are shown).
    $controlsValues = @('Play/pause', 'Next song', 'None', 'Ambient sound mode', 'Volume up', 'Volume down')
    $missingValues = @($controlsValues | Where-Object { -not (Has-Text $controlsTexts $_) })
    Check 'Controls: the device-reported gesture actions are shown' ($missingValues.Count -eq 0) "missing=$($missingValues -join ', ')"
    Assert-NoAddress 'Controls' $controlsTexts
    "  shots: live-a3954-controls.png, live-a3954-controls-slides.png ($($controlsTexts.Count) visible labels)"

    "== Case: the four features, the language and the status =="
    Click-Tab 'Case'
    Check 'Case: the page title is shown' (Wait-Title -Match 'Case' -TimeoutSec 10)
    Start-Sleep -Milliseconds 800
    Save-Shot 'live-a3954-case'
    Publish-Shot 'live-a3954-case'
    $case = Collect-Page -Steps 1
    $caseTexts = $case.Texts
    $caseMust = @(
        'Case screen', 'Status',
        'Atmospheric', 'Remote camera', 'Find device', 'Spatial audio on case',
        'Case language',
        'Case battery', 'Case firmware', '01.58'
    )
    $missing = @($caseMust | Where-Object { -not (Has-Text $caseTexts $_) })
    Check 'Case: the feature toggles, language row and status rows populate' ($missing.Count -eq 0) "missing=$($missing -join ', ')"
    $languages = @('Japanese', 'English', 'Chinese')
    $shownLanguage = @($languages | Where-Object { Has-Text $caseTexts $_ })
    Check 'Case: the device-reported case language is shown' ($shownLanguage.Count -ge 1) "shown=$($shownLanguage -join ', ')"
    $caseBattery = @($caseTexts | Where-Object { $_ -match '^\d+%$' })
    Check 'Case: the case battery value is shown' ($caseBattery.Count -ge 1) "battery=$($caseBattery -join ', ')"
    Assert-NoAddress 'Case' $caseTexts
    "  shot: live-a3954-case.png ($($caseTexts.Count) visible labels)"

    if ($failures.Count -gt 0) {
        ""
        "FAILURES:"
        $failures | ForEach-Object { " - $_" }
        exit 1
    }
    ""
    "A3954 BUTTONS/CASE UI ACCEPTANCE PASSED"
}
finally {
    Stop-App
    $env:APPDATA = $previousAppData
    "screenshots: $shotsDir"
}

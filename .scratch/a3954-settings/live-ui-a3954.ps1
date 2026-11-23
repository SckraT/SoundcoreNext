# Ticket 04 live UI acceptance for the Liberty 4 Pro (A3954).
#
# Starts the built desktop app with an isolated %APPDATA% (English UI, no user
# documents touched), connects the paired soundcore Liberty 4 Pro through the
# real Add Device dialog, then visits Overview, Sound mode, Equalizer, Sound and
# effects, Controls, Comfort, Power and prompts, Connections and About the
# device, capturing each page and checking the A3954 rows populated.
#
# It never clicks a setting control: only tabs/pages change, so no device value
# is written through the UI. Screenshots and the UIA check do not print a
# Bluetooth address or a serial, and the run fails if one appears on a page.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\..\wc04-uia\session.ps1"

$shotsDir = Join-Path ([IO.Path]::GetTempPath()) 'openscl4-a3954-ui-shots'
$runRoot = Join-Path ([IO.Path]::GetTempPath()) 'openscl4-a3954-ui-appdata'
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

function Has-Text([string]$Text) {
    return [bool]((Texts) -contains $Text)
}

function Assert-NoAddress([string]$Page) {
    $texts = Texts
    $address = $texts | Where-Object { $_ -match '([0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}' } | Select-Object -First 1
    # The About page's static blurb mentions "serial numbers"; only a serial-row
    # label or a serial-shaped value would be a leak.
    $serial = $texts | Where-Object { $_ -match '^Serial( number)?\b' -or $_ -match '^SN[: ]' } | Select-Object -First 1
    Check "$Page shows no Bluetooth address" (-not $address) "$address"
    Check "$Page shows no serial" (-not $serial) "$serial"
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

    "== Pages =="
    $pages = @(
        @{ Tab = 'Overview'; Shot = 'live-a3954-overview'; Must = @('Sound mode', 'Equalizer'); Wait = 8 },
        @{ Tab = 'Sound mode'; Shot = 'live-a3954-sound-mode'; Must = @('Mode and strength', 'Wind noise suppression', 'Transparency', 'Normal', 'Noise canceling'); Wait = 8 },
        @{ Tab = 'Equalizer'; Shot = 'live-a3954-equalizer'; Must = @('Acoustic', 'Rock', 'Equalizer bands: 8'); Wait = 15 },
        @{ Tab = 'Sound and effects'; Shot = 'live-a3954-effects'; Must = @('LDAC', 'Spatial audio', 'Spatial audio music mode', 'Spatial audio mode', 'Sound leak compensation', 'Turn on spatial audio first'); Wait = 8 },
        @{ Tab = 'Controls'; Shot = 'live-a3954-controls'; Must = @('Button remapping', 'Not available in this version', 'Noise canceling', 'Transparency', 'Normal'); Wait = 8 },
        @{ Tab = 'Comfort'; Shot = 'live-a3954-comfort'; Must = @('Wearing detection', 'Easy chat', 'Easy chat wait time', 'Sound leak compensation', 'Limit high volume', 'dB limit', 'Refresh rate'); Wait = 8 },
        @{ Tab = 'Power and prompts'; Shot = 'live-a3954-power'; Must = @('Auto power off', 'Low battery prompt'); Wait = 8 },
        @{ Tab = 'Connections'; Shot = 'live-a3954-connections'; Must = @('Dual connections'); Wait = 25 },
        @{ Tab = 'About the device'; Shot = 'live-a3954-about'; Must = @('Left battery', 'Right battery', 'Case battery', 'Left firmware', 'Right firmware', 'Case firmware', '04.29', '01.58'); Wait = 8 }
    )
    foreach ($page in $pages) {
        try { Click-Tab $page.Tab } catch { "  note: tab click for $($page.Tab): $_" }
        $shown = Wait-Title -Match ([regex]::Escape($page.Tab)) -TimeoutSec $page.Wait
        Start-Sleep -Milliseconds 700
        $texts = Texts
        $missing = @($page.Must | Where-Object { $texts -notcontains $_ })
        Check "$($page.Tab): page title and rows populate" ($shown -and $missing.Count -eq 0) "missing=$($missing -join ', '); visible=$($texts.Count)"
        if ($page.Tab -eq 'Connections') {
            # Entering the page queues a read-only `0B 01` list read; the
            # remembered devices appear in a panel headed "Connected devices"
            # (the settings row itself is labelled "Paired devices").
            $dualShown = $false
            $deadline = (Get-Date).AddSeconds(25)
            do {
                if ((Texts) -contains 'Connected devices') { $dualShown = $true; break }
                Start-Sleep -Milliseconds 300
            } while ((Get-Date) -lt $deadline)
            Check 'Connections: the remembered-device list is read and shown' $dualShown
            $connectedRows = @(Texts | Where-Object { $_ -match ' — connected$' })
            Check 'Connections: the list carries the device-reported connected marker' ($connectedRows.Count -ge 1) "connected rows: $($connectedRows.Count)"
        }
        Save-Shot $page.Shot
        Assert-NoAddress $page.Tab
        "  shot: $($page.Shot).png ($($texts.Count) visible labels)"
    }

    # The 1..=11 scale control itself: one slider named for the scale, holding
    # the position the device reported on connect (6, normal).
    $slider = $null
    Click-Tab 'Sound mode'
    [void](Wait-Title -Match 'Sound mode' -TimeoutSec 8)
    Start-Sleep -Milliseconds 800
    $slider = Find-AllByType -Root $win -ControlType 'Slider' | Where-Object { $_.Current.Name -eq 'Sound mode from 1 to 11' } | Select-Object -First 1
    $sliderValue = $null
    if ($slider) {
        $pattern = $null
        if ($slider.TryGetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern, [ref]$pattern)) {
            $sliderValue = $pattern.Current.Value
        }
    }
    Check 'Sound mode: the 1..=11 scale slider is present' ([bool]$slider)
    Check 'Sound mode: the scale shows the connected position 6' ($sliderValue -eq 6) "value=$sliderValue"

    if ($failures.Count -gt 0) {
        ""
        "FAILURES:"
        $failures | ForEach-Object { " - $_" }
        exit 1
    }
    ""
    "A3954 UI ACCEPTANCE PASSED"
}
finally {
    Stop-App
    $env:APPDATA = $previousAppData
    "screenshots: $shotsDir"
}

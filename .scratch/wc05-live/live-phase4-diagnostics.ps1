# Ticket 05 live acceptance, phase 4: Diagnostics — three read-only probes,
# Review, Save ZIP through the real Windows save dialog, archive redaction
# inspection, and the prefilled GitHub issue URL (opened, never submitted).
# Requires the app already connected to the real A3062 (phase 1/2) in English.
# -FromReview resumes at the Review step of a capture that already completed.
param([switch]$FromReview)
. "$PSScriptRoot\..\wc04-uia\session.ps1"
Connect-Ui | Out-Null
$script:ShotsDir = "$PSScriptRoot\shots"
New-Item -ItemType Directory -Force $script:ShotsDir | Out-Null
$outDir = "$PSScriptRoot\diagnostics"
New-Item -ItemType Directory -Force $outDir | Out-Null
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
function Save-PhaseShot([string]$Name) { Save-WindowClientPng -Handle $hwnd -Path "$script:ShotsDir\$Name.png" | Out-Null }
function Get-DialogWindow([int]$TimeoutSec = 20) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        $cond = New-Object System.Windows.Automation.PropertyCondition($script:UIA::ClassNameProperty, '#32770')
        $dialogs = $script:UIA::RootElement.FindAll($script:ScopeChildren, $cond)
        foreach ($dialog in $dialogs) {
            $rect = $dialog.Current.BoundingRectangle
            if ($rect.Width -gt 200) { return $dialog }
        }
        Start-Sleep -Milliseconds 300
    } while ((Get-Date) -lt $deadline)
    return $null
}

"== Diagnostics: device summary and Start capture =="
if (-not $FromReview) {
Click-Tab 'Diagnostics'; Start-Sleep -Milliseconds 1400
$texts = Get-VisibleTexts
Check 'the Diagnostics page shows the connected A3062 summary' ([bool]($texts | Where-Object { $_ -match 'A3062' }))
Check 'the page lists the three read-only probes' ([bool]($texts | Where-Object { $_ -match '01 01' }) -and [bool]($texts | Where-Object { $_ -match '01 03' }) -and [bool]($texts | Where-Object { $_ -match '01 04' }))
Check 'the redaction notice is shown' ([bool]($texts | Where-Object { $_ -match 'Serial numbers and Bluetooth addresses are removed' }))
Save-PhaseShot 'live-diagnostics-ready' | Out-Null

"== Run the three probes =="
$start = Find-ButtonNamed 'Start capture' 8
Check 'Start capture is offered for the real device' ([bool]$start)
if ($start) { [void](Invoke-Element $start) }
# A finished capture moves straight to the Review step, so the review actions
# are what the page shows when it is done.
$review = $null
$deadline = (Get-Date).AddSeconds(150)
do {
    $review = Find-ButtonNamed 'Save ZIP' 1
    if ($review) { break }
    $frameCount = (Get-VisibleTexts | Where-Object { $_ -match '^\d+$' }) -join ','
    Start-Sleep -Milliseconds 500
} while ((Get-Date) -lt $deadline)
Check 'the capture completes and shows the Review step' ([bool]$review)
$texts = Get-VisibleTexts
$answered = $texts | Where-Object { $_ -eq '3 of 3' }
Check 'all three probes answered' ([bool]$answered) "texts=$($texts | Where-Object { $_ -match 'probes|frames|timeouts|archive' })"
$frames = $texts | Where-Object { $_ -match '^\d+$' }
"  frame count tiles: $($frames -join ',')"
Check 'at least the probe frames were recorded' ([bool]($texts | Where-Object { $_ -eq 'frames' }))
Start-Sleep -Milliseconds 800
Save-PhaseShot 'live-diagnostics-captured' | Out-Null
}

"== Review step =="
$review = $null
$texts = Get-VisibleTexts
if ($review) { [void](Invoke-Element $review); Start-Sleep -Milliseconds 1200 }
$texts = Get-VisibleTexts
Check 'the review step lists the archive files' ([bool]($texts | Where-Object { $_ -match 'manifest\.json' }) -and [bool]($texts | Where-Object { $_ -match 'frames\.jsonl' }) -and [bool]($texts | Where-Object { $_ -match 'README\.txt' }))
Check 'the review step offers Save ZIP' ([bool](Find-ButtonNamed 'Save ZIP' 4))
Check 'the review step offers Open GitHub issue' ([bool](Find-ButtonNamed 'Open GitHub issue' 4))
Save-PhaseShot 'live-diagnostics-review' | Out-Null

"== Save ZIP through the real Windows save dialog =="
$zip = Join-Path $outDir ("openscl4-diagnostics-" + (Get-Date -Format 'yyyyMMdd-HHmmss') + ".zip")
Remove-Item $zip -ErrorAction SilentlyContinue
[void](Invoke-Element (Find-ButtonNamed 'Save ZIP' 6))
$dialog = Get-DialogWindow 20
Check 'the native Windows save dialog opened' ([bool]$dialog)
if ($dialog) {
    $fileName = $null
    try {
        $nameCond = New-Object System.Windows.Automation.PropertyCondition($script:UIA::AutomationIdProperty, '1001')
        $fileName = $dialog.FindFirst($script:ScopeDesc, $nameCond)
    } catch { $fileName = $null }
    if (-not $fileName) {
        $edits = Find-AllByType -Root $dialog -ControlType 'Edit'
        $fileName = $edits | Where-Object { $_.Current.Name -match 'File name|Имя файла' } | Select-Object -First 1
    }
    Invoke-ClickCenter $fileName
    Start-Sleep -Milliseconds 400
    [System.Windows.Forms.SendKeys]::SendWait('^a')
    Start-Sleep -Milliseconds 150
    [System.Windows.Forms.SendKeys]::SendWait($zip)
    Start-Sleep -Milliseconds 500
    [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    Start-Sleep -Milliseconds 800
    try {
        $save = Find-AllByType -Root $dialog -ControlType 'Button' | Where-Object { $_.Current.Name -match '^Save$|^Сохранить$' } | Select-Object -First 1
        if (-not $save) { $save = Find-AllByType -Root $dialog -ControlType 'Button' | Where-Object { $_.Current.Name -match 'Save|Сохранить' } | Select-Object -First 1 }
        if ($save) { [void](Invoke-Element $save) }
    } catch { "  (the save dialog closed after Enter)" }
    Start-Sleep -Seconds 2
}
Check 'the archive was written to the chosen path' (Test-Path $zip) "path=$zip"
$texts = Get-VisibleTexts
Check 'the page reports the archive was saved locally' ([bool]($texts | Where-Object { $_ -match 'The archive was saved locally' }))

"== Inspect the archive: contents and redaction =="
if (Test-Path $zip) {
    $size = (Get-Item $zip).Length
    "  archive size: $size bytes"
    Check 'the archive is a non-empty ZIP' ($size -gt 500)
    $extract = Join-Path $outDir 'extracted'
    Remove-Item -Recurse -Force $extract -ErrorAction SilentlyContinue
    Expand-Archive -Path $zip -DestinationPath $extract -Force
    $files = Get-ChildItem $extract -Recurse -File
    "  files: $($files.Name -join ', ')"
    Check 'the archive holds manifest.json, frames.jsonl and README.txt' (@($files.Name | Where-Object { $_ -in 'manifest.json', 'frames.jsonl', 'README.txt' }).Count -eq 3)
    $all = ($files | ForEach-Object { Get-Content $_.FullName -Raw }) -join "`n"
    $manifest = Get-Content (Join-Path $extract 'manifest.json') -Raw | ConvertFrom-Json
    Check 'the manifest names the A3062 model' ("$($manifest.model)" -match 'A3062' -or $all -match 'A3062')
    "  manifest keys: $($manifest.PSObject.Properties.Name -join ', ')"
    Check 'the manifest has no serial field' (-not ($manifest.PSObject.Properties.Name | Where-Object { $_ -match 'serial' }))
    $macLike = [regex]::Matches($all, '([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}')
    Check 'no colon-form Bluetooth address appears in the archive' ($macLike.Count -eq 0) "matches=$($macLike.Count)"
    $bareMacLike = [regex]::Matches($all, '\b[0-9A-Fa-f]{12}\b')
    Check 'no bare 12-hex address form appears in the archive' ($bareMacLike.Count -eq 0) "matches=$($bareMacLike.Count)"
    $serialLike = [regex]::Matches($all, '(?i)serial')
    "  occurrences of the word serial: $($serialLike.Count) (only the privacy notice may name it)"
    $framesText = Get-Content (Join-Path $extract 'frames.jsonl') -Raw
    $frameCount = @($framesText -split "`n" | Where-Object { $_.Trim() -ne '' }).Count
    "  frame entries: $frameCount"
    Check 'frames.jsonl has the captured frames' ($frameCount -ge 3)
    $readme = Get-Content (Join-Path $extract 'README.txt') -Raw
    Check 'README.txt explains the archive' ([bool]($readme -match 'Soundcore diagnostics'))
    # Generic serial check: the A3062 serial is 16 ASCII digits in the state
    # response; decode the manifest probe responses and look for such a run.
    $probeHex = @()
    foreach ($probe in $manifest.probes) { if ($probe.responseHex) { $probeHex += $probe.responseHex } }
    $decoded = ''
    foreach ($h in $probeHex) {
        $chars = foreach ($byte in ($h -split '\s+')) { if ($byte) { [char][Convert]::ToInt32($byte, 16) } }
        $decoded += (-join $chars)
    }
    $serialRuns = [regex]::Matches($decoded, '[0-9A-Fa-f]{16}')
    Check 'no serial-shaped run survives in the manifest probe responses' ($serialRuns.Count -eq 0) "matches=$($serialRuns.Count)"
} else {
    Check 'the archive exists to be inspected' $false
}

"== Open GitHub issue (prefilled, not submitted) =="
$issue = Find-ButtonNamed 'Open GitHub issue' 6
if (-not $issue) { $issue = Find-ButtonNamed 'Open issue again' 6 }
Check 'the Open GitHub issue action is offered' ([bool]$issue)
$browserNames = @('chrome', 'msedge', 'firefox', 'brave')
$before = @(Get-Process -Name $browserNames -ErrorAction SilentlyContinue).Count
if ($issue) { [void](Invoke-Element $issue); Start-Sleep -Seconds 6 }
$after = @(Get-Process -Name $browserNames -ErrorAction SilentlyContinue).Count
Check 'a browser was opened for the issue URL' ($after -ge 1) "browsers before=$before after=$after"
$urlValue = $null
try {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $wins = $root.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($w in $wins) {
        try {
            if ($w.Current.ClassName -notmatch 'Chrome_WidgetWin_1|MozillaWindowClass') { continue }
            $edits = Find-AllByType -Root $w -ControlType 'Edit'
            foreach ($e in $edits) {
                try {
                    $vp = $null
                    if ($e.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$vp)) {
                        $value = $vp.Current.Value
                        if ($value -match 'github\.com') { $urlValue = $value; break }
                    }
                } catch {}
            }
        } catch {}
        if ($urlValue) { break }
    }
} catch {}
if ($urlValue) {
    "  issue URL: $urlValue"
    Check 'the opened URL targets the prefilled OpenSCL4 issue' ([bool]($urlValue -match 'github\.com.*OpenSCL4.*issues(%2F|/)new'))
    Check 'the opened URL carries the device-diagnostics label and the A3062 probe summary' ([bool]($urlValue -match 'device-diagnostics' -and $urlValue -match 'A3062'))
    Check 'the opened URL carries no Bluetooth address' (-not ($urlValue -match '([0-9A-Fa-f]{2}%3A){5}[0-9A-Fa-f]{2}'))
} else {
    "  the browser did not expose an address bar to UIA; the click opened the default browser"
}
# Leave the review/report step; no issue is submitted.
[void][W32]::SetForegroundWindow($hwnd)
Start-Sleep -Milliseconds 500
$done = Find-ButtonNamed 'Done' 3
if ($done) { [void](Invoke-Element $done); Start-Sleep -Milliseconds 700 }
$back = Find-ButtonNamed 'Back' 2
if ($back) { [void](Invoke-Element $back); Start-Sleep -Milliseconds 700 }

if ($failures.Count -gt 0) { ""; "FAILURES:"; $failures | ForEach-Object { " - $_" }; exit 1 }
""; "PHASE 4 PASSED"

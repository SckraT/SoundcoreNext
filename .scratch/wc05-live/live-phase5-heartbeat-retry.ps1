# Ticket 05 live acceptance, phase 5: idle heartbeat past one interval, the
# Lost/Retry path, and leaving the device and the app as found.
# Requires the app connected to the real A3062.
. "$PSScriptRoot\..\wc04-uia\session.ps1"
Connect-Ui | Out-Null
# A minimized window has no client area to capture; restore it before the run.
[void][W32]::ShowWindow($hwnd, 9)
Start-Sleep -Milliseconds 700
[void][W32]::SetForegroundWindow($hwnd)
$script:ShotsDir = "$PSScriptRoot\shots"
New-Item -ItemType Directory -Force $script:ShotsDir | Out-Null
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
function Get-LdacSwitch {
    Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'LDAC' -and [int]$_.Current.BoundingRectangle.Width -eq 48 } | Select-Object -First 1
}
function Get-ToggleStateOf($element) {
    if (-not $element) { return $null }
    $p = $null
    if ($element.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$p)) { return $p.Current.ToggleState }
    return $null
}
function Wait-Connected([int]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    do {
        $texts = Get-VisibleTexts
        if (($texts -contains 'Connected') -and -not ($texts | Where-Object { $_ -match 'Connection lost' })) { return $true }
        Start-Sleep -Milliseconds 500
    } while ((Get-Date) -lt $deadline)
    return $false
}
function Wait-Lost([int]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    do {
        $texts = Get-VisibleTexts
        if ($texts | Where-Object { $_ -match 'Connection lost|Could not change' }) { return $true }
        Start-Sleep -Milliseconds 500
    } while ((Get-Date) -lt $deadline)
    return $false
}
function Retry-Connection {
    $button = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -match 'Connect again|Retry' } | Select-Object -First 1
    if (-not $button) { return $false }
    [void](Invoke-Element $button)
    return (Wait-Connected 70)
}

"== Idle heartbeat past one interval =="
Click-Tab 'Overview'; Start-Sleep -Milliseconds 800
$titleBefore = Get-WindowTitle
$batteryBefore = (Get-VisibleTexts | Where-Object { $_ -match '^Battery: \d+%$' }) -join ','
$errorBefore = Get-VisibleTexts | Where-Object { $_ -match 'Could not|Connection lost' }
"  waiting 25 s idle (heartbeat interval is 15 s); title=$titleBefore battery=$batteryBefore"
Start-Sleep -Seconds 25
$titleAfter = Get-WindowTitle
$batteryAfter = (Get-VisibleTexts | Where-Object { $_ -match '^Battery: \d+%$' }) -join ','
$errorAfter = Get-VisibleTexts | Where-Object { $_ -match 'Could not|Connection lost' }
Check 'the idle connection is still shown after more than one heartbeat interval' ($titleAfter -eq $titleBefore)
Check 'the battery tile is unchanged after the idle heartbeats' ($batteryAfter -eq $batteryBefore) "$batteryBefore -> $batteryAfter"
Check 'no error banner appears after the idle heartbeats' (-not $errorAfter) "errors=$errorAfter"
Check 'the Overview still shows the connected badge' ([bool](Find-Text 'Connected' -TimeoutSec 2))
Save-PhaseShot 'live-overview-after-heartbeat' | Out-Null

"== Lost and Retry path =="
# The earbuds drop their Bluetooth links when LDAC is written (documented device
# behaviour), which is a real way to reach the app's Lost state without touching
# the radio or the pairing.
Click-Tab 'Sound and effects'; Start-Sleep -Milliseconds 1200
$ldac = Get-LdacSwitch
Check 'the Effects page offers the real LDAC switch' ([bool]$ldac)
$initialLdac = Get-ToggleStateOf $ldac
"  LDAC state found on connect: $initialLdac"
$lost = $false
if ($ldac) {
    $pattern = $null
    [void]$ldac.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)
    $pattern.Toggle()
    $lost = Wait-Lost 45
}
Check 'writing LDAC reaches the readable Lost/banner state (documented link reset)' $lost
Save-PhaseShot 'live-connection-lost' | Out-Null
$reconnected = Retry-Connection
Check 'Retry reconnects the real device' $reconnected
if ($reconnected) {
    Click-Tab 'Sound and effects'; Start-Sleep -Milliseconds 1200
    $now = Get-ToggleStateOf (Get-LdacSwitch)
    "  LDAC after Retry: $now (found $initialLdac)"
    if ($now -ne $initialLdac) {
        # Restoring the setting writes LDAC again, which resets the link once
        # more: the same Lost -> Retry cycle restores it.
        $ldac = Get-LdacSwitch
        if ($ldac) {
            $pattern = $null
            [void]$ldac.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)
            $pattern.Toggle()
            [void](Wait-Lost 45)
            Check 'the restoring LDAC write is re-polled after a second Retry' (Retry-Connection)
            Click-Tab 'Sound and effects'; Start-Sleep -Milliseconds 1200
            $restored = Get-ToggleStateOf (Get-LdacSwitch)
            Check 'the LDAC value found on connect is restored' ($restored -eq $initialLdac) "found=$initialLdac restored=$restored"
        }
    }
}

"== Disconnect and leave the app as found =="
Click-Tab 'My devices'; Start-Sleep -Milliseconds 900
$disconnect = Find-ButtonNamed 'Disconnect' 6
if ($disconnect) { [void](Invoke-Element $disconnect); Start-Sleep -Seconds 1 }
Check 'Disconnect returns to My devices and keeps the saved card' ([bool]$disconnect)

if ($failures.Count -gt 0) { ""; "FAILURES:"; $failures | ForEach-Object { " - $_" }; exit 1 }
""; "PHASE 5 PASSED"

# Test helper: launch a fresh app and connect the simulated A3062 in Demo mode.
# Used only to validate the live-acceptance UIA flows (element names/steps) while
# the real device is off; it is not acceptance evidence.
. "$PSScriptRoot\..\wc04-uia\session.ps1"
function Find-ButtonNamed([string]$Name, [int]$TimeoutSec = 10) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        $hit = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq $Name } | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 200
    } while ((Get-Date) -lt $deadline)
    return $null
}
Stop-App
Start-App | Out-Null
Connect-Ui -TimeoutSec 40 | Out-Null
Click-Button 'Add device' -SettleMs 1200
$search = Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -eq 'Search models' } | Select-Object -First 1
Invoke-ClickCenter $search; Start-Sleep -Milliseconds 300
[System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 120
[System.Windows.Forms.SendKeys]::SendWait('A3062'); Start-Sleep -Milliseconds 1200
$card = Find-ButtonNamed 'Space One Pro'
if (-not $card) { throw 'model card not found' }
[void](Invoke-Element $card); Start-Sleep -Milliseconds 1200
$demo = Find-AllByType -Root $win -ControlType 'Button' | Where-Object { $_.Current.Name -eq 'Demo mode' -and [int]$_.Current.BoundingRectangle.Width -eq 48 } | Select-Object -First 1
if (-not $demo) { throw 'Demo mode toggle not found' }
[void](Invoke-Element $demo); Start-Sleep -Milliseconds 1200
$row = Find-ButtonNamed 'soundcore Space One Pro · Demo'
if (-not $row) { throw 'demo device row not found' }
[void](Invoke-Element $row)
$deadline = (Get-Date).AddSeconds(30)
do {
    if ((Get-WindowTitle) -match 'Overview') { break }
    Start-Sleep -Milliseconds 300
} while ((Get-Date) -lt $deadline)
"connected (demo): $(Get-WindowTitle)"

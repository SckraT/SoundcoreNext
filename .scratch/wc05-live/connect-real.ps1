# Connect the paired A3062 through the real Add Device UI (no Demo mode).
# Returns $true when the Overview shows the connected device.
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
function Connect-RealDevice {
    Connect-Ui -TimeoutSec 40 | Out-Null
    if ((Get-WindowTitle) -notmatch 'My devices') { Click-Tab 'My devices'; Start-Sleep -Milliseconds 800 }
    Click-Button 'Add device' -SettleMs 1200
    if (-not (Find-Text 'Find your model.' -TimeoutSec 8)) { throw 'the model picker did not open' }
    $search = Find-AllByType -Root $win -ControlType 'Edit' | Where-Object { $_.Current.Name -eq 'Search models' } | Select-Object -First 1
    Invoke-ClickCenter $search; Start-Sleep -Milliseconds 300
    [System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 120
    [System.Windows.Forms.SendKeys]::SendWait('A3062'); Start-Sleep -Milliseconds 1200
    $card = Find-ButtonNamed 'Space One Pro'
    if (-not $card) { throw 'the Space One Pro model card was not found' }
    [void](Invoke-Element $card); Start-Sleep -Milliseconds 1200
    $row = Find-ButtonNamed 'soundcore Space One Pro' 8
    if (-not $row) { throw 'the paired Space One Pro row was not found' }
    [void](Invoke-Element $row)
    $deadline = (Get-Date).AddSeconds(60)
    do {
        if ((Get-WindowTitle) -match 'Overview' -and (Find-Text 'Connected' -TimeoutSec 1)) { return $true }
        Start-Sleep -Milliseconds 400
    } while ((Get-Date) -lt $deadline)
    "  connect failed; visible: $((Get-VisibleTexts) -join ' | ')"
    return $false
}

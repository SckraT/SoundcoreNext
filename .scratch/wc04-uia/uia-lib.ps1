# Shared UI Automation helpers for the ticket 04 demo acceptance.
# Load with: . "$PSScriptRoot\uia-lib.ps1"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class W32 {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int cmd);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr hWnd, int index);
    [DllImport("user32.dll")] public static extern bool AdjustWindowRectEx(ref RECT rect, uint style, bool menu, uint exStyle);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string name);
}
"@

[void][W32]::SetProcessDPIAware()

$script:UIA = [System.Windows.Automation.AutomationElement]
$script:ScopeDesc = [System.Windows.Automation.TreeScope]::Descendants
$script:ScopeChildren = [System.Windows.Automation.TreeScope]::Children

function Get-AppWindow {
    param([int]$ProcessId, [int]$TimeoutSec = 30)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        $cond = New-Object System.Windows.Automation.PropertyCondition($script:UIA::ProcessIdProperty, $ProcessId)
        $windows = $script:UIA::RootElement.FindAll($script:ScopeChildren, $cond)
        $best = $null
        $bestArea = -1
        foreach ($element in $windows) {
            $c = $element.Current
            $rect = $c.BoundingRectangle
            $area = [int]$rect.Width * [int]$rect.Height
            if ($area -gt $bestArea) { $best = $element; $bestArea = $area }
        }
        if ($best -and $bestArea -gt 10000) { return $best }
        Start-Sleep -Milliseconds 200
    } while ((Get-Date) -lt $deadline)
    throw "no visible top-level window for process $ProcessId"
}

function Get-WindowHandle {
    param($Element)
    $Element.Current.NativeWindowHandle
}

function Find-ByName {
    param($Root, [string]$Name, [int]$TimeoutSec = 10, [switch]$Prefix)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        if ($Prefix) {
            $cond = New-Object System.Windows.Automation.PropertyCondition($script:UIA::NameProperty, $Name, [System.Windows.Automation.PropertyConditionFlags]::IgnoreCase)
        } else {
            $cond = New-Object System.Windows.Automation.PropertyCondition($script:UIA::NameProperty, $Name)
        }
        $found = $Root.FindFirst($script:ScopeDesc, $cond)
        if ($found) { return $found }
        Start-Sleep -Milliseconds 150
    } while ((Get-Date) -lt $deadline)
    return $null
}

function Find-AllByType {
    param($Root, [string]$ControlType)
    $type = [System.Windows.Automation.ControlType]::$ControlType
    $cond = New-Object System.Windows.Automation.PropertyCondition($script:UIA::ControlTypeProperty, $type)
    $Root.FindAll($script:ScopeDesc, $cond)
}

function Get-Tree {
    param($Root, [int]$MaxDepth = 12, [switch]$All)
    $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $lines = New-Object System.Collections.Generic.List[string]
    $script:treeIndex = 0
    function Walk($el, $depth) {
        if ($depth -gt $MaxDepth) { return }
        $indent = '  ' * $depth
        $c = $el.Current
        $patterns = @()
        foreach ($p in @([System.Windows.Automation.InvokePattern]::Pattern,
                         [System.Windows.Automation.TogglePattern]::Pattern,
                         [System.Windows.Automation.SelectionItemPattern]::Pattern,
                         [System.Windows.Automation.ValuePattern]::Pattern,
                         [System.Windows.Automation.ExpandCollapsePattern]::Pattern,
                         [System.Windows.Automation.RangeValuePattern]::Pattern)) {
            $obj = $null
            try {
                if ($el.TryGetCurrentPattern($p, [ref]$obj)) {
                    $patterns += ($p.ProgrammaticName -replace 'PatternIdentifiers\.Pattern', '')
                }
            } catch {}
        }
        $extra = ''
        $obj = $null
        try {
            if ($el.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$obj)) { $extra += " state=$($obj.Current.ToggleState)" }
        } catch {}
        $obj = $null
        try {
            if ($el.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$obj)) { $extra += " selected=$($obj.Current.IsSelected)" }
        } catch {}
        $obj = $null
        try {
            if ($el.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$obj)) { $extra += " value='$($obj.Current.Value)'" }
        } catch {}
        $rect = $c.BoundingRectangle
        $typeName = $c.ControlType.ProgrammaticName.Replace('ControlType.', '')
        $patternText = $patterns -join ','
        $lines.Add("$indent[$typeName] '$($c.Name)' id='$($c.AutomationId)' enabled=$($c.IsEnabled) offscreen=$($c.IsOffscreen) rect=$([int]$rect.X),$([int]$rect.Y),$([int]$rect.Width)x$([int]$rect.Height) patterns=$patternText$extra")
        $child = $walker.GetFirstChild($el)
        while ($child) {
            Walk $child ($depth + 1)
            $child = $walker.GetNextSibling($child)
        }
    }
    Walk $Root 0
    $lines
}

function Invoke-Element {
    param($Element, [switch]$ClickFallback)
    if (-not $Element) { throw "Invoke-Element: null element" }
    $p = $null
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$p)) {
        $p.Invoke(); return 'invoke'
    }
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$p)) {
        $p.Select(); return 'select'
    }
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$p)) {
        $p.Toggle(); return 'toggle'
    }
    if ('System.Windows.Automation.LegacyIAccessiblePattern' -as [type]) {
        if ($Element.TryGetCurrentPattern([System.Windows.Automation.LegacyIAccessiblePattern]::Pattern, [ref]$p)) {
            $p.DoDefaultAction(); return 'legacy'
        }
    }
    if ($ClickFallback) { Invoke-ClickCenter $Element; return 'click' }
    throw "element '$($Element.Current.Name)' has no invoke pattern"
}

function Invoke-ClickCenter {
    param($Element)
    $rect = $Element.Current.BoundingRectangle
    $x = [int]($rect.X + $rect.Width / 2)
    $y = [int]($rect.Y + $rect.Height / 2)
    [void][W32]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 60
    [W32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [W32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 150
}

function Set-ToggleState {
    param($Element, [bool]$On)
    $p = $null
    if (-not $Element.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$p)) {
        throw "element '$($Element.Current.Name)' has no toggle pattern"
    }
    $want = if ($On) { [System.Windows.Automation.ToggleState]::On } else { [System.Windows.Automation.ToggleState]::Off }
    if ($p.Current.ToggleState -ne $want) { $p.Toggle() }
}

function Get-ToggleState {
    param($Element)
    $p = $null
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$p)) { return $p.Current.ToggleState }
    return $null
}

function Set-ClientSize {
    param([IntPtr]$Handle, [int]$Width, [int]$Height)
    $style = [uint32][W32]::GetWindowLong($Handle, -16)
    $exStyle = [uint32][W32]::GetWindowLong($Handle, -20)
    $rect = New-Object W32+RECT
    $rect.Left = 0; $rect.Top = 0; $rect.Right = $Width; $rect.Bottom = $Height
    [void][W32]::AdjustWindowRectEx([ref]$rect, $style, $false, $exStyle)
    $outerW = $rect.Right - $rect.Left
    $outerH = $rect.Bottom - $rect.Top
    [void][W32]::SetWindowPos($Handle, [IntPtr]::Zero, 40, 40, $outerW, $outerH, 0x0040) # SWP_SHOWWINDOW
    Start-Sleep -Milliseconds 300
}

function Get-ClientSize {
    param([IntPtr]$Handle)
    $rect = New-Object W32+RECT
    [void][W32]::GetClientRect($Handle, [ref]$rect)
    "$($rect.Right - $rect.Left)x$($rect.Bottom - $rect.Top)"
}

function Save-WindowPng {
    param([IntPtr]$Handle, [string]$Path, [switch]$ClientOnly)
    $w32rect = New-Object W32+RECT
    if ($ClientOnly) {
        if (-not [W32]::GetClientRect($Handle, [ref]$w32rect)) { throw "GetClientRect failed" }
        $pt = New-Object W32+POINT
        $pt.X = 0; $pt.Y = 0
        [void][W32]::ClientToScreen($Handle, [ref]$pt)
        $w = $w32rect.Right; $h = $w32rect.Bottom
    } else {
        if (-not [W32]::GetWindowRect($Handle, [ref]$w32rect)) { throw "GetWindowRect failed" }
        $pt = New-Object W32+POINT
        $pt.X = $w32rect.Left; $pt.Y = $w32rect.Top
        $w = $w32rect.Right - $w32rect.Left; $h = $w32rect.Bottom - $w32rect.Top
    }
    $bmp = New-Object System.Drawing.Bitmap($w, $h)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    # PW_RENDERFULLCONTENT = 2: also renders redirected/DirectComposition content.
    $ok = [W32]::PrintWindow($Handle, $hdc, 2)
    $g.ReleaseHdc($hdc)
    $g.Dispose()
    if (-not $ok) { throw "PrintWindow failed for handle $Handle" }
    if ($ClientOnly) {
        # PrintWindow draws the whole window into the bitmap's top-left area; the
        # window frame occupies the first (client origin) offset.
        $crop = New-Object System.Drawing.Rectangle($pt.X - $w32rect.Left, $pt.Y - $w32rect.Top, $w, $h)
        # Simpler: re-capture whole window and crop using the screen offset.
    }
    $dir = Split-Path -Parent $Path
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force $dir | Out-Null }
    $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    $Path
}

# Crops a whole-window capture down to the client area, based on the current
# window/client rectangles.
function Save-WindowClientPng {
    param([IntPtr]$Handle, [string]$Path)
    $wr = New-Object W32+RECT; [void][W32]::GetWindowRect($Handle, [ref]$wr)
    $cr = New-Object W32+RECT; [void][W32]::GetClientRect($Handle, [ref]$cr)
    $pt = New-Object W32+POINT; $pt.X = 0; $pt.Y = 0
    [void][W32]::ClientToScreen($Handle, [ref]$pt)
    $offX = $pt.X - $wr.Left; $offY = $pt.Y - $wr.Top
    $w = $cr.Right; $h = $cr.Bottom
    $frame = New-Object System.Drawing.Bitmap(($wr.Right - $wr.Left), ($wr.Bottom - $wr.Top))
    $g = [System.Drawing.Graphics]::FromImage($frame)
    $hdc = $g.GetHdc()
    [void][W32]::PrintWindow($Handle, $hdc, 2)
    $g.ReleaseHdc($hdc); $g.Dispose()
    $crop = New-Object System.Drawing.Bitmap($w, $h)
    $gc = [System.Drawing.Graphics]::FromImage($crop)
    $gc.DrawImage($frame, (New-Object System.Drawing.Rectangle(0,0,$w,$h)), (New-Object System.Drawing.Rectangle($offX,$offY,$w,$h)), [System.Drawing.GraphicsUnit]::Pixel)
    $gc.Dispose()
    $frame.Dispose()
    $dir = Split-Path -Parent $Path
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force $dir | Out-Null }
    $crop.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $crop.Dispose()
    $Path
}

function Wait-For {
    param([scriptblock]$Condition, [int]$TimeoutSec = 15, [string]$What = 'condition')
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    do {
        $result = & $Condition
        if ($result) { return $result }
        Start-Sleep -Milliseconds 150
    } while ((Get-Date) -lt $deadline)
    throw "timed out waiting for $What"
}


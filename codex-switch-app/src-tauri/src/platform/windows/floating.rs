use crate::{locate_data_dir, FloatingBarPayload, CREATE_NO_WINDOW};
use std::{fs, os::windows::process::CommandExt, path::PathBuf, process::Command};

fn floating_state_path() -> PathBuf {
    locate_data_dir().join("floating_state.json")
}

fn floating_script_path() -> PathBuf {
    locate_data_dir().join("floating_bar.ps1")
}

fn floating_pid_path() -> PathBuf {
    locate_data_dir().join("floating_bar.pid")
}

fn floating_script_text() -> &'static str {
    r#"
param(
  [Parameter(Mandatory=$true)][string]$StatePath,
  [Parameter(Mandatory=$true)][string]$PidPath,
  [Parameter(Mandatory=$true)][int]$OwnerPid,
  [string]$WakeAddr = '127.0.0.1',
  [int]$WakePort = 45421
)

$ErrorActionPreference = 'SilentlyContinue'
[System.IO.File]::WriteAllText($PidPath, [string]$PID)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Text;
using System.Windows.Forms;
using System.Runtime.InteropServices;

public static class DpiAwareness {
  private static readonly IntPtr PerMonitorAwareV2 = new IntPtr(-4);

  [DllImport("user32.dll", SetLastError = true)]
  private static extern bool SetProcessDpiAwarenessContext(IntPtr value);

  [DllImport("user32.dll", SetLastError = true)]
  private static extern IntPtr SetThreadDpiAwarenessContext(IntPtr value);

  [DllImport("user32.dll", SetLastError = true)]
  private static extern bool SetProcessDPIAware();

  [DllImport("user32.dll", SetLastError = true)]
  private static extern uint GetDpiForWindow(IntPtr hwnd);

  public static void EnablePerMonitorV2() {
    try {
      if (SetProcessDpiAwarenessContext(PerMonitorAwareV2)) return;
    } catch (EntryPointNotFoundException) {}

    try {
      if (SetThreadDpiAwarenessContext(PerMonitorAwareV2) != IntPtr.Zero) return;
    } catch (EntryPointNotFoundException) {}

    try { SetProcessDPIAware(); } catch (EntryPointNotFoundException) {}
  }

  public static float GetWindowScale(IntPtr hwnd) {
    try {
      uint dpi = GetDpiForWindow(hwnd);
      return dpi > 0 ? dpi / 96.0f : 0.0f;
    } catch (EntryPointNotFoundException) {
      return 0.0f;
    }
  }
}

public class ScrollTextPanel : Panel {
  public string DisplayText = "";
  public Font DisplayFont = new Font("Segoe UI", 10, FontStyle.Bold);
  public Color DisplayColor = Color.FromArgb(30, 41, 59);
  public int OffsetX = 0;
  public bool AlignRight = false;

  public ScrollTextPanel() {
    this.DoubleBuffered = true;
    this.SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.UserPaint, true);
  }

  protected override void OnPaint(PaintEventArgs e) {
    base.OnPaint(e);
    e.Graphics.TextRenderingHint = TextRenderingHint.ClearTypeGridFit;
    using (SolidBrush brush = new SolidBrush(DisplayColor)) {
      float x = OffsetX;
      if (AlignRight) {
        SizeF size = e.Graphics.MeasureString(DisplayText, DisplayFont);
        x = this.ClientSize.Width - size.Width;
      }
      e.Graphics.DrawString(DisplayText, DisplayFont, brush, new PointF(x, -1));
    }
  }
}

public class CodexFloatingForm : Form {
  private const int WS_BORDER = 0x00800000;
  private const int WS_EX_TOOLWINDOW = 0x00000080;
  private const int WM_DPICHANGED = 0x02E0;
  private const int DWMWA_WINDOW_CORNER_PREFERENCE = 33;
  private const int DWMWCP_ROUND = 2;
  private const uint SWP_NOSIZE = 0x0001;
  private const uint SWP_NOMOVE = 0x0002;
  private const uint SWP_NOACTIVATE = 0x0010;
  private const uint SWP_SHOWWINDOW = 0x0040;
  private static readonly IntPtr HWND_TOPMOST = new IntPtr(-1);
  private static readonly IntPtr HWND_NOTOPMOST = new IntPtr(-2);

  [DllImport("dwmapi.dll")]
  private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int attrValue, int attrSize);

  [DllImport("user32.dll", SetLastError = true)]
  private static extern bool SetWindowPos(IntPtr hwnd, IntPtr insertAfter, int x, int y, int width, int height, uint flags);

  public event EventHandler DpiScaleChanged;
  public uint PendingDpi { get; private set; }

  protected override CreateParams CreateParams {
    get {
      CreateParams cp = base.CreateParams;
      cp.Style |= WS_BORDER;
      cp.ExStyle |= WS_EX_TOOLWINDOW;
      return cp;
    }
  }

  protected override void OnHandleCreated(EventArgs e) {
    base.OnHandleCreated(e);
    int preference = DWMWCP_ROUND;
    try {
      DwmSetWindowAttribute(this.Handle, DWMWA_WINDOW_CORNER_PREFERENCE, ref preference, sizeof(int));
    } catch {}
  }

  public void ReassertTopmost() {
    if (!IsHandleCreated || IsDisposed) return;
    uint flags = SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE | SWP_SHOWWINDOW;
    SetWindowPos(Handle, HWND_NOTOPMOST, 0, 0, 0, 0, flags);
    SetWindowPos(Handle, HWND_TOPMOST, 0, 0, 0, 0, flags);
  }

  protected override void WndProc(ref Message m) {
    if (m.Msg == WM_DPICHANGED) {
      PendingDpi = unchecked((uint)(m.WParam.ToInt64() & 0xFFFF));
    }
    base.WndProc(ref m);
    if (m.Msg == WM_DPICHANGED && DpiScaleChanged != null) {
      DpiScaleChanged(this, EventArgs.Empty);
    }
  }
}
"@

[DpiAwareness]::EnablePerMonitorV2()
$form = New-Object CodexFloatingForm
$form.Text = 'Codex Switch Floating'
$form.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::None
$form.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
$form.TopMost = $true
$form.ShowInTaskbar = $false
$form.AutoScaleMode = [System.Windows.Forms.AutoScaleMode]::None
$form.BackColor = [System.Drawing.Color]::FromArgb(249, 251, 252)

# Per-monitor aware forms use physical pixels. Scale every fixed layout value
# so a 200% display keeps the same compact visual size without clipping text.
$dpiGraphics = $form.CreateGraphics()
$script:dpiScale = [Math]::Max(1.0, ($dpiGraphics.DpiX / 96.0))
$dpiGraphics.Dispose()
$form.Width = [Math]::Round(150 * $script:dpiScale)
$form.Height = [Math]::Round(30 * $script:dpiScale)
$form.Padding = New-Object System.Windows.Forms.Padding([Math]::Round(10 * $script:dpiScale), 0, [Math]::Round(10 * $script:dpiScale), 0)

function New-FloatingDisplayFont {
  # Use physical pixels so font size follows the same effective-DPI ratio as the form.
  return [System.Drawing.Font]::new('Segoe UI', [single](14 * $script:dpiScale), [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
}

function Get-FloatingTextHostHeight {
  param($Font)
  $textHeight = [System.Windows.Forms.TextRenderer]::MeasureText('Ag', $Font).Height
  return [Math]::Max([Math]::Round(18 * $script:dpiScale), $textHeight + [Math]::Round(2 * $script:dpiScale))
}

function Get-FloatingWorkingArea {
  if ($script:dragging) {
    return [System.Windows.Forms.Screen]::FromPoint([System.Windows.Forms.Cursor]::Position).WorkingArea
  }
  return [System.Windows.Forms.Screen]::FromRectangle($form.Bounds).WorkingArea
}

$startArea = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
$form.Left = [Math]::Max($startArea.Left, $startArea.Right - $form.Width - [Math]::Round(24 * $script:dpiScale))
$form.Top = $startArea.Top + [Math]::Round(120 * $script:dpiScale)

$titleHost = New-Object ScrollTextPanel
$titleHost.Left = [Math]::Round(10 * $script:dpiScale)
$titleHost.Top = [Math]::Round(6 * $script:dpiScale)
$titleHost.Width = [Math]::Round(84 * $script:dpiScale)
$titleHost.Height = [Math]::Round(18 * $script:dpiScale)
$titleHost.BackColor = $form.BackColor
$titleHost.DisplayFont = New-FloatingDisplayFont
$titleHost.DisplayColor = [System.Drawing.Color]::FromArgb(30, 41, 59)
$titleHost.DisplayText = 'Codex Switch'
$form.Controls.Add($titleHost)

$script:fullTitle = $titleHost.DisplayText
$script:titleHovering = $false
$script:titleScrollX = 0
$script:titleFullWidth = 0
$script:titleDisplayChars = 12
$script:titleSlotWidth = [Math]::Round(84 * $script:dpiScale)
$script:barWidth = [Math]::Round(160 * $script:dpiScale)

$usage = New-Object ScrollTextPanel
$usage.Left = [Math]::Round(108 * $script:dpiScale)
$usage.Top = [Math]::Round(6 * $script:dpiScale)
$usage.Width = [Math]::Round(42 * $script:dpiScale)
$usage.Height = [Math]::Round(18 * $script:dpiScale)
$usage.BackColor = $form.BackColor
$usage.DisplayFont = New-FloatingDisplayFont
$usage.DisplayColor = [System.Drawing.Color]::FromArgb(117, 103, 183)
$usage.DisplayText = '-'
$usage.AlignRight = $true
$form.Controls.Add($usage)
$textHostHeight = Get-FloatingTextHostHeight $titleHost.DisplayFont
$titleHost.Height = $textHostHeight
$usage.Height = $textHostHeight
$form.Height = [Math]::Max([Math]::Round(30 * $script:dpiScale), $textHostHeight + [Math]::Round(12 * $script:dpiScale))
$script:usageSlotWidth = [System.Windows.Forms.TextRenderer]::MeasureText('100%', $usage.DisplayFont).Width + [Math]::Round(2 * $script:dpiScale)

$dragging = $false
$dragMoved = $false
$dragOffset = New-Object System.Drawing.Point(0, 0)
$dragMouseDown = {
  $script:dragging = $true
  $script:dragMoved = $false
  $sender = $this
  $point = $sender.PointToScreen((New-Object System.Drawing.Point($_.X, $_.Y)))
  $script:dragOffset = New-Object System.Drawing.Point(($point.X - $form.Left), ($point.Y - $form.Top))
}
$dragMouseMove = {
  if (-not $script:dragging) { return }
  $cursor = [System.Windows.Forms.Cursor]::Position
  $nextLeft = $cursor.X - $script:dragOffset.X
  $nextTop = $cursor.Y - $script:dragOffset.Y
  if ([Math]::Abs($nextLeft - $form.Left) -gt 2 -or [Math]::Abs($nextTop - $form.Top) -gt 2) {
    $script:dragMoved = $true
  }
  $form.Left = $nextLeft
  $form.Top = $nextTop
  Sync-FloatingDpi
}
function Clamp-FloatingToScreen {
  $bounds = Get-FloatingWorkingArea
  if ($form.Left -lt $bounds.Left) { $form.Left = $bounds.Left }
  if ($form.Top -lt $bounds.Top) { $form.Top = $bounds.Top }
  if ($form.Right -gt $bounds.Right) { $form.Left = $bounds.Right - $form.Width }
  if ($form.Bottom -gt $bounds.Bottom) { $form.Top = $bounds.Bottom - $form.Height }
}

function Ensure-FloatingVisible {
  if ($form.IsDisposed) { return }
  if ($form.WindowState -ne [System.Windows.Forms.FormWindowState]::Normal) {
    $form.WindowState = [System.Windows.Forms.FormWindowState]::Normal
  }
  if (-not $form.Visible) { $form.Show() }
  if (-not $form.TopMost) { $form.TopMost = $true }

  $screens = @([System.Windows.Forms.Screen]::AllScreens)
  $targetArea = $null
  $largestIntersection = 0
  foreach ($screen in $screens) {
    $intersection = [System.Drawing.Rectangle]::Intersect($form.Bounds, $screen.WorkingArea)
    $area = [long]$intersection.Width * [long]$intersection.Height
    if ($area -gt $largestIntersection) {
      $largestIntersection = $area
      $targetArea = $screen.WorkingArea
    }
  }

  $minimumVisible = [Math]::Round(20 * $script:dpiScale)
  if ($null -eq $targetArea -or $largestIntersection -lt ($minimumVisible * $minimumVisible)) {
    $targetArea = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    $form.Left = [Math]::Max($targetArea.Left, $targetArea.Right - $form.Width - [Math]::Round(24 * $script:dpiScale))
    $form.Top = $targetArea.Top + [Math]::Round(120 * $script:dpiScale)
  }

  if ($form.Left -lt $targetArea.Left) { $form.Left = $targetArea.Left }
  if ($form.Top -lt $targetArea.Top) { $form.Top = $targetArea.Top }
  if ($form.Right -gt $targetArea.Right) { $form.Left = $targetArea.Right - $form.Width }
  if ($form.Bottom -gt $targetArea.Bottom) { $form.Top = $targetArea.Bottom - $form.Height }
  $form.ReassertTopmost()
}
$dragMouseUp = {
  $script:dragging = $false
  Clamp-FloatingToScreen
}
$form.Add_MouseDown($dragMouseDown)
$form.Add_MouseMove($dragMouseMove)
$form.Add_MouseUp($dragMouseUp)
$titleHost.Add_MouseDown($dragMouseDown)
$titleHost.Add_MouseMove($dragMouseMove)
$titleHost.Add_MouseUp($dragMouseUp)
$usage.Add_MouseDown($dragMouseDown)
$usage.Add_MouseMove($dragMouseMove)
$usage.Add_MouseUp($dragMouseUp)

$restoreMainWindow = {
  try {
    $client = New-Object System.Net.Sockets.TcpClient
    $client.Connect($WakeAddr, $WakePort)
    $bytes = [System.Text.Encoding]::UTF8.GetBytes('restore')
    $client.GetStream().Write($bytes, 0, $bytes.Length)
    $client.Close()
  } catch {}
}
$script:lastTitleClickAt = [DateTime]::MinValue
$titleClickMouseUp = {
  if ($script:dragMoved) {
    $script:lastTitleClickAt = [DateTime]::MinValue
    return
  }
  $now = [DateTime]::UtcNow
  if (($now - $script:lastTitleClickAt).TotalMilliseconds -le 420) {
    $script:lastTitleClickAt = [DateTime]::MinValue
    & $restoreMainWindow
    return
  }
  $script:lastTitleClickAt = $now
}
$form.Add_DoubleClick($restoreMainWindow)
$usage.Add_DoubleClick($restoreMainWindow)
$titleHost.Add_MouseUp($titleClickMouseUp)

function Update-TitleDisplay {
  $displayTitle = $script:fullTitle
  if (-not $script:titleHovering -and -not [string]::IsNullOrEmpty($displayTitle) -and $displayTitle.Length -gt $script:titleDisplayChars) {
    $displayTitle = $displayTitle.Substring(0, $script:titleDisplayChars)
  }
  $titleHost.DisplayText = $displayTitle
  $fullSize = [System.Windows.Forms.TextRenderer]::MeasureText($displayTitle, $titleHost.DisplayFont)
  $script:titleFullWidth = $fullSize.Width + [Math]::Round(8 * $script:dpiScale)
  if (-not $script:titleHovering -or $script:titleFullWidth -le $titleHost.Width) {
    $script:titleScrollX = 0
    $titleHost.OffsetX = 0
  }
  $titleHost.Invalidate()
}

function Update-TitleSlotWidth {
  if ([string]::IsNullOrEmpty($script:fullTitle)) {
    $script:titleSlotWidth = [Math]::Round(24 * $script:dpiScale)
    return
  }
  $sampleLength = [Math]::Min($script:titleDisplayChars, $script:fullTitle.Length)
  $sample = $script:fullTitle.Substring(0, $sampleLength)
  $script:titleSlotWidth = [System.Windows.Forms.TextRenderer]::MeasureText($sample, $titleHost.DisplayFont).Width
}

$titleScrollTimer = New-Object System.Windows.Forms.Timer
$titleScrollTimer.Interval = 16
$titleScrollTimer.Add_Tick({
  if (-not $script:titleHovering) { return }
  if ($script:titleFullWidth -le $titleHost.Width) { return }
  $script:titleScrollX -= 1
  $limit = -1 * ($script:titleFullWidth - $titleHost.Width + [Math]::Round(18 * $script:dpiScale))
  if ($script:titleScrollX -lt $limit) {
    $script:titleScrollX = 0
  }
  $titleHost.OffsetX = $script:titleScrollX
  $titleHost.Invalidate()
})
$titleHoverEnter = {
  $script:titleHovering = $true
  Update-TitleDisplay
  if ($script:titleFullWidth -le $titleHost.Width) {
    $script:titleHovering = $false
    Update-TitleDisplay
    return
  }
  $script:titleScrollX = 0
  $titleHost.OffsetX = 0
  $titleScrollTimer.Start()
}
$titleHoverLeave = {
  $hostBounds = $titleHost.RectangleToScreen($titleHost.ClientRectangle)
  if ($hostBounds.Contains([System.Windows.Forms.Cursor]::Position)) { return }
  $script:titleHovering = $false
  $script:titleScrollX = 0
  $titleScrollTimer.Stop()
  $titleHost.OffsetX = 0
  Update-TitleDisplay
}
$titleHost.Add_MouseEnter($titleHoverEnter)
$titleHost.Add_MouseLeave($titleHoverLeave)

function Layout-FloatingBar {
  $pad = [Math]::Round(20 * $script:dpiScale)
  Update-TitleSlotWidth
  $targetWidth = [Math]::Max($script:barWidth, $script:titleSlotWidth + $script:usageSlotWidth + $pad)
  $oldRight = $form.Right
  $form.Width = $targetWidth
  $titleHost.Left = [Math]::Round(10 * $script:dpiScale)
  $titleHost.Top = [Math]::Floor(($form.Height - $titleHost.Height) / 2)
  $titleHost.Width = $script:titleSlotWidth
  Update-TitleDisplay
  $usage.Left = $targetWidth - $script:usageSlotWidth - [Math]::Round(10 * $script:dpiScale)
  $usage.Top = [Math]::Floor(($form.Height - $usage.Height) / 2)
  $usage.Width = $script:usageSlotWidth
  $bounds = Get-FloatingWorkingArea
  if ($script:dragging) {
    return
  }
  if ($oldRight -gt 0) {
    $form.Left = $oldRight - $form.Width
  }
  if ($form.Left -lt $bounds.Left) { $form.Left = $bounds.Left }
  if ($form.Top -lt $bounds.Top) { $form.Top = $bounds.Top }
  if ($form.Right -gt $bounds.Right) { $form.Left = $bounds.Right - $form.Width }
  if ($form.Bottom -gt $bounds.Bottom) { $form.Top = $bounds.Bottom - $form.Height }
}

function Sync-FloatingDpi {
  param([double]$ReportedDpi = 0)
  $nextScale = if ($ReportedDpi -gt 0) { $ReportedDpi / 96.0 } else { [DpiAwareness]::GetWindowScale($form.Handle) }
  if ($nextScale -le 0 -or [Math]::Abs($nextScale - $script:dpiScale) -lt 0.01) { return }

  $script:dpiScale = $nextScale
  $oldTitleFont = $titleHost.DisplayFont
  $oldUsageFont = $usage.DisplayFont
  $titleHost.DisplayFont = New-FloatingDisplayFont
  $usage.DisplayFont = New-FloatingDisplayFont
  try { $oldTitleFont.Dispose() } catch {}
  try { $oldUsageFont.Dispose() } catch {}

  $textHostHeight = Get-FloatingTextHostHeight $titleHost.DisplayFont
  $titleHost.Height = $textHostHeight
  $usage.Height = $textHostHeight
  $form.Height = [Math]::Max([Math]::Round(30 * $script:dpiScale), $textHostHeight + [Math]::Round(12 * $script:dpiScale))
  $form.Padding = New-Object System.Windows.Forms.Padding([Math]::Round(10 * $script:dpiScale), 0, [Math]::Round(10 * $script:dpiScale), 0)
  $script:barWidth = [Math]::Round(160 * $script:dpiScale)
  $script:usageSlotWidth = [System.Windows.Forms.TextRenderer]::MeasureText('100%', $usage.DisplayFont).Width + [Math]::Round(2 * $script:dpiScale)
  $script:titleScrollX = 0
  $titleHost.OffsetX = 0
  Layout-FloatingBar

  if ($script:dragging) {
    $cursor = [System.Windows.Forms.Cursor]::Position
    $script:dragOffset = New-Object System.Drawing.Point(($cursor.X - $form.Left), ($cursor.Y - $form.Top))
  }
}

$form.Add_DpiScaleChanged({ Sync-FloatingDpi $form.PendingDpi })

function Get-UsageDisplayColor {
  param([string]$Text)
  $match = [regex]::Match($Text, '([0-9]+(?:\.[0-9]+)?)')
  if (-not $match.Success) {
    return [System.Drawing.Color]::FromArgb(117, 103, 183)
  }
  $value = [double]$match.Groups[1].Value
  if ($value -lt 35) {
    return [System.Drawing.Color]::FromArgb(191, 111, 135)
  }
  if ($value -lt 70) {
    return [System.Drawing.Color]::FromArgb(104, 123, 189)
  }
  return [System.Drawing.Color]::FromArgb(117, 103, 183)
}

function Update-FloatingContent {
  if (-not (Test-Path -LiteralPath $StatePath)) { return }
  $raw = Get-Content -LiteralPath $StatePath -Raw
  if ([string]::IsNullOrWhiteSpace($raw)) { return }
  $state = $raw | ConvertFrom-Json
  if ($state.title) {
    $script:fullTitle = [string]$state.title
    if (-not $script:titleHovering) { $script:titleScrollX = 0 }
    Update-TitleDisplay
  }
  if ($state.usage) {
    $usage.DisplayText = [string]$state.usage
    $usage.DisplayColor = Get-UsageDisplayColor $usage.DisplayText
    $usage.Invalidate()
  }
  Layout-FloatingBar
}

function Test-OwnerAlive {
  try {
    $owner = Get-Process -Id $OwnerPid -ErrorAction SilentlyContinue
    return $null -ne $owner
  } catch {
    return $false
  }
}

$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 1000
$timer.Add_Tick({
  if (-not (Test-OwnerAlive)) {
    $form.Close()
    return
  }
  Update-FloatingContent
  Ensure-FloatingVisible
})
$timer.Start()
$form.Add_Shown({
  Update-FloatingContent
  Ensure-FloatingVisible
})
$form.Add_FormClosed({
  $timer.Stop()
  $titleScrollTimer.Stop()
  Remove-Item -LiteralPath $PidPath -Force
})

[System.Windows.Forms.Application]::EnableVisualStyles()
[System.Windows.Forms.Application]::Run($form)
"#
}

fn ensure_floating_script() -> Result<PathBuf, String> {
    let path = floating_script_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&path, floating_script_text()).map_err(|error| {
        format!(
            "Failed to write floating companion script '{}': {error}",
            path.display()
        )
    })?;
    Ok(path)
}

fn floating_process_id() -> Option<u32> {
    let text = fs::read_to_string(floating_pid_path()).ok()?;
    text.trim().parse::<u32>().ok()
}

fn process_running(pid: u32) -> bool {
    let mut command = Command::new("tasklist.exe");
    command.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let Ok(output) = command.output() else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let pid_field = format!(",\"{pid}\",");
    text.lines().any(|line| {
        line.to_ascii_lowercase().starts_with("\"powershell.exe\"") && line.contains(&pid_field)
    })
}

pub(crate) fn stop_external_floating_bar() {
    if let Some(pid) = floating_process_id() {
        let mut command = Command::new("taskkill.exe");
        command.args(["/PID", &pid.to_string(), "/T", "/F"]);
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        let _ = command.output();
    }
    let _ = fs::remove_file(floating_pid_path());
}

fn write_floating_state(payload: &FloatingBarPayload) -> Result<(), String> {
    fs::create_dir_all(locate_data_dir()).map_err(|error| error.to_string())?;
    let text = serde_json::to_string_pretty(payload).map_err(|error| error.to_string())?;
    fs::write(floating_state_path(), text).map_err(|error| {
        format!(
            "Failed to write floating state '{}': {error}",
            floating_state_path().display()
        )
    })
}

pub(crate) fn start_external_floating_bar(payload: &FloatingBarPayload) -> Result<(), String> {
    write_floating_state(payload)?;
    if let Some(pid) = floating_process_id() {
        if process_running(pid) {
            return Ok(());
        }
        let _ = fs::remove_file(floating_pid_path());
    }
    let script = ensure_floating_script()?;
    let mut command = Command::new("powershell.exe");
    command.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-STA", "-File"]);
    command.arg(script);
    command.arg("-StatePath");
    command.arg(floating_state_path());
    command.arg("-PidPath");
    command.arg(floating_pid_path());
    command.arg("-OwnerPid");
    command.arg(std::process::id().to_string());
    command.arg("-WakeAddr");
    command.arg("127.0.0.1");
    command.arg("-WakePort");
    command.arg("45421");
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let child = command
        .spawn()
        .map_err(|error| format!("Failed to start floating companion: {error}"))?;
    fs::write(floating_pid_path(), child.id().to_string()).map_err(|error| error.to_string())?;
    Ok(())
}

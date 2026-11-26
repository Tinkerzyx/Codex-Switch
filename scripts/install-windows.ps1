param([Parameter(Mandatory = $true)][string]$Installer)
$ErrorActionPreference = 'Stop'
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$installerPath = [IO.Path]::GetFullPath($Installer)
$bundleRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot 'codex-switch-app\src-tauri\target'))
if (-not $installerPath.StartsWith($bundleRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or -not (Test-Path -LiteralPath $installerPath -PathType Leaf)) {
    throw 'Installer must be an existing build artifact inside this repository.'
}
$version = (Get-Content -LiteralPath (Join-Path $repositoryRoot 'codex-switch-app\package.json') -Raw | ConvertFrom-Json).version
if ([IO.Path]::GetFileName($installerPath) -notlike "*_${version}_*-setup.exe") { throw 'Installer filename does not match the current version.' }

$dataPath = if ([string]::IsNullOrWhiteSpace($env:CODEX_SWITCH_DATA_DIR)) { Join-Path $env:LOCALAPPDATA 'Codex Switch' } else { $env:CODEX_SWITCH_DATA_DIR }
$dataPath = [IO.Path]::GetFullPath($dataPath)
$backupPath = Join-Path $env:LOCALAPPDATA ("Codex Switch Backups\upgrade-{0}-{1}" -f $version, (Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
$protectedFiles = @('profiles.json', 'settings.json', 'app-settings.json', 'cloudbase_session.json', 'cloudbase.json', 'login-recovery.json', 'switch-recovery.json')
$protectedDirs = @('profiles', 'backups', 'trash', 'web-sessions', 'five-hour-probes', 'codex_switch_data')

Get-Process -Name 'codex-switch-app' -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 300

# Keep profile data outside the installation directory during uninstall, even when
# an older installer placed the application and its data in the same directory.
$hashes = @{}
$files = @()
foreach ($name in $protectedFiles) {
    $path = Join-Path $dataPath $name
    if (Test-Path -LiteralPath $path -PathType Leaf) { $files += Get-Item -LiteralPath $path }
}
foreach ($name in $protectedDirs) {
    $path = Join-Path $dataPath $name
    if (Test-Path -LiteralPath $path -PathType Container) { $files += Get-ChildItem -LiteralPath $path -File -Recurse -Force }
}
foreach ($file in $files) {
    $relative = $file.FullName.Substring($dataPath.Length).TrimStart('\')
    $hashes[$relative] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
    $destination = Join-Path $backupPath $relative
    New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
    Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
}

function Restore-ProfileData {
    foreach ($relative in $hashes.Keys) {
        $destination = Join-Path $dataPath $relative
        $matches = (Test-Path -LiteralPath $destination -PathType Leaf) -and ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -eq $hashes[$relative])
        if (-not $matches) {
            New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $backupPath $relative) -Destination $destination -Force
        }
    }
}

$roots = @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall', 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall')
$installed = Get-ChildItem -Path $roots -ErrorAction SilentlyContinue | Get-ItemProperty | Where-Object { $_.DisplayName -eq 'Codex Switch' } | Select-Object -First 1
try {
    if ($installed -and $installed.UninstallString) {
        $uninstall = [regex]::Match($installed.UninstallString, '^\s*"([^"]+\.exe)"|^\s*(.+?\.exe)(?:\s|$)')
        $uninstaller = if ($uninstall.Groups[1].Success) { $uninstall.Groups[1].Value } else { $uninstall.Groups[2].Value }
        if (-not $uninstaller -or -not (Test-Path -LiteralPath $uninstaller -PathType Leaf)) { throw 'Registered uninstaller was not found.' }
        $installDirectory = Split-Path -Parent $uninstaller
        $process = Start-Process -FilePath $uninstaller -ArgumentList "/S _?=$installDirectory" -WindowStyle Hidden -PassThru -Wait
        if ($process.ExitCode -notin @(0, 3010)) { throw "Silent uninstall failed: $($process.ExitCode)" }
    }
    $process = Start-Process -FilePath $installerPath -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -notin @(0, 3010)) { throw "Silent installation failed: $($process.ExitCode)" }
} finally { Restore-ProfileData }

foreach ($relative in $hashes.Keys) {
    if ((Get-FileHash -LiteralPath (Join-Path $dataPath $relative) -Algorithm SHA256).Hash -ne $hashes[$relative]) { throw "Profile verification failed: $relative" }
}
$installed = Get-ChildItem -Path $roots -ErrorAction SilentlyContinue | Get-ItemProperty | Where-Object { $_.DisplayName -eq 'Codex Switch' } | Select-Object -First 1
if (-not $installed -or $installed.DisplayVersion -ne $version) { throw "Installed version could not be verified as $version." }
$installDirectory = if ($installed.InstallLocation) { $installed.InstallLocation.Trim().Trim('"') } else { Split-Path -Parent ([regex]::Match($installed.UninstallString, '"([^"]+\.exe)"').Groups[1].Value) }
$executable = Join-Path $installDirectory 'codex-switch-app.exe'
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) { throw 'Installed executable was not found.' }
if ((Get-Item -LiteralPath $executable).VersionInfo.ProductVersion -notlike "$version*") { throw 'Executable product version does not match the installer.' }
Start-Process -FilePath $executable -WindowStyle Hidden
Write-Output ("Installed Codex Switch {0}; verified {1} preserved data files. Backup: {2}" -f $version, $hashes.Count, $backupPath)

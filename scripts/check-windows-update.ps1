param([Parameter(Mandatory = $true)][string]$Installer)
$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ("Reclaim update בדיקה " + [guid]::NewGuid())
$installed = Join-Path $root 'Installed app'
$stage = Join-Path $root 'Updater cache'
$previousData = $env:LOCALAPPDATA
New-Item -ItemType Directory -Path $stage -Force | Out-Null
try {
    $env:LOCALAPPDATA = Join-Path $root 'Profile'
    $setup = (Resolve-Path $Installer).Path
    $first = Start-Process $setup -ArgumentList "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /DIR=`"$installed`"" -Wait -PassThru
    if ($first.ExitCode -ne 0) { throw "Initial installation failed: $($first.ExitCode)" }
    $target = Join-Path $installed 'reclaim.exe'
    if (-not (Test-Path $target)) { throw 'Setup did not install Reclaim' }
    Copy-Item $target (Join-Path $stage 'reclaim-update.exe')
    Copy-Item $setup (Join-Path $stage 'reclaim-windows-x86_64-setup.exe')
    $version = [regex]::Match((Get-Content Cargo.toml -Raw), '(?m)^version = "([^"]+)"').Groups[1].Value
    $launcherFile = Join-Path $root 'handoff.ps1'
    @'
param($Stage, $Target, $Version)
$ErrorActionPreference = 'Stop'
$package = Join-Path $Stage 'reclaim-windows-x86_64-setup.exe'
$manifest = Join-Path $Stage 'update.json'
@{
    schema = 1; parent = $PID; target = $Target; version = $Version
    asset = 'reclaim-windows-x86_64-setup.exe'
    url = "https://github.com/sudo-py-dev/reclaim/releases/download/v$Version/reclaim-windows-x86_64-setup.exe"
    digest = (Get-FileHash $package -Algorithm SHA256).Hash.ToLowerInvariant()
    size = (Get-Item $package).Length
} | ConvertTo-Json | Set-Content $manifest -Encoding utf8NoBOM
$helper = Join-Path $Stage 'reclaim-update.exe'
Start-Process $helper -ArgumentList "--apply-update `"$manifest`"" | Out-Null
for ($attempt = 0; $attempt -lt 300; $attempt++) {
    if (Test-Path (Join-Path $Stage 'failed')) { throw (Get-Content (Join-Path $Stage 'failed') -Raw) }
    if (Test-Path (Join-Path $Stage 'ready')) { exit 0 }
    Start-Sleep -Milliseconds 100
}
throw 'Updater did not acknowledge the handoff'
'@ | Set-Content $launcherFile -Encoding utf8NoBOM
    # Wait only for the old process, not its surviving updater/app descendants.
    $launcher = Start-Process pwsh -ArgumentList "-NoProfile -File `"$launcherFile`" `"$stage`" `"$target`" `"$version`"" -PassThru
    if (-not $launcher.WaitForExit(40000)) { throw 'Updater handoff timed out' }
    if ($launcher.ExitCode -ne 0) { throw 'Updater handoff failed' }
    $log = Join-Path $env:LOCALAPPDATA 'Reclaim/logs/reclaim.log'
    $finished = $false
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if (Test-Path (Join-Path $stage 'failed')) { throw (Get-Content (Join-Path $stage 'failed') -Raw) }
        if ((Test-Path (Join-Path $stage 'complete')) -and (Test-Path $log)) {
            if ((Get-Content $log -Raw) -match 'First application frame completed') { $finished = $true; break }
        }
        Start-Sleep -Seconds 1
    }
    if (-not $finished) { throw 'Silent update did not complete and relaunch the app' }
    Write-Output 'Silent update waited for shutdown, installed, and relaunched Reclaim successfully.'
} finally {
    Get-Process -Name reclaim,reclaim-update -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith($root) } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    if (Test-Path (Join-Path $stage 'installer.log')) { Get-Content (Join-Path $stage 'installer.log') | Write-Output }
    $uninstaller = Join-Path $installed 'unins000.exe'
    if (Test-Path $uninstaller) { Start-Process $uninstaller -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -Wait }
    $env:LOCALAPPDATA = $previousData
    Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
}

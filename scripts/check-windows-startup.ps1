param([Parameter(Mandatory = $true)][string]$Binary)
$ErrorActionPreference = 'Stop'
$previousData = $env:LOCALAPPDATA
$testData = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
$process = $null
try {
    $env:LOCALAPPDATA = $testData
    $process = Start-Process -FilePath (Resolve-Path $Binary).Path -PassThru
    $log = Join-Path $testData 'Reclaim/logs/reclaim.log'
    $ready = $false
    for ($attempt = 0; $attempt -lt 60; $attempt++) {
        Start-Sleep -Seconds 1
        if ($process.HasExited) { throw "Reclaim exited during startup: $($process.ExitCode)" }
        if (Test-Path $log) {
            $text = Get-Content $log -Raw
            if ($text -match 'ERROR:') { throw 'Reclaim reported a startup error' }
            if ($text -match 'First application frame completed') { $ready = $true; break }
        }
    }
    if (-not $ready) { throw 'Reclaim did not render its first frame within 60 seconds' }
    Start-Sleep -Seconds 2
    if ($process.HasExited) { throw 'Reclaim exited immediately after its first frame' }
    Write-Output 'Reclaim opened and rendered its first application frame.'
} finally {
    if (Test-Path "$testData/Reclaim/logs/reclaim.log") {
        Get-Content "$testData/Reclaim/logs/reclaim.log" | Write-Output
    }
    if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    $env:LOCALAPPDATA = $previousData
    Remove-Item $testData -Recurse -Force -ErrorAction SilentlyContinue
}

param([Parameter(Mandatory = $true)][string]$Binary)
$ErrorActionPreference = 'Stop'

# Inspect imports: CI runners have the redistributable installed, so launching
# alone would conceal the missing-runtime failure seen on clean Windows PCs.
$vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
$installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $installation) { throw 'Visual C++ build tools were not found' }
$dumpbin = Get-ChildItem "$installation/VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe" |
    Sort-Object FullName -Descending | Select-Object -First 1
if (-not $dumpbin) { throw 'dumpbin.exe was not found' }
$imports = & $dumpbin.FullName /DEPENDENTS (Resolve-Path $Binary).Path
if ($LASTEXITCODE -ne 0) { throw 'Unable to inspect Windows executable imports' }
$imports | Write-Output
if ($imports -match '(?i)\b(vcruntime\d+[^\s]*|msvcp\d+[^\s]*|concrt\d+[^\s]*|ucrtbase|api-ms-win-crt-[^\s]*)\.dll\b') {
    throw 'Windows executable depends on an external Visual C++ runtime; build with crt-static'
}
Write-Output 'Windows executable does not require the Visual C++ Redistributable.'

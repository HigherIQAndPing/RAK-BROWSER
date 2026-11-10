[CmdletBinding()]
param(
    [string]$Executable = 'target/release/rak.exe',
    [string]$OutputDirectory = 'dist'
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repo
$exe = (Resolve-Path -LiteralPath $Executable).Path

# PE-Maschinenkennung pruefen, statt eine andere Architektur als x64 auszuliefern.
$bytes = [System.IO.File]::ReadAllBytes($exe)
if ($bytes.Length -lt 64 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
    throw 'Die Eingabedatei ist keine Windows-EXE.'
}
$peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
if ($peOffset -lt 0 -or $peOffset -gt ($bytes.Length - 6)) {
    throw 'Ungueltiger PE-Header.'
}
if ([BitConverter]::ToUInt32($bytes, $peOffset) -ne 0x00004550 -or
    [BitConverter]::ToUInt16($bytes, $peOffset + 4) -ne 0x8664) {
    throw 'Das Windows-Paket erfordert eine x64-EXE.'
}

# Native Abhaengigkeiten pruefen: WebView2Loader und die MSVC-CRT muessen
# statisch eingebunden sein. Windows-System-DLLs bleiben Systemvoraussetzungen.
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) {
    throw 'vswhere fehlt. Fuer die Paketpruefung werden Visual Studio C++ Build Tools benoetigt.'
}
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $vs) { throw 'Keine MSVC-x64-Build-Tools gefunden.' }
$toolsRoot = Join-Path $vs 'VC/Tools/MSVC'
$dumpbin = Get-ChildItem -LiteralPath $toolsRoot -Directory |
    Sort-Object Name -Descending |
    ForEach-Object { Join-Path $_.FullName 'bin/Hostx64/x64/dumpbin.exe' } |
    Where-Object { Test-Path -LiteralPath $_ } |
    Select-Object -First 1
if (-not $dumpbin) { throw 'dumpbin.exe fuer x64 wurde nicht gefunden.' }
$dependencies = & $dumpbin /nologo /DEPENDENTS $exe 2>&1
if ($LASTEXITCODE -ne 0) { throw "DLL-Pruefung fehlgeschlagen: $dependencies" }
$dependencies | ForEach-Object { Write-Host $_ }
if (($dependencies -join "`n") -match '(?i)\b(?:vcruntime\d+(?:_\d+)?|msvcp\d+(?:_\d+)?|concrt\d+|WebView2Loader)\.dll\b') {
    throw 'Die EXE haengt von einer externen MSVC-CRT oder WebView2Loader.dll ab. Statische Verknuepfung pruefen.'
}

$stage = Join-Path $OutputDirectory 'RAK-windows-x64'
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Copy-Item -LiteralPath $exe -Destination (Join-Path $stage 'RAK.exe')
foreach ($name in @('start.bat', 'README.txt', 'setup-webview2.bat')) {
    Copy-Item -LiteralPath (Join-Path $repo $name) -Destination (Join-Path $stage $name)
}
$archive = Join-Path $OutputDirectory 'RAK-windows-x64.zip'
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -Force
Write-Host "Portables Windows-x64-Paket erstellt: $archive"
Write-Host 'Enthaelt RAK.exe, start.bat, README.txt und den optionalen WebView2-Link.'
Write-Host 'WebView2 Runtime wird nicht gebuendelt und muss auf dem Zielsystem vorhanden sein.'

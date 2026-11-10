@echo off
setlocal
cd /d "%~dp0"
if exist "%~dp0RAK.exe" (
  start "" "%~dp0RAK.exe"
  exit /b 0
)
if exist "%~dp0target\release\rak.exe" (
  start "" "%~dp0target\release\rak.exe"
  exit /b 0
)
echo.
echo RAK.exe wurde nicht gefunden. Rust ist zum Starten NICHT erforderlich.
echo Lade RAK-windows-x64.zip unter GitHub Releases herunter:
echo https://github.com/HigherIQAndPing/RAK-BROWSER/releases
echo Das Repository ist privat: Melde dich mit einem berechtigten GitHub-Konto an.
echo Entpacke das gesamte ZIP und starte RAK.exe oder start.bat aus diesem Ordner.
echo Falls es noch kein Release gibt: Unter Actions den neuesten erfolgreichen
echo Lauf von "Windows portable" oeffnen und das Artefakt RAK-windows-x64 laden.
echo Dieses Skript installiert weder Rust noch WebView2.
start "" "https://github.com/HigherIQAndPing/RAK-BROWSER/releases"
pause
exit /b 1

@echo off
chcp 65001 >nul
cd /d "%~dp0"
where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust wurde nicht gefunden. Bitte zuerst von https://rustup.rs installieren,
  echo danach alle Fenster schliessen und diese Datei erneut starten.
  pause
  exit /b 1
)
echo Baue RAK im optimierten Release-Modus - das dauert beim ersten Mal einige Minuten ...
cargo build --release --locked
if errorlevel 1 (
  echo.
  echo Der Build ist fehlgeschlagen. Bitte die Fehlermeldung oben kopieren.
  pause
  exit /b 1
)
copy /y "target\release\rak.exe" "RAK.exe" >nul
if errorlevel 1 (
  echo Die EXE konnte nicht in den Projektordner kopiert werden. Ist RAK.exe gerade geoeffnet?
  pause
  exit /b 1
)
echo.
echo Fertig! Starte jetzt RAK.exe per Doppelklick oder mit start.bat.
pause

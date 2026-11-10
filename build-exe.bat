@echo off
setlocal
cd /d "%~dp0"
echo Dieses Skript ist NUR fuer Entwickler, die RAK selbst kompilieren wollen.
echo Zum normalen Benutzen die fertige RAK-windows-x64.zip herunterladen.
echo.
where cargo >nul 2>nul
if errorlevel 1 (
  echo Cargo wurde nicht gefunden. Nur fuer einen eigenen Build:
  echo Rust von https://rustup.rs sowie Visual Studio C++ Build Tools installieren.
  echo Benoetigt werden MSVC C++ x64/x86 Build Tools und ein Windows SDK.
  echo Danach ein neues Terminal oeffnen. Dieses Skript installiert nichts.
  pause
  exit /b 1
)
where rustc >nul 2>nul
if errorlevel 1 (
  echo rustc wurde nicht gefunden. Bitte die Rust-Installation pruefen.
  pause
  exit /b 1
)
rustc -vV | findstr /x /c:"host: x86_64-pc-windows-msvc" >nul
if errorlevel 1 (
  echo Fuer diesen Build wird Rust fuer Windows x64 mit MSVC benoetigt.
  echo Entwickler koennen die Toolchain explizit mit rustup waehlen:
  echo   rustup toolchain install stable-x86_64-pc-windows-msvc
  echo   rustup default stable-x86_64-pc-windows-msvc
  pause
  exit /b 1
)
rem Der erwartete Ausgabeordner gilt unabhaengig von globalen Cargo-Variablen.
set "CARGO_BUILD_TARGET=x86_64-pc-windows-msvc"
set "CARGO_TARGET_DIR=%~dp0target"
echo Baue RAK im optimierten Release-Modus mit statischer MSVC-C-Laufzeit ...
call cargo build --release --locked --target-dir "%~dp0target"
if errorlevel 1 (
  echo.
  echo Der Build ist fehlgeschlagen. Bitte die Fehlermeldung oben pruefen.
  pause
  exit /b 1
)
copy /y "%~dp0target\x86_64-pc-windows-msvc\release\rak.exe" "%~dp0RAK.exe" >nul
if errorlevel 1 (
  echo Die EXE konnte nicht kopiert werden. Ist RAK.exe gerade geoeffnet?
  pause
  exit /b 1
)
echo.
echo Fertig! RAK.exe oder start.bat doppelklicken. Rust wird dann nicht gebraucht.
echo Microsoft Edge WebView2 Runtime muss auf dem Zielsystem vorhanden sein.
pause
exit /b 0

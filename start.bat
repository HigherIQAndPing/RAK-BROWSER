@echo off
cd /d "%~dp0"
if exist "RAK.exe" (
  start "" "RAK.exe"
) else (
  echo RAK.exe gibt es noch nicht. Bitte zuerst build-exe.bat ausfuehren.
  pause
)

@echo off
setlocal
echo RAK benoetigt Microsoft Edge WebView2 Runtime.
echo Unter Windows 11 ist sie normalerweise schon vorhanden.
echo Dieses optionale Skript oeffnet nur die offizielle Microsoft-Downloadseite.
echo Es laedt nichts selbst herunter und installiert nichts automatisch.
echo Falls die Runtime fehlt: Evergreen Standalone Installer - x64 waehlen,
echo oder den Evergreen Bootstrapper von Microsoft verwenden.
echo.
start "" "https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section"
exit /b 0

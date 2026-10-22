@echo off
rem Makes Steam show Survarium while you play on Skhid (skhid-steam.ps1).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0skhid-steam.ps1" %*
pause

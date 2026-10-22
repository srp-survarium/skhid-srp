@echo off
rem Undoes skhid-steam.bat: Steam starts Survarium as before.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0skhid-steam.ps1" -Remove %*
pause

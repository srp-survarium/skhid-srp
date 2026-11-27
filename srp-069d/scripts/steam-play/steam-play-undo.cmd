@echo off
rem Undo steam-play-setup: deletes %LOCALAPPDATA%\Survarium-Steam\launcher.cfg (only that file, only if the setup wrote it).
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0steam-play-undo.ps1" %*
echo.
pause

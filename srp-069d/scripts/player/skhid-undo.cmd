@echo off
rem Undo skhid-setup: deletes the start file Skhid.cmd (only that file, only if skhid-setup wrote it).
rem Usage: skhid-undo.cmd [-Folder <folder>]   (default: your desktop)
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0skhid-undo.ps1" %*
echo.
pause

@echo off
rem Survarium 0.69d on a Skhid server without Steam: writes the start file Skhid.cmd on your desktop.
rem Usage: skhid-setup.cmd -Server <address> [-Language english] [-Game <game folder>] [-Folder <folder>] [-UpdatePort 65001]
rem Double-clicked without arguments it asks for the address. Details: skhid-setup.ps1, docs/guide-player.md.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0skhid-setup.ps1" %*
echo.
pause

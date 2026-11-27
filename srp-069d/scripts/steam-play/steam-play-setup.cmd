@echo off
rem Survarium on an srp server from the Steam Play button: writes %LOCALAPPDATA%\Survarium-Steam\launcher.cfg.
rem Usage: steam-play-setup.cmd -Server <address> [-Language russian] [-Port 65000] [-UpdatePort 65001]
rem Without -Language the game uses the language set in Steam (Properties -> General -> Language).
rem Double-clicked without arguments it asks for the address. Details: steam-play-setup.ps1, docs/notes/steam.md.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0steam-play-setup.ps1" %*
echo.
pause

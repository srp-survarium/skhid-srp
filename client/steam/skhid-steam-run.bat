@echo off
rem Steam starts this (Play on Survarium, or the "Survarium" shortcut). It
rem starts Skhid's launcher as Skhid.bat does, then waits until the game has
rem closed: Steam shows Survarium as played while this runs. The arguments
rem Steam adds (the original game's command) are not used.
set "X86=%~dp0game\binaries\x86"
if not exist "%X86%\launcher_skhid.exe" set "X86=%~dp0..\game\binaries\x86"
if not exist "%X86%\launcher_skhid.exe" goto missing
start "" /d "%X86%" "%X86%\launcher_skhid.exe"
powershell -NoProfile -ExecutionPolicy Bypass -WindowStyle Minimized -File "%~dp0skhid-steam.ps1" -WaitForGame
exit /b 0
:missing
echo launcher_skhid.exe was not found. Put the skhid-steam files in the
echo Skhid game folder, beside Skhid.bat.
pause
exit /b 1

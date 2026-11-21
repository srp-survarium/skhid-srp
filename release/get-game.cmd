@echo off
setlocal
rem Downloads Skhid revision @@REVISION@@ into Survarium beside this file, or under the optional folder argument.
set "SKHID_DOWNLOAD_SCRIPT=%~f0"
set "SKHID_DOWNLOAD_PARENT=%~1"
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -Command "$s=[IO.File]::ReadAllText($env:SKHID_DOWNLOAD_SCRIPT); $m='#'+'__SKHID_POWERSHELL__'; & ([ScriptBlock]::Create($s.Substring($s.IndexOf($m)+$m.Length)))"
set "SKHID_DOWNLOAD_RESULT=%ERRORLEVEL%"
if not "%SKHID_DOWNLOAD_RESULT%"=="0" echo Download did not finish. Run this file again to retry or resume.
pause
exit /b %SKHID_DOWNLOAD_RESULT%
#__SKHID_POWERSHELL__

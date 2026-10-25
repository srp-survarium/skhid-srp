@echo off
rem Skhid-network-setup.cmd: lets the Skhid game's three programs out through
rem Windows Firewall and, if you say yes, excludes the game's program folder
rem (game\binaries\x86) from Microsoft Defender's scans. It turns nothing off: the firewall and
rem Defender stay on. It belongs beside Skhid.bat, in the folder that holds
rem the folder named game.
rem
rem   Skhid-network-setup.cmd                add the rules (asks for administrator rights)
rem   Skhid-network-setup.cmd --undo         remove exactly what it added
rem   Skhid-network-setup.cmd --dry-run      print the commands, change nothing
rem   --defender-yes or --defender-no        answer the Defender question beforehand
rem
rem Running it again adds no second rule: each rule is removed by its name
rem and added again (so a moved game folder gets its new paths).
setlocal EnableExtensions DisableDelayedExpansion
set "MODE=add"
set "DRY="
set "ELEVATED="
set "DEFENDER=ask"
:args
if "%~1"=="" goto args_done
if /i "%~1"=="--undo" (set "MODE=undo") else ^
if /i "%~1"=="/undo" (set "MODE=undo") else ^
if /i "%~1"=="--dry-run" (set "DRY=1") else ^
if /i "%~1"=="--defender-yes" (set "DEFENDER=yes") else ^
if /i "%~1"=="--defender-no" (set "DEFENDER=no") else ^
if /i "%~1"=="--elevated" (set "ELEVATED=1") else goto bad_option
shift
goto args
:args_done

set "TOP=%~dp0"
set "TOP=%TOP:~0,-1%"
set "X86=%TOP%\game\binaries\x86"
set "MARKER=%TOP%\Skhid-network-setup.defender"
set "PROGRAMS=launcher_skhid.exe survarium_skhid.exe updater_skhid.exe"
if not exist "%X86%\survarium_skhid.exe" goto wrong_place

echo Skhid network setup
echo ===================
if "%MODE%"=="undo" (
    echo Removes the Windows Firewall rules named "Skhid ..." that this script
    echo added, and the Microsoft Defender exclusion of the game's program
    echo folder if this script added it.
) else (
    echo Adds Windows Firewall rules that let the game's three programs, in
    echo game\binaries\x86, connect out, UDP and TCP:
    echo   launcher_skhid.exe, survarium_skhid.exe, updater_skhid.exe
    echo Then it asks whether to exclude that folder from Microsoft Defender's
    echo scans. It does not turn the firewall or Defender off.
)
echo Game folder: "%TOP%"
echo.
if defined DRY (
    echo Dry run: the commands are printed, nothing is changed.
    echo.
)
if defined DRY if not defined ELEVATED call :elevation
if defined DRY goto main

rem Windows Firewall rules need administrator rights.
fltmc >nul 2>&1
if not errorlevel 1 goto main
if defined ELEVATED goto not_admin
echo Windows Firewall rules need administrator rights: Windows asks for them
echo next (User Account Control). The script then runs again in a new window.
echo.
call :elevation
powershell -NoProfile -ExecutionPolicy Bypass -Command "try { Start-Process -FilePath $env:ComSpec -ArgumentList $env:SKHID_CMDLINE -Verb RunAs -ErrorAction Stop } catch { exit 1 }"
if errorlevel 1 goto declined
exit /b 0

rem The elevated run's command line, for cmd.exe: cmd /c takes the outer
rem quotes off what follows it, so the script's quoted path and its options
rem are in one more pair (a folder with a space and a bracket or an &).
rem With --dry-run it is printed, and is a dry run's.
:elevation
set "SKHID_SELF=%~f0"
set "SKHID_ARGS=--elevated"
if "%MODE%"=="undo" set "SKHID_ARGS=%SKHID_ARGS% --undo"
if "%DEFENDER%"=="yes" set "SKHID_ARGS=%SKHID_ARGS% --defender-yes"
if "%DEFENDER%"=="no" set "SKHID_ARGS=%SKHID_ARGS% --defender-no"
if defined DRY set "SKHID_ARGS=%SKHID_ARGS% --dry-run"
set "SKHID_CMDLINE=/d /c ""%SKHID_SELF%" %SKHID_ARGS%""
if not defined DRY exit /b 0
setlocal EnableDelayedExpansion
echo [dry-run] elevate: cmd.exe !SKHID_CMDLINE!
endlocal
exit /b 0

:main
if "%MODE%"=="undo" goto undo

rem --- add -------------------------------------------------------------
for %%P in (%PROGRAMS%) do call :allow %%P
echo.
call :blocking
call :defender_add
echo.
echo Done. To remove all of it: Skhid-network-setup.cmd --undo
goto end

rem --- undo ------------------------------------------------------------
:undo
for %%P in (%PROGRAMS%) do call :remove %%P
call :defender_remove
echo.
echo Done.
goto end

rem The two rules of one program: out, TCP and UDP (the answers come back
rem without a rule; nothing else comes in).
:allow
for %%T in (TCP UDP) do call :rule %1 out %%T
echo Firewall: allowed %1 out (TCP and UDP)
exit /b 0

:rule
set "NAME=Skhid %~1 %3 %2"
call :run_quiet netsh advfirewall firewall delete rule name="%NAME%"
call :run netsh advfirewall firewall add rule name="%NAME%" dir=%2 action=allow program="%X86%\%~1" protocol=%3 enable=yes profile=any
if errorlevel 1 echo   FAILED: the rule "%NAME%" could not be added.
exit /b 0

:remove
for %%T in (TCP UDP) do call :run_quiet netsh advfirewall firewall delete rule name="Skhid %~1 %%T out"
echo Firewall: removed the rules "Skhid %~1 ..."
exit /b 0

rem A rule that blocks one of the programs wins over the ones above; Windows
rem makes one when its "allow access" question is cancelled. Said, not removed:
rem it is not this script's.
:blocking
if defined DRY (
    echo [dry-run] look for firewall rules that block the three programs
    exit /b 0
)
set "SKHID_X86=%X86%"
powershell -NoProfile -ExecutionPolicy Bypass -Command "foreach ($p in 'launcher_skhid.exe','survarium_skhid.exe','updater_skhid.exe') { $path = Join-Path $env:SKHID_X86 $p; Get-NetFirewallApplicationFilter -Program $path -ErrorAction SilentlyContinue | Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.Action -eq 'Block' -and $_.Enabled -eq 'True' } | ForEach-Object { Write-Output ('WARNING: the firewall rule [' + $_.DisplayName + '] blocks ' + $p + '. Delete it in Windows Defender Firewall, Advanced settings, Outbound Rules.') } }" 2>nul
exit /b 0

rem The exclusion is of game\binaries\x86 alone, where the programs that get
rem flagged are (never the folder the archive was unpacked into: Downloads).
:defender_add
set "SKHID_EXCLUDED=%X86%"
if "%TOP:~2%"=="" (
    echo Defender: not offered, the game is at the top of a drive. Move it into a folder first.
    exit /b 0
)
if "%DEFENDER%"=="no" goto defender_skipped
if "%DEFENDER%"=="yes" goto defender_yes
echo.
echo Microsoft Defender can falsely flag the game's patched files (survarium_skhid.exe,
echo bugtrap.dll, the updater) and block or delete them. An exclusion of the game's
echo program folder only stops that:
echo   "%X86%"
choice /c YN /m "Add the Defender exclusion"
if errorlevel 2 goto defender_skipped
:defender_yes
if defined DRY (
    echo [dry-run] powershell Add-MpPreference -ExclusionPath "%X86%"
    echo [dry-run] write "%MARKER%"
    exit /b 0
)
powershell -NoProfile -ExecutionPolicy Bypass -Command "try { if ((Get-MpPreference -ErrorAction Stop).ExclusionPath -contains $env:SKHID_EXCLUDED) { exit 3 }; Add-MpPreference -ExclusionPath $env:SKHID_EXCLUDED -ErrorAction Stop } catch { exit 1 }"
if errorlevel 3 (
    echo Defender: this folder was excluded already: nothing changed.
    exit /b 0
)
if errorlevel 1 (
    echo Defender: the exclusion could not be added. Another antivirus may be in
    echo charge: add the folder to its exclusions yourself.
    exit /b 0
)
rem The path as it is, an & in it no command: delayed expansion.
setlocal EnableDelayedExpansion
> "%MARKER%" echo(!SKHID_EXCLUDED!
endlocal
echo Defender: game\binaries\x86 is excluded from scans.
exit /b 0
:defender_skipped
echo Defender: no exclusion added.
exit /b 0

:defender_remove
if not exist "%MARKER%" (
    echo Defender: this script added no exclusion here: nothing to remove.
    exit /b 0
)
set /p "SKHID_EXCLUDED=" < "%MARKER%"
if defined DRY (
    echo [dry-run] powershell Remove-MpPreference -ExclusionPath "%SKHID_EXCLUDED%"
    echo [dry-run] delete "%MARKER%"
    exit /b 0
)
powershell -NoProfile -ExecutionPolicy Bypass -Command "try { Remove-MpPreference -ExclusionPath $env:SKHID_EXCLUDED -ErrorAction Stop } catch { exit 1 }"
if errorlevel 1 (
    echo Defender: the exclusion of "%SKHID_EXCLUDED%" could not be removed.
    exit /b 0
)
del "%MARKER%"
echo Defender: the exclusion of "%SKHID_EXCLUDED%" is removed.
exit /b 0

rem A command, or with --dry-run its words.
:run
if defined DRY goto run_dry
%*
exit /b
:run_quiet
if defined DRY goto run_dry
%* >nul 2>&1
exit /b 0
:run_dry
echo [dry-run] %*
exit /b 0

:bad_option
echo Unknown option: %1
echo Use: Skhid-network-setup.cmd [--undo] [--dry-run] [--defender-yes ^| --defender-no]
exit /b 2

:wrong_place
echo survarium_skhid.exe was not found in
echo   "%X86%"
echo Put this file beside Skhid.bat, in the folder that holds the folder named
echo game, and run it from there.
goto end

:not_admin
echo Not done: this window has no administrator rights.
goto end

:declined
echo Not done: Windows did not give administrator rights.
goto end

:end
echo.
if not defined DRY pause
exit /b 0

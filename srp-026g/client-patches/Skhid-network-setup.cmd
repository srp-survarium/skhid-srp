@echo off
rem Skhid-network-setup.cmd: lets the Skhid game's three programs out through
rem Windows Firewall. It turns nothing off: the firewall stays on. It belongs
rem beside Skhid.bat, in the folder that holds the folder named game. It
rem speaks Russian when the Windows display language is Russian, Ukrainian
rem or Belarusian, English otherwise. The Microsoft Defender exclusion it
rem once asked about is Skhid-defender-setup.cmd's now.
rem
rem   Skhid-network-setup.cmd                add the rules (asks for administrator rights)
rem   Skhid-network-setup.cmd --undo         remove exactly what it added
rem   Skhid-network-setup.cmd --dry-run      print the commands, change nothing
rem
rem Started without administrator rights, it asks Windows for them (the User
rem Account Control prompt) and runs again in a new window, elevated.
rem
rem Running it again adds no second rule: each rule is removed by its name
rem and added again (so a moved game folder gets its new paths).
rem
rem The code page: this file is kept in UTF-8, and make_client_026g.py ships
rem it in code page 866 (DOS Cyrillic). cmd reads each line of a script in the
rem console's code page at the time, so it switches the console to 866 here,
rem before the first line with a Russian letter, and back at the end: the
rem text is right on a Russian Windows (866 already) and on any other. (Not
rem 65001: Windows 7's cmd stops reading a script in it.) Every line above
rem the chcp is ASCII.
setlocal EnableExtensions DisableDelayedExpansion
set "SKHID_CP="
for /f "tokens=2 delims=:" %%C in ('chcp') do set "SKHID_CP=%%C"
if defined SKHID_CP set "SKHID_CP=%SKHID_CP: =%"
if defined SKHID_CP set "SKHID_CP=%SKHID_CP:.=%"
chcp 866 >nul 2>&1
rem The language: the Windows display language, as PowerShell has it (no
rem PowerShell, or no answer: English). A line a player reads is there
rem twice, %RU% Russian and %EN% English: one of the two is an echo, the
rem other a rem. SKHID_RU tells PowerShell the same.
set "SKHID_RU="
for /f %%L in ('powershell -NoProfile -NonInteractive -Command "(Get-UICulture).TwoLetterISOLanguageName" 2^>nul') do for %%R in (ru uk be) do if /i "%%L"=="%%R" set "SKHID_RU=1"
set "RU=rem"
set "EN=echo"
if defined SKHID_RU set "RU=echo"
if defined SKHID_RU set "EN=rem"
set "RC=0"
rem The script's own path and folder, before any shift (a shift moves %0 too).
set "SKHID_SELF=%~f0"
set "TOP=%~dp0"
set "TOP=%TOP:~0,-1%"
cd /d "%~dp0"
set "MODE=add"
set "DRY="
set "ELEVATED="
:args
if "%~1"=="" goto args_done
if /i "%~1"=="--undo" (set "MODE=undo") else ^
if /i "%~1"=="/undo" (set "MODE=undo") else ^
if /i "%~1"=="--dry-run" (set "DRY=1") else ^
if /i "%~1"=="--elevated" (set "ELEVATED=1") else goto bad_option
shift
goto args
:args_done

set "X86=%TOP%\game\binaries\x86"
set "PROGRAMS=launcher_skhid.exe survarium_skhid.exe updater_skhid.exe"
if not exist "%X86%\survarium_skhid.exe" goto wrong_place

rem (No echo of Russian text inside a bracketed block: a ")" in it would end
rem the block.)
%RU% Настройка сети Skhid
%RU% ====================
%EN% Skhid network setup
%EN% ===================
if "%MODE%"=="undo" goto intro_undo
%RU% Добавляет правила брандмауэра Windows, которые разрешают трём программам
%RU% игры из папки game\binaries\x86 исходящие соединения (TCP и UDP):
%EN% Adds Windows Firewall rules that allow the game's three programs in
%EN% the folder game\binaries\x86 outgoing connections (TCP and UDP):
echo   launcher_skhid.exe, survarium_skhid.exe, updater_skhid.exe
%RU% Брандмауэр остаётся включённым.
%EN% The firewall stays on.
goto intro_done
:intro_undo
%RU% Удаляет правила брандмауэра Windows "Skhid ...", которые добавил этот
%RU% файл.
%EN% Removes the Windows Firewall rules "Skhid ..." that this file added.
:intro_done
%RU% Папка игры: "%TOP%"
%EN% Game folder: "%TOP%"
echo.
if not defined DRY goto intro_dry_done
%RU% Пробный запуск: команды только выводятся, ничего не меняется.
%EN% Dry run: the commands are only printed, nothing is changed.
echo.
:intro_dry_done
if defined DRY if not defined ELEVATED call :elevation
if defined DRY goto main

rem Windows Firewall rules need administrator rights: fltmc ends with 0 only
rem with them (not "errorlevel 1", which a negative exit code passes).
fltmc >nul 2>&1
if %errorlevel% equ 0 goto main
if defined ELEVATED goto not_admin
%RU% Для правил брандмауэра нужны права администратора. Сейчас Windows
%RU% спросит разрешение (контроль учётных записей): нажмите "Да". Тогда этот
%RU% файл запустится ещё раз, в новом окне.
%EN% The firewall rules need administrator rights. Windows will now ask
%EN% for permission (User Account Control): click "Yes". Then this file
%EN% will start again, in a new window.
echo.
call :elevation
powershell -NoProfile -ExecutionPolicy Bypass -Command "try { Start-Process -FilePath $env:ComSpec -ArgumentList $env:SKHID_CMDLINE -Verb RunAs -ErrorAction Stop } catch { exit 1 }"
if errorlevel 1 goto declined
goto quit

rem The elevated run's command line, for cmd.exe: cmd /c takes the outer
rem quotes off what follows it, so the script's quoted path and its options
rem are in one more pair (a folder with a space and a bracket or an &).
rem With --dry-run it is printed, and is a dry run's.
:elevation
set "SKHID_ARGS=--elevated"
if "%MODE%"=="undo" set "SKHID_ARGS=%SKHID_ARGS% --undo"
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
echo.
%RU% Готово. Чтобы убрать всё это: Skhid-network-setup.cmd --undo
%EN% Done. To remove all this: Skhid-network-setup.cmd --undo
goto end

rem --- undo ------------------------------------------------------------
:undo
for %%P in (%PROGRAMS%) do call :remove %%P
rem The Defender exclusion it once added is Skhid-defender-setup.cmd's to undo.
if exist "%TOP%\Skhid-network-setup.defender" %RU% Defender: исключение папки игры удаляет Skhid-defender-setup.cmd --undo
if exist "%TOP%\Skhid-network-setup.defender" %EN% Defender: the game folder's exclusion is removed by Skhid-defender-setup.cmd --undo
echo.
%RU% Готово.
%EN% Done.
goto end

rem The two rules of one program: out, TCP and UDP (the answers come back
rem without a rule; nothing else comes in).
:allow
for %%T in (TCP UDP) do call :rule %1 out %%T
%RU% Брандмауэр: %1 разрешены исходящие соединения (TCP и UDP)
%EN% Firewall: %1 is allowed outgoing connections (TCP and UDP)
exit /b 0

:rule
set "NAME=Skhid %~1 %3 %2"
call :run_quiet netsh advfirewall firewall delete rule name="%NAME%"
call :run netsh advfirewall firewall add rule name="%NAME%" dir=%2 action=allow program="%X86%\%~1" protocol=%3 enable=yes profile=any
if errorlevel 1 %RU%   ОШИБКА: правило "%NAME%" не добавлено.
if errorlevel 1 %EN%   ERROR: the rule "%NAME%" is not added.
exit /b 0

:remove
for %%T in (TCP UDP) do call :run_quiet netsh advfirewall firewall delete rule name="Skhid %~1 %%T out"
%RU% Брандмауэр: правила "Skhid %~1 ..." удалены
%EN% Firewall: the rules "Skhid %~1 ..." are removed
exit /b 0

rem A rule that blocks one of the programs wins over the ones above; Windows
rem makes one when its "allow access" question is cancelled. Said, not removed:
rem it is not this script's. (The menu names are those of a Windows in that
rem language.)
:blocking
if not defined DRY goto blocking_look
echo [dry-run] look for firewall rules that block the three programs
exit /b 0
:blocking_look
set "SKHID_X86=%X86%"
powershell -NoProfile -ExecutionPolicy Bypass -Command "foreach ($p in 'launcher_skhid.exe','survarium_skhid.exe','updater_skhid.exe') { $path = Join-Path $env:SKHID_X86 $p; Get-NetFirewallApplicationFilter -Program $path -ErrorAction SilentlyContinue | Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.Action -eq 'Block' -and $_.Enabled -eq 'True' } | ForEach-Object { if ($env:SKHID_RU) { Write-Output ('ВНИМАНИЕ: правило брандмауэра [' + $_.DisplayName + '] блокирует ' + $p + '. Удалите его: Брандмауэр Защитника Windows, Дополнительные параметры, Правила для исходящего подключения.') } else { Write-Output ('WARNING: the firewall rule [' + $_.DisplayName + '] blocks ' + $p + '. Delete it: Windows Defender Firewall, Advanced settings, Outbound Rules.') } } }" 2>nul
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
%RU% Неизвестный параметр: %1
%EN% Unknown option: %1
%RU% Запуск: Skhid-network-setup.cmd [--undo] [--dry-run]
%EN% Usage: Skhid-network-setup.cmd [--undo] [--dry-run]
set "RC=2"
goto quit

:wrong_place
%RU% Не найден survarium_skhid.exe в папке
%EN% survarium_skhid.exe is not in the folder
echo   "%X86%"
%RU% Если его удалил Microsoft Defender, запустите Skhid-defender-setup.cmd.
%EN% If Microsoft Defender deleted it, run Skhid-defender-setup.cmd.
%RU% Положите этот файл рядом со Skhid.bat, в папку, где лежит папка game,
%RU% и запустите его оттуда.
%EN% Put this file beside Skhid.bat, in the folder that holds the folder
%EN% game, and start it from there.
goto end

:not_admin
%RU% Ничего не изменено: у этого окна нет прав администратора.
%EN% Nothing changed: this window has no administrator rights.
goto end

rem No at the User Account Control prompt (or no prompt at all).
:declined
%RU% Ничего не изменено: Windows не дала прав администратора.
%RU% Запустите этот файл ещё раз и в окне Windows нажмите "Да", чтобы он
%RU% настроил брандмауэр.
%EN% Nothing changed: Windows gave no administrator rights.
%EN% Start this file again and click "Yes" in the Windows prompt, so that it
%EN% sets up the firewall.
goto end

:end
echo.
if defined DRY goto quit
%RU% Нажмите любую клавишу, чтобы закрыть окно.
%EN% Press any key to close the window.
pause >nul

rem The console's own code page back (a window it was started from).
:quit
if defined SKHID_CP chcp %SKHID_CP% >nul 2>&1
exit /b %RC%

@echo off
rem Skhid-network-setup.cmd: lets the Skhid game's three programs out through
rem Windows Firewall. It turns nothing off: the firewall stays on. It belongs
rem beside Skhid.bat, in the folder that holds the folder named game. What it
rem says is in Russian (its players are). The Microsoft Defender exclusion it
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
echo Настройка сети Skhid
echo ====================
if "%MODE%"=="undo" goto intro_undo
echo Добавляет правила брандмауэра Windows, которые разрешают трём программам
echo игры из папки game\binaries\x86 исходящие соединения (TCP и UDP):
echo   launcher_skhid.exe, survarium_skhid.exe, updater_skhid.exe
echo Брандмауэр остаётся включённым.
goto intro_done
:intro_undo
echo Удаляет правила брандмауэра Windows "Skhid ...", которые добавил этот
echo файл.
:intro_done
echo Папка игры: "%TOP%"
echo.
if not defined DRY goto intro_dry_done
echo Пробный запуск: команды только выводятся, ничего не меняется.
echo.
:intro_dry_done
if defined DRY if not defined ELEVATED call :elevation
if defined DRY goto main

rem Windows Firewall rules need administrator rights: fltmc ends with 0 only
rem with them (not "errorlevel 1", which a negative exit code passes).
fltmc >nul 2>&1
if %errorlevel% equ 0 goto main
if defined ELEVATED goto not_admin
echo Для правил брандмауэра нужны права администратора. Сейчас Windows
echo спросит разрешение (контроль учётных записей): нажмите "Да". Тогда этот
echo файл запустится ещё раз, в новом окне.
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
echo Готово. Чтобы убрать всё это: Skhid-network-setup.cmd --undo
goto end

rem --- undo ------------------------------------------------------------
:undo
for %%P in (%PROGRAMS%) do call :remove %%P
rem The Defender exclusion it once added is Skhid-defender-setup.cmd's to undo.
if exist "%TOP%\Skhid-network-setup.defender" echo Defender: исключение папки игры удаляет Skhid-defender-setup.cmd --undo
echo.
echo Готово.
goto end

rem The two rules of one program: out, TCP and UDP (the answers come back
rem without a rule; nothing else comes in).
:allow
for %%T in (TCP UDP) do call :rule %1 out %%T
echo Брандмауэр: %1 разрешены исходящие соединения (TCP и UDP)
exit /b 0

:rule
set "NAME=Skhid %~1 %3 %2"
call :run_quiet netsh advfirewall firewall delete rule name="%NAME%"
call :run netsh advfirewall firewall add rule name="%NAME%" dir=%2 action=allow program="%X86%\%~1" protocol=%3 enable=yes profile=any
if errorlevel 1 echo   ОШИБКА: правило "%NAME%" не добавлено.
exit /b 0

:remove
for %%T in (TCP UDP) do call :run_quiet netsh advfirewall firewall delete rule name="Skhid %~1 %%T out"
echo Брандмауэр: правила "Skhid %~1 ..." удалены
exit /b 0

rem A rule that blocks one of the programs wins over the ones above; Windows
rem makes one when its "allow access" question is cancelled. Said, not removed:
rem it is not this script's. (The menu names are the Russian Windows' own.)
:blocking
if not defined DRY goto blocking_look
echo [dry-run] look for firewall rules that block the three programs
exit /b 0
:blocking_look
set "SKHID_X86=%X86%"
powershell -NoProfile -ExecutionPolicy Bypass -Command "foreach ($p in 'launcher_skhid.exe','survarium_skhid.exe','updater_skhid.exe') { $path = Join-Path $env:SKHID_X86 $p; Get-NetFirewallApplicationFilter -Program $path -ErrorAction SilentlyContinue | Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.Action -eq 'Block' -and $_.Enabled -eq 'True' } | ForEach-Object { Write-Output ('ВНИМАНИЕ: правило брандмауэра [' + $_.DisplayName + '] блокирует ' + $p + '. Удалите его: Брандмауэр Защитника Windows, Дополнительные параметры, Правила для исходящего подключения.') } }" 2>nul
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
echo Неизвестный параметр: %1
echo Запуск: Skhid-network-setup.cmd [--undo] [--dry-run]
set "RC=2"
goto quit

:wrong_place
echo Не найден survarium_skhid.exe в папке
echo   "%X86%"
echo Положите этот файл рядом со Skhid.bat, в папку, где лежит папка game,
echo и запустите его оттуда.
goto end

:not_admin
echo Ничего не изменено: у этого окна нет прав администратора.
goto end

rem No at the User Account Control prompt (or no prompt at all).
:declined
echo Ничего не изменено: Windows не дала прав администратора.
echo Запустите этот файл ещё раз и в окне Windows нажмите "Да", чтобы он
echo настроил брандмауэр.
goto end

:end
echo.
if defined DRY goto quit
echo Нажмите любую клавишу, чтобы закрыть окно.
pause >nul

rem The console's own code page back (a window it was started from).
:quit
if defined SKHID_CP chcp %SKHID_CP% >nul 2>&1
exit /b %RC%

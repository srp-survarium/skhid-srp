@echo off
rem Skhid-network-setup.cmd: lets the Skhid game's three programs out through
rem Windows Firewall and, if you say yes, excludes the game's program folder
rem (game\binaries\x86) from Microsoft Defender's scans. It turns nothing off: the firewall and
rem Defender stay on. It belongs beside Skhid.bat, in the folder that holds
rem the folder named game. What it says is in Russian (its players are).
rem
rem   Skhid-network-setup.cmd                add the rules (asks for administrator rights)
rem   Skhid-network-setup.cmd --undo         remove exactly what it added
rem   Skhid-network-setup.cmd --dry-run      print the commands, change nothing
rem   --defender-yes or --defender-no        answer the Defender question beforehand
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
cd /d "%~dp0"
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

rem (No echo of Russian text inside a bracketed block: a ")" in it would end
rem the block.)
echo Настройка сети Skhid
echo ====================
if "%MODE%"=="undo" goto intro_undo
echo Добавляет правила брандмауэра Windows, которые разрешают трём программам
echo игры из папки game\binaries\x86 исходящие соединения (TCP и UDP):
echo   launcher_skhid.exe, survarium_skhid.exe, updater_skhid.exe
echo Потом спрашивает, добавить ли эту папку в исключения Microsoft Defender.
echo Брандмауэр и Defender остаются включёнными.
goto intro_done
:intro_undo
echo Удаляет правила брандмауэра Windows "Skhid ...", которые добавил этот
echo файл, и исключение папки игры из проверки Microsoft Defender, если его
echo добавил он.
:intro_done
echo Папка игры: "%TOP%"
echo.
if not defined DRY goto intro_dry_done
echo Пробный запуск: команды только выводятся, ничего не меняется.
echo.
:intro_dry_done
if defined DRY if not defined ELEVATED call :elevation
if defined DRY goto main

rem Windows Firewall rules need administrator rights.
fltmc >nul 2>&1
if not errorlevel 1 goto main
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
echo Готово. Чтобы убрать всё это: Skhid-network-setup.cmd --undo
goto end

rem --- undo ------------------------------------------------------------
:undo
for %%P in (%PROGRAMS%) do call :remove %%P
call :defender_remove
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

rem The exclusion is of game\binaries\x86 alone, where the programs that get
rem flagged are (never the folder the archive was unpacked into: Downloads).
:defender_add
set "SKHID_EXCLUDED=%X86%"
if not "%TOP:~2%"=="" goto defender_offer
echo Defender: исключение не предлагается, игра лежит в корне диска. Сначала
echo перенесите её в папку.
exit /b 0
:defender_offer
if "%DEFENDER%"=="no" goto defender_skipped
if "%DEFENDER%"=="yes" goto defender_yes
echo.
echo Microsoft Defender может по ошибке принять изменённые файлы игры
echo (survarium_skhid.exe, bugtrap.dll, программу обновления) за угрозу и
echo заблокировать или удалить их. Исключение папки с программами игры
echo отключает только это:
echo   "%X86%"
rem The answer is typed (set /p reads it as Unicode), Д or Н in either case,
rem Y or N too. A bare Enter, or no input at all, is a no.
:defender_ask
set "ANSWER="
set /p "ANSWER=Добавить исключение в Defender? Д - да, Н - нет, затем Enter: "
if not defined ANSWER goto defender_skipped
set "ANSWER=%ANSWER:"=%"
if /i "%ANSWER%"=="д" goto defender_yes
if /i "%ANSWER%"=="да" goto defender_yes
if /i "%ANSWER%"=="y" goto defender_yes
if /i "%ANSWER%"=="н" goto defender_skipped
if /i "%ANSWER%"=="нет" goto defender_skipped
if /i "%ANSWER%"=="n" goto defender_skipped
echo Введите Д или Н.
goto defender_ask
:defender_yes
if not defined DRY goto defender_exclude
echo [dry-run] powershell Add-MpPreference -ExclusionPath "%X86%"
echo [dry-run] write "%MARKER%":
call :marker_line
exit /b 0
:defender_exclude
powershell -NoProfile -ExecutionPolicy Bypass -Command "try { if ((Get-MpPreference -ErrorAction Stop).ExclusionPath -contains $env:SKHID_EXCLUDED) { exit 3 }; Add-MpPreference -ExclusionPath $env:SKHID_EXCLUDED -ErrorAction Stop } catch { exit 1 }"
if errorlevel 3 goto defender_had
if errorlevel 1 goto defender_failed
call :marker_line > "%MARKER%"
echo Defender: папка game\binaries\x86 исключена из проверки.
exit /b 0
:defender_had
echo Defender: эта папка уже была в исключениях, ничего не изменено.
exit /b 0
:defender_failed
echo Defender: исключение добавить не удалось. Возможно, защитой управляет
echo другой антивирус: добавьте папку в его исключения сами.
exit /b 0
:defender_skipped
echo Defender: исключение не добавлено.
exit /b 0

rem The marker's one line: the path excluded, as it is. A FOR variable is not
rem parsed again (an & in it is no command) and, without delayed expansion,
rem a ! stays a !. (A Windows path has no * or ?, which FOR would match.) It
rem is written and read in code page 866, which this script runs in.
:marker_line
for %%A in ("%SKHID_EXCLUDED%") do echo(%%~A
exit /b 0

:defender_remove
if exist "%MARKER%" goto defender_unexclude
echo Defender: этот файл не добавлял здесь исключений, удалять нечего.
exit /b 0
:defender_unexclude
set /p "SKHID_EXCLUDED=" < "%MARKER%"
if not defined DRY goto defender_unexclude_run
echo [dry-run] powershell Remove-MpPreference -ExclusionPath "%SKHID_EXCLUDED%"
echo [dry-run] delete "%MARKER%"
exit /b 0
:defender_unexclude_run
powershell -NoProfile -ExecutionPolicy Bypass -Command "try { Remove-MpPreference -ExclusionPath $env:SKHID_EXCLUDED -ErrorAction Stop } catch { exit 1 }"
if not errorlevel 1 goto defender_unexcluded
echo Defender: исключение "%SKHID_EXCLUDED%" удалить не удалось.
exit /b 0
:defender_unexcluded
del "%MARKER%"
echo Defender: исключение "%SKHID_EXCLUDED%" удалено.
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
echo Запуск: Skhid-network-setup.cmd [--undo] [--dry-run] [--defender-yes ^| --defender-no]
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

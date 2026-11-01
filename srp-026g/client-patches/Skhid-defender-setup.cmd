@echo off
rem Skhid-defender-setup.cmd: excludes the game's program folder
rem (game\binaries\x86) from Microsoft Defender's scans, so that it stops
rem taking the patched files for a threat. It turns nothing off: Defender
rem stays on. It belongs beside Skhid.bat, in the folder that holds the
rem folder named game. What it says is in Russian (its players are).
rem
rem   Skhid-defender-setup.cmd               add the exclusion (asks for administrator rights)
rem   Skhid-defender-setup.cmd --undo        remove it, if this file added it
rem   Skhid-defender-setup.cmd --dry-run     print the commands, change nothing
rem
rem Started without administrator rights, it asks Windows for them (the User
rem Account Control prompt) and runs again in a new window, elevated.
rem
rem The marker of the exclusion is Skhid-network-setup.defender, the name of
rem the earlier Skhid-network-setup.cmd, which asked for it: one it made is
rem undone here too.
rem
rem The code page: this file is kept in UTF-8, and make_client_026g.py ships
rem it in code page 866 (DOS Cyrillic). cmd reads each line of a script in
rem the console's code page at the time, so it switches the console to 866 here,
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
set "MARKER=%TOP%\Skhid-network-setup.defender"
set "SKHID_MARKER=%MARKER%"
set "SKHID_EXCLUDED=%X86%"
if not exist "%X86%\survarium_skhid.exe" goto wrong_place

rem (No echo of Russian text inside a bracketed block: a ")" in it would end
rem the block.)
echo Исключение Skhid в Microsoft Defender
echo =====================================
if "%MODE%"=="undo" goto intro_undo
echo Microsoft Defender может по ошибке принять изменённые файлы игры
echo (survarium_skhid.exe, bugtrap.dll, программу обновления) за угрозу и
echo заблокировать или удалить их. Этот файл добавляет папку с программами
echo игры в исключения Defender:
echo   "%X86%"
echo Defender остаётся включённым.
goto intro_done
:intro_undo
echo Удаляет исключение папки игры из проверки Microsoft Defender, если его
echo добавил этот файл.
:intro_done
echo Папка игры: "%TOP%"
echo.
if not defined DRY goto intro_dry_done
echo Пробный запуск: команды только выводятся, ничего не меняется.
echo.
:intro_dry_done
if defined DRY if not defined ELEVATED call :elevation
if defined DRY goto main

rem Defender's settings need administrator rights: fltmc ends with 0 only
rem with them (not "errorlevel 1", which a negative exit code passes).
fltmc >nul 2>&1
if %errorlevel% equ 0 goto main
if defined ELEVATED goto not_admin
echo Для исключения в Defender нужны права администратора. Сейчас Windows
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
rem The exclusion is of game\binaries\x86 alone, where the programs that get
rem flagged are (never the folder the archive was unpacked into: Downloads).
if not "%TOP:~2%"=="" goto add
echo Defender: исключение не добавлено, игра лежит в корне диска. Сначала
echo перенесите её в папку.
goto end
:add
if not defined DRY goto exclude
echo [dry-run] powershell Add-MpPreference -ExclusionPath "%X86%"
echo [dry-run] write "%MARKER%":
call :marker_line
goto done
rem PowerShell writes the marker (the path excluded, UTF-8 with a byte order
rem mark): every letter of the path is kept, Ukrainian or Polish too, which
rem code page 866 has not.
:exclude
powershell -NoProfile -ExecutionPolicy Bypass -Command "try { if ((Get-MpPreference -ErrorAction Stop).ExclusionPath -contains $env:SKHID_EXCLUDED) { exit 3 }; Add-MpPreference -ExclusionPath $env:SKHID_EXCLUDED -ErrorAction Stop; [IO.File]::WriteAllText($env:SKHID_MARKER, $env:SKHID_EXCLUDED, [Text.Encoding]::UTF8) } catch { exit 1 }"
if errorlevel 3 goto had
if errorlevel 1 goto failed
echo Defender: папка game\binaries\x86 исключена из проверки.
goto done
:had
echo Defender: эта папка уже была в исключениях, ничего не изменено.
goto end
:failed
echo Defender: исключение добавить не удалось. Возможно, защитой управляет
echo другой антивирус: добавьте папку в его исключения сами.
goto end
:done
echo.
echo Готово. Чтобы убрать исключение: Skhid-defender-setup.cmd --undo
goto end

rem A dry run's picture of the marker's line: the path, as it is. A FOR
rem variable is not parsed again (an & in it is no command) and, without
rem delayed expansion, a ! stays a !. (A Windows path has no * or ?, which
rem FOR would match.)
:marker_line
for %%A in ("%SKHID_EXCLUDED%") do echo(%%~A
exit /b 0

rem --- undo ------------------------------------------------------------
rem PowerShell reads the marker and says what it removed. ReadAllText takes
rem the byte order mark's UTF-8; a marker without one is of the earlier
rem script (#453), written by cmd in the console's code page: 866 here.
:undo
if exist "%MARKER%" goto unexclude
echo Defender: этот файл не добавлял здесь исключений, удалять нечего.
goto end
:unexclude
if not defined DRY goto unexclude_run
echo [dry-run] powershell Remove-MpPreference -ExclusionPath (the path in "%MARKER%")
echo [dry-run] delete "%MARKER%"
goto end
:unexclude_run
powershell -NoProfile -ExecutionPolicy Bypass -Command "$p = ''; try { $p = [IO.File]::ReadAllText($env:SKHID_MARKER, [Text.Encoding]::GetEncoding(866)).Trim(); Remove-MpPreference -ExclusionPath $p -ErrorAction Stop; Write-Output ('Defender: исключение ' + [char]34 + $p + [char]34 + ' удалено.') } catch { Write-Output ('Defender: исключение ' + [char]34 + $p + [char]34 + ' удалить не удалось.'); exit 1 }"
if errorlevel 1 goto end
del "%MARKER%"
goto end

:bad_option
echo Неизвестный параметр: %1
echo Запуск: Skhid-defender-setup.cmd [--undo] [--dry-run]
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
echo добавил исключение.
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

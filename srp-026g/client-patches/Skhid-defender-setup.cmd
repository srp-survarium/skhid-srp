@echo off
rem Skhid-defender-setup.cmd: excludes the game's program folder
rem (game\binaries\x86) from Microsoft Defender's scans, so that it stops
rem taking the patched files for a threat, then puts back the Skhid files
rem Defender already deleted (updater_skhid.exe too, which nothing else
rem restores). It turns nothing off: Defender stays on. It belongs beside
rem Skhid.bat, in the folder that holds the folder named game. It speaks
rem Russian when the Windows display language is Russian, Ukrainian or
rem Belarusian, English otherwise.
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
set "MARKER=%TOP%\Skhid-network-setup.defender"
set "SKHID_MARKER=%MARKER%"
set "SKHID_EXCLUDED=%X86%"
rem Beside the game: its exe there, or (Defender deleted it) the updater's address.
if not exist "%X86%\survarium_skhid.exe" if not exist "%X86%\updater_skhid.url" goto wrong_place

rem (No echo of Russian text inside a bracketed block: a ")" in it would end
rem the block.)
%RU% Исключение Skhid в Microsoft Defender
%EN% Skhid exclusion in Microsoft Defender
echo =====================================
if "%MODE%"=="undo" goto intro_undo
%RU% Microsoft Defender может по ошибке принять изменённые файлы игры
%RU% (survarium_skhid.exe, bugtrap.dll, программу обновления) за угрозу и
%RU% заблокировать или удалить их. Этот файл добавляет папку с программами
%RU% игры в исключения Defender:
%EN% Microsoft Defender may mistake the game's changed files
%EN% (survarium_skhid.exe, bugtrap.dll, the updater) for a threat and
%EN% block or delete them. This file adds the folder with the game's
%EN% programs to Defender's exclusions:
echo   "%X86%"
%RU% и скачивает заново файлы Skhid, которые Defender уже удалил.
%EN% and downloads again the Skhid files Defender already deleted.
%RU% Defender остаётся включённым.
%EN% Defender stays on.
goto intro_done
:intro_undo
%RU% Удаляет исключение папки игры из проверки Microsoft Defender, если его
%RU% добавил этот файл.
%EN% Removes the game folder's exclusion from Microsoft Defender's scans, if
%EN% this file added it.
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

rem Defender's settings need administrator rights: fltmc ends with 0 only
rem with them (not "errorlevel 1", which a negative exit code passes).
fltmc >nul 2>&1
if %errorlevel% equ 0 goto main
if defined ELEVATED goto not_admin
%RU% Для исключения в Defender нужны права администратора. Сейчас Windows
%RU% спросит разрешение (контроль учётных записей): нажмите "Да". Тогда этот
%RU% файл запустится ещё раз, в новом окне.
%EN% The Defender exclusion needs administrator rights. Windows will now ask
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
rem The exclusion is of game\binaries\x86 alone, where the programs that get
rem flagged are (never the folder the archive was unpacked into: Downloads).
if not "%TOP:~2%"=="" goto add
%RU% Defender: исключение не добавлено, игра лежит в корне диска. Сначала
%RU% перенесите её в папку.
%EN% Defender: no exclusion added, the game is in the root of a drive. First
%EN% move it into a folder.
goto end
:add
if not defined DRY goto exclude
echo [dry-run] powershell Add-MpPreference -ExclusionPath "%X86%"
echo [dry-run] write "%MARKER%":
call :marker_line
call :restore
goto done
rem PowerShell writes the marker (the path excluded, UTF-8 with a byte order
rem mark): every letter of the path is kept, Ukrainian or Polish too, which
rem code page 866 has not.
:exclude
powershell -NoProfile -ExecutionPolicy Bypass -Command "try { if ((Get-MpPreference -ErrorAction Stop).ExclusionPath -contains $env:SKHID_EXCLUDED) { exit 3 }; Add-MpPreference -ExclusionPath $env:SKHID_EXCLUDED -ErrorAction Stop; [IO.File]::WriteAllText($env:SKHID_MARKER, $env:SKHID_EXCLUDED, [Text.Encoding]::UTF8) } catch { exit 1 }"
if errorlevel 3 goto had
if errorlevel 1 goto failed
%RU% Defender: папка game\binaries\x86 исключена из проверки.
%EN% Defender: the folder game\binaries\x86 is excluded from scans.
call :restore
goto done
:had
%RU% Defender: эта папка уже была в исключениях, ничего не изменено.
%EN% Defender: this folder was already excluded, nothing changed.
call :restore
goto end
:failed
%RU% Defender: исключение добавить не удалось. Возможно, защитой управляет
%RU% другой антивирус: добавьте папку в его исключения сами.
%EN% Defender: the exclusion could not be added. Perhaps another antivirus
%EN% manages protection: add the folder to its exclusions yourself.
goto end
:done
echo.
%RU% Готово. Чтобы убрать исключение: Skhid-defender-setup.cmd --undo
%EN% Done. To remove the exclusion: Skhid-defender-setup.cmd --undo
goto end

rem A dry run's picture of the marker's line: the path, as it is. A FOR
rem variable is not parsed again (an & in it is no command) and, without
rem delayed expansion, a ! stays a !. (A Windows path has no * or ?, which
rem FOR would match.)
:marker_line
for %%A in ("%SKHID_EXCLUDED%") do echo(%%~A
exit /b 0

rem --- the files Defender deleted --------------------------------------
rem Once the folder is excluded: the ZIP of the server updater_skhid.url
rem names (else skhid.link, as Skhid-without-updater.bat has it; https only,
rem plain http for this computer alone, as the updater takes it), checked
rem against the sha256 its /api/client-files gives, and of the ZIP the
rem files that are missing put in place; none that is there is overwritten.
rem The ZIP is downloaded and unpacked in game\binaries\x86 itself: inside
rem the exclusion (Defender leaves the copies alone) and not in %TEMP% (the
rem files moved from there have the game folder's permissions; an elevated
rem run's TEMP is the administrator's, and the updater could not replace
rem them later). Both go afterwards. Then each file that was missing is
rem looked for again: one that is not there is said. A ZIP left by an
rem interrupted run is used again when it is the one announced, else
rem deleted and downloaded anew; a staging folder left is deleted.
rem Unpacked by .NET 4.5's ZipFile (Windows 8.1, 10, 11, and 7 with WMF
rem 5.1), else, only where there is no ZipFile, by the Windows shell; an
rem error unpacking changes nothing. Windows 7 as it came (PowerShell 2, no
rem TLS 1.2) cannot download the ZIP and says so.
:restore
set "SKHID_SITE="
if exist "%X86%\updater_skhid.url" set /p SKHID_SITE=<"%X86%\updater_skhid.url"
call :site_check
if not defined SKHID_OK set "SKHID_SITE=https://skhid.link"
set "SKHID_TOP=%TOP%"
if defined DRY goto restore_dry
powershell -NoProfile -ExecutionPolicy Bypass -Command "$ErrorActionPreference = 'Stop'; function say($r, $e) { if ($env:SKHID_RU) { $r } else { $e } }; function sha($p) { $s = [IO.File]::OpenRead($p); try { [BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash($s)).Replace('-', '') } finally { $s.Close() } }; $top = $env:SKHID_TOP; $site = $env:SKHID_SITE.Trim().TrimEnd('/'); $zip = [IO.Path]::Combine($env:SKHID_EXCLUDED, '.skhid-restore.zip'); $stage = [IO.Path]::Combine($env:SKHID_EXCLUDED, '.skhid-restore'); $want = @(); $code = 0; $step = 'stage'; try { if ([IO.Directory]::Exists($stage)) { [IO.Directory]::Delete($stage, $true) }; $step = 'get'; try { [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor 3072 } catch {}; $web = New-Object Net.WebClient; $sha = ''; foreach ($l in ($web.DownloadString($site + '/api/client-files') -split [char]10)) { $w = -split $l; if ($w[0] -eq 'sha256') { $sha = $w[1] } }; if (-not $sha) { throw 'no sha256' }; $had = ''; try { $had = sha $zip } catch {}; if ($had -ne $sha) { $step = 'stage'; [IO.File]::Delete($zip); $step = 'get'; $web.DownloadFile($site + '/download', $zip) }; $step = 'sha'; if ((sha $zip) -ne $sha) { throw 'sha256' }; $step = 'unpack'; [void][IO.Directory]::CreateDirectory($stage); $zipfile = $true; try { Add-Type -AssemblyName System.IO.Compression.FileSystem } catch { $zipfile = $false }; if ($zipfile) { [IO.Compression.ZipFile]::ExtractToDirectory($zip, $stage) } else { $sh = New-Object -ComObject Shell.Application; $sh.NameSpace($stage).CopyHere($sh.NameSpace($zip).Items(), 1556) }; $files = [IO.Directory]::GetFiles($stage, '*', 'AllDirectories'); if (-not $files) { throw 'nothing unpacked' }; $step = 'move'; foreach ($f in $files) { $rel = $f.Substring($stage.Length + 1); $t = [IO.Path]::Combine($top, $rel); if (-not (Test-Path -LiteralPath $t)) { $want += $rel; try { [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($t)); [IO.File]::Move($f, $t) } catch {} } } } catch { $code = 1; if ($step -eq 'stage') { say 'Восстановление: не удалось удалить .skhid-restore, оставшийся с прошлого раза. Ничего не изменено.' 'Restore: .skhid-restore, left from an earlier run, could not be removed. Nothing changed.' } elseif ($step -eq 'get') { say ('Восстановление: не удалось скачать файлы Skhid с ' + $site + '. Ничего не изменено.') ('Restore: could not download the Skhid files from ' + $site + '. Nothing changed.') } elseif ($step -eq 'sha') { say 'Восстановление: скачанный архив повреждён (не тот sha256). Ничего не изменено.' 'Restore: the download is damaged (wrong sha256). Nothing changed.' } elseif ($step -eq 'unpack') { say 'Восстановление: Windows не смогла распаковать архив. Ничего не изменено. Запустите Skhid.bat (лаунчер).' 'Restore: Windows could not unpack the archive. Nothing changed. Start Skhid.bat (the launcher).' } else { say 'Восстановление: не удалось положить файлы на место.' 'Restore: the files could not be put in place.' } } finally { try { if ([IO.Directory]::Exists($stage)) { [IO.Directory]::Delete($stage, $true) } } catch {}; try { [IO.File]::Delete($zip) } catch {} }; $back = @($want | Where-Object { Test-Path -LiteralPath ([IO.Path]::Combine($top, $_)) }); $lost = @($want | Where-Object { -not (Test-Path -LiteralPath ([IO.Path]::Combine($top, $_))) }); if ($back) { say 'Восстановлены файлы Skhid:' 'Restored Skhid files:'; foreach ($r in $back) { '  ' + $r } }; if ($lost) { $code = 1; say 'Не восстановлены (возможно, Defender удалил их снова):' 'Not restored (Defender may have deleted them again):'; foreach ($r in $lost) { '  ' + $r } } elseif ($code -eq 0 -and -not $back) { say 'Все файлы Skhid на месте, восстанавливать нечего.' 'No Skhid file is missing, nothing to restore.' }; exit $code"
exit /b 0

rem SKHID_OK when SKHID_SITE is https://<host>..., or http:// to localhost
rem or 127.0.0.1 (the updater's rule). Delayed expansion: the address is
rem only looked at, never parsed as a command.
:site_check
set "SKHID_OK="
if not defined SKHID_SITE exit /b 0
setlocal EnableDelayedExpansion
set "ok="
set "local="
if /i "!SKHID_SITE:~0,8!"=="https://" if not "!SKHID_SITE:~8,1!"=="" if not "!SKHID_SITE:~8,1!"=="/" set "ok=1"
if /i "!SKHID_SITE:~0,16!"=="http://localhost" set "local=1"
if "!SKHID_SITE:~0,16!"=="http://127.0.0.1" set "local=1"
if defined local if "!SKHID_SITE:~16,1!"=="" set "ok=1"
if defined local if "!SKHID_SITE:~16,1!"==":" set "ok=1"
if defined local if "!SKHID_SITE:~16,1!"=="/" set "ok=1"
endlocal & set "SKHID_OK=%ok%"
exit /b 0
:restore_dry
setlocal EnableDelayedExpansion
echo [dry-run] download !SKHID_SITE!/download, check it against the sha256 of !SKHID_SITE!/api/client-files
echo [dry-run] put back its files missing in "!TOP!", overwriting none
endlocal
exit /b 0

rem --- undo ------------------------------------------------------------
rem PowerShell reads the marker and says what it removed. ReadAllText takes
rem the byte order mark's UTF-8; a marker without one is of the earlier
rem script (#453), written by cmd in the console's code page: 866 here.
:undo
if exist "%MARKER%" goto unexclude
%RU% Defender: этот файл не добавлял здесь исключений, удалять нечего.
%EN% Defender: this file added no exclusion here, nothing to remove.
goto end
:unexclude
if not defined DRY goto unexclude_run
echo [dry-run] powershell Remove-MpPreference -ExclusionPath (the path in "%MARKER%")
echo [dry-run] delete "%MARKER%"
goto end
:unexclude_run
powershell -NoProfile -ExecutionPolicy Bypass -Command "$p = ''; try { $p = [IO.File]::ReadAllText($env:SKHID_MARKER, [Text.Encoding]::GetEncoding(866)).Trim(); Remove-MpPreference -ExclusionPath $p -ErrorAction Stop; if ($env:SKHID_RU) { Write-Output ('Defender: исключение ' + [char]34 + $p + [char]34 + ' удалено.') } else { Write-Output ('Defender: the exclusion ' + [char]34 + $p + [char]34 + ' is removed.') } } catch { if ($env:SKHID_RU) { Write-Output ('Defender: исключение ' + [char]34 + $p + [char]34 + ' удалить не удалось.') } else { Write-Output ('Defender: the exclusion ' + [char]34 + $p + [char]34 + ' could not be removed.') }; exit 1 }"
if errorlevel 1 goto end
del "%MARKER%"
goto end

:bad_option
%RU% Неизвестный параметр: %1
%EN% Unknown option: %1
%RU% Запуск: Skhid-defender-setup.cmd [--undo] [--dry-run]
%EN% Usage: Skhid-defender-setup.cmd [--undo] [--dry-run]
set "RC=2"
goto quit

:wrong_place
%RU% Не найден survarium_skhid.exe в папке
%EN% survarium_skhid.exe is not in the folder
echo   "%X86%"
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
%RU% добавил исключение.
%EN% Nothing changed: Windows gave no administrator rights.
%EN% Start this file again and click "Yes" in the Windows prompt, so that it
%EN% adds the exclusion.
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

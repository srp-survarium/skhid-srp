@echo off
rem Installs Survarium 0.26g and Skhid, keeps them updated, and starts the game.
rem No launcher, no updater. Put it in an empty folder (or the game's folder,
rem the one that holds the folder named game) and start it. Windows 10 or later.
rem Game language: english, russian, ukrainian, german, polish or serbian.
rem Left empty, it is the language of Windows (english for any other).
set LANGUAGE=
set URL=https://skhid.link
set GAME_URL=https://drive.usercontent.google.com/download?id=1LTK2rzJPELLXxtTavZwdzr247NFwT_GK^&export=download^&confirm=t
set GAME_SHA=7435558eeb17098fc9337c4ba9470402cae4c9be974ef5d9bc544f19b122a028
cd /d "%~dp0"
rem A second double-click while this window runs says so and stops: this one
rem holds skhid-install.lock open until it ends.
set STARTED=
9>"skhid-install.lock" call :main
if defined STARTED exit /b
echo Skhid is already being installed in another window. Wait for it to finish.
pause
exit /b 1

:main
set STARTED=1

if defined LANGUAGE goto language_done
set LANGUAGE=english
set SYS_LANG=
for /f %%l in ('powershell -NoProfile -Command "(Get-UICulture).TwoLetterISOLanguageName"') do set SYS_LANG=%%l
if "%SYS_LANG%"=="ru" set LANGUAGE=russian
if "%SYS_LANG%"=="uk" set LANGUAGE=ukrainian
if "%SYS_LANG%"=="de" set LANGUAGE=german
if "%SYS_LANG%"=="pl" set LANGUAGE=polish
if "%SYS_LANG%"=="sr" set LANGUAGE=serbian
:language_done

rem 1. The Skhid files, when the server has new ones.
set FILES_SHA=
set HAVE_SHA=
for /f "tokens=1,2" %%a in ('curl.exe -fsS "%URL%/api/client-files"') do if "%%a"=="sha256" set FILES_SHA=%%b
if not defined FILES_SHA goto no_server
if exist "game\binaries\x86\skhid-files.sha256" set /p HAVE_SHA=<"game\binaries\x86\skhid-files.sha256"
if "%HAVE_SHA%"=="%FILES_SHA%" goto files_done
echo Downloading the Skhid files...
curl.exe -fL -o skhid-files.zip "%URL%/download" || goto failed
call :sha skhid-files.zip %FILES_SHA% || goto failed
rem Not this file (cmd reads on in a running script from where it was: a new
rem one would be read wrongly), nor the updater (the file antivirus deletes).
tar -xf skhid-files.zip --exclude Skhid-without-updater.bat --exclude updater_skhid.exe || goto failed
del skhid-files.zip
> "game\binaries\x86\skhid-files.sha256" echo %FILES_SHA%
:files_done

rem 2. The original game, when it is missing: 2.9 GB download, about 10 GB free needed.
if exist "game\resources.db" if exist "game\binaries\x86\survarium.exe" goto game_done
set ARCHIVE=game\.skhid-game.7z
if exist "%ARCHIVE%" call :sha "%ARCHIVE%" %GAME_SHA% >nul && goto extract
echo Downloading Survarium 0.26g (2.9 GB)...
rem A part already as big as the whole (a broken download) cannot be resumed.
if exist "%ARCHIVE%" powershell -NoProfile -Command "if ((Get-Item '%ARCHIVE%').Length -ge 2906544103) { Remove-Item '%ARCHIVE%' }"
curl.exe -fL -C - -o "%ARCHIVE%" "%GAME_URL%"
if errorlevel 1 goto game_download_failed
call :sha "%ARCHIVE%" %GAME_SHA% || (del "%ARCHIVE%" & goto failed)
:extract
echo Unpacking Survarium 0.26g...
set STAGE=game\.skhid-extract
if exist "%STAGE%" rmdir /s /q "%STAGE%"
"game\binaries\x86\7zr.exe" x "%ARCHIVE%" -o"%STAGE%" -y "Survarium/game/resources.db" "Survarium/game/binaries/x86/survarium.exe" "Survarium/game/resources/sources/splash_screens/development_engine.bmp" || goto failed
if not exist "game\resources\sources\splash_screens" mkdir "game\resources\sources\splash_screens"
move /y "%STAGE%\Survarium\game\resources.db" "game\resources.db" >nul || goto failed
move /y "%STAGE%\Survarium\game\binaries\x86\survarium.exe" "game\binaries\x86\survarium.exe" >nul || goto failed
move /y "%STAGE%\Survarium\game\resources\sources\splash_screens\development_engine.bmp" "game\resources\sources\splash_screens\" >nul || goto failed
rmdir /s /q "%STAGE%"
del "%ARCHIVE%"
:game_done

rem 3. resources.db's patches: exactly the ones the server lists.
echo Checking the game's patches...
powershell -NoProfile -Command "$ErrorActionPreference='Stop'; $json = (curl.exe -fsS '%URL%/patches') -join ''; if ($LASTEXITCODE -ne 0 -or -not $json) { throw 'Cannot get the patch list' }; $list = $json | ConvertFrom-Json; $keep = @(); foreach ($p in $list.PSObject.Properties) { $f = 'game\resources.db.' + $p.Name; $keep += 'resources.db.' + $p.Name; $h = $p.Value.sha256; if ((Test-Path $f) -and (Get-FileHash $f).Hash -eq $h) { continue }; $ok = $false; foreach ($u in (@($p.Value.urls) + @($p.Value.url) | Where-Object { $_ })) { Write-Host ('Downloading ' + $p.Name + '...'); curl.exe -fL -o ($f + '.part') $u; if ($LASTEXITCODE -eq 0 -and (Get-FileHash ($f + '.part')).Hash -eq $h) { Move-Item -Force ($f + '.part') $f; $ok = $true; break } }; if (-not $ok) { throw ('Cannot download ' + $p.Name) } }; Get-ChildItem 'game\resources.db.patch_*' | Where-Object { $keep -notcontains $_.Name } | Remove-Item" || goto failed

rem 4. Play. It must start in game\binaries\x86: the game finds its certificate from there.
cd /d "game\binaries\x86"
start "" survarium_skhid.exe -localization=%LANGUAGE% -mt_allocator=768 -minimum_resources_memory_size=768
exit /b 0

rem Exit code 0 when the file's SHA-256 is the expected one.
:sha
powershell -NoProfile -Command "if ((Get-FileHash '%~1').Hash -ne '%~2') { Write-Host 'Download damaged: %~nx1. Start this file again.'; exit 1 }"
exit /b %errorlevel%

:no_server
echo Cannot reach %URL%. Check the internet connection, or see VPN.txt.
pause
exit /b 1

:game_download_failed
rem 33: the mirror cannot go on from where it stopped: next time from the start.
if %errorlevel%==33 del "%ARCHIVE%"
:failed
echo.
echo Something went wrong, so the game is not started. Check the internet
echo connection and the free space (about 10 GB for a new game), close the
echo game if it is open, then start this file again: it goes on from where
echo it stopped.
pause
exit /b 1

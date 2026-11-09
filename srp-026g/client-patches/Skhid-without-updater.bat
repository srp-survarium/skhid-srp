@echo off
rem Installs Survarium 0.26g and Skhid, keeps them updated, and starts the game.
rem No launcher, no updater. Put it in an empty folder (or the game's folder,
rem the one that holds the folder named game) and start it. Windows 10 1803 or
rem later (its curl and tar). What each download tried is in
rem skhid-download.log beside this file.
set GAME_SHA=7435558eeb17098fc9337c4ba9470402cae4c9be974ef5d9bc544f19b122a028
set GAME_SIZE=2906544103
cd /d "%~dp0"
set "SKHID_BAT=%~f0"
rem The server: the one updater_skhid.url names, as for the updater.
set URL=https://skhid.link
if exist "game\binaries\x86\updater_skhid.url" set /p URL=<"game\binaries\x86\updater_skhid.url"
rem A second double-click while this window runs says so and stops: this one
rem holds skhid-install.lock open while it installs (not the game, started
rem after it).
set STARTED=
9>"skhid-install.lock" call :main
if not defined STARTED goto locked
if errorlevel 1 exit /b 1

rem 4. Play, as the launcher starts the game (the language is the game's own).
rem It must start in game\binaries\x86: the game finds its certificate from there.
cd /d "game\binaries\x86"
start "" survarium_skhid.exe -mt_allocator=768 -minimum_resources_memory_size=768
exit /b 0

:locked
echo Skhid is already being installed in another window. Wait for it to finish.
pause
exit /b 1

:main
set STARTED=1
where /q curl.exe || goto old_windows
where /q tar.exe || goto old_windows

rem 1. The Skhid files, when the server has new ones or one of them is gone
rem (an antivirus deletes them): skhid-files.txt holds the ZIP's sha256 and,
rem below it, the ZIP's files. A server that cannot be reached (a provider
rem blocks it: VPN.txt): the files there are used, and the game and its
rem patches come from the addresses written into this file.
set SKHID_ONLINE=
set FILES_SHA=
set HAVE_SHA=
for /f "tokens=1,2" %%a in ('curl.exe -fsS --connect-timeout 10 --max-time 60 "%URL%/api/client-files"') do if "%%a"=="sha256" set FILES_SHA=%%b
if not defined FILES_SHA goto offline
set SKHID_ONLINE=1
set LIST=game\binaries\x86\skhid-files.txt
if exist "%LIST%" set /p HAVE_SHA=<"%LIST%"
if not "%HAVE_SHA%"=="%FILES_SHA%" goto files_get
for /f "usebackq skip=1 delims=" %%f in ("%LIST%") do if not exist "%%~ff" goto files_get
goto files_done
:files_get
echo Downloading the Skhid files...
curl.exe -fL -o skhid-files.zip "%URL%/download" || goto failed
call :sha skhid-files.zip %FILES_SHA% || goto failed
rem Not this file (cmd reads on in a running script from where it was: a new
rem one would be read wrongly), nor the updater (the file antivirus deletes).
tar -xf skhid-files.zip --exclude Skhid-without-updater.bat --exclude updater_skhid.exe || goto failed
(echo %FILES_SHA%& tar -tf skhid-files.zip --exclude Skhid-without-updater.bat --exclude updater_skhid.exe) > "%LIST%.part" || goto failed
move /y "%LIST%.part" "%LIST%" >nul || goto failed
del skhid-files.zip
goto files_done
:offline
if not exist "game\binaries\x86\survarium_skhid.exe" goto no_server
if not exist "game\binaries\x86\7zr.exe" goto no_server
echo Cannot reach %URL%: the addresses written into this file are used.
:files_done

rem 2. The original game, when it is missing: 2.9 GB download, about 10 GB
rem free needed. Its addresses are tried in their order; a download that
rem broke off goes on at the next one from the bytes it has.
if exist "game\resources.db" if exist "game\binaries\x86\survarium.exe" goto game_done
set ARCHIVE=game\.skhid-game.7z
echo Downloading Survarium 0.26g (2.9 GB)...
set SKHID_STEP=game
call :ps || goto failed
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

rem 3. resources.db's patches: exactly the ones the server lists (or, when it
rem cannot be reached, the ones written into this file).
echo Checking the game's patches...
set SKHID_STEP=patches
call :ps || goto failed
exit /b 0

rem Exit code 0 when the file's SHA-256 is the expected one.
:sha
powershell -NoProfile -Command "if ((Get-FileHash '%~1').Hash -ne '%~2') { Write-Host 'Download damaged: %~nx1. Start this file again.'; exit 1 }"
exit /b %errorlevel%

rem The PowerShell code at the end of this file, for the step SKHID_STEP.
:ps
powershell -NoProfile -ExecutionPolicy Bypass -Command "$t = [IO.File]::ReadAllText($env:SKHID_BAT); iex $t.Substring($t.LastIndexOf('#' + 'skhid-powershell')); exit $global:skhidStatus"
exit /b %errorlevel%

:no_server
echo Cannot reach %URL%. Check the internet connection, or see VPN.txt.
pause
exit /b 1

:old_windows
echo This file needs Windows 10 version 1803 or later (its curl and tar).
echo Update Windows, or start Skhid.bat (the launcher) instead.
pause
exit /b 1

:failed
echo.
echo Something went wrong, so the game is not started. Check the internet
echo connection and the free space (about 10 GB for a new game), close the
echo game if it is open, then start this file again: it goes on from where
echo it stopped.
echo Still failing? Send skhid-download.log (beside this file) to #bugreports
echo in Discord (tag: install): https://discord.gg/QUmJg3gBhg
pause
exit /b 1

rem Nothing below is run by cmd: it is the PowerShell code :ps runs, and the
rem release's files with their addresses.
#skhid-powershell
# One step, $env:SKHID_STEP: `game` (the original game's archive into
# game\.skhid-game.7z) or `patches` (game\resources.db.patch_* made the
# list's). The addresses: the server's (/game, /patches) when it can be
# reached, else the lines `#skhid-file <name> <sha256> <size> <url>...` at
# the end (only missing patches fetched then, none deleted), written in at build time
# (make_client_026g.py --downloads). Each file's addresses are tried in
# their order; any failure moves on to the next, which goes on from the
# bytes on disk (every address has the same pinned bytes). One line of
# skhid-download.log a try, a failure and a success, as the updater's log
# has them.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$global:skhidStatus = 1
$here = Split-Path -Parent $env:SKHID_BAT
$logFile = Join-Path $here 'skhid-download.log'

function Say([string]$what, [string]$verb, [string]$where, [string]$detail) {
    $line = '{0}  {1,-8} {2}: {3}' -f $what, $verb, $where, $detail
    Add-Content -LiteralPath $logFile -Value ((Get-Date -Format 'yyyy-MM-dd HH:mm:ss') + ' ' + $line) -Encoding UTF8
    Write-Host $line
}

function Get-Sha([string]$path) {
    (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
}

# The release's files written into this file: name -> sha256, size, URLs.
function Get-BuiltIn {
    $files = @{}
    foreach ($line in [IO.File]::ReadAllLines($env:SKHID_BAT)) {
        $words = $line.Trim() -split ' '
        if ($words.Count -ge 5 -and $words[0] -eq '#skhid-file') {
            $files[$words[1]] = [pscustomobject]@{ Sha = $words[2]; Size = [long]$words[3]; Urls = @($words[4..($words.Count - 1)]) }
        }
    }
    $files
}

# GET <path> of the server: its JSON, or $null.
function Get-ServerJson([string]$path) {
    if (-not $env:SKHID_ONLINE) { return $null }
    $answer = (& curl.exe -fs --connect-timeout 10 --max-time 60 ($env:URL + $path)) -join ''
    $code = $LASTEXITCODE
    if ($code -ne 0 -or -not $answer) {
        Say 'skhid-list' 'failed' $env:URL "$path cannot be fetched (curl $code)"
        return $null
    }
    try { return ($answer | ConvertFrom-Json) } catch {
        Say 'skhid-list' 'failed' $env:URL "$path is no list"
        return $null
    }
}

# Why curl stopped, by its exit code.
function Get-CurlReason([int]$code, [string]$status) {
    switch ($code) {
        6 { return 'host not found (DNS)' }
        7 { return 'unreachable (connection refused or no route)' }
        28 {
            if ($status -eq '000') { return 'connect timeout (no connection within 10 s)' }
            return 'too slow: under 50 KB/s for 2 minutes'
        }
        18 { return 'cut short' }
        35 { return "no secure connection (curl $code)" }
        47 { return 'more than 5 redirects' }
        52 { return 'no answer' }
        56 { return 'connection reset' }
        60 { return "no secure connection (curl $code)" }
    }
    "curl error $code"
}

# The title of a web page's first bytes, or ''.
function Get-PageTitle([string]$path) {
    $stream = [IO.File]::OpenRead($path)
    $buffer = New-Object byte[] 8192
    $read = $stream.Read($buffer, 0, $buffer.Length)
    $stream.Close()
    $text = [Text.Encoding]::UTF8.GetString($buffer, 0, $read)
    if ($text -match '(?is)<title[^>]*>(.*?)</title') { return (($matches[1] -replace '\s+', ' ').Trim() -replace '"', "'") }
    ''
}

# <path> made the file of <sha> (and <size>; 0: not known) from <urls>, in
# their order. $true once it is there, checked; else $false, with
# $script:failures the host and why of each address.
function Get-File([string]$what, [string]$path, [string]$sha, [long]$size, [string[]]$urls) {
    $script:failures = @()
    $path = Join-Path $here $path
    $try = "$path.try"
    $headers = "$path.headers"
    if ((Test-Path -LiteralPath $path) -and ($size -eq 0 -or (Get-Item -LiteralPath $path -Force).Length -eq $size) -and (Get-Sha $path) -eq $sha) {
        Say $what 'ok' 'here' 'already downloaded, SHA-256 verified'
        return $true
    }
    $at = 0
    foreach ($url in $urls) {
        $at++
        $where = ([Uri]$url).Host
        $have = 0
        if (Test-Path -LiteralPath $path) { $have = (Get-Item -LiteralPath $path -Force).Length }
        if ($size -gt 0 -and $have -ge $size) { Remove-Item -Force -LiteralPath $path; $have = 0 }
        Say $what 'try' $where "address $at of $($urls.Count), $url, from byte $have"
        Remove-Item -Force -LiteralPath $try, $headers -ErrorAction SilentlyContinue
        $options = @('-sS', '-L', '--max-redirs', '5', '--proto', '=http,https', '--proto-redir', '=http,https',
            '--connect-timeout', '10', '-D', $headers, '-o', $try, '-w', '%{http_code} %{size_download} %{content_type}')
        # Too slow is a failure only while another address remains; a stall
        # (nothing for 2 minutes) always is.
        if ($at -lt $urls.Count) { $options += @('--speed-limit', '51200', '--speed-time', '120') }
        else { $options += @('--speed-limit', '1', '--speed-time', '120') }
        if ($have -gt 0) { $options += @('-r', "$have-") }
        $written = (& curl.exe @options $url) -join ''
        $code = $LASTEXITCODE
        $parts = $written -split ' ', 3
        $status = $parts[0]
        $got = 0
        if ($parts.Count -gt 1) { $got = [long]$parts[1] }
        $type = ''
        if ($parts.Count -gt 2) { $type = $parts[2] }
        $why = $null
        if ($type -like 'text/html*') {
            $title = ''
            if (Test-Path -LiteralPath $try) { $title = Get-PageTitle $try }
            if ($title) { $why = "web page `"$title`", not the file" } else { $why = 'web page, not the file' }
        } elseif ($status -eq '206' -and $have -gt 0) {
            # The part from byte $have on, if that is where it starts.
            $range = Get-Content -LiteralPath $headers | Where-Object { $_ -match '^Content-Range:' } | Select-Object -Last 1
            if ($range -match "bytes $have-") {
                if (Test-Path -LiteralPath $try) {
                    $out = [IO.File]::Open($path, 'Append')
                    $in = [IO.File]::OpenRead($try)
                    $in.CopyTo($out)
                    $in.Close()
                    $out.Close()
                }
            } else {
                $why = 'answered another part of the file than was asked for'
            }
        } elseif ($status -eq '416' -and $have -gt 0) {
            # No part from there: the bytes kept are not a part of the file.
            Remove-Item -Force -LiteralPath $path
            $why = "HTTP 416 from byte ${have}: the bytes kept were deleted, the next try starts from 0"
        } elseif ($status -eq '200') {
            # The whole file again (a server that gives no part).
            if (Test-Path -LiteralPath $try) { Move-Item -LiteralPath $try -Destination $path -Force }
        } elseif ($status -ne '000') {
            $why = "HTTP $status"
        }
        Remove-Item -Force -LiteralPath $try, $headers -ErrorAction SilentlyContinue
        # Cut short with its size known: said with the bytes it has, below.
        if (-not $why -and $code -ne 0 -and -not ($code -eq 18 -and $size -gt 0)) { $why = Get-CurlReason $code $status }
        if ($code -eq 28 -and $status -ne '000' -and $at -ge $urls.Count) { $why = 'stalled: nothing received for 2 minutes' }
        if (-not $why) {
            $length = 0
            if (Test-Path -LiteralPath $path) { $length = (Get-Item -LiteralPath $path -Force).Length }
            if (-not (Test-Path -LiteralPath $path)) {
                $why = 'an empty answer'
            } elseif ($size -gt 0 -and $length -lt $size) {
                $why = "cut short at byte $length of $size"
            } elseif ((Get-Sha $path) -ne $sha) {
                Remove-Item -Force -LiteralPath $path
                $why = "wrong SHA-256 (not $sha)"
            } else {
                Say $what 'ok' $where "$length bytes, SHA-256 verified"
                return $true
            }
        }
        Say $what 'failed' $where "$why ($got bytes received)"
        $script:failures += "${where}: $why"
    }
    $false
}

function Show-Failed([string]$what) {
    Write-Host ''
    Write-Host "$what could not be downloaded from any of its addresses:"
    foreach ($failure in $script:failures) { Write-Host "  $failure" }
}

function Invoke-Step {
    if ($env:SKHID_STEP -eq 'game') {
        $urls = $null
        $offered = Get-ServerJson '/game'
        if ($offered -and $offered.sha256 -eq $env:GAME_SHA -and [long]$offered.size -eq [long]$env:GAME_SIZE) {
            $urls = @($offered.urls)
        } else {
            $own = (Get-BuiltIn)['survarium-026g.7z']
            if ($own -and $own.Sha -eq $env:GAME_SHA) { $urls = $own.Urls }
        }
        if (-not $urls) {
            Write-Host 'No address of the original game is known: the server cannot be reached and none is written into this file.'
            return
        }
        if (Get-File 'skhid-game' $env:ARCHIVE $env:GAME_SHA ([long]$env:GAME_SIZE) $urls) { $global:skhidStatus = 0 } else { Show-Failed 'Survarium 0.26g' }
        return
    }
    # The server's list when it can be reached (a list it cannot give is a
    # failure, as ever); else the rows written in, of which a missing or
    # damaged patch is fetched and nothing is deleted: this file is never
    # updated, and the game may need newer patches than it names.
    $files = @()
    if ($env:SKHID_ONLINE) {
        $list = Get-ServerJson '/patches'
        if ($null -eq $list) {
            Write-Host "The list of the game's patches cannot be had from $($env:URL)."
            return
        }
        foreach ($p in $list.PSObject.Properties) {
            $files += [pscustomobject]@{ Name = 'resources.db.' + $p.Name; Sha = $p.Value.sha256; Size = 0; Urls = @(@($p.Value.urls) + @($p.Value.url) | Where-Object { $_ }) }
        }
    } else {
        $own = Get-BuiltIn
        foreach ($name in ($own.Keys | Where-Object { $_ -like 'resources.db.patch_*' } | Sort-Object { [int]($_ -replace '\D', '') })) {
            $files += [pscustomobject]@{ Name = $name; Sha = $own[$name].Sha; Size = $own[$name].Size; Urls = $own[$name].Urls }
        }
        if (-not $files) {
            Write-Host 'The list of the game''s patches cannot be had: the server cannot be reached and none is written into this file.'
            return
        }
    }
    foreach ($file in $files) {
        $place = Join-Path $here ('game\' + $file.Name)
        if ((Test-Path -LiteralPath $place) -and (Get-Sha $place) -eq $file.Sha) { continue }
        Write-Host ('Downloading ' + $file.Name + '...')
        if (-not (Get-File $file.Name ('game\' + $file.Name + '.part') $file.Sha $file.Size $file.Urls)) {
            Show-Failed $file.Name
            return
        }
        Move-Item -LiteralPath ($place + '.part') -Destination $place -Force
    }
    if ($env:SKHID_ONLINE) {
        $keep = @($files | ForEach-Object { $_.Name })
        Get-ChildItem (Join-Path $here 'game\resources.db.patch_*') | Where-Object { $keep -notcontains $_.Name } | Remove-Item
    }
    $global:skhidStatus = 0
}

Invoke-Step
# The release's files and their addresses, one a line, written in at build
# time (make_client_026g.py --downloads):

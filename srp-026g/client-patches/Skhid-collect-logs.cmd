@echo off
rem Skhid-collect-logs.cmd: puts the Skhid game's logs and the state of its
rem folder into one ZIP on the Desktop, for a player to send when nothing
rem else helped. It changes nothing and needs no administrator rights. It
rem belongs beside Skhid.bat, in the folder that holds the folder named
rem game. It speaks Russian when the Windows display language is Russian,
rem Ukrainian or Belarusian, English otherwise.
rem
rem   Skhid-collect-logs.cmd                 make the ZIP
rem   Skhid-collect-logs.cmd --dry-run       say what it would collect, collect nothing
rem
rem What it takes (the PowerShell code at the end of this file):
rem - the logs: updater_skhid.log, srp_patches.log, the launcher's, the
rem   game's own (Documents\Survarium\Survarium*.log), DXVK's,
rem   skhid-download.log of Skhid-without-updater.bat; each through a filter
rem   that removes every line naming a password, a digest, a token, a key, a
rem   cookie, a command line or a URL with a user in it (the crash
rem   reporter's, client/bugtrap/src/sanitize.rs), and the record it starts;
rem - the crash reporter's text reports (SRP-crash-*.txt, beside the game
rem   and in the temporary folder); its dumps are only listed, never taken
rem   (their memory may hold the password);
rem - the updater's small state files and play_skhid.bat;
rem - the list of the folder game (names, sizes, dates) and of this
rem   folder's Skhid files (no other name of it), the sha256 of the game's
rem   programs, its certificate, resources.db and every resources.db.patch_*
rem   (against the server's list), the exe's patch revision;
rem - Windows' version and display language, the free disk space, the
rem   Defender exclusion and Skhid's firewall rules (read, never changed),
rem   the antivirus programs' names, the network adapters' names, Windows'
rem   error events of the game's programs, and what the servers the updater
rem   and the game use answer (each address once, 10 s at most).
rem Never: user.cfg (the saved sign-in), any password or token, a dump, a
rem file outside the game's folder but its logs and crash reports, or the
rem names of other files beside the game.
rem
rem The code page: this file is kept in UTF-8, and make_client_026g.py ships
rem it in code page 866 (DOS Cyrillic). cmd reads each line of a script in
rem the console's code page at the time, so it switches the console to 866 here,
rem before the first line with a Russian letter, and back at the end: the
rem text is right on a Russian Windows (866 already) and on any other. (Not
rem 65001: Windows 7's cmd stops reading a script in it.) Every line above
rem the chcp is ASCII. PowerShell reads its code below in 866 too.
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
set "DRY="
:args
if "%~1"=="" goto args_done
if /i "%~1"=="--dry-run" (set "DRY=1") else goto bad_option
shift
goto args
:args_done

set "X86=%TOP%\game\binaries\x86"
set "SKHID_TOP=%TOP%"
rem Beside the game: its program folder there (whatever Defender left in it).
if not exist "%X86%\" goto wrong_place

rem (No echo of Russian text inside a bracketed block: a ")" in it would end
rem the block.)
%RU% Сбор логов Skhid
%EN% Skhid log collection
echo ====================
%RU% Собирает в один ZIP на рабочем столе то, что поможет понять, почему игра
%RU% не запускается, не обновляется или не входит на сервер:
%EN% Puts into one ZIP on the Desktop what helps us find out why the game does
%EN% not start, update or sign in:
%RU%   - логи игры, лаунчера и программы обновления, текст отчётов о сбоях;
%EN%   - the logs of the game, the launcher and the updater, the crash reports' text;
%RU%   - список файлов папки game и sha256 программ игры, resources.db и патчей;
%EN%   - the list of the files in the folder game, the sha256 of the game's programs,
%EN%     resources.db and its patches;
%RU%   - версию Windows, свободное место, исключение Defender, правила брандмауэра
%RU%     Skhid и то, отвечают ли серверы игры и патчей.
%EN%   - the Windows version, the free space, the Defender exclusion, Skhid's firewall
%EN%     rules and whether the game's and the patches' servers answer.
%RU% Пароли и сохранённый вход (user.cfg), дампы памяти и другие файлы компьютера
%RU% он не берёт; строки логов со словами вроде password или token удаляются.
%RU% Он ничего не меняет и ничего не отправляет: файл отправляете вы.
%EN% It never takes passwords, the saved sign-in (user.cfg), memory dumps or any
%EN% other file of the computer; log lines with words like password or token are
%EN% removed. It changes nothing and sends nothing: you send the file.
%RU% Папка игры: "%TOP%"
%EN% Game folder: "%TOP%"
echo.
if defined DRY goto dry

%RU% Подождите минуту-другую: sha256 больших файлов считается не сразу...
%EN% Wait a minute or two: the sha256 of the large files takes a while...
echo.
powershell -NoProfile -ExecutionPolicy Bypass -Command "$t = [IO.File]::ReadAllText($env:SKHID_SELF, [Text.Encoding]::GetEncoding(866)); iex $t.Substring($t.LastIndexOf('#' + 'skhid-powershell')); exit $global:skhidStatus"
if errorlevel 9009 goto no_powershell
if errorlevel 1 set "RC=1"
goto end

:dry
%RU% Пробный запуск: ничего не собирается, только выводится, что было бы собрано.
%EN% Dry run: nothing is collected, only what would be is printed.
echo.
echo [dry-run] logs: Documents\Survarium, "%X86%", "%TOP%\game", "%TOP%", "%TOP%\temp"
echo [dry-run] crash reports: SRP-crash-*.txt in "%X86%" and the temporary folder
echo [dry-run] sha256: game\binaries\x86 programs, the certificate, game\resources.db, game\resources.db.patch_*
echo [dry-run] servers: updater_skhid.url's (else https://skhid.link) /api/client-files, /patches, /game and their hosts
echo [dry-run] write: Desktop\Skhid-logs-^<date^>-^<time^>.zip
goto end

:bad_option
%RU% Неизвестный параметр: %1
%EN% Unknown option: %1
%RU% Запуск: Skhid-collect-logs.cmd [--dry-run]
%EN% Usage: Skhid-collect-logs.cmd [--dry-run]
set "RC=2"
goto quit

:wrong_place
%RU% Не найдена папка
%EN% There is no folder
echo   "%X86%"
%RU% Положите этот файл рядом со Skhid.bat, в папку, где лежит папка game,
%RU% и запустите его оттуда.
%EN% Put this file beside Skhid.bat, in the folder that holds the folder
%EN% game, and start it from there.
set "RC=1"
goto end

:no_powershell
%RU% Не найден PowerShell, без него собрать логи нельзя. Отправьте сами файл
%RU% game\binaries\x86\updater_skhid.log и логи из папки Документы\Survarium.
%EN% PowerShell is not found, and the logs cannot be collected without it.
%EN% Send game\binaries\x86\updater_skhid.log and the logs in the folder
%EN% Documents\Survarium yourself.
set "RC=1"
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

rem Nothing below is run by cmd: it is the PowerShell code the powershell
rem line above runs.
#skhid-powershell
# Reads only: nothing of the game, its folder or Windows is changed. It
# writes its own folder Skhid-logs-<date>-<time> on the Desktop (else in
# the game folder, when the Desktop cannot be written), makes the ZIP of it
# beside it and deletes the folder. Windows PowerShell 2 (Windows 7) to 5.1;
# PowerShell 7 elsewhere (the tests). What Windows has not (Defender's,
# the firewall's cmdlets, the event log) is said in the report, not failed.
$ErrorActionPreference = 'Stop'
$global:skhidStatus = 1
$top = $env:SKHID_TOP
# A game in a drive's root: "D:" alone is D:'s current folder.
if ($top -match '^[A-Za-z]:$') { $top += [char]92 }
$latin1 = [Text.Encoding]::GetEncoding(28591)
$state = @{ Report = New-Object Collections.ArrayList; Patches = @{}; PatchesFrom = '' }

function Say([string]$ru, [string]$en) { if ($env:SKHID_RU) { Write-Host $ru } else { Write-Host $en } }
function Line([string]$text) { [void]$state.Report.Add($text) }
function Under([string]$from, [string[]]$parts) { foreach ($p in $parts) { $from = [IO.Path]::Combine($from, $p) }; $from }
function Why($e) { while ($e.InnerException) { $e = $e.InnerException }; ($e.Message -replace '\s+', ' ').Trim() }
function Utc($time) { $time.ToUniversalTime().ToString('yyyy-MM-dd HH:mm:ss') }
function Relative([string]$path) { $path.Substring($top.Length).TrimStart([char]92, [char]47) }
# One part of the report: what fails in it is said there, and the rest goes on.
function Part([string]$title, [scriptblock]$body) {
    Line ''
    Line ('== ' + $title)
    try { & $body } catch { Line ('  (not read: ' + (Why $_.Exception) + ')') }
}

$x86 = Under $top @('game', 'binaries', 'x86')
$game = Under $top @('game')
$temp = [IO.Path]::GetTempPath()
$documents = [Environment]::GetFolderPath('MyDocuments')
$gameDocuments = ''
if ($documents) { $gameDocuments = Under $documents @('Survarium') }

# Shared reads: a log the game or the updater is writing is read as it is.
function Open-Shared([string]$path) {
    New-Object IO.FileStream($path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]'ReadWrite, Delete')
}
function Read-Bytes([string]$path, [long]$from, [long]$count) {
    $stream = Open-Shared $path
    try {
        [void]$stream.Seek($from, [IO.SeekOrigin]::Begin)
        $buffer = New-Object byte[] ([int]$count)
        $got = 0
        while ($got -lt $count) {
            $n = $stream.Read($buffer, $got, [int]($count - $got))
            if ($n -le 0) { break }
            $got += $n
        }
        if ($got -lt $count) { $short = New-Object byte[] $got; [Array]::Copy($buffer, $short, $got); $buffer = $short }
        , $buffer
    } finally { $stream.Close() }
}
function Get-Sha([string]$path) {
    $stream = Open-Shared $path
    try { ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash($stream)) -replace '-', '').ToLowerInvariant() }
    finally { $stream.Close() }
}

# The patch revision the exe's sign-in says: the byte of `mov byte
# [esp+27h],<revision>` at 0x380862 (client/updater/logic/src/revision.rs).
function Get-Revision([string]$path) {
    $h = Read-Bytes $path 0 4096
    if ($h.Length -lt 64 -or $h[0] -ne 0x4D -or $h[1] -ne 0x5A) { return 'unknown (not a Windows program)' }
    $pe = [long][BitConverter]::ToInt32($h, 0x3C)
    if ($pe -lt 0 -or $pe + 248 -gt $h.Length -or [BitConverter]::ToUInt32($h, $pe) -ne 0x4550) { return 'unknown (not a Windows program)' }
    if ([BitConverter]::ToUInt16($h, $pe + 24) -ne 0x10B) { return 'unknown (not a 32-bit program)' }
    $sections = [long][BitConverter]::ToUInt16($h, $pe + 6)
    $table = $pe + 24 + [long][BitConverter]::ToUInt16($h, $pe + 20)
    $rva = 0x380862 - [long][BitConverter]::ToUInt32($h, $pe + 52)
    for ($i = 0; $i -lt $sections; $i++) {
        $entry = $table + 40 * $i
        if ($entry + 40 -gt $h.Length) { break }
        $within = $rva - [long][BitConverter]::ToUInt32($h, $entry + 12)
        if ($within -ge 0 -and $within + 6 -le [long][BitConverter]::ToUInt32($h, $entry + 8) -and $within + 6 -le [long][BitConverter]::ToUInt32($h, $entry + 16)) {
            $site = [BitConverter]::ToString((Read-Bytes $path ([long][BitConverter]::ToUInt32($h, $entry + 20) + $within) 6))
            if ($site -match '^C6-44-24-27-([0-9A-F]{2})-90$') { return ('patch revision ' + [Convert]::ToInt32($matches[1], 16)) }
            if ($site -eq '8A-0D-9E-AE-9D-00') { return 'patch revision 0 (the original instruction)' }
            return 'unknown (other bytes at the revision site)'
        }
    }
    'unknown (no section holds the revision site)'
}

# The crash reporter's filter (client/bugtrap/src/sanitize.rs): a line that
# names a credential is replaced by a notice, and the lines after it until
# the next record (a native, updater, srp_patches.log or DXVK line start)
# are removed. Bytes are kept as they are (Latin-1 is byte for byte): the
# game writes its log in its own code page.
$sensitive = '(?i)password|passwd|pwd|passphrase|private[_-]key|account[_-]?digest|authorization|api[_ -]?key|access[_ -]key|bearer|basic |token|secret|session[_ -]key|cookie|credential|command[_ -]?line|cmd[_-]?line|://[^/\s@]*@'
$record = '^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d |[A-Za-z0-9_.-]+ <[A-Za-z]+>\t\[\d\d:\d\d:\d\d[.:]\d{3}\]\t|pid \d+ |(info|warn|err|debug|trace): )'
function Clean([byte[]]$bytes, [bool]$cut) {
    $lines = @($latin1.GetString($bytes) -split '(?<=\n)')
    $from = 0
    # A tail: from its first whole record (else its first whole line), so
    # that no cut inside a removed record's lines is kept.
    if ($cut) {
        $from = 1
        for ($i = 1; $i -lt $lines.Length; $i++) { if ($lines[$i] -match $record) { $from = $i; break } }
    }
    $out = New-Object Text.StringBuilder
    $removed = 0
    $hiding = $false
    for ($i = $from; $i -lt $lines.Length; $i++) {
        $line = $lines[$i]
        if ($line -match $sensitive) { [void]$out.Append("[Skhid: sensitive log record removed]`r`n"); $removed++; $hiding = $true }
        elseif ($hiding -and $line -notmatch $record) { $removed++ }
        else { $hiding = $false; [void]$out.Append($line) }
    }
    @{ Bytes = $latin1.GetBytes($out.ToString()); Removed = $removed }
}
# `file` into the folder at `name` (cleaned, at most its last `cap` bytes):
# what was taken, for the report.
function Take($file, [string]$name, [long]$cap) {
    $what = '{0}  {1} bytes, {2} UTC' -f $name, $file.Length, (Utc $file.LastWriteTime)
    $size = [Math]::Min([long]$file.Length, $cap)
    $bytes = Read-Bytes $file.FullName ($file.Length - $size) $size
    if ([Array]::IndexOf($bytes, [byte]0) -ge 0) { return ($what + ', not taken: not text') }
    $clean = Clean $bytes ($size -lt $file.Length)
    $to = Under $stage ($name -split '/')
    [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($to))
    [IO.File]::WriteAllBytes($to, $clean.Bytes)
    if ($size -lt $file.Length) { $what += ", its last $size bytes" }
    if ($clean.Removed) { $what += ", $($clean.Removed) lines removed (credentials)" }
    $state.Taken += $size
    $what
}

# The events of a log that match `filter` (none: nothing, which
# Get-WinEvent says as an error).
function Get-Events($filter) {
    try { @(Get-WinEvent -FilterHashtable $filter -MaxEvents 500 -ErrorAction Stop) }
    catch { if ([string]$_.FullyQualifiedErrorId -notlike 'NoMatchingEventsFound*') { throw } }
}

# What an address answers: its status or why none, the time, a web page's
# title (a provider's block page, a sign-in page). The body when asked for
# (at most 4 MB), else only its first byte.
function Get-Url([string]$url, [bool]$body) {
    $answer = @{ Said = ''; Body = '' }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $request = [Net.WebRequest]::Create($url)
    try {
        $request.Timeout = 10000
        $request.ReadWriteTimeout = 10000
        $request.UserAgent = 'Skhid-collect-logs'
        if (-not $body) { $request.AddRange(0, 0) }
        $response = $null
        try { $response = $request.GetResponse() } catch {
            $e = $_.Exception
            while ($e.InnerException -and -not ($e -is [Net.WebException])) { $e = $e.InnerException }
            if ($e -is [Net.WebException] -and $e.Response) { $response = $e.Response }
            elseif ($e -is [Net.WebException]) { $answer.Said = [string]$e.Status + ': ' + (Why $e) }
            else { throw }
        }
        if ($response) { try {
            $type = [string]$response.ContentType
            $answer.Said = 'HTTP ' + [int]$response.StatusCode + ', ' + $type
            if ($response.ResponseUri -and $response.ResponseUri.Host -ne ([Uri]$url).Host) { $answer.Said += ', from ' + $response.ResponseUri.Host }
            if ($body -or $type -like 'text/html*') {
                $stream = $response.GetResponseStream()
                $buffer = New-Object byte[] 4194304
                $got = 0
                while ($got -lt $buffer.Length) { $n = $stream.Read($buffer, $got, $buffer.Length - $got); if ($n -le 0) { break }; $got += $n; if (-not $body -and $got -ge 8192) { break } }
                $answer.Body = [Text.Encoding]::UTF8.GetString($buffer, 0, $got)
                if ($type -like 'text/html*' -and $answer.Body -match '(?is)<title[^>]*>(.*?)</title') { $answer.Said += ', web page "' + (($matches[1] -replace '\s+', ' ').Trim()) + '"' }
            }
        } finally { $response.Close() } }
    } catch { $answer.Said = 'error: ' + (Why $_.Exception) }
    finally { try { $request.Abort() } catch {} }
    $answer.Said += ', ' + $clock.ElapsedMilliseconds + ' ms'
    $answer
}
# The addresses a name has (a provider's DNS may answer another), and the
# proxy Windows uses for an address, if any (its host alone).
function Get-Names([string]$name) {
    try { (@([Net.Dns]::GetHostAddresses($name)) | ForEach-Object { $_.ToString() }) -join ', ' } catch { 'not found: ' + (Why $_.Exception) }
}
function Get-Proxy([string]$url) {
    try {
        $proxy = [Net.WebRequest]::DefaultWebProxy
        if (-not $proxy) { return '' }
        $via = $proxy.GetProxy([Uri]$url)
        if ($via -and $via.Host -ne ([Uri]$url).Host) { return ' (through the proxy ' + $via.Host + ':' + $via.Port + ')' }
    } catch {}
    ''
}
function Test-Tcp([string]$name, [int]$port) {
    $client = New-Object Net.Sockets.TcpClient
    $clock = [Diagnostics.Stopwatch]::StartNew()
    try {
        $wait = $client.BeginConnect($name, $port, $null, $null)
        if (-not $wait.AsyncWaitHandle.WaitOne(10000, $false)) { return 'no answer within 10 s' }
        $client.EndConnect($wait)
        'connected, ' + $clock.ElapsedMilliseconds + ' ms'
    } catch { 'failed: ' + (Why $_.Exception) + ', ' + $clock.ElapsedMilliseconds + ' ms' }
    finally { $client.Close() }
}

try {
    # The folder: on the Desktop, else in the game folder; a new name.
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $stage = $null
    foreach ($place in @([Environment]::GetFolderPath('Desktop'), $top)) {
        if (-not $place) { continue }
        for ($n = 1; $n -le 9 -and -not $stage; $n++) {
            $name = 'Skhid-logs-' + $stamp
            if ($n -gt 1) { $name += '-' + $n }
            $candidate = Under $place @($name)
            if ([IO.Directory]::Exists($candidate) -or [IO.File]::Exists($candidate + '.zip')) { continue }
            try { [void][IO.Directory]::CreateDirectory($candidate); $stage = $candidate } catch { break }
        }
        if ($stage) { break }
    }
    if (-not $stage) { throw 'no folder could be written on the Desktop or in the game folder' }
    $state.Taken = 0

    Line ('Skhid log collection, ' + (Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz') + ' (' + (Utc (Get-Date)) + ' UTC)')
    Line ('Game folder: ' + $top)

    Part 'Windows' {
        try {
            $nt = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
            $release = $nt.DisplayVersion
            if (-not $release) { $release = $nt.ReleaseId }
            Line ('  ' + $nt.ProductName + ' ' + $release + ', build ' + $nt.CurrentBuild + '.' + $nt.UBR)
        } catch { Line ('  ' + [Environment]::OSVersion.VersionString) }
        $arch = $env:PROCESSOR_ARCHITEW6432
        if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
        Line ('  processor: ' + $arch + ', PowerShell ' + $PSVersionTable.PSVersion + ', display language ' + (Get-UICulture).Name)
        if (Test-Path 'HKCU:\Software\Wine') { Line '  Wine: yes' }
        $drive = New-Object IO.DriveInfo([IO.Path]::GetPathRoot($top))
        Line ('  free on {0}: {1:N1} GB of {2:N1} GB' -f $drive.Name, ($drive.AvailableFreeSpace / 1GB), ($drive.TotalSize / 1GB))
    }

    Say 'Серверы...' 'Servers...'
    Part 'Servers' {
        try { [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor 3072 } catch { Line '  (no TLS 1.2 in this PowerShell)' }
        # The server the updater asks (the defender setup's rule).
        $site = ''
        $file = Under $x86 @('updater_skhid.url')
        if ([IO.File]::Exists($file)) { $site = ([IO.File]::ReadAllText($file) -split '[\r\n]')[0].Trim() }
        if ($site -notmatch '^(https://[^/\s]+|http://(localhost|127\.0\.0\.1)(:\d+)?)(/\S*)?$') { $site = 'https://skhid.link' }
        $site = $site.TrimEnd('/')
        Line ('  the updater asks: ' + $site + (Get-Proxy $site))
        # The hosts asked, in tables the functions below change (a table is
        # shared through the caller's scope; a variable set there is not).
        $hosts = @{}
        $tested = @{ (([Uri]$site).Scheme + '://' + ([Uri]$site).Authority) = 1 }
        function Check([string]$url, [bool]$body) {
            $uri = [Uri]$url
            if (-not $hosts.ContainsKey($uri.Host)) { $hosts[$uri.Host] = 1; Line ('  DNS ' + $uri.Host + ': ' + (Get-Names $uri.Host)) }
            $answer = Get-Url $url $body
            Line ('  GET ' + $url + ': ' + $answer.Said)
            $answer
        }
        # Each other host once: its first address the lists give.
        function Check-Host([string]$url) {
            try { $uri = [Uri]$url } catch { return }
            $key = $uri.Scheme + '://' + $uri.Authority
            if ($tested.ContainsKey($key)) { return }
            $tested[$key] = 1
            [void](Check $url $false)
        }
        [void](Check ($site + '/api/client-files') $true)
        $list = Check ($site + '/patches') $true
        foreach ($m in [regex]::Matches($list.Body, '"(patch_\d+)"\s*:\s*\{([^{}]*)\}')) {
            $sha = [regex]::Match($m.Groups[2].Value, '"sha256"\s*:\s*"([0-9A-Fa-f]{64})"').Groups[1].Value.ToLowerInvariant()
            $state.Patches['resources.db.' + $m.Groups[1].Value] = $sha
            $state.PatchesFrom = $site + '/patches'
            foreach ($u in [regex]::Matches($m.Groups[2].Value, '"(https?://[^"]+)"')) { Check-Host $u.Groups[1].Value }
        }
        $offered = Check ($site + '/game') $true
        foreach ($u in [regex]::Matches($offered.Body, '"(https?://[^"]+)"')) { Check-Host $u.Groups[1].Value }
        # The release's own addresses, written into Skhid-without-updater.bat.
        $alone = Under $top @('Skhid-without-updater.bat')
        if ([IO.File]::Exists($alone)) {
            foreach ($l in [IO.File]::ReadAllLines($alone)) {
                $words = $l.Trim() -split ' '
                if ($words.Length -ge 5 -and $words[0] -eq '#skhid-file') {
                    if (-not $state.PatchesFrom -and $words[1] -like 'resources.db.patch_*') { $state.Patches[$words[1]] = $words[2].ToLowerInvariant() }
                    foreach ($u in $words[4..($words.Length - 1)]) { Check-Host $u }
                }
            }
            if (-not $state.PatchesFrom -and $state.Patches.Count) { $state.PatchesFrom = 'Skhid-without-updater.bat (the server gave no list)' }
        }
        # The sign-in: the login server play_skhid.bat names, else the site's host, TCP 1234.
        $login = ([Uri]$site).Host
        $port = 1234
        $bat = Under $x86 @('play_skhid.bat')
        if ([IO.File]::Exists($bat)) {
            $said = [regex]::Match([IO.File]::ReadAllText($bat), 'embedded in the exe: ([A-Za-z0-9.-]+):(\d+)')
            if ($said.Success) { $login = $said.Groups[1].Value; $port = [int]$said.Groups[2].Value }
        }
        if (-not $hosts.ContainsKey($login)) { Line ('  DNS ' + $login + ': ' + (Get-Names $login)) }
        Line ('  TCP ' + $login + ':' + $port + ' (sign-in): ' + (Test-Tcp $login $port))
        Line ('  (UDP 1235 and 1236, the lobby and the matches, cannot be tried from here)')
    }

    Part 'Network adapters (up)' {
        foreach ($a in [Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces()) {
            if ([string]$a.OperationalStatus -eq 'Up') { Line ('  ' + $a.Name + ': ' + $a.Description + ' (' + $a.NetworkInterfaceType + ')') }
        }
    }

    Say 'Файлы игры (sha256)...' 'The game files (sha256)...'
    Part 'Game files' {
        $files = @()
        foreach ($n in @('survarium_skhid.exe', 'survarium.exe', 'launcher_skhid.exe', 'survarium_launcher.exe', 'updater_skhid.exe', 'bugtrap.dll', '7zr.exe')) { $files += Under $x86 @($n) }
        $files += Under $game @('resources', 'ssl', 'survarium_login_server.crt')
        $files += Under $game @('resources.db')
        $patches = @()
        if ([IO.Directory]::Exists($game)) {
            $patches = @(([IO.DirectoryInfo]$game).GetFiles('resources.db.patch_*') | Sort-Object { [int]('0' + ($_.Name -replace '^resources\.db\.patch_(\d*).*$', '$1')) } | ForEach-Object { $_.FullName })
        }
        foreach ($path in ($files + $patches)) {
            $name = Relative $path
            $file = New-Object IO.FileInfo($path)
            if (-not $file.Exists) { Line ('  ' + $name + ': missing'); continue }
            $what = '  {0}  {1} bytes, {2} UTC' -f $name, $file.Length, (Utc $file.LastWriteTime)
            if ($file.Name -match '\.(exe|dll)$') {
                try { $v = [Diagnostics.FileVersionInfo]::GetVersionInfo($path); if ($v.FileVersion) { $what += ', version ' + $v.FileVersion } } catch {}
            }
            if ($file.Name -eq 'survarium_skhid.exe') { try { $what += ', ' + (Get-Revision $path) } catch { $what += ', revision not read: ' + (Why $_.Exception) } }
            if ($file.Length -gt 64MB) { Write-Host ('  ' + $file.Name + ' ({0:N1} GB)' -f ($file.Length / 1GB)) }
            try { $sha = Get-Sha $path; $what += ', sha256 ' + $sha } catch { $sha = ''; $what += ', not read: ' + (Why $_.Exception) }
            if ($file.Name -like 'resources.db.patch_*' -and $state.PatchesFrom) {
                if (-not $state.Patches.ContainsKey($file.Name)) { $what += ', NOT in the list' }
                elseif ($state.Patches[$file.Name] -eq $sha) { $what += ', matches the list' }
                else { $what += ', DIFFERS from the list (' + $state.Patches[$file.Name] + ')' }
            }
            Line $what
        }
        if ($state.PatchesFrom) {
            Line ('  the patch list: ' + $state.PatchesFrom)
            foreach ($p in ($state.Patches.Keys | Sort-Object)) {
                $path = Under $game @($p)
                if (-not [IO.File]::Exists($path)) { Line ('  ' + (Relative $path) + ': in the list, MISSING here') }
            }
        }
    }

    Say 'Логи...' 'Logs...'
    Part 'Logs taken (newest first)' {
        $name = '^(survarium(_.*)?\.(log|journal)|updater_skhid\.log|launcher(_skhid)?\.log|srp_patches\.log|skhid-download\.log|skhid-wine\.log)$'
        $places = @(@('documents', $gameDocuments), @('game-x86', $x86), @('game', $game), @('game-folder', $top), @('game-folder-temp', (Under $top @('temp'))))
        $found = @()
        foreach ($place in $places) {
            if (-not $place[1] -or -not [IO.Directory]::Exists($place[1])) { Line ('  ' + $place[0] + ': no folder'); continue }
            foreach ($f in ([IO.DirectoryInfo]$place[1]).GetFiles()) { if ($f.Name -match $name) { $found += @{ Place = $place[0]; File = $f } } }
        }
        # Up to 8 MB of each (its end), 40 MB in all: the ZIP stays small enough to send.
        foreach ($item in @($found | Sort-Object { $_.File.LastWriteTimeUtc } -Descending)) {
            $entry = 'logs/' + $item.Place + '/' + $item.File.Name
            if ($state.Taken -ge 40MB) { Line ('  ' + $entry + ', ' + $item.File.Length + ' bytes: not taken, the ZIP is full'); continue }
            try { Line ('  ' + (Take $item.File $entry ([Math]::Min(8MB, 40MB - $state.Taken)))) } catch { Line ('  ' + $entry + ': not read: ' + (Why $_.Exception)) }
        }
        if (-not $found) { Line '  none found' }
    }

    Part 'Crash reports' {
        $places = @(@('game-x86', $x86), @('temp', $temp), @('documents', $gameDocuments))
        $reports = @()
        $dumps = @()
        foreach ($place in $places) {
            if (-not $place[1] -or -not [IO.Directory]::Exists($place[1])) { continue }
            $dir = [IO.DirectoryInfo]$place[1]
            $files = @($dir.GetFiles('SRP-crash-*'))
            if ($place[0] -ne 'temp') { $files += @($dir.GetFiles('*.dmp') | Where-Object { $_.Name -notlike 'SRP-crash-*' }) }
            foreach ($f in $files) {
                if ($f.Name -like 'SRP-crash-*.txt') { $reports += @{ Place = $place[0]; File = $f } }
                else { $dumps += @{ Place = $place[0]; File = $f } }
            }
        }
        $newest = @($reports | Sort-Object { $_.File.LastWriteTimeUtc } -Descending)
        for ($i = 0; $i -lt $newest.Length; $i++) {
            $entry = 'crash/' + $newest[$i].Place + '/' + $newest[$i].File.Name
            if ($i -ge 20) { Line ('  ' + $entry + ': not taken, only the newest 20'); continue }
            try { Line ('  ' + (Take $newest[$i].File $entry 1MB)) } catch { Line ('  ' + $entry + ': not read: ' + (Why $_.Exception)) }
        }
        if (-not $reports) { Line '  no SRP-crash-*.txt' }
        # The dumps hold the game's memory (perhaps the password): named, never taken.
        foreach ($d in @($dumps | Sort-Object { $_.File.LastWriteTimeUtc } -Descending)) {
            Line ('  not taken: {0}/{1}  {2} bytes, {3} UTC' -f $d.Place, $d.File.Name, $d.File.Length, (Utc $d.File.LastWriteTime))
        }
    }

    Part 'State files' {
        $small = @()
        foreach ($n in @('updater_skhid.url', 'updater_skhid.state', 'updater_skhid.patches', 'skhid-files.txt', 'play_skhid.bat')) { $small += , @('game-x86', (Under $x86 @($n))) }
        $small += , @('game-folder', (Under $top @('Skhid-network-setup.defender')))
        foreach ($s in $small) {
            $file = New-Object IO.FileInfo($s[1])
            if (-not $file.Exists) { Line ('  ' + (Relative $s[1]) + ': none'); continue }
            Line ('  ' + (Take $file ('state/' + $s[0] + '/' + $file.Name) 1MB))
        }
    }

    Say 'Defender и брандмауэр...' 'Defender and the firewall...'
    Part 'Microsoft Defender' {
        try {
            $status = Get-MpComputerStatus
            Line ('  real-time protection: ' + $status.RealTimeProtectionEnabled + ', antivirus: ' + $status.AntivirusEnabled + ', version ' + $status.AMProductVersion)
        } catch { Line ('  status not read: ' + (Why $_.Exception)) }
        try {
            # Whether game\binaries\x86 is excluded; no other path of the list is written.
            $paths = @((Get-MpPreference).ExclusionPath | Where-Object { $_ })
            if (@($paths | Where-Object { $_ -like 'N/A*' }).Length) { Line '  the exclusion: not shown (Windows shows exclusions to administrators only)' }
            elseif (@($paths | Where-Object { ($x86 + '\').StartsWith($_.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) }).Length) { Line '  the exclusion: game\binaries\x86 is excluded' }
            else { Line '  the exclusion: game\binaries\x86 is NOT excluded' }
        } catch { Line ('  the exclusion not read: ' + (Why $_.Exception)) }
        $marker = Under $top @('Skhid-network-setup.defender')
        if ([IO.File]::Exists($marker)) { Line '  Skhid-defender-setup.cmd added one (its marker is here)' } else { Line '  no marker of Skhid-defender-setup.cmd' }
        try {
            $events = @(Get-Events @{ LogName = 'Microsoft-Windows-Windows Defender/Operational'; Id = 1116, 1117, 1118, 1119 } |
                Where-Object { $_.Message -match 'survarium|skhid|bugtrap|updater_skhid|launcher_skhid|7zr' } | Select-Object -First 30)
            Line ('  detections of the game''s files: ' + $events.Length)
            foreach ($e in $events) { Line ('  ' + (Utc $e.TimeCreated) + ' UTC, event ' + $e.Id + ': ' + (($e.Message -replace '\s+', ' ').Trim())) }
        } catch { Line ('  detections not read: ' + (Why $_.Exception)) }
    }

    Part 'Other antivirus programs' {
        $products = $null
        try { $products = @(Get-CimInstance -Namespace 'root/SecurityCenter2' -ClassName AntiVirusProduct -ErrorAction Stop) } catch {
            $products = @(Get-WmiObject -Namespace 'root\SecurityCenter2' -Class AntiVirusProduct -ErrorAction Stop)
        }
        foreach ($p in $products) { Line ('  ' + $p.displayName + ' (state ' + $p.productState + ')') }
        if (-not $products) { Line '  none registered' }
    }

    Part 'Firewall rules' {
        foreach ($r in @(Get-NetFirewallRule -DisplayName 'Skhid *' -ErrorAction SilentlyContinue)) { Line ('  ' + $r.DisplayName + ': enabled ' + $r.Enabled + ', ' + $r.Direction + ' ' + $r.Action) }
        foreach ($p in @('launcher_skhid.exe', 'survarium_skhid.exe', 'updater_skhid.exe')) {
            foreach ($r in @(Get-NetFirewallApplicationFilter -Program (Under $x86 @($p)) -ErrorAction SilentlyContinue | Get-NetFirewallRule -ErrorAction SilentlyContinue)) {
                if ($r.DisplayName -notlike 'Skhid *') { Line ('  ' + $r.DisplayName + ' (' + $p + '): enabled ' + $r.Enabled + ', ' + $r.Direction + ' ' + $r.Action) }
            }
        }
    }

    Part 'Windows error events of the game''s programs (30 days)' {
        $events = @(Get-Events @{ LogName = 'Application'; ProviderName = 'Application Error', 'Application Hang', 'Windows Error Reporting', 'SideBySide'; StartTime = (Get-Date).AddDays(-30) } |
            Where-Object { $_.Message -match 'survarium|skhid|bugtrap|7zr' } | Select-Object -First 30)
        foreach ($e in $events) { Line ('  ' + (Utc $e.TimeCreated) + ' UTC, ' + $e.ProviderName + ' ' + $e.Id + ': ' + (($e.Message -replace '\s+', ' ').Trim())) }
        if (-not $events) { Line '  none' }
    }

    # The listing: the folder game whole, and of the folder above it only
    # the Skhid files (other names there are the player's own).
    Say 'Список файлов...' 'The file list...'
    $listing = New-Object Collections.ArrayList
    [void]$listing.Add('size  modified (UTC)  path, relative to ' + $top)
    function List-Entry($item) {
        if ($listing.Count -gt 20000) { return }
        if ($item -is [IO.DirectoryInfo]) {
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { [void]$listing.Add('<link>  ' + (Utc $item.LastWriteTime) + '  ' + (Relative $item.FullName) + '\ (not followed)'); return }
            [void]$listing.Add('<dir>  ' + (Utc $item.LastWriteTime) + '  ' + (Relative $item.FullName) + '\')
            try { foreach ($i in @($item.GetFileSystemInfos() | Sort-Object Name)) { List-Entry $i } } catch { [void]$listing.Add('  (not read: ' + (Why $_.Exception) + ')') }
        } else { [void]$listing.Add([string]$item.Length + '  ' + (Utc $item.LastWriteTime) + '  ' + (Relative $item.FullName)) }
    }
    $others = 0
    foreach ($i in @(([IO.DirectoryInfo]$top).GetFileSystemInfos() | Sort-Object Name)) {
        if ($i.FullName -eq $stage) { continue }
        if ($i.Name -match '^(game|temp|skhid.*|vpn\.txt|readme\.txt|linux\.txt)$') { List-Entry $i } else { $others++ }
    }
    if ($others) { [void]$listing.Add("($others other files and folders beside game, not listed)") }
    if ($listing.Count -gt 20000) { [void]$listing.Add('(more not listed)') }
    [IO.File]::WriteAllLines((Under $stage @('folder.txt')), [string[]]$listing.ToArray(), (New-Object Text.UTF8Encoding $true))
    [IO.File]::WriteAllLines((Under $stage @('report.txt')), [string[]]$state.Report.ToArray(), (New-Object Text.UTF8Encoding $true))

    # The ZIP: .NET 4.5's ZipFile, else Windows 10's tar; else the folder stays.
    Say 'ZIP...' 'ZIP...'
    $zip = $stage + '.zip'
    $zipped = $false
    try {
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $archive = [IO.Compression.ZipFile]::Open($zip, 'Create')
        try {
            foreach ($f in [IO.Directory]::GetFiles($stage, '*', 'AllDirectories')) {
                $entry = $f.Substring($stage.Length + 1).Replace([char]92, [char]47)
                [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $f, $entry, 'Optimal')
            }
        } finally { $archive.Dispose() }
        $zipped = $true
    } catch {
        try { [IO.File]::Delete($zip) } catch {}
        try {
            & tar.exe -a -c -f $zip -C $stage .
            if ($LASTEXITCODE -eq 0 -and [IO.File]::Exists($zip)) { $zipped = $true } else { try { [IO.File]::Delete($zip) } catch {} }
        } catch {}
    }
    Write-Host ''
    if ($zipped) {
        [IO.Directory]::Delete($stage, $true)
        Say 'Готово. Файл с логами:' 'Done. The file with the logs:'
        Write-Host ('  ' + $zip)
        $sent = $zip
    } else {
        Say 'Готово, но Windows не смогла сделать ZIP. Логи в папке:' 'Done, but Windows could not make the ZIP. The logs are in the folder:'
        Write-Host ('  ' + $stage)
        Say 'Сожмите её сами: правой кнопкой мыши, "Отправить", "Сжатая ZIP-папка".' 'Zip it yourself: right-click it, "Send to", "Compressed (zipped) folder".'
        $sent = $stage
    }
    Say 'Отправьте его в канал #bugreports в Discord: https://discord.gg/Pxa4prXfwu' 'Send it to #bugreports in Discord: https://discord.gg/Pxa4prXfwu'
    Say 'Внутри только текст: его можно открыть и посмотреть.' 'It holds only text: you can open it and look.'
    if ($env:OS -eq 'Windows_NT') { try { Start-Process explorer.exe -ArgumentList ('/select,"' + $sent + '"') } catch {} }
    $global:skhidStatus = 0
} catch {
    Say ('Не удалось собрать логи: ' + (Why $_.Exception)) ('The logs could not be collected: ' + (Why $_.Exception))
    if ($stage -and [IO.Directory]::Exists($stage)) { try { [IO.Directory]::Delete($stage, $true) } catch {} }
}

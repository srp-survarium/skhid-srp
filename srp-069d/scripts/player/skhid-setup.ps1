<#
.SYNOPSIS
Writes the start file Skhid.cmd that starts Survarium 0.69d on a Skhid server, without Steam.

.DESCRIPTION
Asks for the server address, finds your game folder, asks the server for its start data and checks it, then writes ONE
file: Skhid.cmd on your desktop (or in -Folder). Double-click Skhid.cmd to play.

The start data is plain text from http://<server>:65001/skhid/client.txt: the server's login address and login port.
The script checks every value before it uses it, downloads nothing else and runs nothing from the server. The login
key that protects the password needs no setup: the server sends it at every sign-in and the game keeps it.

Skhid.cmd holds a marker line and two command lines:
  cd /d "<game folder>\game\binaries\x64"
  start "" survarium.exe -product_name=Survarium-Steam -server=<address>:<port> -localization=<language>
Nothing in the game folder changes. skhid-undo.cmd deletes the file again.

.PARAMETER Server
The server address your server operator gave you (host name or IPv4 address, no port).
.PARAMETER UpdatePort
The server's update-check port, where it serves the start data (default 65001).
.PARAMETER Language
The game language: english, russian, ukrainian, german, french, polish, spanish, portuguese, turkish or chinese
(default english).
.PARAMETER Game
Your game folder (the Survarium folder, or the folder with survarium.exe), if the script does not find it.
.PARAMETER Folder
Where to write Skhid.cmd (default: your desktop).
.PARAMETER LoginServer
The login address for the start file if it differs from -Server (normally leave it out).
.PARAMETER Force
Replace a Skhid.cmd that this script did not write.

.EXAMPLE
skhid-setup.cmd -Server 203.0.113.5
.EXAMPLE
skhid-setup.cmd -Server play.example.org -Language russian -Game "D:\Games\Survarium"
#>
param(
    [string]$Server,
    [int]$UpdatePort = 65001,
    [string]$Language,
    [string]$Game,
    [string]$Folder,
    [string]$LoginServer,
    [switch]$Force
)
$ErrorActionPreference = 'Stop'

function Fail([string]$Text) {
    Write-Host ''
    Write-Host "NOT DONE: $Text" -ForegroundColor Red
    exit 1
}

# The game's languages (the same list as crates/game-data/src/texts.rs, LANGUAGES).
$languages = @('english', 'russian', 'ukrainian', 'german', 'french', 'polish', 'spanish', 'portuguese', 'turkish', 'chinese')
# The first line of every Skhid.cmd this script writes; skhid-undo deletes only a file that starts with it.
$marker = '@rem skhid-setup:'
$hostPattern = '^[A-Za-z0-9.-]{1,200}$'

# ---- the server address and the language
$asked = $false
if (-not $Server) {
    $Server = Read-Host 'Server address (from your server operator, e.g. 203.0.113.5)'
    $asked = $true
}
$Server = "$Server".Trim()
if ($Server -notmatch $hostPattern) { Fail "'$Server' is not a server address (letters, digits, '.' and '-' only; no port, no http://)." }
if ($UpdatePort -lt 1 -or $UpdatePort -gt 65535) { Fail "update port $UpdatePort is not 1..65535." }
if ($LoginServer) {
    $LoginServer = "$LoginServer".Trim()
    if ($LoginServer -notmatch $hostPattern) { Fail "'$LoginServer' is not a server address." }
}
if (-not $PSBoundParameters.ContainsKey('Language') -and $asked) {
    Write-Host "Game languages: $($languages -join ', ')"
    $Language = Read-Host 'Language (press Enter for english)'
}
$Language = "$Language".Trim().ToLowerInvariant()
if (-not $Language) { $Language = 'english' }
if ($Language -cnotmatch '^[a-z]{1,20}$') { Fail "language '$Language': lower-case letters only, e.g. english." }
if ($languages -notcontains $Language) { Write-Warning "'$Language' is not one of the game's languages ($($languages -join ', '))." }

# ---- the game folder
# A folder -> the folder that holds survarium.exe, or $null. Takes the Survarium folder, game\binaries\x64 or the exe.
function Find-Exe([string]$Path) {
    $p = "$Path".Trim().Trim('"')
    if (-not $p) { return $null }
    try {
        if (Test-Path -LiteralPath $p -PathType Leaf) {
            if ((Split-Path -Leaf $p) -eq 'survarium.exe') { return (Resolve-Path -LiteralPath (Split-Path -Parent $p)).ProviderPath }
            return $null
        }
        foreach ($sub in @('game\binaries\x64', 'binaries\x64', 'x64', '.')) {
            $dir = Join-Path $p $sub
            if (Test-Path -LiteralPath (Join-Path $dir 'survarium.exe') -PathType Leaf) { return (Resolve-Path -LiteralPath $dir).ProviderPath }
        }
    } catch { }
    return $null
}

# Where Steam and people usually put the game: every Steam library (libraryfolders.vdf), then common folders.
function Game-Candidates {
    $steam = @()
    try { $steam += ((Get-ItemProperty -Path 'HKCU:\Software\Valve\Steam' -Name SteamPath -ErrorAction Stop).SteamPath -replace '/', '\') } catch { }
    foreach ($k in @('HKLM:\SOFTWARE\WOW6432Node\Valve\Steam', 'HKLM:\SOFTWARE\Valve\Steam')) {
        try { $steam += (Get-ItemProperty -Path $k -Name InstallPath -ErrorAction Stop).InstallPath } catch { }
    }
    foreach ($pf in @(${env:ProgramFiles(x86)}, $env:ProgramFiles)) { if ($pf) { $steam += (Join-Path $pf 'Steam') } }
    $libraries = @()
    foreach ($s in $steam) {
        if (-not $s) { continue }
        $libraries += $s
        $vdf = Join-Path $s 'steamapps\libraryfolders.vdf'
        if (Test-Path -LiteralPath $vdf -PathType Leaf) {
            $text = [System.IO.File]::ReadAllText($vdf)
            foreach ($m in [regex]::Matches($text, '"path"\s+"([^"]+)"')) { $libraries += ($m.Groups[1].Value -replace '\\\\', '\') }
        }
    }
    $out = @()
    foreach ($l in $libraries) { $out += (Join-Path $l 'steamapps\common\Survarium') }
    foreach ($d in [System.IO.DriveInfo]::GetDrives()) {
        if ($d.DriveType -ne 'Fixed' -or -not $d.IsReady) { continue }
        foreach ($sub in @('SteamLibrary\steamapps\common\Survarium', 'Steam\steamapps\common\Survarium', 'Games\Survarium', 'Survarium')) {
            $out += (Join-Path $d.RootDirectory.FullName $sub)
        }
    }
    foreach ($pf in @(${env:ProgramFiles(x86)}, $env:ProgramFiles)) { if ($pf) { $out += (Join-Path $pf 'Survarium') } }
    return $out | Select-Object -Unique
}

$x64 = $null
if ($Game) {
    $x64 = Find-Exe $Game
    if (-not $x64) { Fail "no survarium.exe in '$Game' (looked in it and in its game\binaries\x64 folder)." }
} else {
    foreach ($c in (Game-Candidates)) {
        $x64 = Find-Exe $c
        if ($x64) { break }
    }
    if (-not $x64) {
        Write-Host 'The game folder was not found in the usual places.'
        Write-Host 'In Steam: right-click Survarium -> Manage -> Browse local files, then copy the address bar of that window.'
        $answer = Read-Host 'Game folder (the Survarium folder)'
        $x64 = Find-Exe $answer
        if (-not $x64) { Fail "no survarium.exe in '$answer' (looked in it and in its game\binaries\x64 folder)." }
    }
}
Write-Host "Game found: $x64\survarium.exe"

# ---- the start data from the server (plain text; every value is checked before it is used)
$url = "http://${Server}:$UpdatePort/skhid/client.txt"
try {
    $r = Invoke-WebRequest -UseBasicParsing -TimeoutSec 10 -MaximumRedirection 0 -Uri $url
} catch {
    Fail "no start data from ${Server}:$UpdatePort ($($_.Exception.Message)). Check the address; ask your server operator whether the server runs and TCP port $UpdatePort is open."
}
$text = $r.Content
if ($text -is [byte[]]) { $text = [System.Text.Encoding]::ASCII.GetString($text) }
$text = "$text"
if ($text.Length -gt 4096) { Fail "${Server}:$UpdatePort answered with something that is not Skhid start data." }
$data = @{}
foreach ($line in ($text -split "`n")) {
    if ($line.TrimEnd("`r") -cmatch '^([a-z_]+)=([ -~]*)$' -and -not $data.ContainsKey($Matches[1])) { $data[$Matches[1]] = $Matches[2] }
}
if (-not $data.ContainsKey('format')) { Fail "${Server}:$UpdatePort answered with something that is not Skhid start data. Check the address and the port." }
if ($data['format'] -ne '1') { Fail "the server's start data has format '$($data['format'])'; this script reads format 1. Get the current skhid-setup files from your server operator." }
$portText = $data['login_port']
if ($portText -notmatch '^[0-9]{1,5}$' -or [int]$portText -lt 1 -or [int]$portText -gt 65535) { Fail "the server's login port '$portText' is not 1..65535." }
$loginPort = [int]$portText
$serverHost = $data['login_host']
if ($serverHost -and $serverHost -notmatch $hostPattern) { Fail "the server's login address '$serverHost' is not a server address." }
Write-Host "Server answers: login port $loginPort."

$loginHost = $Server
if ($LoginServer) {
    $loginHost = $LoginServer
} elseif ($serverHost -and $serverHost -ne $Server -and -not $serverHost.StartsWith('127.')) {
    Write-Host "Note: the server gives its address as $serverHost; the start file uses $Server, the address you typed."
    Write-Host "      If the game cannot connect, run this again with: -LoginServer $serverHost"
}

# ---- the start file
if ($Folder) { $dir = "$Folder".Trim().Trim('"') } else { $dir = [Environment]::GetFolderPath('Desktop') }
if (-not $dir -or -not (Test-Path -LiteralPath $dir -PathType Container)) { Fail "the folder '$dir' does not exist." }
$file = Join-Path $dir 'Skhid.cmd'
if (Test-Path -LiteralPath $file) {
    if (Test-Path -LiteralPath $file -PathType Container) { Fail "$file is a folder." }
    $old = [System.IO.File]::ReadAllText($file)
    if (-not $old.StartsWith($marker) -and -not $Force) {
        Fail "$file exists and was not written by this script. Look at it first; run again with -Force to replace it."
    }
}
# The same game line as the server's launch line (servers/srp/src/setup.rs, launch_line); '%' doubled for cmd.
$lines = @(
    "$marker written by skhid-setup for $Server. Double-click to play; skhid-undo.cmd deletes this file.",
    "cd /d `"$($x64 -replace '%', '%%')`"",
    "start `"`" survarium.exe -product_name=Survarium-Steam -server=${loginHost}:$loginPort -localization=$Language"
)
$content = ($lines -join "`r`n") + "`r`n"
# cmd reads a .cmd file in the console's code page: write it in that code page, and refuse a folder name it cannot hold.
$encoding = [Console]::OutputEncoding
$bytes = $encoding.GetBytes($content)
if ($encoding.GetString($bytes) -ne $content) {
    Fail "the game folder '$x64' has letters a start file cannot hold. Use the Steam setup, or a game folder with Latin letters only."
}
[System.IO.File]::WriteAllBytes($file, $bytes)

Write-Host ''
Write-Host "Written: $file" -ForegroundColor Green
foreach ($l in $lines) { Write-Host "    $l" }
Write-Host ''
Write-Host 'Next: double-click Skhid (Skhid.cmd) to start the game, then sign in with your account name and password.'
Write-Host 'Run this again to change the server or the language. Undo at any time: skhid-undo.cmd'

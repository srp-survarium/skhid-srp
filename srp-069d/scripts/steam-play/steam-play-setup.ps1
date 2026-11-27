<#
.SYNOPSIS
Makes the Steam "Play" button of Survarium start the game on an srp server (Steam mode).

.DESCRIPTION
Writes ONE file: %LOCALAPPDATA%\Survarium-Steam\launcher.cfg (your own Windows profile, not the game folder).
The Survarium launcher reads it after the game's own launcher.cfg; each line replaces that one setting:
  update_server    the server's update check (the launcher then shows "latest version" and enables Play)
  file_to_execute  survarium.exe itself (no BattlEye launcher)
  arguments        the server address (and the game language, only with -Language)
  steam 1          Steam mode (sign-in with your own name and password in the Steam login panel)
Nothing in the game folder changes. steam-play-undo.cmd removes the file again.

.PARAMETER Server
The server address your server operator gave you (host name or IPv4 address, no port).
.PARAMETER Port
The server's login port (default 65000).
.PARAMETER UpdatePort
The server's update-check port (default 65001).
.PARAMETER UpdateServer
The update-check address if it differs from -Server (normally leave it out).
.PARAMETER Language
Leave it out to play in the language set in Steam (Survarium -> Properties -> General -> Language); the launcher
passes that language to the game. Give it (lower case, e.g. english, russian) only to play in a different one:
it then comes first on the game's command line and wins.
.PARAMETER Force
Replace a launcher.cfg that this script did not write.
.PARAMETER NoCheck
Skip the test request to the server's update check.

.EXAMPLE
steam-play-setup.cmd -Server 203.0.113.5
.EXAMPLE
steam-play-setup.cmd -Server play.example.org -Language russian
#>
param(
    [string]$Server,
    [int]$Port = 65000,
    [int]$UpdatePort = 65001,
    [string]$UpdateServer,
    [string]$Language,
    [switch]$Force,
    [switch]$NoCheck
)
$ErrorActionPreference = 'Stop'

function Fail([string]$Text) {
    Write-Host ''
    Write-Host "NOT DONE: $Text" -ForegroundColor Red
    exit 1
}

if (-not $Server) {
    $Server = Read-Host 'Server address (from your server operator, e.g. 203.0.113.5)'
}
$Server = "$Server".Trim()
if (-not $UpdateServer) { $UpdateServer = $Server }
$UpdateServer = "$UpdateServer".Trim()
$Language = "$Language".Trim()

$hostPattern = '^[A-Za-z0-9.-]{1,200}$'
if ($Server -notmatch $hostPattern) { Fail "'$Server' is not a server address (letters, digits, '.' and '-' only; no port, no http://)." }
if ($UpdateServer -notmatch $hostPattern) { Fail "'$UpdateServer' is not a server address." }
if ($Port -lt 1 -or $Port -gt 65535) { Fail "port $Port is not 1..65535." }
if ($UpdatePort -lt 1 -or $UpdatePort -gt 65535) { Fail "update port $UpdatePort is not 1..65535." }
if ($Language -and $Language -cnotmatch '^[a-z]{1,20}$') { Fail "language '$Language': lower-case letters only, e.g. russian." }
# No -Language: no -localization here. The launcher appends its own -localization (Steam's game language) after
# `arguments`, and the game keeps the first one, so a -localization here would replace Steam's choice.
$localization = if ($Language) { " -localization=$Language" } else { '' }

# The same lines as the server's `srp check` prints (servers/srp/src/update.rs, launcher_cfg_lines).
$lines = @(
    "update_server ${UpdateServer}:$UpdatePort",
    'file_to_execute game\binaries\x64\survarium.exe',
    "arguments -product_name=Survarium-Steam -server=${Server}:$Port$localization",
    'steam 1',
    'srp_steam_play 1'
)
# The launcher reads a line into 260 characters: a longer line would be cut.
foreach ($l in $lines) {
    if ($l.Length -gt 257) { Fail "this line is too long for the launcher ($($l.Length) characters): $l" }
}

$dir = Join-Path $env:LOCALAPPDATA 'Survarium-Steam'
$file = Join-Path $dir 'launcher.cfg'
if (Test-Path -LiteralPath $file) {
    $old = [System.IO.File]::ReadAllText($file, [System.Text.Encoding]::Unicode)
    if ($old -notmatch '(?m)^srp_steam_play ' -and -not $Force) {
        Fail "$file exists and was not written by this script. Look at it first; run again with -Force to replace it."
    }
}
New-Item -ItemType Directory -Force -Path $dir | Out-Null
# UTF-16 little endian without a byte order mark, CR LF between lines: the format of the game's own launcher.cfg.
$encoding = New-Object System.Text.UnicodeEncoding($false, $false)
[System.IO.File]::WriteAllBytes($file, $encoding.GetBytes(($lines -join "`r`n")))

Write-Host "Written: $file"
foreach ($l in $lines) { Write-Host "    $l" }

if (-not $NoCheck) {
    $url = "http://${UpdateServer}:$UpdatePort/download_type.php"
    try {
        $r = Invoke-WebRequest -UseBasicParsing -TimeoutSec 8 -Uri $url
        if ("$($r.Content)".Trim() -eq 'http') {
            Write-Host "Server check: the update check at ${UpdateServer}:$UpdatePort answers. Good." -ForegroundColor Green
        } else {
            Write-Warning "Server check: ${UpdateServer}:$UpdatePort answered, but not like an srp server. Check the address and port."
        }
    } catch {
        Write-Warning "Server check: no answer from ${UpdateServer}:$UpdatePort ($($_.Exception.Message)). The file is written anyway; the launcher's Play button stays off until this address answers. Ask your server operator, or check your firewall."
    }
}

Write-Host ''
Write-Host 'Next: start Steam, select Survarium in your library and press Play.'
Write-Host 'Undo at any time: steam-play-undo.cmd'

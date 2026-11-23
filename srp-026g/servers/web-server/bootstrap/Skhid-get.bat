@echo off
rem Skhid-get.bat: gets Skhid (Survarium 0.26g) into a Survarium folder beside
rem this file. Plain text: read it. It asks once for administrator rights to let
rem the game through Windows Firewall and exclude its programs from Microsoft
rem Defender scans (both stay on), fetches the Skhid updater, its DLL and 7zr.exe
rem by BitTorrent (the trackers below), else from the addresses below, checks
rem them against the hashes below, runs the updater, which installs the rest,
rem then starts Skhid.bat. Made by release/build-bootstrap.
set "SKHID_GET_SCRIPT=%~f0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$t = [IO.File]::ReadAllText($env:SKHID_GET_SCRIPT); iex $t.Substring($t.IndexOf('<#' + 'PS#>'))"
if errorlevel 1 pause
exit /b
<#PS#>
$InfoHash = '20f7d5fe8d1568877abed84e98f9af1e0b1e2344'
$PieceLength = 4194304
$Trackers = @('udp://tracker.opentrackr.org:1337/announce', 'udp://open.stealth.si:80/announce', 'udp://tracker.torrent.eu.org:451/announce', 'udp://exodus.desync.com:6969/announce')
$Files = @(
    @{ Path = 'game/binaries/x86/7zr.exe'; Size = 602624; Sha256 = '256feca8e274e5da655e2a284fabafd9f554365eb164862089dacd4e8276d282';
        Pieces = @('60455a4e63b6b4b057dc3d9b70b97a32a656bdb2');
        Urls = @('https://dl.dropboxusercontent.com/scl/fi/7k7li4c9dm7evm3jza6uf/7zr.exe?rlkey=lau3xb69jwony0kwfuuyvx12w&dl=1', 'http://2.28.121.133:8090/bootstrap/256feca8e274e5da655e2a284fabafd9f554365eb164862089dacd4e8276d282/7zr.exe') },
    @{ Path = 'game/binaries/x86/skhid_torrent.dll'; Size = 2442752; Sha256 = '0f32badb1a11fa3b5116b56efd4e33726f6b4e9dcc1763f4d0a1c118b6110667';
        Pieces = @('95d1f97154f5c291ddaa42a6bc1223d10a0774e5');
        Urls = @('https://dl.dropboxusercontent.com/scl/fi/jc55dccdnhwcr0ldfh8ch/skhid_torrent.dll?rlkey=7inuu713pcvle14iejz032pef&dl=1', 'http://2.28.121.133:8090/bootstrap/0f32badb1a11fa3b5116b56efd4e33726f6b4e9dcc1763f4d0a1c118b6110667/skhid_torrent.dll') },
    @{ Path = 'game/binaries/x86/updater_skhid.exe'; Size = 807936; Sha256 = 'c69ecc7d091ca66b94502e53f19eb88c48a78d616f47574d80f8b5fab1946ed7';
        Pieces = @('9ea39198c59bd2efb677d9aef7b1c6446b2e1505');
        Urls = @('https://dl.dropboxusercontent.com/scl/fi/p6k7pabkiydt9xdl4se5t/updater_skhid.exe?rlkey=yj395iatxi7hanozdr3oej847&dl=1', 'http://2.28.121.133:8090/bootstrap/c69ecc7d091ca66b94502e53f19eb88c48a78d616f47574d80f8b5fab1946ed7/updater_skhid.exe') }
)
# Skhid-get.bat's body, after the pins build_torrent.py writes
# (srp-026g/docs/notes/torrent-downloads.md, "Bootstrap").
$ErrorActionPreference = 'Stop'
trap { Write-Host "Skhid-get: $($_.Exception.Message)"; exit 1 }
$Block, $MaxPeers = 16384, 40
# Seconds a peer has for its handshake and for each piece; all peers together.
$HandshakeSeconds, $PieceSeconds, $PeersSeconds = 10, 60, 180

function Say($text) { Write-Host $text }
function Hex([byte[]]$bytes) { [BitConverter]::ToString($bytes).Replace('-', '').ToLowerInvariant() }
function Unhex([string]$hex) { , [byte[]](0..($hex.Length / 2 - 1) | ForEach-Object { [Convert]::ToByte($hex.Substring(2 * $_, 2), 16) }) }
function Sha256([byte[]]$data) { Hex ([Security.Cryptography.SHA256]::Create().ComputeHash($data)) }
function Be32([long]$n) { [byte[]](24, 16, 8, 0 | ForEach-Object { ($n -shr $_) -band 255 }) }
function U32([byte[]]$b, [int]$at) { ([long]$b[$at] -shl 24) -bor ([long]$b[$at + 1] -shl 16) -bor ([long]$b[$at + 2] -shl 8) -bor $b[$at + 3] }
function Bytes() {
    $out = New-Object IO.MemoryStream
    foreach ($part in $args) { $out.Write([byte[]]$part, 0, ([byte[]]$part).Length) }
    , $out.ToArray()
}
function Send($connection) { $message = Bytes @args; $connection.Write($message, 0, $message.Length) }
# [IO.Path] and [IO.File]: the cmdlets read [ and ] in a path as wildcards.
function Under($base, $relative) { [IO.Path]::Combine([string[]](@($base) + $relative.Split('/'))) }
function Quoted([string]$text) { "'" + ($text -replace "['‘-‛]", '$0$0') + "'" }

# Run elevated: Skhid-network-setup.cmd's rules, and Skhid-defender-setup.cmd's
# exclusions of game\binaries\x86 and game\torrent with its marker (the folders
# it added, a line each), so that their --undo removes what this adds. Returns
# 1 when the rules failed, 2 when the exclusions did (another antivirus,
# Defender off), 3 both.
function AllowGame([string]$root, [string]$bin) {
    $failed = 0
    try {
        foreach ($rule in 'launcher_skhid.exe TCP out', 'launcher_skhid.exe UDP out', 'survarium_skhid.exe TCP out',
                'survarium_skhid.exe UDP out', 'updater_skhid.exe TCP out', 'updater_skhid.exe UDP out', 'updater_skhid.exe TCP in') {
            $program, $protocol, $direction = $rule.Split(' ')
            $null = netsh advfirewall firewall delete rule name="Skhid $rule"
            $null = netsh advfirewall firewall add rule name="Skhid $rule" dir=$direction action=allow program="$(Join-Path $bin $program)" protocol=$protocol enable=yes profile=any
            if ($LASTEXITCODE) { throw "the firewall rule Skhid $rule" }
        }
    } catch { $failed += 1 }
    try {
        $had = (Get-MpPreference).ExclusionPath
        $new = @($bin, [IO.Path]::Combine($root, 'game', 'torrent') | Where-Object { $had -notcontains $_ })
        foreach ($path in $new) { Add-MpPreference -ExclusionPath $path }
        if ($new) { [IO.File]::WriteAllLines((Join-Path $root 'Skhid-network-setup.defender'), [string[]]$new, [Text.Encoding]::UTF8) }
    } catch { $failed += 2 }
    $failed
}
function AskToAllowGame {
    $command = "`$ErrorActionPreference = 'Stop'; function AllowGame {${function:AllowGame}}; exit (AllowGame $(Quoted $root) $(Quoted $bin))"
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
    try {
        $run = Start-Process powershell -Verb RunAs -WindowStyle Hidden -Wait -PassThru `
            -ArgumentList '-NoProfile', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', $encoded
    } catch {
        Say 'No firewall rules or Defender exclusion: Windows may block the game.'
        return
    }
    Say $(if ($run.ExitCode -band 1) { 'No firewall rules: Windows may block the game.' } else { 'Firewall rules: added.' })
    Say $(if ($run.ExitCode -band 2) { 'No Defender exclusion of game\binaries\x86 and game\torrent: Defender may take the game for a threat.' } else { 'Defender exclusion of game\binaries\x86 and game\torrent: added.' })
}

# The torrent's v1 bytes: each file from a piece's start, zeros to the next one
# (only a file's own bytes are asked for).
$pieces = @()
$offset = 0
foreach ($file in $Files) {
    $file.At = $offset
    for ($p = 0; $p -lt $file.Pieces.Count; $p++) {
        $pieces += , @{ At = $offset + $p * $PieceLength; Length = [math]::Min($PieceLength, $file.Size - $p * $PieceLength); Sha1 = $file.Pieces[$p] }
    }
    $offset += $file.Pieces.Count * $PieceLength
}
$torrent = New-Object byte[] ($Files[-1].At + $Files[-1].Size)
$done = New-Object bool[] $pieces.Count

# The first missing piece the peer has (no bitfield: any).
function Wanted($bitfield) {
    for ($i = 0; $i -lt $pieces.Count; $i++) {
        if (-not $done[$i] -and ($null -eq $bitfield -or ($bitfield.Length -gt ($i -shr 3) -and ($bitfield[$i -shr 3] -band (128 -shr ($i % 8)))))) { return $i }
    }
    -1
}
function Check([int]$i) {
    $at = $pieces[$i].At
    $sha = Hex ([Security.Cryptography.SHA1]::Create().ComputeHash($torrent, $at, [math]::Min($PieceLength, $torrent.Length - $at)))
    if ($sha -ne $pieces[$i].Sha1) { throw 'a piece that is not the pinned one' }
    $done[$i] = $true
}

# BEP 15: the IPv4 peers a UDP tracker gives.
function Announce($tracker, $peerId) {
    if ($tracker -notmatch '^udp://([^:/]+):(\d+)') { return }
    $udp = New-Object Net.Sockets.UdpClient
    try {
        $udp.Client.ReceiveTimeout = 10000
        $udp.Connect($matches[1], [int]$matches[2])
        $id = [byte[]]$peerId[16..19]
        $from = New-Object Net.IPEndPoint ([Net.IPAddress]::Any), 0
        function Ask([long]$action, [byte[]]$ask) {
            $null = $udp.Send($ask, $ask.Length)
            $answer = $udp.Receive([ref]$from)
            if ($answer.Length -lt 16 -or (U32 $answer 0) -ne $action -or (Hex $answer[4..7]) -ne (Hex $id)) { throw 'not an answer' }
            , $answer
        }
        $connected = Ask 0 (Bytes (Unhex '0000041727101980') (Be32 0) $id)
        $answer = Ask 1 (Bytes $connected[8..15] (Be32 1) $id (Unhex $InfoHash) $peerId (New-Object byte[] 8) `
            (Be32 0) (Be32 $torrent.Length) (New-Object byte[] 8) (Be32 2) (New-Object byte[] 4) $id (Be32 $MaxPeers) (Unhex '1ae1'))
        for ($at = 20; $at + 6 -le $answer.Length; $at += 6) {
            '{0}.{1}.{2}.{3}:{4}' -f $answer[$at], $answer[$at + 1], $answer[$at + 2], $answer[$at + 3], ($answer[$at + 4] * 256 + $answer[$at + 5])
        }
    } catch {} finally { $udp.Close() }
}

function Deadline([int]$seconds) { $script:until = @([DateTime]::UtcNow.AddSeconds($seconds), $script:peersEnd) | Sort-Object | Select-Object -First 1 }
# The time is looked at after every read: a peer that trickles bytes still runs out of it.
function ReadExactly($connection, [int]$count) {
    $buffer = New-Object byte[] $count
    for ($got = 0; $got -lt $count; $got += $n) {
        $n = $connection.Read($buffer, $got, $count - $got)
        if ($n -le 0) { throw 'the peer closed the connection' }
        if ([DateTime]::UtcNow -gt $script:until) { throw 'no piece in time' }
    }
    , $buffer
}

# BEP 3: the missing pieces the peer has, 8 blocks asked at a time. A piece that is not
# its pin, a message over a block, or no new piece in time drops the peer.
function FromPeer($address, $peerId) {
    $hostName, $port = $address.Split(':')
    $tcp = New-Object Net.Sockets.TcpClient
    try {
        if (-not $tcp.ConnectAsync($hostName, [int]$port).Wait(5000)) { throw 'no answer' }
        $connection = $tcp.GetStream()
        $connection.ReadTimeout = 20000
        Deadline $HandshakeSeconds
        $hello = Bytes ([byte[]]@(19)) ([Text.Encoding]::ASCII.GetBytes('BitTorrent protocol')) (New-Object byte[] 8) (Unhex $InfoHash) $peerId
        Send $connection $hello
        $answer = ReadExactly $connection 68
        if ((Hex $answer[0..19]) -ne (Hex $hello[0..19]) -or (Hex $answer[28..47]) -ne $InfoHash) { throw 'not a peer of this torrent' }
        Deadline $PieceSeconds
        Send $connection (Be32 1) ([byte[]]@(2))
        $choked = $true; $bitfield = $null; $piece = -1
        while ($true) {
            if (-not $choked -and $piece -lt 0) {
                if (($piece = Wanted $bitfield) -lt 0) { return }
                $length = $pieces[$piece].Length; $next = 0; $received = 0; $asked = @{}
            }
            while (-not $choked -and $next -lt $length -and $asked.Count -lt 8) {
                $asked[[string]$next] = [math]::Min($Block, $length - $next)
                Send $connection (Be32 13) ([byte[]]@(6)) (Be32 $piece) (Be32 $next) (Be32 $asked[[string]$next])
                $next += $asked[[string]$next]
            }
            $size = U32 (ReadExactly $connection 4) 0
            if ($size -eq 0) { continue }
            if ($size -gt $Block + 13) { throw 'a message over the limit' }
            $message = ReadExactly $connection ([int]$size)
            switch ($message[0]) {
                0 { $choked = $true; $next = 0; $received = 0; $asked = @{} }
                1 { $choked = $false }
                4 { $i = U32 $message 1; if ($null -ne $bitfield -and $size -eq 5 -and ($i -shr 3) -lt $bitfield.Length) { $bitfield[$i -shr 3] = $bitfield[$i -shr 3] -bor (128 -shr ($i % 8)) } }
                5 { $bitfield = New-Object byte[] ($size - 1); [Array]::Copy($message, 1, $bitfield, 0, $size - 1) }
                7 {
                    $begin = if ($size -ge 9) { U32 $message 5 } else { -1 }
                    if ($begin -lt 0 -or (U32 $message 1) -ne $piece -or $asked[[string]$begin] -ne $size - 9) { throw 'a block not asked for' }
                    [Array]::Copy($message, 9, $torrent, $pieces[$piece].At + $begin, $size - 9)
                    $asked.Remove([string]$begin); $received += $size - 9
                    if ($received -eq $length) {
                        Check $piece
                        if ((Wanted $null) -lt 0) { return }
                        $piece = -1
                        Deadline $PieceSeconds
                    }
                }
            }
        }
    } catch { Say "  peer ${address}: $($_.Exception.Message)" } finally { $tcp.Close() }
}

# A file's bytes, its pin checked: from the peers' pieces, else from its addresses in their order.
function FileBytes($file) {
    $bytes = New-Object byte[] $file.Size
    [Array]::Copy($torrent, $file.At, $bytes, 0, $file.Size)
    if ((Sha256 $bytes) -eq $file.Sha256) { return , $bytes }
    foreach ($url in $file.Urls) {
        try {
            Say "  $($file.Path): $url"
            $bytes = (New-Object Net.WebClient).DownloadData($url)
            if ($bytes.Length -eq $file.Size -and (Sha256 $bytes) -eq $file.Sha256) { return , $bytes }
            Say '    not the pinned file'
        } catch { Say "    $($_.Exception.Message)" }
    }
    throw "$($file.Path): no peer and no address gave it. Try again later, or ask in Discord: discord.gg/QUmJg3gBhg"
}

function Fetch {
    $peerId = Bytes ([Text.Encoding]::ASCII.GetBytes('-SK0002-')) ([Guid]::NewGuid().ToByteArray()[0..11])
    Say 'Looking for people who share the Skhid updater (BitTorrent)...'
    $peers = @(@(foreach ($tracker in $Trackers) { Announce $tracker $peerId }) | Select-Object -Unique -First $MaxPeers)
    Say "  $($peers.Count) peer(s)"
    $script:peersEnd = [DateTime]::UtcNow.AddSeconds($PeersSeconds)
    foreach ($peer in $peers) {
        if ((Wanted $null) -lt 0 -or [DateTime]::UtcNow -gt $script:peersEnd) { break }
        FromPeer $peer $peerId
    }
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $fetched = @{}
    foreach ($file in $Files) { $fetched[$file.Path] = FileBytes $file }
    $null = [IO.Directory]::CreateDirectory($bin)
    foreach ($file in $Files) {
        $path = Under $root $file.Path
        [IO.File]::WriteAllBytes("$path.part", $fetched[$file.Path])
        if ([IO.File]::Exists($path)) { [IO.File]::Delete($path) }
        [IO.File]::Move("$path.part", $path)
    }
    Say "The updater, its DLL and 7zr.exe: in $bin, each SHA-256 checked."
}

$root = Under ([IO.Path]::GetDirectoryName($env:SKHID_GET_SCRIPT)) 'Survarium'
$bin = Under $root 'game/binaries/x86'
$updater = Under $bin 'updater_skhid.exe'
$null = [IO.Directory]::CreateDirectory($root)
AskToAllowGame
if ([IO.File]::Exists($updater)) { Say "$updater is there already: the updater brings the rest." } else { Fetch }
Say 'The updater installs the game now (about 10 GB; it can take a while)...'
$run = Start-Process -FilePath $updater -ArgumentList 'check', 'torrent' -WorkingDirectory $bin -Wait -PassThru
if ($run.ExitCode -ne 0) { throw "the updater stopped (exit $($run.ExitCode)): see the newest updater_skhid.*.log in $(Under $root 'game/logs')" }
Start-Process -FilePath (Under $root 'Skhid.bat') -WorkingDirectory $root

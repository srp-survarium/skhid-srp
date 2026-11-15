# Skhid-get.bat's body, after the pins build_torrent.py writes
# (srp-026g/docs/notes/torrent-downloads.md, "Bootstrap").
$ErrorActionPreference = 'Stop'
trap { Write-Host "Skhid-get: $($_.Exception.Message)"; exit 1 }
$Block, $PeersAtMost = 16384, 40
# Seconds for a peer's handshake, for its next checked piece, for all the peers.
$HandshakeSeconds, $PieceSeconds, $PeersSeconds = 10, 60, 180
$Bits = 128, 64, 32, 16, 8, 4, 2, 1

function Say($text) { Write-Host $text }
function Hex([byte[]]$bytes) { [BitConverter]::ToString($bytes).Replace('-', '').ToLowerInvariant() }
function Unhex([string]$hex) { , [byte[]](0..($hex.Length / 2 - 1) | ForEach-Object { [Convert]::ToByte($hex.Substring(2 * $_, 2), 16) }) }
function Sha1([byte[]]$data) { Hex ([Security.Cryptography.SHA1]::Create().ComputeHash($data)) }
function Sha256([byte[]]$data) { Hex ([Security.Cryptography.SHA256]::Create().ComputeHash($data)) }
function Be32([long]$n) { [byte[]](24, 16, 8, 0 | ForEach-Object { ($n -shr $_) -band 255 }) }
function U32([byte[]]$b, [int]$at) { ([long]$b[$at] -shl 24) -bor ([long]$b[$at + 1] -shl 16) -bor ([long]$b[$at + 2] -shl 8) -bor [long]$b[$at + 3] }
function RandomBytes([int]$count) { [byte[]](1..$count | ForEach-Object { Get-Random -Maximum 256 }) }
function Bytes() {
    $stream = New-Object IO.MemoryStream
    foreach ($part in $args) { $stream.Write([byte[]]$part, 0, ([byte[]]$part).Length) }
    , $stream.ToArray()
}
function Send($stream, [byte[]]$message) { $stream.Write($message, 0, $message.Length) }
# A path under a folder, from its parts with / between.
function Under($base, $relative) { [IO.Path]::Combine([string[]](@($base) + $relative.Split('/'))) }
function Quoted([string]$text) { "'" + ($text -replace "['\u2018-\u201B]", '$0$0') + "'" }

# Run elevated: Skhid-network-setup.cmd's rules and Skhid-defender-setup.cmd's
# marker, so that their --undo removes what this adds.
function AllowGame([string]$root, [string]$bin) {
    $rules = 'launcher_skhid.exe TCP out', 'launcher_skhid.exe UDP out', 'survarium_skhid.exe TCP out',
        'survarium_skhid.exe UDP out', 'updater_skhid.exe TCP out', 'updater_skhid.exe UDP out', 'updater_skhid.exe TCP in'
    foreach ($rule in $rules) {
        $program, $protocol, $direction = $rule.Split(' ')
        $null = netsh advfirewall firewall delete rule name="Skhid $rule"
        $null = netsh advfirewall firewall add rule name="Skhid $rule" dir=$direction action=allow program="$(Join-Path $bin $program)" protocol=$protocol enable=yes profile=any
        if ($LASTEXITCODE) { throw "the firewall rule Skhid $rule" }
    }
    if ((Get-MpPreference).ExclusionPath -notcontains $root) {
        Add-MpPreference -ExclusionPath $root
        [IO.File]::WriteAllText((Join-Path $root 'Skhid-network-setup.defender'), $root, [Text.Encoding]::UTF8)
    }
}
function AllowGameAsAdministrator {
    $command = "`$ErrorActionPreference = 'Stop'; function AllowGame {${function:AllowGame}}; AllowGame $(Quoted $root) $(Quoted $bin)"
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
    try {
        $run = Start-Process powershell -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ArgumentList '-NoProfile', '-EncodedCommand', $encoded
        if ($run.ExitCode -eq 0) { Say 'Firewall rules and the Defender exclusion of the Survarium folder: added.'; return }
    } catch {}
    Say 'No firewall rules or Defender exclusion: Windows may block the game.'
}

# Every file starts a piece; each file's last piece, but the last file's, is padded with zeros.
$pieces = @()
for ($f = 0; $f -lt $Files.Count; $f++) {
    $file = $Files[$f]
    for ($p = 0; $p -lt $file.Pieces.Count; $p++) {
        $pieces += , @{ File = $f; At = $p * $PieceLength; Length = [math]::Min($PieceLength, $file.Size - $p * $PieceLength);
            Sha1 = $file.Pieces[$p]; Padded = ($f -lt $Files.Count - 1 -and $p -eq $file.Pieces.Count - 1) }
    }
}
$data = @{}
foreach ($file in $Files) { $data[$file.Path] = New-Object byte[] $file.Size }
$done = New-Object bool[] $pieces.Count

function HasPiece($has, [int]$i) {
    $byte = [math]::Floor($i / 8)
    ($null -eq $has) -or ($has.Length -gt $byte -and ([int]$has[$byte] -band $Bits[$i % 8]))
}
# The first missing piece the peer has (with no bitfield: any).
function Wanted($has) { for ($i = 0; $i -lt $pieces.Count; $i++) { if (-not $done[$i] -and (HasPiece $has $i)) { return $i } }; -1 }
function KeepPiece([int]$piece, [byte[]]$buffer) {
    $whole = $buffer
    if ($pieces[$piece].Padded) { $whole = Bytes $buffer (New-Object byte[] ($PieceLength - $buffer.Length)) }
    if ((Sha1 $whole) -ne $pieces[$piece].Sha1) { throw 'a piece that is not the pinned one' }
    [Array]::Copy($buffer, 0, $data[$Files[$pieces[$piece].File].Path], $pieces[$piece].At, $buffer.Length)
    $done[$piece] = $true
}
function Fetched([int]$f) {
    for ($i = 0; $i -lt $pieces.Count; $i++) { if ($pieces[$i].File -eq $f -and -not $done[$i]) { return $false } }
    (Sha256 $data[$Files[$f].Path]) -eq $Files[$f].Sha256
}

# BEP 15: the IPv4 peers a UDP tracker gives.
function Announce($tracker, $peerId) {
    if ($tracker -notmatch '^udp://([^:/]+):(\d+)') { return @() }
    $udp = New-Object Net.Sockets.UdpClient
    try {
        $udp.Client.ReceiveTimeout = 10000
        $udp.Connect($matches[1], [int]$matches[2])
        $id = RandomBytes 4
        $ask = Bytes (Unhex '0000041727101980') (Be32 0) $id
        $null = $udp.Send($ask, $ask.Length)
        $from = New-Object Net.IPEndPoint ([Net.IPAddress]::Any), 0
        $answer = $udp.Receive([ref]$from)
        if ($answer.Length -lt 16 -or (U32 $answer 0) -ne 0 -or (Hex $answer[4..7]) -ne (Hex $id)) { return @() }
        $left = 0; foreach ($file in $Files) { $left += $file.Size }
        $ask = Bytes $answer[8..15] (Be32 1) $id (Unhex $InfoHash) $peerId (New-Object byte[] 8) `
            (Be32 0) (Be32 $left) (New-Object byte[] 8) (Be32 2) (New-Object byte[] 4) $id (Be32 $PeersAtMost) (Unhex '1ae1')
        $null = $udp.Send($ask, $ask.Length)
        $answer = $udp.Receive([ref]$from)
        if ($answer.Length -lt 20 -or (U32 $answer 0) -ne 1 -or (Hex $answer[4..7]) -ne (Hex $id)) { return @() }
        for ($at = 20; $at + 6 -le $answer.Length -and ($at - 20) / 6 -lt $PeersAtMost; $at += 6) {
            '{0}.{1}.{2}.{3}:{4}' -f $answer[$at], $answer[$at + 1], $answer[$at + 2], $answer[$at + 3], ($answer[$at + 4] * 256 + $answer[$at + 5])
        }
    } catch { @() } finally { $udp.Close() }
}

# Checked after every read: a peer that trickles bytes still runs out of time.
function ReadExactly($stream, [int]$count) {
    $buffer = New-Object byte[] $count
    $got = 0
    while ($got -lt $count) {
        $n = $stream.Read($buffer, $got, $count - $got)
        if ($n -le 0) { throw 'the peer closed the connection' }
        $got += $n
        if ([DateTime]::UtcNow -gt $script:until) { throw 'no piece in time' }
    }
    , $buffer
}
function GiveTime([int]$seconds) {
    $script:until = [DateTime]::UtcNow.AddSeconds($seconds)
    if ($script:until -gt $script:peersEnd) { $script:until = $script:peersEnd }
}
function Handshake($stream, $peerId) {
    $stream.ReadTimeout = $HandshakeSeconds * 1000
    GiveTime $HandshakeSeconds
    $hello = Bytes ([byte[]]@(19)) ([Text.Encoding]::ASCII.GetBytes('BitTorrent protocol')) (New-Object byte[] 8) (Unhex $InfoHash) $peerId
    Send $stream $hello
    $answer = ReadExactly $stream 68
    if ((Hex $answer[0..19]) -ne (Hex $hello[0..19]) -or (Hex $answer[28..47]) -ne $InfoHash) { throw 'not a peer of this torrent' }
}

# BEP 3: the missing pieces the peer has, 8 blocks asked at a time. A piece
# that is not its pin, a message longer than a block, or no new piece in
# $PieceSeconds drops the peer.
function FromPeer($address, $peerId) {
    $hostName, $port = $address.Split(':')
    $tcp = New-Object Net.Sockets.TcpClient
    try {
        $wait = $tcp.BeginConnect($hostName, [int]$port, $null, $null)
        if (-not $wait.AsyncWaitHandle.WaitOne(5000, $false)) { throw 'no answer' }
        $tcp.EndConnect($wait)
        $stream = $tcp.GetStream()
        Handshake $stream $peerId
        $stream.ReadTimeout = 20000
        GiveTime $PieceSeconds
        Send $stream (Bytes (Be32 1) ([byte[]]@(2)))
        $choked = $true; $has = $null; $piece = -1
        while ($true) {
            if (-not $choked -and $piece -lt 0) {
                $piece = Wanted $has
                if ($piece -lt 0) { return }
                $buffer = New-Object byte[] $pieces[$piece].Length; $next = 0; $received = 0; $asked = @{}
            }
            while (-not $choked -and $next -lt $buffer.Length -and $asked.Count -lt 8) {
                $length = [math]::Min($Block, $buffer.Length - $next)
                Send $stream (Bytes (Be32 13) ([byte[]]@(6)) (Be32 $piece) (Be32 $next) (Be32 $length))
                $asked[[string]$next] = $length
                $next += $length
            }
            $length = U32 (ReadExactly $stream 4) 0
            if ($length -eq 0) { continue }
            if ($length -gt $Block + 13) { throw 'a message over the limit' }
            $message = ReadExactly $stream ([int]$length)
            switch ($message[0]) {
                0 { $choked = $true; $next = 0; $received = 0; $asked = @{} }
                1 { $choked = $false }
                4 { if ($null -ne $has -and $length -eq 5) { $i = U32 $message 1; $byte = [math]::Floor($i / 8); if ($byte -lt $has.Length) { $has[$byte] = [int]$has[$byte] -bor $Bits[$i % 8] } } }
                5 { $has = New-Object byte[] ($length - 1); [Array]::Copy($message, 1, $has, 0, $length - 1) }
                7 {
                    if ($length -lt 9) { throw 'a short piece message' }
                    $begin = U32 $message 5; $size = $length - 9
                    if ((U32 $message 1) -ne $piece -or $asked[[string]$begin] -ne $size) { throw 'a block not asked for' }
                    [Array]::Copy($message, 9, $buffer, $begin, $size)
                    $asked.Remove([string]$begin); $received += $size
                    if ($received -eq $buffer.Length) {
                        KeepPiece $piece $buffer
                        $piece = -1
                        if ((Wanted $null) -lt 0) { return }
                        GiveTime $PieceSeconds
                    }
                }
            }
        }
    } catch { Say "  peer ${address}: $($_.Exception.Message)" } finally { $tcp.Close() }
}

function FromAddresses($file) {
    foreach ($url in $file.Urls) {
        try {
            Say "  $($file.Path): $url"
            $got = (New-Object Net.WebClient).DownloadData($url)
            if ($got.Length -eq $file.Size -and (Sha256 $got) -eq $file.Sha256) { return , $got }
            Say '    not the pinned file'
        } catch { Say "    $($_.Exception.Message)" }
    }
    throw "$($file.Path): no peer and no address gave it. Try again later, or ask in Discord: discord.gg/QUmJg3gBhg"
}

function Fetch {
    $peerId = Bytes ([Text.Encoding]::ASCII.GetBytes('-SK0002-')) (RandomBytes 12)
    Say 'Looking for people who share the Skhid updater (BitTorrent)...'
    $peers = @(@(foreach ($tracker in $Trackers) { Announce $tracker $peerId }) | Select-Object -Unique | Select-Object -First $PeersAtMost)
    Say "  $($peers.Count) peer(s)"
    $script:peersEnd = [DateTime]::UtcNow.AddSeconds($PeersSeconds)
    foreach ($peer in $peers) {
        if ((Wanted $null) -lt 0 -or [DateTime]::UtcNow -gt $script:peersEnd) { break }
        FromPeer $peer $peerId
    }
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    for ($f = 0; $f -lt $Files.Count; $f++) {
        if (-not (Fetched $f)) { $data[$Files[$f].Path] = FromAddresses $Files[$f] }
    }
}

# [IO.File], not Remove-Item or Move-Item: those read [ and ] in a path as wildcards.
function Install {
    $null = [IO.Directory]::CreateDirectory($bin)
    foreach ($file in $Files) {
        $path = Under $root $file.Path
        [IO.File]::WriteAllBytes("$path.part", $data[$file.Path])
        if ([IO.File]::Exists($path)) { [IO.File]::Delete($path) }
        [IO.File]::Move("$path.part", $path)
    }
    Say "The updater, its DLL and 7zr.exe: in $bin, each SHA-256 checked."
}

$root = Under ([IO.Path]::GetDirectoryName($env:SKHID_GET_SCRIPT)) 'Survarium'
$bin = Under $root 'game/binaries/x86'
$updater = Under $bin 'updater_skhid.exe'
$null = [IO.Directory]::CreateDirectory($root)
AllowGameAsAdministrator
if ([IO.File]::Exists($updater)) { Say "$updater is there already: the updater brings the rest." } else { Fetch; Install }
Say 'The updater installs the game now (about 10 GB; it can take a while)...'
$run = Start-Process -FilePath $updater -ArgumentList 'check', 'torrent' -WorkingDirectory $bin -Wait -PassThru
if ($run.ExitCode -ne 0) { throw "the updater stopped (exit $($run.ExitCode)): see $(Under $bin 'updater_skhid.log')" }
Start-Process -FilePath (Under $root 'Skhid.bat') -WorkingDirectory $root

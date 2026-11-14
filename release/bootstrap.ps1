# The body of Skhid-get.bat (release/build_bootstrap.py puts the pins before
# it): gets the updater and 7zr.exe by BitTorrent from anyone who shares the
# bootstrap torrent (srp-026g/docs/notes/torrent-downloads.md, "Bootstrap"),
# else from the listed addresses, each piece and each file checked against
# the pins; lays them out in a Survarium folder beside the script; runs the
# updater, which installs the rest; starts Skhid.bat. Windows PowerShell 2
# (Windows 7) and later; no administrator rights. Needs our server at no step
# before the updater's.
$ErrorActionPreference = 'Stop'
trap { Write-Host "Skhid-get: $($_.Exception.Message)"; exit 1 }
$Block = 16384
# What a peer may send: one message at most this long (a piece's block and
# its header, or a bitfield); a peer that sends more is dropped.
$MaxMessage = $Block + 13
$PeersAtMost = 40

function Say($text) { Write-Host $text }
function Hex([byte[]]$bytes) { ($bytes | ForEach-Object { $_.ToString('x2') }) -join '' }
function Unhex([string]$hex) {
    $out = New-Object byte[] ($hex.Length / 2)
    for ($i = 0; $i -lt $out.Length; $i++) { $out[$i] = [Convert]::ToByte($hex.Substring(2 * $i, 2), 16) }
    , $out
}
function Sha1([byte[]]$data) { Hex ((New-Object Security.Cryptography.SHA1Managed).ComputeHash($data)) }
function Sha256([byte[]]$data) { Hex ((New-Object Security.Cryptography.SHA256Managed).ComputeHash($data)) }
# (No -shl or -shr: PowerShell 2 has neither.)
function Be32([long]$n) { [byte[]]@(([math]::Floor($n / 16777216) % 256), ([math]::Floor($n / 65536) % 256), ([math]::Floor($n / 256) % 256), ($n % 256)) }
$Bits = 128, 64, 32, 16, 8, 4, 2, 1
function U32([byte[]]$b, [int]$at) { ([long]$b[$at] * 16777216) + ([long]$b[$at + 1] * 65536) + ([long]$b[$at + 2] * 256) + [long]$b[$at + 3] }
function Bytes() {
    $stream = New-Object IO.MemoryStream
    foreach ($part in $args) { $stream.Write([byte[]]$part, 0, ([byte[]]$part).Length) }
    , $stream.ToArray()
}

# The pieces, in the torrent's order: every file starts a piece; a file's
# last piece is filled with zeros (the pad file after it) but the last
# file's.
$pieces = @()
for ($f = 0; $f -lt $Files.Count; $f++) {
    $file = $Files[$f]
    for ($p = 0; $p -lt $file.Pieces.Count; $p++) {
        $length = [math]::Min($PieceLength, $file.Size - $p * $PieceLength)
        $pieces += , @{ File = $f; At = $p * $PieceLength; Length = $length; Sha1 = $file.Pieces[$p];
            Padded = ($f -lt $Files.Count - 1 -and $p -eq $file.Pieces.Count - 1) }
    }
}
$data = @{}
foreach ($file in $Files) { $data[$file.Path] = New-Object byte[] $file.Size }
$done = New-Object bool[] $pieces.Count

function Missing() { for ($i = 0; $i -lt $pieces.Count; $i++) { if (-not $done[$i]) { return $i } }; -1 }

# The peers of the UDP trackers (BEP 15): IPv4 address and port each.
function Announce($tracker, $peerId) {
    if ($tracker -notmatch '^udp://([^:/]+):(\d+)') { return @() }
    $udp = New-Object Net.Sockets.UdpClient
    try {
        $udp.Client.ReceiveTimeout = 10000
        $udp.Connect($matches[1], [int]$matches[2])
        $id = [byte[]](1..4 | ForEach-Object { Get-Random -Maximum 256 })
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
        $found = @()
        for ($at = 20; $at + 6 -le $answer.Length -and $found.Count -lt $PeersAtMost; $at += 6) {
            $found += , ('{0}.{1}.{2}.{3}:{4}' -f $answer[$at], $answer[$at + 1], $answer[$at + 2], $answer[$at + 3], ($answer[$at + 4] * 256 + $answer[$at + 5]))
        }
        $found
    } catch { @() } finally { $udp.Close() }
}

function ReadExactly($stream, [int]$count) {
    $buffer = New-Object byte[] $count
    $got = 0
    while ($got -lt $count) {
        $n = $stream.Read($buffer, $got, $count - $got)
        if ($n -le 0) { throw 'the peer closed the connection' }
        $got += $n
    }
    , $buffer
}

# One peer (BEP 3): the handshake, then the missing pieces it has, a block
# at a time in a queue of 8; a piece that is not its pin drops the peer.
function FromPeer($address, $peerId) {
    $host_, $port = $address.Split(':')
    $tcp = New-Object Net.Sockets.TcpClient
    try {
        $wait = $tcp.BeginConnect($host_, [int]$port, $null, $null)
        if (-not $wait.AsyncWaitHandle.WaitOne(5000, $false)) { throw 'no answer' }
        $tcp.EndConnect($wait)
        $stream = $tcp.GetStream()
        $stream.ReadTimeout = 20000
        $hello = Bytes ([byte[]]@(19)) ([Text.Encoding]::ASCII.GetBytes('BitTorrent protocol')) (New-Object byte[] 8) (Unhex $InfoHash) $peerId
        $stream.Write($hello, 0, $hello.Length)
        $answer = ReadExactly $stream 68
        if ((Hex $answer[0..19]) -ne (Hex $hello[0..19]) -or (Hex $answer[28..47]) -ne $InfoHash) { throw 'not a peer of this torrent' }
        $interested = Bytes (Be32 1) ([byte[]]@(2))
        $stream.Write($interested, 0, 5)
        $choked = $true; $has = $null
        $piece = -1; $buffer = $null; $next = 0; $received = 0; $asked = @{}
        while ($true) {
            if (-not $choked -and $piece -lt 0) {
                $piece = -1
                for ($i = 0; $i -lt $pieces.Count; $i++) {
                    $byte = [math]::Floor($i / 8)
                    $hasIt = ($has -eq $null) -or (($has.Length -gt $byte) -and ([int]$has[$byte] -band $Bits[$i % 8]))
                    if (-not $done[$i] -and $hasIt) { $piece = $i; break }
                }
                if ($piece -lt 0) { return }
                $buffer = New-Object byte[] $pieces[$piece].Length; $next = 0; $received = 0; $asked = @{}
            }
            while (-not $choked -and $piece -ge 0 -and $next -lt $buffer.Length -and $asked.Count -lt 8) {
                $length = [math]::Min($Block, $buffer.Length - $next)
                $ask = Bytes (Be32 13) ([byte[]]@(6)) (Be32 $piece) (Be32 $next) (Be32 $length)
                $stream.Write($ask, 0, $ask.Length)
                $asked[[string]$next] = $length
                $next += $length
            }
            $length = U32 (ReadExactly $stream 4) 0
            if ($length -eq 0) { continue }
            if ($length -gt $MaxMessage) { throw 'a message over the limit' }
            $message = ReadExactly $stream ([int]$length)
            switch ($message[0]) {
                0 { $choked = $true; $next = 0; $received = 0; $asked = @{} }
                1 { $choked = $false }
                4 { if ($has -ne $null -and $length -eq 5) { $i = U32 $message 1; $byte = [math]::Floor($i / 8); if ($byte -lt $has.Length) { $has[$byte] = [int]$has[$byte] -bor $Bits[$i % 8] } } }
                5 { $has = $message[1..($message.Length - 1)]; if ($has -isnot [array]) { $has = @($has) } }
                7 {
                    if ($length -lt 9) { throw 'a short piece message' }
                    $index = U32 $message 1; $begin = U32 $message 5; $size = $length - 9
                    if ($index -ne $piece -or $asked[[string]$begin] -ne $size) { throw 'a block not asked for' }
                    [Array]::Copy($message, 9, $buffer, $begin, $size)
                    $asked.Remove([string]$begin); $received += $size
                    if ($received -eq $buffer.Length) {
                        $whole = $buffer
                        if ($pieces[$piece].Padded) { $whole = Bytes $buffer (New-Object byte[] ($PieceLength - $buffer.Length)) }
                        if ((Sha1 $whole) -ne $pieces[$piece].Sha1) { throw 'a piece that is not the pinned one' }
                        $file = $Files[$pieces[$piece].File]
                        [Array]::Copy($buffer, 0, $data[$file.Path], $pieces[$piece].At, $buffer.Length)
                        $done[$piece] = $true
                        $piece = -1
                        if ((Missing) -lt 0) { return }
                    }
                }
            }
        }
    } catch { Say "  peer ${address}: $($_.Exception.Message)" } finally { $tcp.Close() }
}

# A path under a folder, from its parts with / between, with this system's
# separator (.NET 2's Combine takes two).
function Under($base, $relative) { foreach ($part in $relative.Split('/')) { $base = [IO.Path]::Combine($base, $part) }; $base }
$root = Under ([IO.Path]::GetDirectoryName($env:SKHID_GET_SCRIPT)) 'Survarium'
$bin = Under $root 'game/binaries/x86'
$updater = Under $bin 'updater_skhid.exe'
if (Test-Path $updater) {
    Say "$updater is there already: the updater brings the rest."
} else {
    $peerId = Bytes ([Text.Encoding]::ASCII.GetBytes('-SK0001-')) ([byte[]](1..12 | ForEach-Object { Get-Random -Maximum 256 }))
    Say 'Looking for people who share the Skhid updater (BitTorrent)...'
    $peers = @()
    foreach ($tracker in $Trackers) { $peers += Announce $tracker $peerId }
    $peers = @($peers | Select-Object -Unique | Select-Object -First $PeersAtMost)
    Say "  $($peers.Count) peer(s)"
    foreach ($peer in $peers) {
        if ((Missing) -lt 0) { break }
        FromPeer $peer $peerId
    }
    try { [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]3072 } catch { }
    foreach ($file in $Files) {
        $all = $true; for ($i = 0; $i -lt $pieces.Count; $i++) { if ($Files[$pieces[$i].File].Path -eq $file.Path -and -not $done[$i]) { $all = $false } }
        if ($all -and (Sha256 $data[$file.Path]) -eq $file.Sha256) { continue }
        $data[$file.Path] = $null
        foreach ($url in $file.Urls) {
            try {
                Say "  $($file.Path): $url"
                $got = (New-Object Net.WebClient).DownloadData($url)
                if ($got.Length -eq $file.Size -and (Sha256 $got) -eq $file.Sha256) { $data[$file.Path] = $got; break }
                Say '    not the pinned file'
            } catch { Say "    $($_.Exception.Message)" }
        }
        if ($data[$file.Path] -eq $null) { throw "$($file.Path): no peer and no address gave it. Try again later, or ask in Discord: discord.gg/QUmJg3gBhg" }
    }
    $null = [IO.Directory]::CreateDirectory($bin)
    foreach ($file in $Files) {
        $path = Under $root $file.Path
        [IO.File]::WriteAllBytes("$path.part", $data[$file.Path])
        if (Test-Path $path) { Remove-Item $path }
        Move-Item "$path.part" $path
    }
    Say "The updater and 7zr.exe: in $bin, each SHA-256 checked."
}
if ($env:SKHID_GET_NO_START) { exit 0 }
Say 'The updater installs the game now (about 10 GB; it can take a while)...'
$run = Start-Process -FilePath $updater -ArgumentList 'check' -WorkingDirectory $bin -Wait -PassThru
if ($run.ExitCode -ne 0) { throw "the updater stopped (exit $($run.ExitCode)): see $(Under $bin 'updater_skhid.log')" }
Start-Process -FilePath (Under $root 'Skhid.bat') -WorkingDirectory $root

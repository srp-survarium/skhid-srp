$ErrorActionPreference = 'Stop'
@@DIAGNOSTICS@@
try {
    if (-not [Environment]::Is64BitOperatingSystem) {
        throw 'This downloader requires 64-bit Windows.'
    }
    $parent = $env:SKHID_DOWNLOAD_PARENT
    if ([string]::IsNullOrWhiteSpace($parent)) {
        $parent = [IO.Path]::GetDirectoryName($env:SKHID_DOWNLOAD_SCRIPT)
    }
    $parent = [IO.Directory]::CreateDirectory($parent).FullName
    $tools = Join-Path $parent '.skhid-downloader'
    $null = [IO.Directory]::CreateDirectory($tools)
    $aria2 = Join-Path $tools 'aria2-1.37.0-win-64bit-build1\aria2c.exe'

    if (-not (Test-Path -LiteralPath $aria2 -PathType Leaf)) {
        Write-Host 'Getting the portable aria2 downloader (2.4 MB)...'
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        $archive = Join-Path $tools 'aria2-1.37.0-win-64bit-build1.zip'
        $part = $archive + '.part'
        Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/aria2/aria2/releases/download/release-1.37.0/aria2-1.37.0-win-64bit-build1.zip' -OutFile $part
        if ((Get-FileHash -LiteralPath $part -Algorithm SHA256).Hash.ToLowerInvariant() -ne '67d015301eef0b612191212d564c5bb0a14b5b9c4796b76454276a4d28d9b288') {
            Remove-Item -LiteralPath $part -Force
            throw 'The downloader archive failed its checksum.'
        }
        Move-Item -LiteralPath $part -Destination $archive -Force
        Expand-Archive -LiteralPath $archive -DestinationPath $tools -Force
    }
    if ((Get-FileHash -LiteralPath $aria2 -Algorithm SHA256).Hash.ToLowerInvariant() -ne 'be2099c214f63a3cb4954b09a0becd6e2e34660b886d4c898d260febfe9d70c2') {
        throw 'The portable downloader failed its checksum. Remove .skhid-downloader and retry.'
    }

    $encodedTorrent = @'
@@TORRENT_BASE64@@
'@
    $torrentBytes = [Convert]::FromBase64String($encodedTorrent)
    $hasher = [Security.Cryptography.SHA256]::Create()
    try { $torrentHash = ([BitConverter]::ToString($hasher.ComputeHash($torrentBytes))).Replace('-', '').ToLowerInvariant() }
    finally { $hasher.Dispose() }
    if ($torrentHash -ne '@@TORRENT_SHA256@@') { throw 'The embedded torrent failed its checksum.' }
    $torrentFile = Join-Path $tools 'skhid-rev@@REVISION@@.torrent'
    [IO.File]::WriteAllBytes($torrentFile, $torrentBytes)

    $game = Join-Path $parent 'Survarium'
    Write-Host "Downloading Skhid revision @@REVISION@@ into $game"
    Write-Host 'Game files come from torrent peers. Rerun this file to resume; pieces are verified.'
    $downloadArgs = @(
        '--no-conf=true', "--dir=$parent", '--check-integrity=true',
        '--auto-file-renaming=false', '--file-allocation=none',
        '--enable-dht=true', '--enable-peer-exchange=true',
        '--max-overall-upload-limit=1M', '--bt-hash-check-seed=false',
        '--seed-time=0', '--summary-interval=0',
        '--bt-tracker=http://bt.t-ru.org/ann?magnet,udp://open.demonii.com:1337/announce,udp://explodie.org:6969/announce',
        $torrentFile
    )
    $logFile = Join-Path $tools ('download-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
    Write-Host "Diagnostics: $logFile"
    Write-Host 'Configured trackers (attempts and replies appear below):'
@@TRACKER_DISPLAY@@
    Write-Host '  http://bt.t-ru.org/ann?magnet'
    Write-Host '  udp://open.demonii.com:1337/announce'
    Write-Host '  udp://explodie.org:6969/announce'
    Invoke-TorrentDownload $aria2 $downloadArgs $logFile

    $torrentFolder = Join-Path $game 'game\torrent'
    $null = [IO.Directory]::CreateDirectory($torrentFolder)
    [IO.File]::WriteAllBytes((Join-Path $torrentFolder 'skhid.torrent'), $torrentBytes)
    Write-Host "Download complete: $game"
    Write-Host "Start the game using $(Join-Path $game 'Skhid.bat')"
    exit 0
} catch {
    Write-Host "Skhid-get: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}

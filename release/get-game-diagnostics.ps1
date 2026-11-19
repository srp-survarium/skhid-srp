# PowerShell 5.1. Read aria2's own tracker events; no second announcing client.
function Format-TorrentEvent([string]$line) {
    if ($line -match '^GET (\S*info_hash=\S*) HTTP/') {
        $script:trackerRequestPath = ($matches[1] -split '\?')[0]
        return $null
    }
    if ($line -match '^Host: (.+)' -and $script:trackerRequestPath) {
        $message = 'Tracker asked: ' + $matches[1].Trim() + $script:trackerRequestPath
        $script:trackerRequestPath = $null
        return $message
    }
    if ($line -match '(CUID#[0-9]+) - Response received:') {
        $script:trackerResponseConnection = $matches[1]
        return $null
    }
    if ($line -match 'HTTP Server sends') {
        $script:trackerResponseConnection = $null
        return $null
    }
    if ($line -match '^HTTP/1\.[01] [0-9]{3}' -and $script:trackerResponseConnection) {
        $message = 'Tracker HTTP response: ' + $line + ' (' + $script:trackerResponseConnection + ')'
        $script:trackerResponseConnection = $null
        return $message
    }
    if ($line -match 'HttpServer\.cc') { return $null }
    # Never echo RPC request bodies (which contain the local authentication token).
    if ($line -match 'UDPT (?:send|resend|received|timeout|fail) (?:CONNECT|ANNOUNCE|ERROR)') {
        return ($line -replace '^.*?(UDPT )', '$1')
    }
    if ($line -match 'Now processing.*tracker response|Tracker request .*failed|Tracker returned|Complete:|Incomplete:|Interval:|Adding peer|Checkout peer|Peer .*returned|Connecting to') {
        return ($line -replace '^.*?\] ', '')
    }
    if ($line -match '(?:\[(?:ERROR|WARN)\]|errorCode=|Exception:|Download complete|IPv[46] BitTorrent|IPv[46] DHT)') {
        return $line
    }
    return $null
}

function Invoke-TorrentDownload([string]$program, [string[]]$arguments, [string]$logFile) {
    $listener = New-Object Net.Sockets.TcpListener ([Net.IPAddress]::Loopback), 0
    $listener.Start()
    $rpcPort = $listener.LocalEndpoint.Port
    $listener.Stop()
    $secret = [Guid]::NewGuid().ToString('N')
    $gid = [Guid]::NewGuid().ToString('N').Substring(0, 16)
    $rpcUri = "http://127.0.0.1:$rpcPort/jsonrpc"
    $writer = New-Object IO.StreamWriter $logFile, $false, ([Text.UTF8Encoding]::new($false))
    $writer.AutoFlush = $true
    function Report([string]$message) {
        $message = '[' + (Get-Date -Format 'HH:mm:ss') + '] ' + $message
        Write-Host $message
        $writer.WriteLine($message)
    }
    function Rpc([string]$method, [object[]]$parameters = @()) {
        $body = @{jsonrpc='2.0'; id='skhid'; method=('aria2.' + $method); params=(@('token:' + $secret) + $parameters)} | ConvertTo-Json -Depth 8 -Compress
        $answer = Invoke-RestMethod -Uri $rpcUri -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 2
        if ($answer.error) { throw $answer.error.message }
        return $answer.result
    }
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName = $program
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $options = @('--enable-rpc=true', '--rpc-listen-all=false', "--rpc-listen-port=$rpcPort", "--rpc-secret=$secret", "--gid=$gid", "--stop-with-process=$PID", '--enable-color=false', '--console-log-level=debug', '--show-console-readout=false') + $arguments
    # Windows CommandLineToArgvW quoting, including trailing backslashes and spaces.
    $start.Arguments = ($options | ForEach-Object { '"' + (($_ -replace '(\\*)"', '$1$1\"') -replace '(\\+)$', '$1$1') + '"' }) -join ' '
    $process = $null
    $outRead = $null
    $errRead = $null
    try {
        $process = [Diagnostics.Process]::Start($start)
        $outRead = $process.StandardOutput.ReadLineAsync()
        $errRead = $process.StandardError.ReadLineAsync()
        $nextPoll = [DateTime]::UtcNow
        $started = [DateTime]::UtcNow
        $lastState = 'starting'
        Report 'Tracker lists describe advertised peers. Only the Connected lines confirm a connection.'
        while (-not $process.HasExited -or $null -ne $outRead -or $null -ne $errRead) {
            # Bound each batch so a busy transfer cannot starve the progress display.
            for ($i = 0; $i -lt 300 -and $null -ne $outRead -and $outRead.IsCompleted; $i++) {
                $line = $outRead.GetAwaiter().GetResult()
                if ($null -eq $line) { $outRead = $null; break }
                $eventLine = Format-TorrentEvent $line
                if ($eventLine) { Report $eventLine }
                $outRead = $process.StandardOutput.ReadLineAsync()
            }
            for ($i = 0; $i -lt 100 -and $null -ne $errRead -and $errRead.IsCompleted; $i++) {
                $line = $errRead.GetAwaiter().GetResult()
                if ($null -eq $line) { $errRead = $null; break }
                if ($line) { Report $line }
                $errRead = $process.StandardError.ReadLineAsync()
            }
            if (-not $process.HasExited -and [DateTime]::UtcNow -ge $nextPoll) {
                $nextPoll = [DateTime]::UtcNow.AddSeconds(5)
                try {
                    $status = Rpc 'tellStatus' @($gid, @('status','totalLength','completedLength','downloadSpeed','uploadSpeed','connections','numSeeders','errorMessage','verifiedLength','verifyIntegrityPending'))
                } catch {
                    if (([DateTime]::UtcNow - $started).TotalSeconds -gt 20) { throw "Cannot read downloader status: $($_.Exception.Message)" }
                    continue
                }
                $lastState = $status.status
                $total = [double]$status.totalLength
                $percent = if ($total -gt 0) { 100 * [double]$status.completedLength / $total } else { 0 }
                $checking = if ($status.verifiedLength -or $status.verifyIntegrityPending -eq 'true') { ' Checking existing files.' } else { '' }
                Report ('{0}: {1:N1}% | down {2:N1} KiB/s | up {3:N1} KiB/s | connected {4} ({5} seeds).{6}' -f $lastState, $percent, ([double]$status.downloadSpeed / 1KB), ([double]$status.uploadSpeed / 1KB), $status.connections, $status.numSeeders, $checking)
                if ($lastState -eq 'error') { throw $status.errorMessage }
                if ($lastState -eq 'removed') { throw 'Download was removed.' }
                if ($lastState -eq 'complete') { $null = Rpc 'shutdown'; break }
                if ($lastState -eq 'active') {
                    try {
                        $peers = @(Rpc 'getPeers' @($gid))
                    } catch {
                        # It may finish between tellStatus and getPeers. The next
                        # status poll decides completion; diagnostics must not stop it.
                        Report 'Peer details temporarily unavailable; checking download status again.'
                        continue
                    }
                    if ($peers.Count -eq 0) { Report 'Connected: none yet. Tracker counts do not prove reachability.' }
                    foreach ($peer in $peers) {
                        $kind = if ($peer.seeder -eq 'true') { 'seed' } else { 'partial peer' }
                        $endpoint = if ($peer.ip.Contains(':')) { '[' + $peer.ip + ']:' + $peer.port } else { $peer.ip + ':' + $peer.port }
                        Report ('Connected {0} | {1} | down {2:N1} KiB/s | up {3:N1} KiB/s' -f $endpoint, $kind, ([double]$peer.downloadSpeed / 1KB), ([double]$peer.uploadSpeed / 1KB))
                    }
                }
            }
            Start-Sleep -Milliseconds 20
        }
        if ($lastState -ne 'complete') { throw "Downloader exited before completion. Rerun to resume; diagnostics: $logFile" }
    } finally {
        if ($null -ne $process -and -not $process.HasExited) {
            try { $null = Rpc 'shutdown' } catch { }
            if (-not $process.WaitForExit(5000)) { $process.Kill(); $process.WaitForExit() }
        }
        if ($null -ne $process) { $process.Dispose() }
        $writer.Dispose()
    }
}

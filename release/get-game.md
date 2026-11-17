# Standalone full-game torrent downloader

`Skhid-get.bat` is a separate Windows 10/11 x64 downloader using pinned portable
aria2 1.37.0. It downloads the whole game from peers, resumes partial files,
checks pieces, and stops when complete. It does not change firewall rules or
launch the game. Its embedded torrent stays pinned to the named released revision.
It is separate from the three-file updater bootstrap (`bootstrap.ps1`).

Build from the released torrent (do not generate a new torrent):

```
python3 release/build-get-game.py --torrent skhid-rev64.torrent --revision 64 --out Skhid-get.bat
```

The script verifies both the portable downloader and the embedded torrent by SHA-256.
It lists configured trackers, then shows actual aria2 tracker requests, responses,
failures and retry events. HTTP replies include seed/leecher counts when the tracker
supplies them; UDP replies also include returned-peer counts. Tracker counts are not
summed and do not prove reachability. Discovered addresses and actual connected peers
are distinct; connected peers include IP, port, seed status and transfer rates, refreshed
every five seconds. A timestamped diagnostic log is kept in `.skhid-downloader`.
RPC is loopback-only with a random per-run secret, omitted from logs. Closing the wrapper
stops its aria2 child; an interrupted download retains its pieces for resumption.

The existing torrent trackers remain. This wrapper also asks RuTracker's
`http://bt.t-ru.org/ann?magnet`, Demonii and Explodie. RuTracker returned a valid peer-list
reply for revision 64 during the implementation; that does not verify the returned peer
or guarantee future acceptance. Demonii and Explodie explicitly permit public use at
https://demonii.com/ and https://explodie.org/opentracker.html. No bulk tracker list,
I2P-only endpoints, or raw-IP aliases are imported. Trackers cannot alter the embedded
content hashes. Adding trackers does not repair blocked incoming connections.

Publication: serve the built BAT as `/Skhid-get.bat` from the Caddy volume's
`/data/skhid-downloads/Skhid-get.bat`. `docker-compose.yml` keeps that route across
proxy recreation. Updating this static file does not require a game-server restart or
a game revision. Keep a previous copy, stage the new bytes, verify their SHA-256, then
rename atomically. Validate and gracefully reload Caddy when initially adding the route.
For each subsequent game release, rebuild the published BAT from that release's torrent.

Validation: `tools/check get-game` transfers a 1 MiB fixture through a local tracker
and seed using real aria2 and PowerShell. It checks a rejected tracker, successful replies,
seed IP/rates, completion, and the downloaded bytes, including paths containing spaces.
This test runs PowerShell on Linux; it is not an end-to-end Windows GUI test.

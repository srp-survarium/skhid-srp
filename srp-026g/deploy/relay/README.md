# Regional game relay

A relay forwards login TCP and lobby/match/voice UDP to the same srp instance.
The game account and match stay on the origin. The relay never has the sign-in
cipher's secret key. Source addresses travel in a strict IPv4 PROXY v2 envelope;
only configured WireGuard peers may supply it. Direct players still use the
original sockets and wire format. This changes no simulation or world data.

The servers know a peer by its address and the route it came by, direct or
through a given relay (`udp_transport::relay::Peer`): login, lobby, match seats
and voice all key by it, and an answer leaves by the route its peer came by.
Datagrams that name an address by another route are another peer's: they never
continue, redirect or displace a link that came the other way. Limits and bans
still count the client's own address (the PROXY header's for a relayed peer).
srp parses `SRP_RELAYS` once at its start and gives the result to the four
servers; a wrong value starts none of them.

The player picks the route in the launcher's "Relay" row (auto, on, off:
Automatic, the relay, direct; `client/updater/README.md`, "The route"); a player who never
picked has Automatic in Russia (the Windows region) and direct elsewhere;
direct found blocked while the relay answers becomes Automatic by itself.
Automatic fallback runs before PLAY starts the game. With `updater_skhid.relay`
installed and Automatic, the updater probes direct login TCP and lobby/match UDP concurrently,
in rounds, and tries the relay only when no round answers whole (3 rounds when
nothing answers, 5 when a part does, one more after a game played direct; 35 s
at most; `client/updater/README.md`, "The check"): through the relay the ping
is higher, so one lost packet must not choose it. Each probe carries 64 KiB
each way within its 5 s round (login TCP one stream, lobby and match UDP 130
datagrams of 508 bytes, missing ones sent again), since some providers cut a
flow to foreign hosting after its first tens of KB: a small echo would pass and
the game then fail. The server echoes exactly what it got, as it gets it (a
login connection up to 64 KiB, a datagram that starts `SKHID-ROUTE-1:` up to
508 bytes), without creating an account, session or match. The relay's
per-address budgets carry the burst. A successful route is passed as
`-server=IP:port`; the origin advertises the relay for lobby and match, and voice
uses match + 1. Explicit `-server=` and replay options bypass route selection.
A failed route never switches a running match: launch again to choose anew.

The updater's HTTPS requests to the configured login hostname on port 443 also
fall back (Automatic), on network errors only, to the relay's port 8080 CONNECT
service; with the relay chosen they go through it only, with direct never.
Under Wine a request through it has the whole address in its request line
(`GET https://skhid.link:443/...`), which Caddy answers as the path alone.
It permits only `skhid.link:443` (or the configured `--https-origin`). TLS stays
end to end, with normal hostname/certificate verification. Other download hosts
retain their existing mirror behavior. This is not a general HTTP proxy and does
not change browser or system proxy settings. A new player needs the release
containing this updater/configuration before automatic fallback is available.

Browsers get the site under a name of the relay's own (`playskhid.link`): the
same web service passes its port 443 through the tunnel to the origin's Caddy,
unopened. The certificate and its key stay on the origin, and the relay sees
no page and no password. Caddy takes the visitor's address from the PROXY
header, from the tunnel addresses of `SRP_RELAYS` only; anyone else's header is
read and ignored (`check_site.py`). Downloads through that name count against
the relay's traffic.

## Installation

The default tunnel addresses are origin `10.77.0.1/32`, relay `10.77.0.2/32`.
Check they do not overlap either host's networks. Origin initiates WireGuard to
the relay on UDP 51820 with a 25-second keepalive, so it needs no new inbound
provider firewall port. The relay needs inbound TCP 443/1234/8080, UDP 1235–1237/51820.
Only IPv4 players and consecutive game ports are supported by this first setup.

1. Build the native bundle (command below) and install
   `srp-026g/target/relay-dist/` as `/opt/skhid-relay/` on the relay.
   Copy this deployment directory to each host. As root run `bash setup.sh prepare` on
   each. This prints only its public key. Private keys are generated and kept
   on their own host, never in git or the player's download.
2. On the relay run `bash setup.sh configure relay ORIGIN_PUBLIC_KEY RELAY_IP`.
   On the origin run `bash setup.sh configure origin RELAY_PUBLIC_KEY RELAY_IP`.
   This installs and starts only the tunnel; it leaves the forwarder stopped.
   It adds its own nftables table, never flushes the host/Docker rules, and
   rejects the trusted peer's source address arriving outside WireGuard.
   The tunnel has only host routes to its peer, never a default route.
3. Check `wg show skhid-relay latest-handshakes` and ping each tunnel address.
   Test both forwarders with
   `SKHID_NATIVE_RELAY=/opt/skhid-relay/run python3 -m unittest -v test_forwarder test_native`.
4. Prepare a normal server/player release with this branch. Set origin's
   `SRP_RELAYS=10.77.0.2=RELAY_IP` in its compose environment. This is a trust
   boundary: NEVER trust an Internet source IP here, and install the tunnel's
   anti-spoof firewall rules first. Deployment goes through the owner's usual
   release workflow and go; setup.sh does not restart srp or edit its .env.
5. After the backend is updated, start the relay with
   `systemctl enable --now skhid-relay skhid-relay-web`. Verify direct and relayed probes, and
   sign-in/lobby/match/voice with a test account before enabling client fallback.
6. In private release settings set `SKHID_RELAY_SERVER=RELAY_IP:1234`
   (`SKHID_TEST_RELAY_SERVER` for a test release). `release/build-zip` passes
   `--relay=` to the client packager, ships the two-line route configuration,
   and points PLAY at `updater_skhid.exe play`. The normal release creates the
   revision; a feature branch does not reserve one.

The setup's firewall defaults are the live ports. A test server on 2234–2237
needs those ports in the tunnel allow-list and the native forwarder's
`--login-port 2234` option. Origin and public ports must match, including the PROXY destination.
The server processes invalid envelopes as dropped packets, not fatal socket
errors. Both ends bound their return-address tables and expire idle entries;
UDP replies go only to clients observed on that frontend within 120 seconds.

## The site under the relay's name

1. Point the name's A record (`playskhid.link`, the owner's own domain; `www` too if it is to
   answer) at the relay, in place of the registrar's parking records. The game's relay address
   stays an IPv4 address (`--relay`): the name is the site's only.
2. On both hosts run `bash setup.sh configure ...` again with the same
   arguments: it reloads the tunnel's firewall with port 443 (the tunnel stays
   up), and on the relay installs this forwarder and its service. Nothing else
   may hold the relay's port 443 (`ss -ltn 'sport = :443'` is empty): the web
   service does not start without it, the updater's port 8080 neither. Then
   `systemctl restart skhid-relay-web` there, which cuts the updater downloads
   running through it: once players have the relay, on the owner's go.
3. On the origin add the name to `SRP_WEB_DOMAIN` in `.env` (`SRP_RELAYS` is
   step 4 above) and make Caddy anew with this compose file:
   `docker compose --profile tls up -d --no-deps caddy26`. The site's HTTPS is
   away for a few seconds, so this needs the owner's go; a release does not do
   it (it makes only `srp26` anew). The name's certificate comes by TLS-ALPN
   through the relay, so steps 1 and 2 come first. Caddy reads `SRP_RELAYS`
   only when it is made: make it anew whenever that setting changes.
4. Check `https://playskhid.link/api/stats`, then that the page sees the
   visitor and not the relay: the admin page must answer the jump host under
   this name too. If it does not, Caddy sees another source than the relay's
   tunnel address (Docker's published port keeps it) and ignores the header.

To take it away: remove the name from `SRP_WEB_DOMAIN`, make `caddy26` anew,
remove the A record.

## How to read the relay's logs

On the relay: `journalctl -u skhid-relay` (the game: login TCP, lobby, match and
voice UDP) and `journalctl -u skhid-relay-web` (the updater's `CONNECT` and the
`site`). Every line is one event or one flow, never one per packet; what can
repeat per packet or per connection is said at its 1st, 10th, 100th, ... time,
with the count so far (`; 100 such so far`). On the origin the sign-in lines name
the route: `from 198.51.100.9:50122 via relay 203.0.113.8`, and each route check
has its line.

Start and stop. The first line of a run is `ready:` with what it serves:

    ready: 203.0.113.8, login TCP 1234, lobby UDP 1235, match UDP 1236, voice UDP 1237, to the server 10.77.0.1 at the same ports (bound to 0.0.0.0); idle 120 s, 8192 clients a port, 2000/s (burst 4000) an address; 128 logins at once, 2/s (burst 10) an address
    ready: CONNECT on 0.0.0.0:8080 to skhid.link:443 only, 128 at once; site on 0.0.0.0:443 to 10.77.0.1:443, 256 at once; 16 an address, 2/s (burst 10) an address; idle 120 s

A stop (`systemctl stop` or `restart`) says `stopping on SIGTERM:` with what was
open, then a line for each flow or connection marked `at stop`, and the process
exits with 0. A `ready:` with no `stopping` before it is a restart after a crash:
systemd's own lines in the same journal say how it ended (`Main process exited,
code=killed, status=6/ABRT` and `Scheduled restart job`). A thread of
`srp-relay` that panics ends the whole process (its panic message is just
above), so no port stops forwarding while the others go on.

    stopping on SIGTERM: 3 lobby, 5 match, 2 voice flows and 1 login connections open
    match 198.51.100.9:50122 at stop: 1834.2 s, up 120345 packets 10234567 bytes, down 98000 packets 40234567 bytes

A login connection's line, when it ends. `route check` is the updater's check
(it starts `SKHID-ROUTE-1:`), `login` anything else:

    login 198.51.100.9:50122: 0.8 s, up 412 bytes, down 1350 bytes, closed
    route check 198.51.100.9:50130: 1.2 s, up 65536 bytes, down 65536 bytes, closed
    route check 198.51.100.9:50131: 4.9 s, up 30000 bytes, down 28000 bytes, up: client: Connection reset by peer (os error 104)
    login 198.51.100.9:50140: 120.0 s, up 23 bytes, down 0 bytes, up: idle 120 s, no reply
    login 198.51.100.9:50141 not forwarded: the server 10.77.0.1:1234 not reached: Connection refused (os error 111)

- `closed`: both sides ended it. Otherwise each direction that did not close says
  why: `up` is client to server, `down` server to client; `client:` or `server:`
  names the socket that failed; `idle 120 s` nothing came; `<side> took nothing
  for 10 s` that side stopped reading. The first named is usually the cause, the
  other its consequence.
- `no reply`: the server sent nothing back. With `not forwarded ... not reached`,
  srp is not listening or the tunnel is down (`wg show skhid-relay
  latest-handshakes`).
- A route check cut by the player's provider has fewer than 65536 bytes and an
  error; the origin's line for the same check says what it echoed:
  `Route check from 198.51.100.9:50131 via relay 203.0.113.8: 4.9 s, 28000 bytes
  echoed, Connection reset by peer (os error 104)` (else `whole`, `closed by the
  client before the whole`, or `cut at 30 s`).

A UDP flow is one client on one port. Its line comes when it has been idle 120 s
(or at the stop); the seconds run from its first forwarded datagram to its last:

    match 198.51.100.9:50122 idle: 1834.2 s, up 120345 packets 10234567 bytes, down 98000 packets 40234567 bytes
    lobby 198.51.100.9:50122 idle: 4.1 s, up 9 packets 980 bytes, down 0 packets 0 bytes, no reply
    match 198.51.100.9:50122: 214 packets up in 10 s, none down: the server does not answer it

The last is said while the flow lasts, once, after 10 s of lobby or match
datagrams without a reply (voice is not said: a lone talker gets nothing back).
Many of them at once, or `no reply` on every flow, mean the origin does not
answer: srp down or the tunnel down. Down far below up in a long match flow means
the player lost much of what the server sent, on the way from the relay.

Refused and dropped, each said at its 1st, 10th, 100th, ...:

    login 198.51.100.9:50150 refused: over the allowance of 198.51.100.9 (2/s, burst 10); 10 such so far
    login 198.51.100.9:50151 refused: 128 connections open; 1 such so far
    match: a datagram from 198.51.100.9:50122 dropped: over its address's allowance (2000/s, burst 4000); 100 such so far
    match: a datagram from 198.51.100.9:50122 dropped: 8192 other clients are open; 1 such so far
    match: the server's socket failed: Connection refused (os error 111); 10 such so far
    match: a datagram not sent to the server: Resource temporarily unavailable (os error 11); 1 such so far
    match: a reply for 198.51.100.9:50122 dropped: not heard from within 120 s; 10 such so far
    CONNECT 198.51.100.9:45854 refused: 16 connections of its address open; 1 such so far

- `the server's socket failed: Connection refused`: the origin's port answered
  that nothing listens there (srp down or restarting). `not sent to the server`
  or `not sent to <client>` with `Resource temporarily unavailable`: the socket
  was full for 1 ms and the datagram was dropped (congestion here).
- `a reply ... not heard from within 120 s`: the server still sends to a client
  that left; a few after each match are normal.
- `over its address's allowance` on many datagrams of one address: a flood, or
  more players behind one address than the allowance carries.
- The web service's lines are `CONNECT` (updater downloads) and `site`, with the
  same parts: `CONNECT 198.51.100.9:45838: 0.0 s, up 0 bytes, down 0 bytes,
  refused: not the origin` is a request for another host.

## Checks and rollback

`tools/check regional-relay relay-packaging cargo:udp-transport
cargo:login-server cargo:lobby-server cargo:match-server updater-replays`
checks parsing, forged headers, concurrent clients, TCP/UDP round trips, address
advertising, fallback selection, Windows build and release packaging;
`tools/check relay-site` the site through a relay against a real Caddy. Run the
normal `tools/check` before a PR too.

To stop forwarding: `systemctl disable --now skhid-relay skhid-relay-web`. Remove the fallback
setting for a later player release. Direct connections keep working. Remove
`SRP_RELAYS` in a normal server release before dismantling the tunnel, and
make `caddy26` anew then: until that it believes the tunnel address. Its
anti-spoof rules deliberately remain even when the interface goes down.
To remove it later, stop/disable `wg-quick@skhid-relay` and delete only the
`inet skhid_relay` nftables table and this setup's files. Keep SSH access and the
server's existing firewall unchanged.

Protocol references: [PROXY v2](https://www.haproxy.org/download/2.9/doc/proxy-protocol.txt)
and [WireGuard setup](https://www.wireguard.com/quickstart/).

## Native packet path and measured overhead

Production game traffic uses `srp-relay` (the `udp-transport` crate's binary),
not the Python forwarding reference. Each UDP port has a receive thread per
direction, fixed packet buffers, immediate sends, and a bounded table of observed
clients. It adds no reliability layer, timer batching or per-packet payload
allocation. WireGuard does encryption in the kernel. The separate
`skhid-relay-web` process handles HTTPS downloads with a lower CPU weight.

Build the portable Linux server bundle with:

```
nix develop ./srp-026g --command bash srp-026g/deploy/relay/build.sh
```

Install `srp-026g/target/relay-dist/` as `/opt/skhid-relay/`. It includes the exact
ELF loader and runtime libraries, with SHA256SUMS; the service starts `run`.
Install both `relay.service` as `skhid-relay.service` and `relay-web.service` as
`skhid-relay-web.service`. Enable the web service with the game forwarder only
when the release is ready. Neither service changes the machine's default route.

On the supplied Debian host, 2026-11-17, `benchmark.py` measured 1,000 round trips
with 512-byte payloads through a loopback echo backend:

| Path | Median RTT | p99 RTT | Lost |
|---|---:|---:|---:|
| Backend directly | 0.0130 ms | 0.0219 ms | 0/1,000 |
| Native relay and same backend | 0.0259 ms | 0.0356 ms | 0/1,000 |

Added median RTT: 0.0129 ms. This isolates the forwarding process; it does not
include Internet routing, the game's own processing, or WireGuard. The separate
WireGuard host-to-host ping was 9.83–10.08 ms (3/3 received). These are measurements
of this host and route at that time, not latency guarantees for players.
Reproduce with `SKHID_NATIVE_RELAY=/opt/skhid-relay/run python3 benchmark.py` from
this directory. All benchmark/game stand-in sockets bind loopback only.

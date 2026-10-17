# packet-sim: a bad network between the game and a local srp

A forwarder that sits where srp tells the game to connect and impairs the
traffic on its way to where srp really listens. For reproducing
network-dependent bugs with the **real** game (a late join that never gets
prepared, a match lost to a blackout), with srp-bot, or with anything else
that speaks to srp. Python 3.10+, standard library only, one file
(`packet_sim.py`); the system has no Python on PATH, so use
`nix shell nixpkgs#python3 -c python3 ...`.

## What it does

- **Forwards** UDP per port (lobby, match) and TCP (login, web). Each UDP
  client gets its own upstream socket, so srp still sees one address per
  client; the simulator prints which (`match#2: client 127.0.0.1:50123 via
  127.0.0.1:41234`), and srp's log names the `via` address.
- **Impairs** per port, per client, per direction (`up` client to srp,
  `down` srp to client):
  - `loss_pct`: independent loss;
  - `burst_enter_pct`, `burst_exit_pct`, `burst_loss_pct`: Gilbert-Elliott
    burst loss (per packet; mean burst `100 / burst_exit_pct` packets,
    long-run loss `enter / (enter + exit)`);
  - `delay_ms`, `jitter_ms`: fixed delay plus normal jitter (sigma); jitter
    keeps the order, so with dense traffic the mean delay grows a little;
  - `reorder_pct`, `reorder_gap_ms`: a packet held back so later ones
    overtake it;
  - `duplicate_pct`: a second copy shortly after;
  - `rate_kbps`, `queue_ms`: a bandwidth cap with a queue of at most
    `queue_ms` and tail drop.
- **Scripted events**: `drop` (blackout), `hold` (a stall: everything waits
  until the window ends, then arrives in order) or `set` (other link
  settings for a while), `at` seconds after the start, after the client's
  first packet on that port (`anchor: session`) or after the client's first
  match datagram, either way, that carries message XX (`anchor: id:XX`, e.g.
  `id:DA`, the game's "loaded": the moment its first world goes), for
  `duration` seconds, optionally again `every` seconds.
- **By message** (match port): `ids: "E5,E8"` makes a rule or event take
  only datagrams that carry one of these message ids (hex; read as
  transport.md §2 to §4 lays them out, split parts by their message's id,
  0xE0 chunks and late-join answers as srp reads them; low-level ones are
  0xFA..0xFE), and `count: N` makes a `drop`/`hold` take at most N of them a
  client, direction and window (the first copy of a reliable message, say).
  Logged drops of the match port carry their `ids`.
- **Clients** are chosen with `peer`: `*`, `N` (the N-th client to send on
  that port, from 1), `addr:HOST:PORT`, `new-after:S` (a session that began
  at S s or later), `next-after:S` (the first of those on each port, alone),
  `late:S` (a session that began while another client of that port had been
  there S s: a late joiner); a comma list of these.
- **TCP** is not lossy (it retransmits by itself): a `drop` or `hold` event
  stalls the stream for its window, and `delay_ms` delays each chunk.
- **Reproducible**: every packet draws the same random numbers whatever
  happens to it, from a stream seeded by the seed, port, client and
  direction; the same seed and packet sequence give the same verdicts.
- **Live**: stats every `--stats-every` s (in>out, drops by reason,
  duplicates, reorders, holds, mean added delay), a final table, `--stats-json`.
  `--log FILE` writes JSON lines of every drop, hold, duplicate and reorder
  (`--log-all`: every packet; `--log-bytes N`: the first N bytes in hex),
  sessions opening and closing, and each event hitting a client.
- **Live control** (`--control PORT`, loopback UDP): add an event now, arm one
  for the next client to join, add a rule, clear, read the stats.
- A UDP client silent `--session-idle` s (30) is forgotten; its next packet
  starts a new session (new upstream port, new `late`/`new-after` check),
  under the same number.

## Where it sits

srp cannot advertise a port other than the one it binds, so the two use the
same ports on two loopback addresses (all of 127/8 is loopback on Linux):

```
game  ->  127.0.0.1:{login,lobby,match,web}   packet-sim
          127.0.0.2:{login,lobby,match,web}   srp: SRP_BIND_HOST=127.0.0.2,
                                              SRP_PUBLIC_HOST=127.0.0.1, SRP_LOCAL_PEERS=off
```

`SRP_LOCAL_PEERS=off` matters: with it on, srp tells a loopback client the
address that client reached it at, which is 127.0.0.2, past the simulator.
The game keeps its `-server=127.0.0.1:<login port>`.

## Commands

```sh
PS=srp-026g/scripts/packet-sim/packet_sim.py
python3 $PS profiles                       # the shipped profiles
python3 $PS run --srp-ports 2234,2235,2236,18088 --profile warp-like --control 9900
python3 $PS run --udp match=2236/127.0.0.2:2236 --set 'dir=down loss_pct=5' \
                --event 'port=match peer=2 action=drop at=30 duration=3'
python3 $PS local --config private/local-clients-sim/config.json --profile lossy-3pct --bots 2
python3 $PS ctl --control 9900 stats
python3 $PS ctl --control 9900 drop match 3              # all match clients, 3 s, now
python3 $PS ctl --control 9900 drop match 4 --on-join --after 1   # the next client to join (it alone), 1..5 s after
python3 $PS ctl --control 9900 drop match 3 --on-join --ids E4 --count 1 --dir down   # its first 0xE4 part
python3 $PS ctl --control 9900 drop match 4 --on-join --from-id DA --dir down   # 4 s from its 0xDA: its first world
python3 $PS ctl --control 9900 hold lobby 2 --peer 1 --dir down
python3 $PS ctl --control 9900 event 'port=match peer=late:10 anchor=session action=set loss_pct=30 duration=20'
python3 $PS ctl --control 9900 set 'port=match dir=up delay_ms=150'
python3 $PS ctl --control 9900 clear                     # drop the ctl events
```

- `run`: the forwarder alone, srp started elsewhere (with the settings
  above). `--srp-ports LOGIN,LOBBY,MATCH,WEB` forwards srp's four ports from
  `--listen-host` (127.0.0.1) to the same ports on `--target-host`
  (127.0.0.2); `--udp`/`--tcp NAME=[HOST:]PORT/[HOST:]PORT` any other.
- `--profile NAME|FILE`, `--seed N`, `--set SPEC` (an always-on rule after
  the profile's), `--event SPEC` (a timed one); a SPEC is `key=value` words
  or JSON. `--duration S` stops after S seconds.
- `local`: srp + the simulator from a local-clients config (below), plus
  `--bots N` srp-bot players through the simulator (one process each, so
  they make a match; `--late-bots M --late-after S`: the last M start S s
  later, as late joiners), plus `--clients N` headless game windows. Ctrl-C
  stops everything it started. Defaults: `--control` the match port + 10,
  `--log runtime/packet-sim/sim.jsonl`; srp's and the bots' logs are in
  `runtime/packet-sim/` beside the config.
- `click --display :N X Y` and `shot --display :N FILE.png`: drive and look at
  a headless game. The game reads the mouse as DirectInput deltas, so `click`
  homes the pointer with relative moves first (needs `xdotool`; `shot` needs
  `xwd` and ImageMagick).

## Profiles (`profiles/*.json`)

| profile | what |
|---|---|
| `clean` | nothing: the baseline |
| `lossy-3pct` | 3% loss both ways, 20 ms, 4 ms jitter |
| `warp-like` | a consumer VPN: 45 ms, 12 ms jitter, 0.3% loss and short bursts, rare reordering, a 300 ms stall every 40 s |
| `blackout-during-late-join` | a late joiner (`late:10`) loses all match traffic 1 to 5 s after its first match packet |
| `lossy-late-join` | a late joiner's first 20 s on the match port: 25% loss plus bursts |
| `match-blackouts` | the match port only: 3 s blackouts of everyone every 60 s from 30 s |
| `thin-downlink` | srp to client capped at 384 kbit/s (1.5 s queue), 60 ms |
| `late-join-world-outage` | a late joiner gets nothing from srp on the match port for 4 s from its 0xDA: its first world without its clock (the never-caught-up join of 2026-10-17) |

A profile is `{"description", "seed", "link": {...}, "up": {...}, "down":
{...}, "rules": [...], "events": [...]}`; a rule has `port`, `peer`, `dir`
and link settings, an event also `action`, `at`, `duration`, `every`,
`anchor`, `label`.

## With the game

The launcher is `srp-026g/scripts/local-clients/run.py` (branch
`tools/local-game-clients`; the main checkout has a copy; `--run-py` points
elsewhere). Never change `private/local-clients/config.json` for this: copy it.

1. `mkdir -p private/local-clients-sim && cp private/local-clients/config.json private/local-clients-sim/`
   and in the copy give a new `id` (any UUID), **ports no other server
   uses** (check `ss -lunt`; on both 127.0.0.1 and 127.0.0.2), the server
   build to test in `server.binary`/`working_dir`, and in `client_env` what
   headless DXVK needs (for NVIDIA `VK_ICD_FILENAMES`). The copy gets its own
   `data/` (accounts) and `runtime/` (prefixes, logs) beside it.
2. Ask the owner before any game window. Windows run only on a headless X
   display, never the desktop: `local --clients N` refuses without
   `--display`, refuses the session's own display, and drops
   `WAYLAND_DISPLAY`/`XDG_SESSION_TYPE` from the game's environment (else it
   dies with a divide by zero). `--xvfb` starts Xvfb there (`--xvfb-bin` if
   it is not on PATH: `nix build nixpkgs#xvfb`). Each prefix gets
   `HKCU\Software\Wine\X11 Driver UseXRandR=N, UseXVidMode=N` before its
   start (DXVK on Xvfb needs it). `--display-per-client` puts client i on
   display :N+i so the windows do not cover one another.
3. In each window click **Enter** (`click`; `shot` to see where) and then Play.
   The clients are run.py's: `run.py --config <copy> status/stop` work on them.
   Leave `virtual_desktop` off in the copy: run.py gives the exe by its bare
   name, which Wine's `explorer /desktop=` does not find (only the empty
   desktop comes up). A new account starts in the tutorial with Play greyed
   out: the drag the tutorial asks for does not work through `click` (Flash
   takes no relative-motion drag), so press its own "Skip tutorial" (640,645),
   OK (600,424), then pick a faction in the Missions panel (1010,260) and
   Join (1055,664); Play is then at (638,22). `SRP_TUTORIAL` stays on.

### A late-join repro

A match of the first client and bots, and a late joiner whose match traffic
is blacked out while it joins:

```sh
python3 $PS local --config private/local-clients-sim/config.json \
    --profile blackout-during-late-join --bots 1 \
    --clients 2 --display :91 --display-per-client --xvfb
```

For quick matches put in the copy's `server.env`: `SRP_BOT_ACCOUNTS` is
filled in by `--bots`; `SRP_MATCH_MIN_PLAYERS=2`,
`SRP_MATCH_GATHER_SECONDS=0`, `SRP_MATCH_JOIN_EXTENSION_SECONDS=0`,
`SRP_MATCH_MAX_COUNTDOWN_SECONDS=3` (otherwise each Play adds 30 s to the
queue's countdown) and `SRP_GROWING_GATHER_SECONDS=3` (games of revision 5
on queue in the growing queue, which otherwise gathers for 120 s), and
`SRP_LATE_JOIN_PATIENCE_SECONDS=1` when one newcomer would make the sides
uneven (1 v 1 plus one: it waits 60 s for a partner otherwise). Client 1 and the bot press Play and make the match;
once it has run 10 s or more, client 2 presses Play and joins late; its
first 1 to 5 s on the match port are lost (`event late-join blackout hits
match#N ...` in the output, and a `drop:event` count in its stats). A game
that loads in a few seconds sends nothing while it loads and has had its
0xE3 and 0xE4 before 1 s, so this blackout takes only keep-alives; to hit its
first world anchor the window on its 0xDA (`--from-id DA`, above).
Watch `shot --display :92 s.png` for the endless loading screen, and
`runtime/packet-sim/server.log` for the join. To place the blackout by hand
instead, use `--profile clean` and arm it just before client 2 presses Play:
`ctl --control <match port + 10> drop match 4 --on-join --after 1`. Without
the game: `--bots 2 --late-bots 1 --late-after 40`.

## Tests

```sh
python3 -m unittest discover -s srp-026g/scripts/packet-sim -p 'test_*.py'
```

The model (loss rates, burst lengths, order under jitter, reordering,
duplicates, the rate cap, holds, seeding), rules, selectors and profiles,
the registry edit, and real UDP and TCP forwarding to echo servers (one
upstream port per client, delay, a drop for one client, a late joiner's
blackout, live control, the log).

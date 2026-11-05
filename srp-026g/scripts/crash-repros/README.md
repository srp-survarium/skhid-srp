# Crash repros in the local 0.26g game

Scripts that reproduce the rev47 crashes of 2026-11-04 in a real local game,
headless, and rerun them on a fix's build. The game windows run on an Xorg
dummy display (60 Hz; Xvfb's 0 Hz crashes DXVK) with openbox, the sound goes
to a null sink (`pactl load-module module-null-sink sink_name=srp_repro`,
`PULSE_SINK=srp_repro`), and one game runs at a time (`flock` on a lock file).
Launching game clients needs the owner's go (skill `skhid-local-game`).

`$CR_WORK/env.sh` sets `DISPLAY`, `PATH` (xdotool, ImageMagick, ffmpeg) and
`PULSE_SINK`; `lib.sh` reads the run.py config (`CFG`, default
`private/local-clients-repro/config.json`: own ports, data and accounts).

| file | what |
|---|---|
| `replay.sh` | plays a `.sur` (from `srp-recording-to-sur`) and keeps the crash report, the game log and `srp_patches.log` |
| `lib.sh` | window, click and admin helpers (lobby and match clicks are relative moves) |
| `hold.sh` | under the lock: run.py start, hold until `$CR_WORK/release` exists, stop |
| `fire.sh` | PvB match: look up at the sky through the roof, fire the kit's Vepr-12 buckshot (9 shells + reload, n times), record the audio |
| `client2.sh` | starts one more client in its run.py prefix (a restarted game) |
| `c5b.sh` | kills a client in a match, starts it again and signs it in a moment before the match's time is up |
| `pair.sh` | a labelled before/after image |

## Replays (crashes 1 and 2)

```sh
SRP_GAME_DATA=private/release/rev47/content/server-game-data.db \
  cargo run -p match-server --bin srp-recording-to-sur -- rec.srprec \
  --game-modes <live /srv/srp/data/game_modes copy> --out <dir> [--keep-key-frames 79550,854800]
WINE=... WINESERVER=... WINEPREFIX=<own inactive prefix> DISPLAY=:83 \
  scripts/crash-repros/replay.sh <game dir> survarium_skhid.<build>.exe <dir>/rec.sur <out> 1000
```

The converter needs `SRP_GAME_DATA` of the recording's revision (without it no
0xE4 of a live match parses). The replay loads each key frame as a full
resync, so a world that holds a picked-up gun is loaded as a resyncing client
loads it.

- `rec026g_20261104-150325_5bfde2e7` (crash 1's match): crashes at the
  294650 ms key frame, `inventory_item::deserialize` +0x77 (read of 0x10C),
  right after place 11 (holding place 5's picked gun since 291550) was removed
  at 294609; the same on #1246's build.
- `rec026g_20261104-153456_71a7a62a` (crash 2's match): crashes at the
  789800 ms key frame in `bone_matrices_computer` from
  `player::collision_transform` (crash 2's stack), right after place 6's
  picked gun (place 4's kit-1 slot 7, in slot 10) was re-installed from the
  loaded record; the same on #1245+#1246. Patch 95 logs nothing there.

## World over 32 KB (crash 3, and the bullet section)

One client, Player vs Bots 10 v 10, Rudnya, Protective device, the kit's slot 7
a Vepr-12 with 120 buckshot (admin: `item=vepr_12`, `item=ammo_12mm_buck2`;
kit slots in `g26_kit_slots` while the server is stopped):
`fire.sh <tag> 12`. On main the client asserted after about 45 shells:
`mutable buffer overflow: size[32425], capacity[32768], need[65536]` in
`game_state_history_item::hash` (`mutable_buffer::resize` 0x7ABD0). On #1248's
client and server: 96 shells, no crash, no resync.

## Results screen with no match loaded (crash 5)

Two clients in a room's match (mode 5 only: `SRP_MATCH_MODES=5`); `FREEZE=1
c5next.sh <exe> <tag> 45` waits for the room's next match (time up = its "play
begins" log line + 902 s), kills the second client 85 s before the end, starts
it again, signs it in 45 s before the end, and with FREEZE stops the game
(SIGSTOP) from the lobby's "goes to match" until the lobby has sent it the
results. The room reseats a signed-in game 17-20 s after its sign-in. Not
reproduced yet: signed in too late the server gives no results screen ("its
game has loaded no match since it signed in"); frozen 120 s the lobby dropped
the link the same millisecond it sent the results (a 45 s lead keeps the
freeze near 25 s; that run was stopped by the owner's wrap-up).

## Side mouse buttons (PR #1257)

Lobby: hover SHOP (522, 30), `xdotool mousedown 8; mouseup 8`: main opens the
shop.

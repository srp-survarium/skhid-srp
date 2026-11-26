# The sync log (`SKHID_SYNC_LOG`, patch 210)

A diagnostic, never players': the owner's own game, started with `SKHID_SYNC_LOG=on`, writes to the
run's patch log (`game\logs\srp_patches.<run's start>.log`) what can tick a player at a time off the
50 ms grid. It is in every build and only reads; off, each of its three hooks tests one cell and
returns.

## Why

The desyncs of 2026-11-25 and 26 (Freeze Tag's matches 0x421b579c and 0x64f5afb0, and the same in
Team Deathmatch's 0x26010b5d and mode 6's 0x0074d85d) first differ between games in one player's
stamina (`player_stamina`, base_player +6C0h, serialized after the game effects):

- in 64f5afb0 at 334700 a game's place 2 had its last spend's time (+74h) 334697, the others'
  334650, the value the same; at 128750 the time 128713 against 128700 and the value 13 ulps apart;
- elsewhere the value 1 ulp apart with the same times (0074d85d place 5 at 472350, place 4 at 476500).

Both are one game ticking a player's stamina once more, at a time off the grid. The stamina is ticked
by `base_player::tick_animations` (0x2D8EE0, the call at 0x2D8FC2), which `base_player::tick` calls at
each step of the world and `base_player::apply_new_input` calls at the input's own time (0x2D8A01).
The stamina's x87 spend (`player_stamina::sprint` 0x329AB7..0x329AD1) over a tick in two parts
(47 + 3 ms) stores another value than over the whole tick in about one tick in four (a host test of
its instructions, 1 000 000 ticks); a tick 25 + 25 in one in two. The x87 control word is not it:
53 and 64 bits give the same stores (2 000 000 tries); only 24 bits differs, which nothing sets.
The recordings have no input relayed at 334697 or 128713, so the off-grid time came from inside that
game. Where is what this log is for.

## What it writes

One line for:

- **an input applied at an off-grid time** (apply_new_input's entry, `carry.input`, before Freeze
  Tag's held exit): the place, the time, the player's last tick time (+324h) and who called
  (player::set_input 0x1F36D0 is apply_new_input's only caller; the line gives set_input's own
  return, which tells the world's step (process_events 0x2D2180's apply) from any other caller);
- **an animation tick** (`sync.animations`, tick_animations' entry, which ticks the stamina too):
  one apply_new_input makes at an off-grid time (`by input`), and one a step makes from an off-grid
  time (`by step`, its previous tick off the grid: a step that went on from a part, a precise
  input's in every game alike, or a frame's part that stayed); any other caller's off the grid;
- **the server's world loaded** (a join or a resync: game_world_core::deserialize called by
  0x2D0B70, the 0xE8 handler's), with its time: the server log's "player N starts from the snapshot
  at T ms".

Each input and tick line ends with `live` (a time played for the first time) or `replay (newest T)`
(at or before the core's `maximum_time`), and after which load (`rollback`, `frame`, `server`,
`load_history_state` (the debug hash-mismatch compare and dump_game_state only), or the caller's address) with that state's time.

Not listed: a step to a frame's part (the world stepped past its newest state to the frame's time,
off-grid from an on-grid tick), which the next frame loads over (game_world_core::move 0x2D3568);
had one stayed, the next step's line (`by step` from an off-grid time) shows it. The game is not
told of a hash disagreement: the server resyncs it (0xE5, then the world), and the world's line is
that.

The check's lines (a made-up player and callers):

```
srp: sync log: on (SKHID_SYNC_LOG)
srp: sync: input p2 t334697 prev 334650 via set_input from 0x002d1f2a live
srp: sync: tick p2 t334697 prev 334650 by input live
srp: sync: tick p2 t334700 prev 334697 by step replay (newest 334750) after a load of 334600 by rollback
srp: sync: the server's world loaded (a join or a resync), its time 92250
srp: sync: 5 lines held back in second 400
```

At most 20 lines a second of game time (the next line of another second says how many were held
back) and 50 000 a run (its last line says so).

## Turning it on

- **The owner's own install** (Linux): `SKHID_SYNC_LOG=on ./Skhid-local.sh launcher`. The launcher,
  the updater and Wine pass the environment on to the game.
- **Windows**: `set SKHID_SYNC_LOG=on` in the `cmd` that then starts `Skhid.bat`.
- **Local clients** (`scripts/local-clients/run.py`): `SKHID_SYNC_LOG=on python3 run.py ...`; run.py
  passes its own environment on.

`on` or `1`, any case; anything else, or unset, is off. It is read once, at the first hook that asks.

After a desync (the game resynced: a short freeze and a jump, or the server log's "is out of sync" /
"no majority"), send the run's `srp_patches.<date>.log` from `game\logs` (the newest file of the run;
a long run goes on in `_2`, `_3`), with the match's time of day. The lines by `step` from an off-grid
time with no input line of that place and time before them, in a game whose stamina differed, name
the path; with the server log's ticks and the recording of the match, the caller's address is the
next step of the reading.

## Checked

`tools/check sync-log` (`check_sync_log_026g.py`, Unicorn): the three sites from their entry to
their span's end in the default build beside the original, on a made-up player, core and history
item: the registers and every byte of the made-up memory the original's, the log off and on; off,
the switch read once and nothing written; on, the lines above for 334697 and none for an input or an
input's tick at 334700 or for a step to a frame's part; the budget. The patches crate's tests
(`sync_log::tests`) cover the same rules and the line formats.

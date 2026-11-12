# The game's own collision world, read out and laid against the export

What the tools are for and what they found: `docs/notes/collision_world.md`. A test tool for a worker's own
game on Windows; nothing here is in the players' download, and nothing is written into the game.

| file | what it does |
|---|---|
| `bw_run_replay.py` | starts the game on a hidden desktop with a replay (`-match_replay=`), waits until the map's Bullet world has stopped loading, captures it and ends that one process; `--fpu N` also samples every thread's x87 control word and MXCSR |
| `bwmem.py` | the memory sources: a live process (read from outside), a recorder, a capture file (`.bwc`: the sparse memory image the walk touched) |
| `bw_walk.py` | walks `bullet_physics_world` → the dynamics world's objects → shapes, meshes, face data (layouts of the 0.26g exe, in its header) |
| `bw_report.py` | what a capture holds: objects by kind, group and mask, shapes, margins, scalings |
| `bw_diff.py` | a capture against the export of the same project (`mapdata-flat <file.mapdata> <out.mdf>`): placement, every mesh vertex, every face's walker behaviour, every primitive's exact shape |
| `bw_probe.py` | vertical probes of a capture's walker world (what the game has under a recorded body) |
| `bw_show.py` | one library's game shape beside its export library |
| `fpu_sample.py` | the control-word sampler by itself (`python fpu_sample.py <pid>`) |
| `fpu_eip.py` | the engine threads' control words by code range: a sample counts where the thread's own instruction pointer is (the walk tick, `btCollisionWorld`, the convex cast, GJK) (`python fpu_eip.py <pid>`) |
| `step_trace.py` | every step of every character controller of a running game, taken at the step (`python step_trace.py <pid> --out steps.bdt`): attaches as a debugger and stops on the CPU's debug registers at `updateAction`'s call of `player_step` (before and after), `set_transform` and `atomic_tick`; no byte of the game is changed (`docs/notes/body_trace.md`) |
| `step_file.py` | the trace's file: format, writer, reader, the controller's fields by the exe's own names |
| `step_report.py` | what a trace holds: the ticks on the 50 ms grid, the chain of whole steps, a look-ahead part against its whole step, the callers |
| `step_vs_recording.py` | a replay's trace against the recording it was made from (`srp-rec-table`'s `rec_worlds.csv`): the uploaded bodies against the replaying game's own |
| `test_bw.py`, `test_step_trace.py` | the capture format and the walk on a made-up memory image; the trace's file and reports on a made-up trace |

```sh
# a replay of the project (docs/notes/replay_sur.md): srp-recording-to-sur <recording> --out <dir>
python bw_run_replay.py --exe <game>\binaries\x86\survarium_skhid.exe --replay <dir>\<name>.sur --out level_07_4.bwc
cargo run --release -p mapdata --bin mapdata-flat -- <maps>\level_07_day_4.mapdata level_07_day_4.mdf
python bw_diff.py level_07_4.bwc level_07_day_4.mdf --out diff.txt --tsv diff.tsv
python -m unittest discover -s srp-026g/scripts/collision-world -p 'test_*.py'

# the game's own steps: leave the game running (--keep prints its pid), trace it, end it yourself
python bw_run_replay.py --replay <dir>\<name>.sur --out level_07_4.bwc --keep
python step_trace.py <pid> --seconds 60 --out steps.bdt
python step_report.py steps.bdt --show 40 --from 20000
SRP_GAME_DATA=<server game data db> srp-rec-table <recording> <tables dir>
python step_vs_recording.py steps.bdt <tables dir>/rec_worlds.csv
```

- The live reader is Windows only (`ReadProcessMemory`); a capture is read anywhere (`bw_diff.py`,
  `bw_probe.py` and `bw_show.py` need numpy).
- `bw_run_replay.py` refuses to start while another `survarium*` process runs, and ends only the process it
  started. The game shares the user's `Documents\Survarium`: `user.cfg` is compared before and after and put
  back if the game changed it. The game's sound still plays.
- `step_trace.py` is a debugger of the one process it is given: a worker's own test game, never another
  person's. The game may run slower than real time while traced (every stop is a round trip to the script); it
  detaches with the stops taken off and leaves the game running.
- Captures, `.mdf` files, traces and reports are the game's content: keep them out of the repository.

# Every run's logs in files of their own (patch 147)

The owner, 2026-11-15: "logs are never overwritten and are stored under new files, this gives us
more info and history on what crashes or not."

## The rules

- **Every run, new files.** Each start of the game, the updater and the client patches writes
  its logs to files made for that run, named by the run's start; nothing of an earlier run is
  ever written over or cut. Old runs' files are only ever deleted.
- **The name:** `<kind>.<stamp>.log`, the stamp the run's start in the player's local time to
  the second, `YYYY-MM-DD_HH-MM-SS` (`2026-11-15_14-03-07`): names sort by time. A further file
  of the same stamp (a second program started in the same second, a log's next part) is
  `<kind>.<stamp>_2.log` .. `_99`, past the highest the stamp has: a name a file has, or had
  before it was deleted, is never taken.
  - **The engine's file is made when it is named** (CREATE_NEW, empty; the engine then opens it
    create_always), not only when the engine opens it: two games started in the same second (two
    local clients sharing one game folder) both took the first name and wrote one file, and each
    one's console was empty. The console (`console_impl::tick` 0x1D0D0) shows the log file by
    its own count of lines (+4438h) and of bytes written (+4450h), reading from the file's start
    (`goto_line` 0x112740); with the other game's lines in it, its last lines lie past its own
    byte count, where `process_next_line` (0x1129A0) reads empty lines. A name another game made
    after the walk refuses the CREATE_NEW and the next is taken.
- **Retention:** before a run's file is made, files of its kind are deleted, the least recently
  written first, while there are 100 or more or they hold 500 MiB or more, so a kind keeps at
  most the newest 100 runs or 500 MiB, whichever is smaller. A kind's files are only the names
  that parse as its run files.
  - By last write, not by the stamp: a clock set back would otherwise make the run before (maybe
    the crashed one) the oldest.
  - The running run's files (its stamp's: a long run's earlier parts) count but are never deleted.
  - A file Windows does not delete (read-only, open) is passed over; the next one goes.
- **The old names** (`Survarium_<user>.log`, `srp_patches.log`, `srp_patches.old.log`,
  `updater_skhid.log`) are no longer written; since 2026-11-17 the files a player has under them
  are deleted with the old runs (below). Tools and texts that looked for the old name now take the
  newest run file of that kind (below).
- **No change** for the game servers, for what a match computes or for replays.

## Their folder: `game\logs`

The owner, 2026-11-17: "logs are now spammed in x86 folder", "this should be like torrents in its
own folder"; "lots of logs are spammed in Documents too". The run files of the client patches and
the updater, first written beside the exe in `game\binaries\x86` (up to 100 of each kind there),
and the engine's own, first written in `Documents\Survarium` beside user.cfg and the replays, are
in `game\logs\` now, the folder beside `game\torrent\`:

```
game\
  binaries\x86\     the programs
  logs\             Survarium_<user>.<stamp>.log, srp_patches.<stamp>.log,
                    updater_skhid.<stamp>.log, skhid-wine.log
    seed\           the seeding updater's updater_skhid.<stamp>.log
  torrent\          the torrents and their state
Documents\Survarium\  user.cfg, replays, ... and survarium_launcher_<user>.log (the original
                    launcher still rewrites it), left as they were
```

- **Who makes it:** whichever writes first. The updater (`Layout::logs_dir`) makes it at its every
  start; the game makes it before the engine's log and each patches' file (`Kind::make_folder`,
  CreateDirectoryA through the exe's import, `..\..\logs` from its working folder), so a game
  started without the updater (local clients, `run.py`) writes there too. `Skhid.sh` makes it for
  `skhid-wine.log`.
- **The engine's log** is named `..\..\logs\Survarium_<user>.<stamp>.log` (the stem the engine
  made, its folder `game\logs`, relative to the game's working folder like the patches' log). A
  name longer than the engine's string or MAX_PATH (a user name near 230 bytes) keeps the engine's
  `Documents\Survarium\Survarium_<user>.log`, as before.
- **The seeding updater** (`updater_skhid.exe seed`, at every lobby) keeps its runs in
  `game\logs\seed\` (they were in `game\binaries\x86\updater_skhid.seed-logs\`): in `game\logs`
  with the launcher's updater's, they would push those out of the kept 100.
- **Pruning** is as before, per kind and per folder: 100 runs or 500 MiB of `Survarium_<user>`
  (each Windows user's its own kind), of `srp_patches`, of `updater_skhid`, and of the seeding runs.
  Files of a kind left in its old place no longer count.
- **The old logs are deleted, not kept** (the owner: "we don't need preserving history"). A file
  a program holds open is skipped (the next start tries again).
  - **The engine's, by the game:** at each start, before naming its log, the exe deletes in the
    folder the engine named (`Documents\Survarium`) the run files of its kind
    (`Survarium_<user>.<stamp>[_<n>].log`, this Windows user's, the stem parsed as the engine's log
    is) and its unstamped `Survarium_<user>.log` (written no more since #1964), by DeleteFileA
    (`Kind::delete_all`); the patches' line says how many. The game, not the updater: it knows the
    engine's folder and its user's stem, and the updater's tests run with the tester's own
    Documents. The journal, `survarium_launcher_<user>.log` (the original launcher still writes
    it), another user's logs, user.cfg, the replays and the rest of Documents stay.
  - **Beside the updater, by the updater:** every updater run (the launcher, `ready` in
    `play_skhid.bat` / `Skhid.sh`, a replay link, a seed), before its own file is made, deletes in
    `game\binaries\x86` the stamped `srp_patches.<stamp>[_<n>].log` and
    `updater_skhid.<stamp>[_<n>].log`, the unstamped `srp_patches.log`, `srp_patches.old.log` and
    `updater_skhid.log`, and the seeding runs of `updater_skhid.seed-logs\` (the folder removed once
    empty), then logs how many went.
  - **`Skhid.sh`** deletes the old `skhid-wine.log` beside it.
  - Readers have no fallback to the old places: the crash reporter takes the run logs from
    `game\logs` only.
- **Not a game file.** Nothing the updater installs, checks or deletes is in `game\logs`: the ZIP
  and the torrents name their files (`release/build-torrent`'s list, the ZIP's own), the install
  replaces and removes only the files of the archive it installed (its record), the check hashes
  only those, and libtorrent sees only the torrent's files. The folder is never hashed, seeded or
  deleted as unknown.

## Which logs

| Log | Before | Now |
|---|---|---|
| The engine's log | `Survarium_<user>.log` in `Documents\Survarium`, made anew (create_always) at every start | `Survarium_<user>.<stamp>.log` in `game\logs` (`Documents\Survarium` from #1964 until 2026-11-17) |
| The client patches' log | `srp_patches.log` beside the exe, appended to; past 8 MiB renamed `srp_patches.old.log` (the one before replaced) | `srp_patches.<stamp>.log` in `game\logs`, made new; past 8 MiB the run goes on in `_2`, `_3`, ... |
| The updater's log | `updater_skhid.log` beside it, appended to; past 256 KiB begun anew (emptied) | `updater_skhid.<stamp>.log` in `game\logs` per updater process (a seed's in `game\logs\seed`); past 16 MiB one last line says it is full |
| `Skhid.sh`'s (Linux, macOS) | `skhid-wine.log` beside the exe, appended to | the same file in `game\logs` |
| The engine's journal, `Documents\Survarium` | `Survarium_<user>.journal`, recorded only with `-record_journal=1` since patch revision 4 (client_patch.md 3.10) | unchanged: off by default, a developer's switch |
| The launcher's | the patched Vostok launcher's own logs; their names are built from the user name, the build and a time stamp (launcher.md 12.2, launcher_updater.md: LIKELY, partly traced, not seen on a player's disk); the Rust launcher is not merged | unchanged (not our code to rename) |
| The crash reporter's | no log of its own: each crash's `SRP-crash-<time>-p<pid>-t<tid>-<n>.zip` / `.dmp` / `.txt`, never over an existing file | unchanged |
| DXVK's `survarium_skhid_d3d11.log` / `_dxgi.log` | DXVK's own, made anew each start | unchanged (DXVK names them) |

- **The engine's log** (patch 147, `rust/patches/src/run_logs.rs`, `vostok/run_log.rs`): the run
  starts at the process's first hook (`memory.allocate_arenas`, order 1, before patch 83's): its
  stamp. logging_initialize (0xE30E0) names the log through generate_log_file_name (0xE29C0, its
  only call) and hands the name to new_log_file (0x1128D0, its only call), which opens it
  create_always; the lohook `logging.log_named` (0xE31F8) between the two puts the run's name in
  the string (assign, when it fits the string's 104h bytes and MAX_PATH; else the old name stays
  and `srp_patches` says so). Same stem, in `game\logs` (above); another Windows user's logs are another kind.
  The folder ends at the path's last separator as the ANSI code page reads it (IsDBCSLeadByte):
  a two-byte character's trail byte 5Ch (Shift-JIS, GBK user names) is no separator.
- **The client patches' log** (`vostok/log.rs`): the run's file is begun at the run's first line,
  with CREATE_NEW, and kept open (appends only, every sharing); past 8 MiB it is closed and the
  next line begins the next name of the stamp. Patch 100's C code (`ad_log`) writes its lines
  through the same log (the Rust entry `log.c_line`), so no game code writes `srp_patches.log` by
  name. Lines logged before the run's start (none in the game; the Unicorn checks run hooks
  without it) go to `srp_patches.log`, appended to.
- **The updater's log** (`client/updater/logic/src/run_log.rs`, `exe/src/log.rs`): the stamp is
  the updater process's start; its file is made new once per process (CREATE_NEW), the oldest
  deleted first. Every message that names the log names this run's file; texts that cannot (the
  Linux and macOS scripts, Skhid.bat, LINUX.txt / MACOS.txt) say the newest `updater_skhid.*.log`
  in `game\logs`. A launcher start is one updater run (two when the updater replaces
  itself), `play_skhid.bat` / `Skhid.sh` add a `ready` run, a replay link is one: 100 updater
  files are about 50 to 100 game starts.

## The crash reporter

- **The crashed run's patches' log first:** the newest (last written) `srp_patches.<stamp>[_<n>].log`
  in `game\logs` (none there: none taken), its last 2 MiB from the first whole line, as
  `logs/game-logs/<that name>`. crash.txt's "Patch log" tail is the same file and names it.
- **Where it looks:** Documents\Survarium, the exe's folder, and for the standard layout
  `game\`, `game\logs\` (the scope `game-logs`), the install folder and its `temp\`; never
  recursively (the seeding runs in `game\logs\seed\` are not taken).
- **The game log tail** in crash.txt: the newest-written run's `Survarium[_<user>].<stamp>.log` in
  `game\logs`, which is the crashed run's (none there: none).
- **Earlier runs' logs** (the engine's, the patches', the updater's run files) are collected whole,
  newest first, with the other recent logs, within the reporter's unchanged limits (32 files,
  the last 48 hours, 16 MiB compressed, 256 MiB read, 5 s). Each is
  `logs/<scope>/source-NN.<game|srp_patches|updater_skhid>.<stamp>.txt`: the stamp and a fixed
  word, never the player's name. The crashed run's own patches' log is not collected twice.
- `srp_patches.old.log` is not collected; the other old unstamped logs are, as before.

## The tools

- **Skhid-collect-logs.cmd** takes the run files of each kind as well as the old names, from
  `game\logs` (`logs/game-logs/` in its ZIP), and the other logs of the exe's folder and
  Documents\Survarium (DXVK's, the launcher's).
- **Reading a player's logs:** the run's engine log, patches' log and updater log share the stamp
  of their start (the updater's is its own process's, a few seconds before the game's).
  "srp_patches.log" in the other notes means the run's `srp_patches.<stamp>.log` now.
- **Local clients** (`scripts/local-clients/run.py`) read no game log by name. Their prefixes
  share the one game folder, so every client's engine log and patches' log is in
  `<game_dir>\logs`, told apart by its stamp (and `_2` .. for clients started in the same
  second); with exes before `game\logs`, a client's game log is the newest
  `Survarium_<user>.*.log` in its prefix's Documents\Survarium.

## Release impact

- A new exe (patch 147, Rust arena), a new updater and a new crash
  reporter: the next revision ships all three with the players' download (`release/release.sh`,
  the `skhid-release` skill). No server, content or `simulation_changes.tsv` change.
- After it, a player's folder holds both: the old files from before, untouched, and the run files
  from then on. A crash report from an older exe has the old names, which the new reporter still
  takes.
- `game\logs` (2026-11-17): again a new exe (patch 147's `vostok/run_log.rs`, `vostok/log.rs`),
  updater, crash reporter, `play_skhid.bat` / `Skhid.sh` texts and Skhid-collect-logs.cmd, one
  revision. The updater of that revision deletes the old logs beside it at its first start (the
  old updater starts it as it replaces itself), and at every start after; the new exe deletes the
  engine's in Documents\Survarium at its first start. No server, content or
  `simulation_changes.tsv` change.

## Checked

- `cargo test -p patches` (`vostok::run_log`: the stamp's text and parse, the names of a kind and
  that only its stamped names are its files, the order of a stamp's files, the retention's
  limits) and Clippy.
- `check_run_logs_026g.py` (Unicorn, the built exe): the hooks in place, allocate_arenas' chain
  order, and the engine's site run to new_log_file on a made-up disk: the run's name; the oldest
  of 100 deleted and nothing else; 600 MiB down to 400; a name taken gives `_2` (engine and
  patches' log); 120 old patches' logs down to 99 and the new one, the old unstamped kept; a path
  with no room keeps the engine's name; a clock set back; a read-only oldest passed over; the
  run's own 600 MiB kept and `_4` after `_3`; a Shift-JIS user name with a 5Ch trail byte; patch
  100's `ad_log` line in the run's file.
- `check_hud_options_026g.py` with the process's start now running the run's start first.
- The updater's and the crash reporter's own tests (their READMEs).
- `game\logs` (2026-11-17): `check_run_logs_026g.py` with the engine's and the patches' files in
  `..\..\logs\` (CreateDirectoryA asked first, files of the kind beside the exe kept; the user's
  runs and unstamped log in Documents deleted, a held one not, another user's, user.cfg and the
  launcher's log untouched; a Shift-JIS user's 100 deleted; a user name too long for the folder
  keeps the engine's name); `test_run_logs.py` under Wine (the old logs beside the updater and the
  seed folder deleted, other files kept, nothing deleted twice); bugtrap's `check_logs.py` and
  `check_report.py` under Wine (the crashed run's logs from `game\logs` only); the collect-logs test of
  `test_release_overlay.py` (its `game-logs` entries; its listing's "other files beside game"
  count fails on main too).
- **Not run in a game.** To see it: start the game twice, then look in `game\logs` for two
  `Survarium_<user>.<stamp>.log`, `srp_patches.<stamp>.log` and `updater_skhid.<stamp>.log` files
  each (and none new in `game\binaries\x86` or `Documents\Survarium`), and make a crash
  report (`SKHID_CRASH_DUMP` is not needed) to see the earlier run's files in its ZIP.

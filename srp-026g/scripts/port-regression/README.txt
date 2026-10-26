Port regression: the whole game, old exe against the Rust-ported exe
=====================================================================

Every 0.26g exe patch is being ported from asm/C to Rust with no change of
behaviour (client-patches/port has the per-patch Unicorn checks). This runs the
real game, headless, with no player, on the OLD exe (main's build) and the NEW
one (the port branch's), and says per test PASS, FAIL or SKIPPED (with why).
It exits 0 only when every test passed; SKIPPED is not a pass.

The owner approves each real run (it starts Wine games, Xorg and a server):
  srp-026g/scripts/port-regression/run.sh --dry-run     # the plan; starts nothing
  srp-026g/scripts/port-regression/run.sh               # the run (refuses while a game is open)
  srp-026g/scripts/port-regression/run.sh --wait --only bots
  run.sh --help lists every option.

Prerequisites (all local, all already on this machine)
- private/local-clients/config.json (run.py's): the game folder, Wine, the
  inactive prefix template with DXVK, the srp server binary and its env;
  srp-bot beside the server binary (cargo build -p srp).
- the original exe: private/function-map/inputs/026g/survarium.exe; the dev
  shell (nix develop ./srp-026g) builds both exes with patch_026g.py from
  `git archive <rev>` (no worktree, no checkout), as port/equiv_026g.py does.
  --revision defaults to the config's SRP_MIN_PATCH_REVISION so the local
  server lets the builds in. --old-exe/--new-exe take ready exes instead.
- nix: Xorg + the dummy driver, openbox, xdotool, python3 are taken from
  nixpkgs by path (nix build --no-link), so every PID is the program's own.

What it never does
- starts while the owner plays: a live run recorded in
  private/local-clients/runtime/state.json, or any survarium*.exe process
  whose WINEPREFIX is not under this run's folder, stops it (or --wait polls,
  --wait-timeout). Checked again before each game and every 30 s during one;
  a game of the owner's that appears stops the test (SKIPPED).
- uses the owner's screen: its own Xorg (dummy driver, 1280x720 at 60 Hz:
  Xvfb's 0 Hz crashes DXVK) on a free display from :120, -nolisten tcp,
  openbox; WAYLAND_DISPLAY and XDG_SESSION_TYPE unset; the exe run directly.
- touches the owner's game, prefixes, accounts or ports: the game is linked
  into <work>/game (exes and DLLs copied there, so crash reports land there),
  the prefixes are copies under <work>/prefixes (deleted at the end unless
  --keep), the server gets free ports from 52234, its own data dir, packet
  log and snapshots. The cleanup kills only what it started, by PID (with the
  start time checked) and wineserver -k of its own prefixes only.

Steps
1. replay (--replay PATH..., default private/recordings/*.srprec)
   A .srprec is converted with srp-recording-to-sur (built in the dev shell
   if missing) and --first-key-frame-only, so the game simulates the whole
   match from the recorded inputs instead of being put right by a recorded
   world every 10 s. The game modes it needs come from --game-modes, or are
   extracted from the game's resources.db and its patches with re-kit's dbgrab.
   Each replay is played with `-match_replay=Z:\...` for --replay-seconds by
   the old exe, then by the new.
   - playback: PASS when both ran without a crash (exit code, bugtrap zip,
     Wine's unhandled-exception lines, fatal log lines) and the new exe's log
     has no ERROR/FATAL kind the old one's lacks. SKIPPED when the old exe
     fails too (no baseline; e.g. a recording of another patch revision).
   - hashes: SKIPPED today. A replay computes its world but sends no 0xDF
     (that goes to a match server only) and no exe patch writes the world CRC
     to a file. When one does, as <Documents>/Survarium/srp_world_hashes.csv
     with `time_ms,crc` rows (--hash-csv names another), the two are compared
     tick by tick and must be identical.
2. bots (--bot-matches label:mode:map,..., default battery 2:61, tdm 4:126,
   research 5:145; --bot-minutes 3)
   Per match: a fresh srp with SRP_MATCH_MODES/MAPS, SRP_BOT_ACCOUNTS and
   srp-bot --drive (it walks, turns, fires) as the other players; the new exe
   and (--bot-clients pair, the default) the old exe as two real clients.
   They sign in by clicks (the game reads relative mouse motion: to the corner,
   then --login-click 1110,550 for Enter and --play-click 640,22 for Play, the
   in-game cursor positions of private/editor-026/game-test/HARNESS.txt).
   - bots:<label>: FAIL on no match, a client gone early, a crash, a server
     "out of sync ... resyncing it", "the clients disagree", "sync is done
     again", a panic or an [ERROR] line.
   - bots:<label>:hashes (pair only): the two clients' 0xDF reports in the
     server's packet log (u8 tick, u32 CRC), paired by tick and arrival, must
     all agree: old and new simulate the same match in lockstep.
   Not covered: "Player vs Bots" (the server's real bots) needs a click on a
   lobby control whose position is not recorded; srp-bot's scripted players
   are used instead.
3. frames (patch 71's srp_frames.csv)
   Patch 71 (the freeze detector, a local-only build) is on feat/freeze-detector
   only, not on main or the port branch, so this is SKIPPED unless
   --frames-old-exe/--frames-new-exe are given (or the old/new exes carry it).
   Then the first replay is played by each, and p50/p95/p99/max of frame_us in
   play/replay frames are compared: FAIL when a percentile grew by more than
   --frames-threshold (10 %) and 0.5 ms, or the longest frame doubled.

Output: <work> (private/port-regression/run-<time>/): results.tsv, logs/,
replay/<name>/{old,new}/ (wine.log, the game's docs: Survarium_*.log,
srp_frames.csv), bots/<label>/ (server.log, packets/, bots.log, client logs,
crash-reports/), frames/. The summary table is printed at the end.

Tests of the helper (no game): python3 -m unittest discover -s
srp-026g/scripts/port-regression -p 'test_*.py'

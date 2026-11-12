# Module reports (client patch 125)

Each game tells the match server what is loaded into it besides Windows' own DLLs, which DLLs
were loaded since its first instruction, and which other programs' always-on-top windows
(overlays) are over it. The server writes it to a log; `release/moderate modules` reads the
log across all games.

## The rules

- **Only logged.** No automatic action, nothing in the db, nothing a player sees. Discord,
  Steam, RTSS/MSI Afterburner, OBS and DXVK put DLLs into the game and windows over it, so an
  entry is read against everyone else's (`release/moderate modules`).
- **Other processes: only whose window is over the game.** The game tells which processes
  have an always-on-top window over its own, by exe file name and folder, window class, styles
  and size. Such a process is opened only to ask Windows where its exe is, with the one right
  that can ask nothing else (`PROCESS_QUERY_LIMITED_INFORMATION`), and the handle is closed at
  once. Nothing else of another process is looked at: not its title, its other windows, what
  a window shows or its memory.
- **Privacy.** A module is told by its file name and where its folder is: below the game's
  folder, below the temporary one, or (any other) its path from the drive with a user's name
  left out (`C:\Users\*\Downloads\tools`; also Wine's `Z:\home\*`). At most 64 bytes of it.
  Modules in the Windows folder are not told.
- **When.** With a match connection's first world (`world_load.loaded`) and when the match
  closes on the game (`match_recovery.close`), once each, on the client-report channel
  (`srp_validate::report`, kind 2; `memory-report.md` describes the channel).
- **Each minute in between** (the owner, 2026-11-11: a window opened and closed again mid-match
  was never seen): a minute after the last look, a frame of the match (`game.tick`) looks at the
  windows over the game again. When it saw otherwise than the last look, a report of moment
  `minute <n>` (minutes since the first world, up to 254) tells those windows alone, so an
  overlay that stays the whole match is told once and an empty one says the windows are gone.
  The modules, loads and code are told only at the first world and the end. Nothing is
  taken from the process list when no window is over the game, the usual look.
- **What is told** (`srp_validate::modules`), the rare kinds first so a long module list cannot
  crowd them out; up to 144 entries a report, more are only counted:
  - **over**: each other process's window that is visible, always on top (`WS_EX_TOPMOST`), above
    the game's window in the windows' order and over some of it: the exe's file name and where
    its folder is (as a module's, a user's name left out; `windows` for the Windows folder, `?`
    where Windows does not tell), the window's class, its extended styles and how much of
    the game's window it covers. Needs no right. Discord's,
    Steam's and a recorder's overlays, the taskbar and a volume pop-up are such windows too:
    read rarest first. The first 16 from the top. Where it cannot be told, one entry says why:
    the game has no window yet, or its window is not in the order;
  - **module**: each module of the loader's list: name, place, image size, the PE header's
    link time and checksum (a build of a DLL, not just its name);
  - **load**: each DLL the loader said it loaded after the game's start, with its place, size
    and PE marks as it was loaded, when, and whether the loader's list still has it. One that was
    loaded and then taken out of the list stays on record. The first 32;
  - **code**: each executable allocation that is no listed module (a DLL mapped by hand, JIT
    code, an overlay's trampolines): kind of memory, protection, sizes, a fingerprint of its
    first 256 executable bytes, the PE marks of a header at its start.
- **Times** are whole seconds since the game's start.

## How

- **The start.** The exe's TLS directory (0xA365A8) names a callback list of ours
  (`module_watch_callbacks`; the original's list was empty). Windows calls a TLS callback on the
  main thread before the exe's entry point, so before the C runtime, `bugtrap.dll` (which the exe
  loads itself, later) and anything loaded afterwards (and then on every new thread, which the
  watch does nothing with). Only the DLLs the exe imports run earlier; one of those replaced by a file
  in the game's folder shows in the module list by its folder.
- **The watch** (`vostok::module_watch`). At the first call it notes the time and the folders
  and registers ntdll's DLL notification (`LdrRegisterDllNotification`). The notification tells
  loads; the DLL's header is read then, mapped already. It comes with the loader's lock held,
  so it only keeps a record in fixed room.
- **Windows over the game.** The game's window is the one its swap chain last presented to
  (`render_output.present`), or the top-level window that one is in. From the top of the
  windows' order (`GetTopWindow`, `GetWindow` next) down to the game's own, each window is asked
  its process, whether it is visible, its extended styles, its rectangle and its class
  (user32's own questions; no message is sent to it). The exe's path of each one found is asked
  with `QueryFullProcessImageNameW`. A window that is not always on top cannot
  stay above the game while it is played, so only those are told: what the player has open
  beside the game is not.
- **Memory is read with `ReadProcessMemory`**, which fails instead of faulting on a page made
  unreadable or freed meanwhile.
- **Windows' functions** are called through their own exports (dynamic rows of the engine
  library). A Windows without one (`LdrRegisterDllNotification` is Vista's) leaves that part off.

## The server and the tool

- The match server logs each part once per connection to `logs/modules.log` (16 files of 4 MB):
  `<name> (account <id>), match <id>, <moment>, part 1/2 of 12 entries: <entry> | <entry> ...`
  (`client_report.rs` gives each entry's form), the moment `loaded`, `ended` or `minute <n>`.
- `release/moderate modules` lists each entry with the accounts that have it, rarest first, a
  module with the folders it was told in; `--account <name>` lists one account's entries with
  how many accounts share each.
## Checked, and not

- Checked: the codec, the folder and name rules, the address-space scan, the
  windows' overlap (`srp-validate`'s tests); the watch's records (the patch
  crate's tests); the server's line and that a report removes no one from a match (match server's tests); the
  watch as Windows calls it, in Unicorn (`check_module_report_026g.py`: the TLS directory, both
  callbacks' stack and registers from every stack skew, the registration once, what is asked and
  kept).
- Run in a game once (2026-11-10, one headless Wine client with DXVK, a bot match, a test build
  of this branch and its server): the game starts with the TLS callback and plays; the report
  at the world's load reached the server's log with the modules, the three DLLs loaded after
  the start (`dxgi.dll`, `BugTrap.dll`, `d3d11.dll`, each with its PE marks as loaded and still
  listed). Wine has one executable image no module
  lists (`code image rx 2000/c000`), not traced.
- Windows over the game, in the same kind of run with Wine's task manager set always on top
  over the game: with the game in a window the report has `over "taskmgr.exe" class "#32770"
  10108 404x453 from windows` (its exe is in Wine's Windows folder; a place outside it has
  only been seen in tests); with the game fullscreen (its window above the task manager, which the
  screen does not show either) and without the task manager, no over entry. On Windows it has
  not run: what the taskbar, Windows' own pop-ups and the usual overlays look like will show
  in the first live reports.
- Unseen so far: the report at a match's close; a minute's report in a game (only the state machine's tests
  and the server's line); Windows; what overlays look like in the log.

## Executable identity and collection

The client now hashes the **file of the running executable**, located with
`GetModuleFileNameW(NULL)`, using SHA-256. It opens that one Unicode path
read-only, denying concurrent writes/deletes, and keeps that same handle through
hashing and any upload. It does not select files by a server-provided path, scan
folders, read DLL files, or upload process memory. A renamed executable is still
identified. The file includes all its bytes, including appended data; the hash
is of the disk file, not of relocated/mutated process memory. A modified client
can lie: this is a client report, not remote attestation.

After the first world, hashing reads at most 8 KiB per frame. An offer carries
only size and SHA-256, logged in `modules.log` as `executable sha256 <hex> size
<bytes>` and included by `release/moderate modules` (also `--account` and
`--json`). Files over 16 MiB, empty files, or read/open failures produce no offer.

The match server replies with `Stored`, `Refused`, or the next window offset.
Only `Need` starts an upload. E0 carriers have the separate impossible prefix
`ff ff ff ff ff ff 04 00`, an explicit length, match id, kind, hash, and size or
offset. Data chunks are at most 400 bytes. E4 replies start `ff 45 58 01` and are
consumed by the Rust message guard before native options handling; only clients
that offered the protocol receive them. The shared codec is
`srp_validate::executable`. Existing report formats remain unchanged.

There is at most one 16-chunk (6,400-byte) window outstanding and four new chunks
per 25 ms (64,000 file bytes/s maximum, less on a slower link/frame rate).
Transport ACK/resend handles packet loss; an unanswered offer is repeated at
most once per second. Chunks within a window may arrive out of order; exact
duplicates are harmless and conflicting duplicates abort. Match and hash must
match. Closing the connection drops the file handle; a later match opens and
hashes again. Known hashes need no file transfer, including after server restart.
Older clients continue reporting normally; an old server sends no upload request.
Deploy the updated server with/before the client.

The server stores verified files as `client-executables/<sha256>.exe` under
`SRP_DATA_DIR` (the working directory when unset), mode 0600 inside a directory created mode 0700.
It computes SHA-256 itself as complete windows arrive, checks the `MZ` header,
and publishes only when size and hash match. Nothing executes these files.
There is no automatic ban or match simulation change. At most 16 uploads are
active, a transfer times out after ten minutes, and stored bytes plus reservations
are capped at 256 MiB (1,024 completed/active files maximum). Partial files are
removed on failure/disconnect/timeout; leftovers after a process crash count
against the capacity on restart. A full/unavailable store refuses new uploads
but still logs hashes. Operators may archive/remove files and restart to rebuild
the in-memory catalog. Raw match recordings can also contain these protocol
packets; playable replays select gameplay records, not executable payloads.

**The release's own executable is not collected.** The release writes the
SHA-256 of the game executable in the players' download into the server's
`.env` (`SRP_RELEASED_EXE_SHA256`, beside `SRP_MIN_PATCH_REVISION`; `release/lib.sh`,
`swap_server`), and the match server reads it once, at its start: the
release's restart. An offer of that hash is answered `Stored` without
looking at the store: no upload, nothing written, nothing reserved; the
hash is logged as any other. Every other hash the server has not stored is
collected as above, and no one is refused or removed by either answer.
Without the setting (a local server) the release's own executable is
collected like any other. A rollback puts the line back as it was. Before
this, the first players after each release (up to 16 at once) each uploaded
the 14 MB executable the release had just given them.

Implementation and validation are recorded below; this source change is not a
release and has not collected a live player's executable.

2026-11-12, branch `feat/executable-hash`, based on `6952d835e`:

- Implemented: shared wire codec, safe Rust client owner and pacing, generated
  Win32 ABI/hooks, bounded server store, moderation output and replay filtering.
- Host-tested under `nix develop` through `tools/check`: client (302 passed,
  two existing ignored tests), `srp-validate`, `udp-transport`, `sign-in-cipher`
  and match server tests and Clippy. The receiver test transfers 4 MiB + 17 bytes
  with reordered/duplicate chunks and verifies the stored bytes, restart dedup,
  corruption rejection, interruption cleanup, capacity and timeout handling.
  A scripted three-player match checks wrong-match/malformed traffic without
  disrupting the match. Moderation grouping tests pass.
- Native-tested: generated engine bindings, hook entries and arena lock agree;
  engine ABI, hooks and existing module-watch callbacks pass in Unicorn
  (`executable-report-native`, input key `3043657a1902`). Arena size `0x11b0a8`,
  SHA-256 `c299bf62d999e3481a14d345225049286a5c742a13357386576cfab316b82ca5`.
- Broader corpus check: **failed** (`world-check`, input key `5f6178a2ea77`):
  250,760 worlds checked, 9,606 accepted, 241,154 refused, none unread or skipped.
  Failures include older revision-50 object layouts, owner IDs and weapon-pickup
  records. The refusal summary is byte-for-byte identical to cached check
  `45d112cb5e2f`, completed at 09:36 CET before this feature's sources were
  created (13:12 CET). This change does not modify the world parser or game data.
  These existing failures remain unresolved, so the full suite is not clean.
  Detailed output remains in
  `/tmp/check-world-check-ss5mqt7q/world-check.log`.
- Client/server world drift: **failed before comparing worlds** (input key
  `aeede6693482`), all 20 chunks stop with Unicorn `UC_ERR_READ_UNMAPPED` at
  `0xa4ccfd`. The same fault exists in earlier logs `b7c1d21bd976` (09:42 CET)
  and `c4955b14d2b8` (09:41 CET). This run therefore establishes no corpus-wide
  client/server agreement. Per-chunk traces: `/tmp/check-world-drift-_7e3zf5z`.
- HUD options: existing `configs.start` registers/stack/settings assertion
  fails (`0e2841881d8d`), matching the earlier 13:08 CET run `bd3e019c3746`.
  No HUD configuration code is changed here. The automatic `tools/check` run
  therefore fails these three broader checks; affected Rust and the new
  executable-report checks pass.
- Played: not yet exercised in a real Windows/Wine match. These tests do not
  establish the real disk-read/frame-time cost or full native upload lifecycle.
- Released: no; no revision bump, deployment, restart or live collection.
  Existing installed clients cannot send files until they receive this feature.

Review and main integration, 2026-11-12:

- Reviewed framing/bounds, match/hash binding, retries and duplicate windows,
  file ownership and cleanup, store reservations/publication, native ABI/hooks
  and replay filtering. No blocking defect found.
- Integrated main `a3af060cd` (distance detail), retaining both sets of engine
  bindings and check registrations. Regenerated the combined arena rather than
  choosing either branch's old lock: size `0x11c44c`, SHA-256
  `7594be1bdbd2489701f167791eb4b953133e220ede48d96ea8a38799a26b4f5f`.
- Combined client tests/Clippy pass (`77ed6b05abf0`); generated ABI/hooks/lock
  and targeted native checks pass (`925d11babbfd`). Affected server/codec/
  transport/cipher checks retain their matching-input passes; `cargo:srp` also
  passes (`ebf346cc3a4e`). Combined world-check (`20d740daa842`) has the same
  refusal summary as the pre-feature baseline, compared byte for byte.
- The distance-detail native and Ruffle menu checks pass on this combined
  arena (`606f218a6035`), covering the main change that shared its placement.
- Combined world-drift (`22193f666e77`) reproduces the same pre-existing
  `0xa4ccfd` fault in all 20 chunks. No corpus-wide agreement is claimed.
- Combined HUD-options (`b8439d25a4a9`) reproduces the same `configs.start`
  assertion. The automatic run finishes with only these three known failures.

Main advanced during verification to `b4e08f97f` (local replay-file opening).
Integrated that change too and regenerated the shared arena: size `0x11c44c`,
SHA-256 `0d4bd09a888d78f6811e55e0930f3d793bf849165d7f860c0d79f2797aa0e070`.
Client tests/Clippy (`55b5f488ecd8`), generated ABI/hooks/lock and native checks
(`006beff71781`), and distance-detail native/menu checks (`a8c013d943bd`)
pass together on this tree. World-check (`78e885993975`) again reports
9,606 accepted and 241,154 refused of 250,760, the same existing baseline.
Replay-file native success/failure, generated hooks and client tests/Clippy
also pass on the final combined arena (`1fc057583b52`).
The final automatic run reproduces only the three documented baseline failures:
world-check above, world-drift (`7fcf3f90b666`, the same `0xa4ccfd` fault), and
HUD-options (`c963856499a8`, the same `configs.start` assertion). All affected
Rust checks and the executable-report, distance-detail and replay-file checks
pass. Real Windows/Wine executable-upload testing and release remain outstanding.

# Skhid updater (Rust, Win32)

`updater_skhid.exe`: the program the patched 0.26g launcher
(`srp-026g/client-patches/launcher`) starts in the place of the game's own
`survarium_updater.exe`. It keeps the few Skhid files of a player's game
(the patched exe, the certificate, the crash reporter, the start scripts,
the launcher and itself) the same as the ZIP the server's page offers, and
the game folder's `resources.db.patch_<n>` files, which the game mounts
over its own `resources.db`, the ones the server lists ("Patches" below).
New content reaches the players in those patches alone; `resources.db`
itself stays as the game ships it (an earlier updater changed parts of it
in place: those get their stock bytes back once, "Restoring resources.db"
below).

An ordinary 32-bit Windows program of about 380 KB, like `../bugtrap`: Rust's
standard library and Microsoft's C runtime, linked in (`+crt-static`, as
`cl /MT`; no runtime DLL beside it), the game's icon, a version resource
(Skhid, "Skhid updater", the release revision) and its `asInvoker` manifest.
Windows 7 and later. It imports `kernel32.dll`, `user32.dll` (its message
box), `winhttp.dll`, `shell32.dll` (the Documents folder, where the game's
user.cfg says the language its messages are in) and `ntdll.dll` (two
functions of the standard library's).

## What it does, and does not

- Asks for Skhid updates at the host in `updater_skhid.url` beside it, else the one it
  was built with (`SKHID_UPDATE_URL`, default `https://skhid.link`). HTTPS
  through Windows' own WinHTTP (its TLS, its certificate store, the system's
  proxy settings); plain HTTP only for `127.0.0.1` and `localhost`. No
  redirect of the update server is followed (a patch file's are, up to 5:
  "Patches"), no cookie kept.
- `GET /api/client-files` → `skhid-client-files 1`, `sha256 <hex>`,
  `size <bytes>`, `min-revision <n>` (the least game revision the login
  server lets in; an srp from before it does not say it); `GET /download`
  → the ZIP. Both are routes of `srp-026g/servers/web-server`.
- Installs only what passes every check, all files or none, and can replace
  the running launcher and itself. The steps, and what a player sees when
  one fails: `srp-026g/client-patches/launcher/README.md` ("How an update
  works", "When something goes wrong").
- If the original `survarium.exe` or `resources.db` is missing, downloads
  the original game from the Google Drive mirror, checks a compiled-in
  SHA-256 and installs the missing original files before the Skhid overlay.
  A new player can unpack the overlay into an empty folder and start
  `Skhid.bat`. No Google login, browser confirmation or administrator
  prompt is needed. Allow about 10 GB of free disk space. The game is
  unpacked with `7zr.exe`, a file of the Skhid archive beside the updater
  (`extractor/`): nothing that is run is downloaded on its own.
- After the Skhid files, at every start: the regions of `resources.db` an
  earlier updater rewrote in place (its record beside the updater names
  them) get their stock bytes back, from the copies it kept, else from
  `GET /resources/<sha256>`. Best effort, once: it never fails the update
  ("Restoring resources.db" below).
- Then, at every start, the patch step: `GET /patches`, the server's list
  of resources.db's patches, each with its SHA-256 and the addresses of its
  file (1 to 8, tried in order: mirrors; plain HTTP or HTTPS, any host);
  every patch file is checked (hashed only when it changed since its last
  hash, or that was a week ago: the quick check), a patch file the list has
  not is deleted, a listed patch that is missing or damaged is downloaded
  from the first address that gives it. Required: PLAY stays off until
  every listed patch is in place ("Patches" below).
- Keeps PLAY off for a game the server would refuse: a game exe that says
  an older revision than the server's `min-revision`. While the launcher
  stays open after the update it asks the server again every 10 s, turns
  PLAY off while a server that answered stops answering, and starts the
  launcher anew (whose updater updates) when the server offers another ZIP
  or revision ("PLAY and the server's revision" below).
- Never takes a Skhid file it cannot read for one it installed ("A file
  that cannot be read" below): blocked by Windows Security, or held by
  another program, the update stops with PLAY off and a box that says what
  to do, with Retry; else the file is repaired, or the failure names it.
- Writes `updater_skhid.state` (what is installed)
  and `updater_skhid.log` (what it did; no account name, no password; it
  does hold the game folder's path) beside itself. `updater_skhid.lock`
  prevents simultaneous installs in the same folder. The patch step writes
  its own files beside it too (listed under "Patches").
- No BitTorrent, no listening port, no UPnP, no registry read or write, no
  service, no crash upload, no administrator rights (its manifest says `asInvoker`;
  without one Windows takes a program named "updater" for an installer and
  asks for elevation).

## How it is started

| Command line | Who | What |
|---|---|---|
| `"<exe>" "update" "\\.\pipe\vostok_updater_pipe-<product>" ...` (also `full_update`) | the launcher | The update, reported over the launcher's pipe in the messages of the original updater (`logic/src/pipe.rs`). It stays until the launcher says stop or goes away, asking the server every 10 s meanwhile ("PLAY and the server's revision") |
| `"<exe>" "copy" "<pipe>" ...` | the unpatched launcher | Answers "copy done" at once: nothing needs copying |
| `updater_skhid.exe check` | a person, a script | The update without a launcher, every patch file hashed whole whatever the quick check's record says: the repair, when a patch file may be damaged in a way the quick check does not see (bytes changed in place with the size and the time kept, as no program does by chance). When the ZIP replaces the updater itself, the new one is run at once (`check`, `quiet` as given) to restore resources.db and do the patch step, and its exit code is this one's. An error is a message box |
| `updater_skhid.exe check quiet` | a script | No box either. Exit code 0: done or nothing to do; 1: failed (the reason is in the log) |
| `updater_skhid.exe ready` (`quiet`: no box) | `play_skhid.bat`, `play_skhid.sh` before they start the game | Reads only: no lock, no server, nothing written but the log. Exit code 1 (and a box that says to start the launcher) when the last patch step did not end `ready` (a new overlay since, too), or the patch files are not now what its list says (one missing, damaged, or one more; looked at with the quick check); else 0 |
| `updater_skhid.exe replay <id> <token> <sha256> <size>` | the patched game (patch revision 14) | "Watch replay": fetches one replay and starts a second game that plays it (below). A problem is a message box |
| `updater_skhid.exe replay <id> <token> <sha256> <size> quiet` | the tests | The same with no window at all: the problem is in the log and the exit code (1) |

Exit codes: 0, 1 (failed), 2 (the launcher's pipe could not be opened), 3
(an internal error; logged).

It must be in the game's `game\binaries\x86` folder. From anywhere else it
says so and does nothing.

## Layout

| Folder | What |
|---|---|
| `logic/` | Everything that needs no Windows, as a library with no dependencies: SHA-256, DEFLATE and CRC-32, the ZIP reader and its name rules, the text formats (the server's answer, the installed state, the earlier updaters' record of resources.db, the list of resources.db's patches, the quick check's record) and what the restore and the patch step decide, the launcher's pipe messages, command-line splitting, the server address and a patch's. `cargo test` there runs on any host |
| `exe/` | The program: WinHTTP, files, the pipe, the install with its undo, the texts a player sees. Built for `i686-win7-windows-msvc` with `-Z build-std=std,panic_abort`, `rust-lld` and Microsoft's C runtime, as `../bugtrap`; `res/` holds its icon and version resource |
| `tests/` | `test_updater.py`: the real exe against a web server on 127.0.0.1, with the test in the launcher's place on a real named pipe (Windows only). `test_db_patches.py`: the patch step and the restore, Windows or Wine. `test_play_gate.py`: PLAY against the server's revision and the watch while the launcher stays open, Windows or Wine. `test_game_download.py`: the original game's download. `test_unreadable.py`: a Skhid file that cannot be read, Windows or Wine. `fake_exe/`: a stand-in for the game and the launcher that writes down how it was started; it also holds a file as the game or an antivirus does, and is the launcher's end of its pipe, for the tests under Wine |

## PLAY and the server's revision

The login server lets a game in when its exe says a patch revision of
`SRP_MIN_PATCH_REVISION` or more (byte 7 of the sign-in's version field:
`srp-026g/client-patches/patch_026g.py`, `REVISION_SITE`), and answers any
other with the game's own "You're using an outdated version of the game."
Nothing else is compared: not the patches, not the ZIP. So an updater that
enables PLAY must know the exe it installed is one the server lets in
(`exe/src/gate.rs`).

**The revision gate.** `/api/client-files` says `min-revision <n>`, the
server's setting. After the overlay the updater reads the revision the
installed `survarium_skhid.exe` says (`logic/src/revision.rs`: the PE's
sections give the file offset of 0x380862, where the patched exe has
`C6 44 24 27 <revision> 90` and the original `8A 0D 9E AE 9D 00`, revision
0). A lower revision than `n` keeps PLAY off ("Error", and "The server lets
in game revision n and up; this game is revision r, and the update to it is
not offered yet (the server is being updated). ..."), before the restore and
the patch step. That is a server whose ZIP is older than its setting: a
release half put in place. An exe whose site is neither (another program, a
build whose site moved) or that cannot be read is logged and let through:
the login server still decides. A server that says no `min-revision` (srp
before it) is not checked.

**The watch.** After its last message to the launcher (PLAY on, or off with
the gate's words) the updater stays, as the launcher wants, and asks
`/api/client-files` every 10 s until the launcher closes (PLAY starts the
game and closes it):

| The server | The updater |
|---|---|
| answers as it did for this run (the same ZIP and `min-revision`) | nothing |
| offers another ZIP or another `min-revision` | starts the launcher anew (state 15, the pipe closed, the launcher's mutex waited for, `launcher_skhid.exe -product_name=<product>` started: as after a new launcher), whose updater updates. A launcher with one of the updater's boxes open closes only once it is dismissed; the new one is never started while the old one holds its mutex (a second launcher would say "already running" and end, and the first then close: none left), and after an hour of waiting the updater ends without starting one |
| answered at the start, then misses two answers in a row (no connection, a status but 503) | state 22: the launcher says "Server maintenance..." and turns PLAY (and Enter) off (`srp-026g/docs/notes/launcher.md` 5.4) |
| answers again after that | starts the launcher anew |
| did not answer at the start (PLAY with what was checked, "No update check: ...") | PLAY stays on while it does not answer; once it does, the launcher starts anew when its ZIP is not the one installed or it says a `min-revision` |
| answers 503 (busy, or reading a new ZIP) | no news |

So a launcher left open over a release turns PLAY off while the server is
down and updates once it is back, instead of starting the old exe into the
"outdated version" refusal.

**Why.** On 2026-10-21 players met that refusal after an update that
enabled PLAY. The release put the new ZIP in place only after the server's
restart and its checks (and the copy of srp.db to the jump host): in that
time the server listed the new patch and let in only the new revision while
it still offered the old ZIP. A launcher started then found the old ZIP
current, downloaded the new patch, and enabled PLAY for the old exe. The
release now puts the ZIP in place while srp26 is stopped
(`release/lib.sh`, `swap_server`); the gate makes such a server loud and the
watch repairs an open launcher by itself. `tests/test_play_gate.py` plays
the half release (with and without `min-revision`), the ZIP coming while
the launcher is open, the server going away and coming back, an offline
start, a launcher with a box open (never a second launcher beside it), the
original exe, an exe the updater does not know, and `check` replacing the
updater.

## A file that cannot be read

Each Skhid file the state says is installed is hashed at every start. One
that Windows does not let the updater read is never "up to date"
(`logic/src/file_error.rs`, `look` in `exe/src/main.rs`); the log names it
and Windows' error code (`<name>: cannot be read: <words> (error <n>)`):

| Windows says | The updater |
|---|---|
| 2, 3: not found | downloads the archive and puts the file in place (as before) |
| 225 `ERROR_VIRUS_INFECTED`, 226 `ERROR_VIRUS_DELETED`: Windows Security blocked it | stops: nothing downloaded (the write would be blocked too), PLAY off, its own box (below) |
| 32, 33: another program holds it from everyone | read again, six times 200 ms apart; still held: stops, PLAY off, its own box |
| anything else (5, access denied; a disk error) | a repair: the archive downloaded and installed again; when that fails, the install's error names the file and Windows' words ("Cannot replace x86\survarium_skhid.exe: Access is denied (error 5). ..."), PLAY off |

225 or 226 at the install's write, rename or a folder's making, and at a
patch file's write, rename, delete or read back, is the same stop.

The stop's box is the updater's own (`MessageBoxW`, Windows' Retry and
Cancel buttons), in the game's language (as "Watch replay"'s, below),
the launcher's line saying "Error" meanwhile. It names the file alone and,
for Windows Security, the way out, one line each:

```text
Windows Security blocked survarium_skhid.exe: a false positive.
1. Windows Security > Protection history > this item > Allow on device (or Restore).
2. Add this folder as an exclusion (Virus & threat protection > Manage settings > Exclusions):
E:\Survarium_SkhidSRP_0_26\game\binaries\x86
Then try again. PLAY stays off until then.
```

```text
survarium_skhid.exe is in use by another program. Close the game, then try again. PLAY stays off until then.
```

Retry runs the update again from the start; Cancel leaves PLAY off (exit
code 1). `check quiet` shows no box: the log has `STOPPED: <file> is
blocked by Windows Security` (or `held by another program`), and the exit
code is 1. A file the player may edit (`.bat`, `.sh`, `.txt`, `.url`) that
cannot be read, is blocked or is held is left alone (logged), as one the
player changed is: the game does not need it. The words do not name the
buttons: they are in Windows' language, which may not be the launcher's.

**Why.** On 2026-10-21 Windows Security did not let a player's updater
write revision 27's `survarium_skhid.exe` (error 225), then, once it was in
place, read it (225 again). The updater took a file it could not read for
one that was the same, said "up to date" and turned PLAY on, and the
launcher crashed at PLAY a minute later.

## A server not reached

When the update server, or its list of patches, cannot be reached at all
(WinHTTP's 12007 name not resolved, 12029 cannot connect, 12002 timeout,
12030 connection cut off: `logic/src/network.rs`), after the usual tries,
and the run ends in an error or in PLAY without an update check, the player
is told in the updater's own box, in the game's language, with Retry
and Cancel, as a stop above. Players in Russia meet it most: their
providers block the server, and a VPN with the game's three programs in its
split tunnelling gets them through (`VPN.txt` beside `Skhid.bat`). Three
lines: what failed, by the error, why it may have, and what to do:

```text
skhid.link didn't answer.
Your internet provider may be blocking it (common in Russia), or a firewall is blocking the game.
Allow the game in your firewall (Skhid-network-setup.cmd); in Russia, use a VPN and add launcher_skhid.exe, survarium_skhid.exe and updater_skhid.exe to it (see VPN.txt). Then try again.
```

The first line is "Couldn't find skhid.link." (12007), "skhid.link refused
the connection." (12029), "skhid.link didn't answer." (12002) or "The
connection to skhid.link was cut off." (12030). Retry runs the update again
from the start; Cancel ends the run as it came out (an error with PLAY off,
or PLAY with what is installed), and the launcher is not told the English
words again: the log has them ("No update check: skhid.link didn't answer.
Your provider (common in Russia) or a firewall may block it: see VPN.txt.
..."). `check quiet` shows no box. A patch file's download address that
cannot be reached is not this box: the list was reached, and the patch
step's error names the file.

**Why.** On 2026-10-25 players in Russia saw "skhid.link did not answer in
time. Check the internet connection and try again.", in English under a
Russian launcher, and found the way out (the three programs in their VPN's
split tunnelling) on Discord.

## Watch replay

The lobby's History offers a replay of a match the player played. The
server sends the game a link `replay <id> <token> <sha256> <size>`; the
patched game (`srp-026g/client-patches`, patch 29, revision 14) starts
`updater_skhid.exe` beside itself with that link as its arguments and does
nothing else with it. The updater (`exe/src/replay.rs`, the rules in
`logic/src/replay.rs`):

1. Takes `replay` only as that exact word, then exactly four arguments:
   8, 32 and 64 lower-case hex digits and 1 to 10 digits, the size 1 byte to
   64 MiB (an optional fifth word `quiet` is the tests' no-window mode).
   Anything else: "This replay link is not valid", nothing fetched.
2. Holds its own lock, `updater_skhid.replay.lock`: an update and a replay
   download never wait for each other; a second replay at the same time is
   told "A replay is already being downloaded".
3. Needs `survarium_skhid.exe` beside it (what PLAY starts).
4. Asks its configured site (`updater_skhid.url`, the same rules as an
   update: HTTPS, plain HTTP for this computer alone, no redirect followed,
   no cookie) for `GET /replays/<token>`, into
   `<game>\replays\srp_<id>.sur.part` (a file of exactly the link's size at
   most), reads it back from the disk, checks its size and SHA-256, and
   renames it to `srp_<id>.sur`. A file of that name with the right size and
   checksum is used again without a request. A connection that fails or
   breaks off is tried three times; a status is not (404 and 410: "expired
   or already used"; 429 and 503: busy; any other, a 3xx too: "error <n>").
5. Keeps the ten newest `srp_<8 hex>.sur` of that folder (the new one
   always among them) and deletes the others and any other id's `.part`;
   no other name there is touched.
6. Makes the check of `updater_skhid.exe ready` (read only, the one
   `play_skhid.bat` and `play_skhid.sh` make): while the last patch step did
   not end `ready` (a new overlay since, too), or the patch files are not
   now what its list says, the game is not started. The player is told that
   Skhid has not finished updating `game\resources.db.patch_*`, to start `Skhid.bat`
   (the launcher) and then ask for the replay again; the log has the
   `ready: NOT READY: ...` line and `replay: FAILED (NotReady)`, the exit
   code is 1. The replay stays in the folder, checked, for the next ask.
7. Starts the game as the launcher's PLAY does, from `game\binaries\x86`:
   `"<x86>\survarium_skhid.exe" -mt_allocator=768
   -minimum_resources_memory_size=768 -match_replay="<the file>"`, no
   `-localization=`: the game plays it in its own language (the one chosen
   in its Settings, game patch 79). The game reads its command line as ANSI
   text and a value in quotes as one value, spaces and all (read in the exe:
   `srp-026g/docs/notes/client_patch.md` section 3.27). The path is given as
   it is when the ANSI code page holds every character of it and it is at
   most 259 bytes; else its short (8.3) form when that does; else the player
   is told to move the game to a folder with Latin letters.

Every problem is a message box in the game's language (the `localization`
line of `<Documents>\survarium\user.cfg`, which the game's Settings and the
launcher's menu write, read only; English for a language the messages are
not written in, or without one) and a line `replay: FAILED (<problem>): <text>`
in `updater_skhid.log`. The log names the token by its first 6 hex digits
only. No progress is shown while the replay downloads (at most 64 MiB).

The one call of the message box in the replay's code is `notify` in
`exe/src/replay.rs`, behind `if !quiet`, and the replay returns from `run`
before any of `main.rs`'s own calls of it: with `quiet` no window is made
(`tests/test_replay.py`, `QuietSource`, reads the source to keep it so).

## Original game download and verification

`logic/src/game.rs` pins the original build-2777 archive at 2,906,544,103
bytes and SHA-256
`7435558eeb17098fc9337c4ba9470402cae4c9be974ef5d9bc544f19b122a028`.
The complete Drive download was checked against archive.org's independently
published SHA-1 `bb30b71fab155b3435917690fd2be87984546163` before pinning it.

**Its addresses** (`exe/src/game.rs`): `GET <server>/game`, srp's
`{"sha256": "<64 hex>", "size": <bytes>, "urls": [...]}` (`patches::parse_game`:
a patch list's entry with its size; 1 to 8 addresses, each one a patch may
have), taken only when its size and SHA-256 are the pinned ones. When the
server cannot be reached, offers none (an srp before `/game`: 404) or offers
another archive, the addresses built into the updater at release time are
used (`SKHID_BUILTIN_GAME`, the `/game` of the row of `release/game.tsv`,
which `release/build-zip` passes to the build; a build
without it has none, and says so). A server that answers always wins. srp's
`/game` is the release's addresses (that row) with the rows of its
`mirrors.tsv` added (see "Mirrors" below).

**The download** is that of a patch's file (`http::get_file`), with the
rules of "Mirrors" below: redirects followed, a web page refused. The
addresses are tried in their order, each once a run; any failure moves on to
the next. The bytes on disk (`game\.skhid-game.7z`) are the same file at
every address, so the next address goes on from them: `Range: bytes=<n>-`;
a `206` from byte n is appended, a `206` of another range is a failure, a
`200` (a server that gives no part) is the whole file again from byte 0. A
file of the whole size that is not the pinned one is deleted and fetched
anew; one that broke off stays for the next start, which goes on from it.
The whole file is checked by its size and SHA-256; a mismatch deletes it and
fails that address. One line of `updater_skhid.log` a try, a failure and the
success (`download::line`; `Skhid-without-updater.bat` writes the same
lines to `skhid-download.log`):

```text
skhid-game  try      2.28.121.133: address 1 of 3, http://2.28.121.133:8090/survarium-026g.7z, from byte 0
skhid-game  failed   2.28.121.133: receive timeout (nothing received for 30 s) (1048576 bytes received)
skhid-game  try      archive.org: address 2 of 3, https://archive.org/download/..., from byte 1048576
skhid-game  ok       archive.org: 2906544103 bytes, SHA-256 verified
```

The reasons (`text::reason`): `connect timeout`, `receive timeout` (no
answer within 30 s, or nothing received for 30 s: WinHTTP's 10 s name
lookup, 10 s connect, 30 s send and receive), `host not found (DNS)`,
`unreachable`, `connection reset`, `HTTP <status>`, `web page "<its
title>", not the file` (the `<title>` of its first 8 KB, any host's: Google
Drive's "Quota exceeded" page among them), `too slow: <n> KB/s over 2
minutes (under 50 KB/s)` (`download::Speed`; only while another address
remains: the last one is waited for, however slow), `cut short at byte <n> of
<size>`, `larger than the file`, `wrong SHA-256`, `more than 5 redirects`.
When every address failed the player is told each one's host and reason,
then where to send the log (`text::reported`, as the crash reporter words
it: `Send game\binaries\x86\updater_skhid.log to #bugreports in Discord
(tag: install): discord.gg/Pxa4prXfwu`; the reason is cut to leave it room
in the launcher's 255 characters, the log has the whole). So does every
other failure the run ends on.

Downloads use a 64 KiB buffer, including in the 32-bit updater. The complete
file is flushed, read back and hashed before extraction.

The extractor is the official standalone `7zr.exe` of 7-Zip 26.04, unchanged
(`extractor/`, with where it is from and its licence). It is not downloaded:
`make_client_026g.py` puts it into the Skhid archive as
`game/binaries/x86/7zr.exe`, beside the updater, and refuses another file
than the pinned one. The updater checks it against its compiled-in size and
SHA-256 before anything is downloaded, and runs no other file. The archive
and the extractor stay locked against writes and deletion until the
extractor exits. There is no verification bypass. (7-Zip's own address for
the file has no version in it: its bytes change with every 7-Zip release,
which is why it is not fetched from there.)

When the game is missing and `7zr.exe` is missing too, or is not that file
(deleted, damaged, an archive only half unpacked), the Skhid files are
installed first: the server's archive brings the extractor, and the game
follows in the same run (after the restart, when the launcher or the updater
was replaced). If the extractor is still not right after that, the updater
says so and downloads nothing.

Only five game files are extracted into a fresh staging folder: the original
exe, resources database, crash DLL, certificate and splash image. The
extracted exe must also match its known SHA-256. Only after successful
extraction and checking are missing files moved into place; existing files
are preserved, and a failed move rolls back files added by that attempt.
The normal verified Skhid overlay then replaces the original certificate
and crash DLL when needed. Existing complete games skip this bootstrap.

HTML confirmation pages, quota pages, interrupted transfers and hash
mismatches fail closed, each at its address. A completed verified archive is
kept if extraction fails and is rehashed before retrying; a part is kept and
gone on from. The archive is deleted after successful installation.

What an attempt that was cut short leaves behind (the PC switched off, the
updater killed while the extractor ran) is removed at the next start, under
the installation lock: every `.skhid-extract-*` folder in the game folder
(up to 6.3 GB each; only the five game files and their folders are deleted,
so a folder with anything else in it is left and named in the log), and,
once the game is there, a `.skhid-game.7z` that is still beside it. The
extractor runs in a Windows job that ends with the updater, so an updater
that is killed takes it along and it does not write on by itself. The
room asked for is the room still needed: an archive that is already
downloaded is not counted again.

What the extractor prints (its messages and its errors, as UTF-8) goes to
`updater_skhid.7zr.txt` beside the updater while it runs. When it has ended,
whatever its exit number, the updater copies those lines into
`updater_skhid.log`, each after `7zr:`, and deletes the file: a failed
extraction then says in the log which file could not be written and why,
not "exit 2" alone. A file left by a killed updater is copied and deleted
at the next start. When the file cannot be made, the extractor runs all the
same.

To add an address, `release/add-mirror survarium-026g.7z <url>`: srp offers
it at once (`mirrors.tsv`), no release or rebuild. Addresses change only
where the bytes come from, never the trusted hash.

`tests/test_game_download.py` runs the actual exe on Windows or Wine against
a local HTTP server, covering every address failing (each reason in the
log), a download that broke off going on at the next address (`Range`, a
`206`, a `200` from byte 0), a part of another range, redirects, a server
without `/game` or with another archive, preservation of existing files, an
extractor that is missing or changed, and what a cut-short attempt left. With `SKHID_TEST_GAME_ARCHIVE` pointing to the pinned archive, it also
downloads, verifies and extracts the full game with the extractor of
`extractor/`, then installs the Skhid overlay. It does not contact Drive or
archive.org.

## Restoring resources.db

Before the patches, the players' ZIP had a plan
(`game/binaries/x86/updater_skhid.resources`) and the updater wrote entries
of it over the game's `resources.db` in place: the settings menu's, the
lobby's and the login screen's Flash movies (`main_menu`, `lobby_menu`,
`login_menu`), the same size and the same game CRC as the stock ones. Those
updaters recorded each region before its first byte was written, in
`updater_skhid.resources.state` beside the updater (the file, its size, the
region's offset and size, the SHA-256 of its stock bytes and of what was
written), noted the one being written in `updater_skhid.resources.journal`,
and kept a copy of the stock bytes in `updater_skhid.backup\<sha256>`.

Content now comes as patches only, so a player's db must be the stock one
again. At every start, after the overlay and before the patch step, under
the installation lock (`exe/src/resources.rs`, `logic/src/resources.rs`):

1. The record is read, and the journal's region added to it (a write that
   was cut short). Lines of another word are skipped; a record that cannot
   be read is none (nothing is changed). Only regions of `game/resources.db`
   are taken.
2. Each region is hashed. Its stock bytes, a db of another size (another
   build) or no db: it leaves the record. Else:
3. The db is opened by the updater alone (share mode 0) and closed again.
   The game keeps `resources.db` open while it runs, so this fails then: the
   log says "the game is running" and nothing is changed this run.
4. The stock bytes come from the copy kept when it checks (its size and
   SHA-256), else from `<site>/resources/<sha256>` into
   `updater_skhid.cache\` (streamed to the disk, checked; two tries).
5. The region is journalled (flushed), the db opened by the updater alone,
   the bytes written at the region's offset, flushed, read back and
   checked; then the journal goes and the region leaves the record.
6. Once the record is empty, it is deleted, and so are the copies kept, the
   cache, the old plan and `updater_skhid.resources.status` (of a plan that
   grew the db; never in a players' ZIP).

Best effort: whatever fails is logged (`resources:`) and tried again at the
next start, the record kept; it never fails the update or keeps PLAY off.
The patches mount over the db, so a region not restored yet is hidden
wherever a patch has that file. A player whose record is lost keeps the
regions as they are: the updater no longer knows where they are. After the
restore the step costs one look for its files. The launcher shows a
restore as an install ("downloading" with the bar for a blob, "Extracting
file <name>" while a region is written).

## Patches

The game (client patch revision 12, patch 27 of
`srp-026g/client-patches/README.md`) mounts the retail `game/resources.db`
and then every `game/resources.db.patch_<n>` there, n = 1 to 255 in order,
a missing number skipped; a later patch's file wins. n is the patch
revision that brought the patch. The updater keeps those files the ones the
server lists (`logic/src/patches.rs`, `exe/src/patches.rs`).

**The list.** `GET <server>/patches`, from the server the updater asks for
everything else (`updater_skhid.url`, else the built-in
`SKHID_UPDATE_URL`; so HTTPS in production). srp writes it so:

```json
{"patch_12": {"sha256": "<64 hex>", "urls": ["http://203.0.113.5/patch_12",
     "https://drive.usercontent.google.com/download?id=<id>&export=download&confirm=t"]},
 "patch_17": {"sha256": "<64 hex>", "urls": ["https://files.example/db/patch_17"]}}
```

A JSON object and nothing else: every key `patch_<n>` with n from 1 to 255
and no leading zero (the game's own rule), no key twice; every value an
object of exactly two members, `sha256` (64 hex characters, either case)
and the file's addresses: `urls`, an array of 1 to 8 strings, none twice,
or `url`, one string (the form of updater 0.2.0, still read: an old list,
the list it kept beside it), never both. An address is `http://` or
`https://`, a host name or a bare IPv4 address, an optional port, and a
path; no credentials, no fragment, no space, no `"` or `\`, no other
scheme; at most 1024 bytes (`url::patch`, which srp-game-data's
`server::is_url` mirrors, both tested with
`logic/tests/data/patch_urls.txt`). No escape in any string, no other
member, no other value. At most 4 MB. `{}` is a list: no patch (every
patch file is deleted). Anything else is refused whole, with nothing
changed. 404 is "no list", not "no patch".

Updater 0.2.0 reads the `url` form only; it never meets the `urls` one. The
release that brings 0.3.0 ships its ZIP and an srp that writes `urls`
together, and an updater asks for the ZIP before the list: 0.2.0 finds a
new `updater_skhid.exe` in it, installs it, and leaves the rest of the
update to it ("the rest of the update is done by the new one": the
launcher restarts, which starts the new updater at once; by hand, `check`
runs the new updater at once). Until then `ready` refuses (a new overlay). Tested
end to end (`test_db_patches.py`, with `SKHID_TEST_OLD_UPDATER`).

**The files.** `GET <address>` of each entry, on whatever host the list
names: a plain file server is enough. Plain HTTP is taken there, to any
host, as it is not for the update server: the file is checked against the
list's SHA-256, and the list came from the update server over HTTPS.

- **Mirrors.** The addresses are tried in their order, each once a round,
  two rounds a run: srp's list gives the release's (`release/patches.tsv`,
  Google Drive's first) with the rows of srp's `mirrors.tsv` added (after
  them, or at a row's place), which `release/add-mirror` writes and srp
  reads again when it changes. Any failure moves on to the
  next address: no
  connection or a broken one, another status than 200, a web page
  (`Content-Type: text/html`: a sign-in page, Google Drive's "cannot scan
  this file for viruses" or "Quota exceeded" page; its title is in the
  log), slower than 50 KB/s over 2 minutes (while another address
  remains), a body shorter than its
  Content-Length, another SHA-256, too many redirects or one to an address
  a list may not name. Each try is a line of the log too (`... (try 1 of
  2): asking`). The second round tries again only the addresses that failed on the
  network (no connection, cut short, a 5xx): another file or a web page
  would be the same 3 s later. The log names each address and its failure
  (`patches: resources.db.patch_<n>: address 2 of 3, <address> (try 1 of
  2): <why>`); PLAY stays off only when every address failed. Not enough
  disk, a file that cannot be written here: no other address would do
  better, the run ends.
- **Redirects** are followed, up to 5 a download (the 6th is a failure),
  WinHTTP's own following being off: each `Location` (absolute, `//host`,
  `/path`, `?query` or relative) is resolved against the address it came
  from and taken only if it is one a list may name (`url::redirect`, then
  `url::patch`: http or https, no credentials, no fragment, ...). From
  HTTPS down to plain HTTP is taken, as plain HTTP is in a list: the
  SHA-256 decides. The log names each hop.
- **Google Drive.** Share the file with "anyone with the link" and list
  `https://drive.usercontent.google.com/download?id=<id>&export=download&confirm=t`
  (`<id>` of the link `https://drive.google.com/file/d/<id>/view`): that
  form gives the file itself, whatever its size, without a cookie or a
  sign-in. The `https://drive.google.com/uc?export=download&id=<id>` form
  redirects to it, and for a large file answers a "cannot scan for viruses"
  web page instead: that address fails and the next is tried. A Drive
  file has a daily download quota; past it, a "Quota exceeded" page, the
  same. `release/add-mirror` checks an address before srp offers it.
- No patch download is resumed: one that broke off is fetched again whole
  (a patch is tens of megabytes; the game's archive is gone on from).
- **The server not reached.** When the list cannot be fetched and the
  folder is not as the list kept says, the list built into the updater at
  release time is used (`SKHID_BUILTIN_PATCHES`, the `/patches` of
  `release/patches.tsv`, which `release/build-zip` passes to the build):
  a provider that blocks the server need not block the patches'
  addresses. A list of no patch is none. It only adds: a listed patch
  missing or damaged is fetched, no other patch file is deleted (the game
  exe may be newer than the updater and need it), and neither the list
  kept nor the status is written (it is not the server's list).
- The Content-Length, when the server sends one, is checked against the
  disk first, and a body shorter than it is an error.

**The quick check.** A patch file is not read at every start. After it is
downloaded or hashed whole, `updater_skhid.patches.checked` beside the
updater records its SHA-256 with what Windows says of it
(`GetFileInformationByHandle`: its size, last-write time, and file ID: the
volume's serial number and the 64-bit file index), read before and after
the hash (a file written to meanwhile is not recorded), and when it was
hashed (`logic/src/checked.rs`):

```text
skhid-patches-checked 1
file resources.db.patch_12 <sha256> <size> <last write> <volume> <index> <hashed at>
end
```

At a later start a listed patch whose file has the same size, time and ID,
whose recorded SHA-256 is the list's, and which was hashed less than 7 days
ago (and not "after" now: a clock set back), counts as checked without a
read. Anything else is hashed: another size, time or ID (written to,
copied back, replaced, restored by hand), another SHA-256 in the list for
it, no record or one that cannot be read (wholly: a record without its
`end` line is none), a hash a week old, a file system that gives no file
index. A file is hashed once a run at most: a download is hashed once (read
back) and known in its place by its file ID after the rename, which keeps
it. The launcher's run, a start without the server and `ready` take the
quick check (`ready` does not write the record); `updater_skhid.exe check`
hashes every file (the repair). The record holds no more than the list's
patches; a delete or a damaged file drops its entry.

What the quick check does not see, until the week is over or `check`:
bytes changed with the size and the time kept. A program that writes
leaves another time; but a copy put over the file that keeps the source's
time (Explorer's "Replace", `cp -p`, a backup restored) can keep the
file's ID too (the file is overwritten in place, not made anew), and then
another file of the same size and the same time is taken unread. On Linux (Wine) the ID is the
inode, which ext4 gives again to a file made after a delete: there a
delete, a new file of the same size and the old time set on it (`touch
-r`) is not seen either. Neither happens by chance with a patch's bytes;
`check` is the repair.

**A run**, after the restore (also when nothing new was installed; under
the installation lock):

1. The list is fetched (three tries, 3 s apart; the launcher's stop is
   heard meanwhile) and read.
2. Every listed patch's file is checked once: by the quick check's record,
   else hashed (`check`: every one hashed). The launcher shows "checking"
   and its bar over the bytes hashed.
3. When anything is to change (a file to delete or to download, or a list
   other than the one kept): `updater_skhid.patches.status` says
   `blocked checking` and the list is kept beside the updater as
   `updater_skhid.patches` (one line, the same form), before the folder is
   touched.
4. Every `game\resources.db.patch_*` that is not a listed patch is deleted:
   one the list no longer has (a rollback), a name the game does not take
   (`_014`, `_0`, `_300`), a download left over (`.skhid-part`).
5. A damaged listed patch is deleted. A missing or damaged one is
   downloaded from its addresses in turn (above) into
   `resources.db.patch_<n>.skhid-part` beside its place (the disk must have
   its Content-Length plus 64 MB free, else nothing is fetched), flushed,
   read back and hashed, and renamed to its name only when it has the
   list's SHA-256. When every address failed, the run ends. A
   `.skhid-part` that failed, or was stopped, is deleted.
6. The folder is checked against the list as `ready` checks it: no other
   patch file, the downloaded files as they were hashed (by their file ID
   in place; hashed again there if the rename did not keep it), the
   others as step 2 found them (nothing touched them since). Then the
   status says `ready`, and the record of the hashes is written.

Before each delete and each rename, `resources.db` is opened by the updater
alone and closed again: the game keeps the db open while it runs (not its
patches), so that is how a running game is told. A file that another
program holds a moment (an antivirus scanning the new file) is tried again,
six times 200 ms apart, for a delete, a create and a rename; so is the db's
open. "The game is running" is said only when the db itself stays held.

The status is written only by a run that got a list it can use: a run that
cannot fetch the list, gets one it cannot read, or is stopped before the
folder is to change leaves it as it was; a run stopped after says `blocked
stopped`. A new overlay (any file installed) makes it `blocked overlay
changed` before anything else: the new files may need other patches, so
only a patch step with the server's list makes it `ready` again. When the
updater or the launcher was replaced, the patch step is the new updater's,
which the launcher starts at once.

When the update server could not be asked this run (the overlay's offline
case), or its list could not be fetched (the network, a status but 404),
nothing is fetched: the game may be played when the last run ended
`ready` and the folder is still what the kept list says (every file
checked, by the quick check); else PLAY stays off. A first start with the server down therefore
keeps PLAY off, and so does a start without the server after a new overlay.

What keeps PLAY off ("Error" in the launcher's line, the reason in its box,
`check quiet` exits 1, the log has `patches: BLOCKED: <why>. The player is
told: <words>`, and `updater_skhid.patches.status` says `blocked <why>` when
the folder was to change), each told with "PLAY stays off until this is
done." after it:

| Found | The player is told |
|---|---|
| The list cannot be fetched (network, a status but 404), and the folder was not checked in place before | "The list of the game's patches could not be fetched. <the network error>" |
| `/patches` answers 404 | "<host> offers no list of the game's patches. Tell the server's operator." |
| The list is not one (above; a Wi-Fi sign-in page too) | "The list of the game's patches from <host> cannot be used (<what>). Tell the server's operator." |
| A file of one address cannot be downloaded (network, a status, a web page, too many redirects, cut short) | "game\resources.db.patch_<n> could not be downloaded. <the network error>" |
| A file of one address has another SHA-256 | "<host> sent game\resources.db.patch_<n> with another checksum than its list says. Tell the server's operator." |
| None of a patch's addresses (two or more) gave its file | "game\resources.db.patch_<n> could not be downloaded from any of its <k> addresses (each is in updater_skhid.log). The first: <its failure>" |
| Not enough disk | "Not enough free disk space to update the game's files in <folder>: N MB needed, M MB free. ..." (N: the file's size and 64 MB) |
| resources.db is held (the game runs) and a patch file is to change | "The game is running, so Skhid could not update the game's patches ... Close the game, then start the launcher again." |
| A patch file stays held by another program | "Another program holds game\resources.db.patch_<n>, so Skhid could not update the game's patches. An antivirus may be scanning it: wait a minute, then start the launcher again." |
| The server could not be asked and the patches were not checked in place | "The game's patches could not be checked: <why the server could not be asked>" |
| Anything else (a file that cannot be written or deleted, the ZIP bringing a `resources.db.patch_*` anywhere) | "Skhid could not finish updating the game's patches ...; the reason is in game\binaries\x86\updater_skhid.log. ..." |

The next start tries again. A file that is never half written in place:
what the game mounts is a checked file or nothing.

`ready` (play_skhid.bat, play_skhid.sh) checks the same as the offline case
(the status `ready`, the folder against the kept list, every file hashed)
and refuses otherwise: also before the launcher has ever run the step, and
after a new overlay until the step has run with the server's list.

## Build and test

Use the dev shell of `srp-026g/flake.nix` (`nix develop ./srp-026g`): the
nightly Rust of the game workspaces with `rust-src`, `llvm-rc` and Microsoft's
C runtime and import libraries for 32-bit x86 (`SKHID_MSVC_LIB`, as
`../bugtrap/README.md` "Build" says). No Windows or Visual Studio installation
is needed. The two locked dependencies of `exe/` are Microsoft's `windows-sys`
and `windows-link`.

```sh
./build.sh                                   # exe/target/i686-win7-windows-msvc/release/updater_skhid.exe
SKHID_UPDATE_URL=https://test.example ./build.sh   # another default server
SKHID_REVISION=27 ./build.sh                 # as release/build-zip builds revision 27
(cd logic && cargo test)
python tests/test_updater.py                 # Windows
python tests/test_db_patches.py              # Windows or Wine (resources.db's patches, the restore)
python tests/test_play_gate.py               # Windows or Wine (PLAY and the server's revision, the watch)
python tests/test_replay.py                  # Windows; the replay, in its no-window mode only
python tests/test_game_download.py           # Windows or Wine
python tests/test_unreadable.py              # Windows or Wine (a Skhid file that cannot be read)
SKHID_TEST_BUNDLE=<a players' zip> python tests/test_updater.py
```

The exe's resources (`exe/res/updater_skhid.rc`, `exe/build.rs`,
`../windows_build.rs`): the icon `exe/res/skhid.ico`, the game's own (the
original Survarium launcher's and `survarium.exe`'s, its 32-bit images of 16
to 256 pixels, PNG-compressed: 45 KB); the version information, CompanyName
and ProductName `Skhid`, FileDescription `Skhid updater`, FileVersion
`<revision>.0.0.0`, ProductVersion `revision <revision>` (`SKHID_REVISION`,
the release's patch revision; unset, a local build: `0.0.0.0`, `local
build`), LegalCopyright, InternalName `updater_skhid`, OriginalFilename
`updater_skhid.exe`; and the manifest `rust-lld` writes (`asInvoker`). As the
revision is in the exe, every release ships a new `updater_skhid.exe`, and
players' updaters replace themselves once at their next launcher start.

The build is reproducible: the same source, toolchain, `SKHID_UPDATE_URL` and
`SKHID_REVISION` give the same exe, byte for byte, in any folder:
- `exe/.cargo/config.toml` gives `rust-lld` `/Brepro` (the PE stamp is a hash
  of the content, not the time) and `/DEBUG:NONE` (no PDB, which would record
  the build folder), and Cargo `trim-paths` (no build, home or toolchain
  folder in the panic locations), as `../bugtrap`;
- `exe` and `logic` are one Cargo workspace (`Cargo.toml` here, one
  `Cargo.lock`). Cargo names a path dependency outside the workspace by its
  absolute path, and that name reaches the exe's symbols and their order. The
  exe still builds to `exe/target/` (`target-dir` in `exe/.cargo/config.toml`);
  `cargo test` in `logic` builds to `target/` here.

`../check-reproducible.sh` builds the updater and BugTrap twice, in two
folders, and compares.

`make_client_026g.py` puts the built exe into the players' ZIP and writes
`updater_skhid.url` for the server the ZIP is made for.

Run on 2026-10-13 (Windows 11): the 28 unit tests and the 18 end-to-end
scenarios pass; the real launcher drove it through an install, a repair, a
replacement of the launcher itself and a start with the server down. Not
run under Wine yet.

Run on 2026-10-14 (Windows 11), with the resource step: the 39 unit tests
and the 30 end-to-end scenarios pass (one skipped without
`SKHID_TEST_BUNDLE`, as before), 12 of them the resource step's (a
4 MB stand-in for `resources.db`, a plan made by `make_client_026g.py`'s
own functions, and one region of the lobby movie's size). Not run yet: the
real launcher, a real `resources.db`, Wine.

Run on 2026-10-15 (Linux, Wine 11.8 staging, `check quiet`), with tails
(since removed with the in-place plans): the 44 unit tests, and the 15 scenarios of `test_resource_tail.py` (a 4 MB
stand-in db grown by 1 MB: a fresh install, found done after, a db grown
already without a record, the plan dropped, a new release replacing the
tail and the table, a tail or a table or the lobby the site lacks or spoils,
a db of another size, an append cut short, a table without its tail, plans
that cannot be read, a plan without a tail staying best effort; `ready`, and
the players' `play_skhid.bat` run by Wine's cmd with a stand-in game,
refusing to start it until the db is in place).
`test_updater.py`'s two changed scenarios (a newer format; a running game
with a required plan) are not run yet: they need Windows.

Run on 2026-10-16 (Linux, Wine 11.8 staging, `check quiet`), with the
patch step: the 64 unit tests; the 16 scenarios of
`test_db_patches.py` (a fresh install of patches 1, 14, 17 and 255 and the
next start downloading nothing; an empty list; the files on a second
server; one missing; one damaged here; patch files the list has not,
names the game does not take and a download left over deleted, and a
rollback; a server file that is wrong, twice; a damaged file the server
spoils too; nine lists that cannot be read, changing nothing; no
`/patches`; a list that cannot be fetched; a file server that cannot be
reached; the server down before and after a checked run; not enough disk,
by a Content-Length past the free space; the ZIP bringing a patch file;
`ready` and play_skhid.bat run by Wine's cmd refusing until the patches are
checked, and again with a patch file added); `test_resource_tail.py` (15)
and `test_game_download.py` (16, four skipped as before) again, with their
servers now listing no patch. `test_updater.py`'s two changed scenarios (a
first start with the server down, a server without client files: PLAY
stays off now) and a game that holds a patch file are not run: they need
Windows. play_skhid.sh is not run with Wine (it calls the same `ready`).

Run on 2026-10-16 (Linux, Wine 11.8 staging), after the review of the
patch step: the 55 unit tests; the 32 scenarios of `test_db_patches.py`
(26 of the patch step: those of the run before, and a new overlay that is
not played with the patches checked for the old one, also without the
server; a moment's 500 of `/patches` after a checked run, which keeps it
`ready`, and a Wi-Fi sign-in page in the place of the list, which does not
touch the status; a redirect not followed; a file cut short; the launcher's
stop in a download and in the pause before the list is asked again (the
stand-in on the pipe); "checking" with its bar; each file hashed once a
run; a running game that holds resources.db; an antivirus that holds the
new file a moment before its rename, and a patch file another program keeps
holding; a patch file anywhere in the ZIP. Six of the restore: from the
copies kept, from the server with one blob spoilt, a running game, a write
cut short, the server down, a db of another build);
`test_game_download.py` (16, four skipped as before); and
`srp-026g/client-patches`' `test_release_overlay.py` and
`test_linux_launcher.py` (40, one skipped). `test_updater.py` is not run:
it needs Windows; its tests of the in-place plans are gone with them, and
one checks the restart after a new updater with a patch listed.
Run on 2026-10-16 (Windows 11), "Watch replay" on top of the patch step
(`main` decacfa): the 63 unit tests of `logic/` and the 14 tests of
`tests/test_replay.py` pass (every updater started with `quiet`; the game a
stand-in), among them a replay refused while the patch step did not end
`ready` (a blocked status, none at all, a list that cannot be read) and
while a listed patch is missing, damaged or one more is there, and played
once the folder is the list. `tests/test_updater.py`, `tests/test_db_patches.py`
and `tests/test_game_download.py` were not run on Windows (they start the
updater, or `play_skhid.bat` its `ready`, without `quiet`, which can show a
box); the update's code is unchanged but for the command line being read
once more for the replay's word.

Run on 2026-10-16 (Linux, Wine 11.8 staging), 0.3.0 (mirrors, redirects,
the quick check): the 66 unit tests; the 48 scenarios of
`test_db_patches.py` (42 of the patch step: those before, with a wrong
file now tried once and a redirect now followed; the first address down,
a web page, a file of another checksum, each moving on to the next
address; every address failing, the network one tried twice and the others
once; five redirects followed across two servers and six refused, a
redirect to an address a list may not name; the quick check: a launcher's
run and `ready` reading no patch file, bytes changed in place with the size
and time kept going unseen until `check`, a touched file and a copy put
back hashed again, a hash 6 days old standing and one 8 days old or in the
future done again, a record lost or spoilt, a new checksum in the list, a
download known in place by its file ID, the server down; a list of the
earlier form, and the one 0.2.0 kept read offline and by `ready`; and,
with `SKHID_TEST_OLD_UPDATER`, 0.2.0 (built from main before this change)
installing 0.3.0 without asking for `/patches`, then 0.3.0, as the launcher
starts it, doing the patches of a `urls` list through a redirect. Six of
the restore, as before); `test_game_download.py` (16, four skipped as
before); `srp-026g/client-patches`' `test_release_overlay.py` and
`test_linux_launcher.py` (40, one skipped). `test_updater.py` is not run:
it needs Windows. The quick check's file ID is Wine's (the file's inode);
on Windows it is NTFS's.

The build of that commit with the replay watcher of #134 (`SKHID_UPDATE_URL`
unset), by `../check-reproducible.sh` the same in two folders:
`updater_skhid.exe` 204,288 bytes, SHA-256
`13c0095632d9d305afc0bed7b7972043ae57ed68e7320654a7da84111d5fed4c`;
`BugTrap.dll` unchanged,
`e8e190edd04b600bf146925e21597dc3b06ca5778aef61991634974b781acc6d`.

Run on 2026-10-21 (Linux, Wine, headless), the build with the standard
library, the C runtime, the icon and the version resource: the 74 unit tests;
`test_db_patches.py` (48, one skipped: `SKHID_TEST_OLD_UPDATER`) and
`test_game_download.py` (16, four skipped as before); `srp-026g/client-patches`'
`test_release_overlay.py` and `test_linux_launcher.py` (41, one skipped); and, by hand, the
updater of `main` before this change (the `no_std` one players have) taking a
ZIP with this one: it replaced itself, and this one, started as the launcher
starts it, did the patches and said `up to date`. `test_updater.py` and
`test_replay.py` are not run: they need Windows. `../check-reproducible.sh`:
the same in two folders, a local build (`SKHID_REVISION` unset)
`updater_skhid.exe` 384,000 bytes, SHA-256
`c9069bafff22ca5ac68b3e930a47bfbbfcd1afe406d728c484d8b608d0f9549f`,
`BugTrap.dll` 214,528 bytes,
`926290f043e71b9b624baa96cd6ddfbb83b4a770cea4b4f1569f34b804effea7`; with
`SKHID_REVISION=27`, `04821712070e2989a9cf6d854020515e45afd8843aa892acc3b10a40d0b65226`
and `5296c55fa24fd7abdd806fb449d1e703d7b0eed23351be8e4f3d1af0786758a6`.

Run on 2026-10-21 (Linux, Wine, the stand-in launcher), with the revision
gate and the watch: the 79 unit tests (the revision site read from a made
PE and, with `SKHID_TEST_GAME_EXE`, from released exes of revisions 26 and
27; `min-revision` read); the 9 scenarios of `test_play_gate.py` (one with
a stand-in launcher that holds its mutex and a modal box); the 48 of
`test_db_patches.py` (one skipped as before; the one of a new overlay now
has `check` run the new updater at once) and the 16 of
`test_game_download.py` (four skipped as before). `test_updater.py` is not
run: it needs Windows. Not seen: "Server maintenance..." in the real
launcher's window.

Run on 2026-10-23 (Linux, Wine, headless), with "A file that cannot be
read": the unit tests of `logic/` (82; the error codes sorted, what the
overlay does with each, the stop's words in the five languages);
`test_unreadable.py` (2: a game exe another program holds from everyone
stops the update with PLAY off and nothing downloaded, and is up to date
once let go; a game exe Wine will not read (access denied) is downloaded
again for a repair, and as Wine will not rename it either, the failure
names it and PLAY stays off); `test_db_patches.py` (48, one skipped as
before), `test_play_gate.py` (9) and `test_game_download.py` (16, four
skipped as before). Not run on Windows: Windows Security's errors 225 and
226 cannot be made by a test, and the box was not seen.

## Not done

- A signature on the ZIP made with a key that is not on the server. Today
  the ZIP is trusted because the HTTPS connection to the server is.
- Resuming a broken download (the ZIP is a few megabytes; it is fetched
  again; a patch, from its next address, else at the next start, whole: by
  the owner's decision).
- Texts in other languages than English (the launcher shows them in an
  ANSI message box), but the stop's box of "A file that cannot be read",
  which is the updater's own.

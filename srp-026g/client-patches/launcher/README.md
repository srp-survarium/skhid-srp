# The Skhid launcher: the 0.26g launcher made to work again (build 2777)

The game's own launcher (`survarium_launcher.exe`), patched so that it

- starts from an unpacked game folder, with no installer, no registry
  values, no service and no administrator rights;
- asks **one server only** (the Skhid one, or the one it is built for): no
  request to any of the game's old sites can leave it;
- shows that server's own page in its window (the best players and the
  matches being played, as two tabs, what the server counts, the legal
  notice);
- starts a **new updater** (`updater_skhid.exe`, `../../../client/updater`)
  that keeps the Skhid files of the game current from the same ZIP the
  server's page offers, the launcher and the updater themselves included,
  and the game's content patches (`resources.db.patch_<n>`) the server
  lists, and enables PLAY only for a game the server lets in;
- starts the game; its language is one setting, the `localization` line of
  the game's user.cfg, which this menu (every language of the game) and the
  game's Settings both write (patch 14).

The original launcher is not in this repository. The tools below work on
your own copy, and the patched exe is made from it.

| | |
|---|---|
| Original | `game/binaries/x86/survarium_launcher.exe`, 4 740 728 bytes, sha256 `710956d2347864effb353064dd7beaee1dd2773c74756d3f6677df71431c5266` (the tools check it and refuse anything else) |
| Result for `https://skhid.link` | `launcher_skhid.exe`, 4 735 488 bytes; the hash depends on the site, the game exe's name and its arguments |
| Status, 2026-10-13 | Built and run on Windows 11 against a local srp server, in a test game folder (see "What was run"). **Not yet** run under Wine or on another Windows version. In the players' download since 2026-10-13 09:50 UTC, and run against the test server and the public one that day (`../README.md`, the rows of the downloads) |

## The files

| File | What |
|---|---|
| `patch_launcher_026g.py` | Builds the patched launcher from the original. Python 3, nothing else |
| `launcher_values_026g.asm` | The new code (304 bytes and three text slots): the values the launcher used to read from the installer's registry key, worked out from where the launcher is; and Enter pressing PLAY. Assembled by every build (`patch_launcher_026g.py`'s `code()`) |
| `launcher_pages.py` | The ten small pages the launcher shows from its own folder while the server's page loads and when it cannot be loaded |
| `check_*_026g.py` | Run the patched code under Unicorn: the registry reads (patch 4), the language (14), the install status (13), the "Allow torrent" box (15), the "Relay" row (16), the file count of a download (17) |
| `selftest_launcher_026g.py` | Builds it in memory and checks it: every changed byte is one named below, the PE is well formed, the hook and its way back, the addresses, the relocations, no old address left, the `.asm` gives the `.bin` |
| `../make_client_026g.py` | Puts all of it into the players' ZIP (the default; `--no-launcher` leaves it out) |
| `../../../client/updater/` | The updater: Rust source, its tests |
| `../../servers/web-server/src/launcher.rs` | The server's side: the page and "which archive do you offer" |

## How to build

```
python patch_launcher_026g.py <game>/binaries/x86/survarium_launcher.exe <out>/launcher_skhid.exe [--site=https://skhid.link]
python selftest_launcher_026g.py <game>/binaries/x86/survarium_launcher.exe
python check_language_026g.py <game>/binaries/x86/survarium_launcher.exe   # the language code, under Unicorn
python check_registry_026g.py <game>/binaries/x86/survarium_launcher.exe   # no registry call where the key was read, under Unicorn
python check_torrent_box_026g.py <game>/binaries/x86/survarium_launcher.exe # the "Allow torrent" box (patch 15), under Unicorn
python check_relay_box_026g.py <game>/binaries/x86/survarium_launcher.exe   # the "Relay" row (patch 16), under Unicorn
python check_file_count_026g.py <game>/binaries/x86/survarium_launcher.exe  # "(5/69)" of a download (patch 17), under Unicorn
```

Each of them assembles `launcher_values_026g.asm` (the arena's NASM 2.16;
`patch_launcher_026g.py`'s `code()`), with the game's languages, so no .bin
is committed.

Normally `make_client_026g.py` does it, with the updater and the pages:

```
../../../client/bugtrap/build.sh
../../../client/updater/build.sh
python ../make_client_026g.py <game folder> <new.zip> --pack-zip --server=<host>:<port> [--site=https://<host>]
```

In the ZIP they are, all in `game/binaries/x86/`: `launcher_skhid.exe`,
`updater_skhid.exe`, `updater_skhid.url` (the site, one line) and the folder
`launcher_skhid/` with `index-<code>.html` and `index-blank-<code>.html` for
`en`, `ru`, `ua`, `de`, `pl`. At the top of the ZIP, beside `game/`, is
`Skhid.bat`: the one file a player starts. It starts `launcher_skhid.exe`
in its folder and does nothing else; if the launcher is not where it looks
(the ZIP was unpacked somewhere else), it says where the files belong.
Beside it is `Skhid.sh`, the same for Linux: `bash Skhid.sh` runs
`play_skhid.sh` (the game), `bash Skhid.sh launcher` the launcher under
Wine.

The same input gives the same ZIP byte for byte. That matters: the updater
installs whenever the ZIP's checksum changes.

## What a player sees

1. Double-click `Skhid.bat` in the game's folder (or `launcher_skhid.exe`
   itself in `game\binaries\x86`). The launcher's window opens as it
   always looked, with the server's page in it. The line at the bottom says
   "Checking game version...".
2. `updater_skhid.exe` asks the server which ZIP it offers now. If it is
   the one installed, and no file of it is missing or damaged, nothing is
   downloaded. If not: the bar fills while the ZIP is downloaded ("...
   left", speed), then "Extracting file ..." for each file. If the launcher
   or the updater itself was among the files, the window closes and opens
   again by itself, and the new updater goes on from here.
3. Then the game's patches: the bar shows "checking" while a patch file
   that changed since it was last checked is hashed, and a patch the
   server lists that is missing or damaged is downloaded, the line saying
   which of how many after what is left of it, "120 MB left (5/69)": 64
   more to go (patch 17) ("Extracting file resources.db.patch_<n>" once it
   is in place). The first start after new
   content downloads it; other starts take a second.
4. "The latest version of the game is installed" and PLAY: only when the
   game's files are the server's ZIP, its exe is a revision the server lets
   in, and every patch the server lists is in place. Otherwise "Error" and a
   box that says why; PLAY stays off (the next start tries again).
5. While the window stays open the updater asks the server every 10
   seconds. Is the server down (a release, a restart), the line says
   "Server maintenance..." and PLAY is off. Is it back, or does it offer a
   new ZIP, the window closes and opens again by itself and updates. So
   PLAY never starts a game the server would refuse as outdated.
6. PLAY (or Enter) starts `survarium_skhid.exe
   -minimum_resources_memory_size=768` from `game\binaries\x86`, and the
   launcher closes. Since game patch 79 (`../../docs/notes/language-option.md`)
   it passes no `-localization=`: the game's language is the one chosen in
   its Settings (user.cfg).
7. The language button (EN, UA, RU, DE, PL, SR, CS, HU, FI, BE, HR, PT) opens a
   menu of every language of the game, each by its own name, the current
   one checked. A pick is the game's language: it is written to the game's
   user.cfg at once, as the game's Settings write it, and the launcher's
   texts and page change with it (English for a language the launcher has
   none in: SR, CS, HU, FI, BE, HR, PT). At its start the launcher shows user.cfg's
   language, else the one the game will start in (patch 14).

"Watch replay" in the lobby's History (game patch revision 14) does not go
through the launcher: the game starts `updater_skhid.exe replay ...`, which
fetches the replay and starts a second `survarium_skhid.exe` with PLAY's
arguments and `-match_replay=<file>` (the game's own language)
(`../../../client/updater/README.md`, "Watch replay").

Gone from the window: the four social buttons, CREATE ACCOUNT (an account is
made by the first sign-in), SHOP (silver and gold are exchanged in the
lobby's exchange window since game update 11, and items bought in the
lobby's shop). PROJECT WEBSITE, in SHOP's old place at the right, opens
`<site>/?lang=<code>` in the player's browser. The settings gear (its
"Allow torrent" and port) is gone too: "Allow torrent" is a box at the left
of the bottom panel, where the social buttons were, kept in user.cfg as the
language is (patch 15); the port is the updater's own
(`../../docs/notes/torrent-downloads.md`). Under it, in a download with a
relay, "Relay: auto / on / off" (patch 16).

The launchers built before SHOP went open `<site>/exchange?lang=<code>`
with it. That address stays: a short page there says, in the launcher's
language, that the exchange is in the game now
(`../../servers/web-server/src/v026g.rs`, `in_the_game_now`). A launcher
that is updated loses the button.

## Matching the original design

The merged PR #23 kept the native window resources but approximated the
embedded page. It replaced `back.jpg` with a flat background, added borders,
used 24-pixel opaque captions instead of the original 32-pixel shadow strips,
and placed the second and third tiles one pixel too low.

The online, loading, and offline pages now share
`../../servers/web-server/launcher-skin/style.css` and the original, unmodified
`back.jpg` and `shadow.png`. Their provenance and SHA-256 hashes are beside
them in `launcher-skin/README.txt`. The shared assets live under `servers/`
so they are included by the existing Docker build too. Both renderers embed
the images; the ZIP still contains ten self-contained HTML pages and needs
no new image files or HTTP routes.

The reference is the game's `game/launcher/index-en.html` and
`public/style.css`, not a recreated screenshot. Measured at 100% scale:

| Element | x, y | width × height |
|---|---|---|
| Browser page | 0, 0 | 899 × 335 |
| Left panel | 15, 10 | 456 × 314 |
| First tile | 479, 11 | 404 × 98 |
| Second tile | 479, 118 | 404 × 98 |
| Third tile | 479, 226 | 404 × 98 |
| Tile captions | 479, 77 / 184 / 292 | 404 × 32 |

Skhid's rankings, counts, maintenance, and legal text replace the old news.
A Discord link at the right of the legal caption opens the project's existing
invite (`https://discord.gg/QUmJg3gBhg`, also used by BugTrap) in the player's
browser. It is available on the online, loading, and offline pages. The link
markup is shared in `launcher-skin/discord-link.html`; no Discord content is
loaded into the launcher.
The removed obsolete controls described above remain removed. The native
window, progress bar, and button artwork remain the game's own; the binary
self-test checks the entire 4,468,736-byte resource section unchanged.

The fixed-layout rankings table truncates long player/clan names with an
ellipsis, keeping level/rating columns within the panel. Caption strips use
the original white, bold, uppercase Arial/Tahoma text, padding, and image.
Amber remains reserved for the Skhid headings/numbers and maintenance text.

The left panel of the online page has two tabs: the rankings and the
matches being played. Their labels share the panel's heading; the one that
shows is amber, the other the grey of the column headings (white under the
pointer). The matches' table is the rankings table's: the same row and
heading heights, a white name with the mode in grey behind it (cut with an
ellipsis, whole in the tooltip), and the leader's score in the rating's
amber. The rules are in the shared `style.css`, so the ten local pages
carry them too (they have no tabs, and look as before).

Verification, 2026-10-13: all 24 rendered page states matched the original
panel/caption rectangles and skin pixels in Firefox at 899 × 335. For the
pixel comparison only, news and Skhid content are removed and all three
captions receive the same text; the original artwork, CSS geometry, and
caption typography remain. The actual pages are also captured and checked
for content overlapping captions or escaping the viewport. This covers all
ten local pages plus English/Ukrainian online pages with ten long names,
an empty board, upcoming maintenance, and downtime with the test badge,
and the matches' tab with three matches, with twelve (every map's name,
the widest numbers) and with none.
It is a browser comparison, not a new Windows/MSHTML runtime test.

The tabs were run in that engine apart from it, the same day: the
exported English page in a Windows `WebBrowser` control (Trident/7.0, off
screen, 899 × 335) behind a stand-in HTTP server sending the page's own
`Content-Security-Policy` and another `/launcher/places/en` answer each
time. A click on "Matches now" showed the matches and hid the rankings;
two refreshes changed the lines and the maintenance caption and left the
tab as chosen; a click on "Top 10 players" went back, and the next refresh
left that too. The heading stayed 30 px and the table 456 px wide. Not
done: a click in the launcher's own window.

The page uses CSS supported by the original embedded browser. Both data URLs
are below the [IE8 limit of 32,768 characters](https://learn.microsoft.com/en-us/previous-versions/cc848897(v=vs.85));
the Rust regression test decodes them and checks their bytes and length.

To repeat the comparison (Python 3, Node 22+, Firefox, Rust):

```sh
# From srp-026g:
LAUNCHER_VISUAL_DIR=/tmp/launcher-visual cargo test -p web-server export_visual_fixtures -- --ignored
python3 client-patches/launcher/launcher_pages.py /tmp/launcher-visual
# Run Firefox separately with a fresh profile:
mkdir -p /tmp/launcher-firefox
firefox --headless --no-remote --profile /tmp/launcher-firefox --remote-debugging-port 9448
# In another terminal, from srp-026g:
node client-patches/launcher/check_visual.mjs /path/to/game/launcher /tmp/launcher-visual
```

The comparison writes `comparison.json`, the original and normalized skin
screenshots, and screenshots of all twenty-four real pages into the fixture
folder. The optional final argument selects a different Firefox debug port.

Captured comparison (899 × 335 Firefox page renders, not native window captures):
[original game](../../docs/images/launcher-look/original.png),
[merged PR #23](../../docs/images/launcher-look/merged.png),
[patched page with Discord](../../docs/images/launcher-look/patched.png),
[Ukrainian maintenance](../../docs/images/launcher-look/maintenance-uk.png),
[Ukrainian offline](../../docs/images/launcher-look/offline-uk.png), and
[measurements](../../docs/images/launcher-look/comparison.json).
The merged/patched screenshots use the same deliberately long test names;
they do not show live player data.

## How an update works

The server (`servers/web-server`, the 0.26g page, in srp's own process:
while srp is stopped none of it answers):

- `GET /api/client-files` answers with the SHA-256 and size of the file it
  hands out at `/download` (`SRP_DOWNLOAD_FILE`), and the least game
  revision the login server lets in (`SRP_MIN_PATCH_REVISION`; 0: none):

  ```
  skhid-client-files 1
  sha256 <64 hex digits>
  size <bytes>
  min-revision <0-255>
  ```

  It reads the file again when its size or time changes; the revision is
  srp's setting, read at its start.
- `GET /patches` is the list of the game's content patches
  (`resources.db.patch_<n>`, each with its SHA-256 and the addresses of its
  file on the patches server): those the server's game data was made of,
  read at srp's start.
- `GET /resources/<sha256>` (64 lower-case hex digits, no other name) hands
  out the file of that name in the folder `resources` beside the download
  file (`./download/resources/` of the compose file): the stock bytes of
  the `resources.db` regions earlier updaters rewrote in place, for the
  updater's one-time restore. Whole or from a `Range`, `HEAD` too, five at
  a time in slots of their own (they never take `/download`'s), "not found"
  for a name that is not there.
- `GET /launcher/v2/<product>/?lang=<en|ru|ua|de|pl>` is the page.
- `GET /launcher/places/<uk|en>` is what changes on that page (the board,
  the matches, the numbers, the maintenance line), each part behind the
  name of its place. The page's one script asks for it every 10 seconds
  and puts in place what changed, so the numbers move while the window is
  open, the page is never loaded again and the tab that was chosen stays.
  (`GET /launcher/fresh/<uk|en>`, the address of the page from before the
  tabs, still answers that page's three parts: a launcher can stay open
  over a deploy.) Of the page before the tabs, seen in the
  real launcher on Windows 11 (its browser calls itself Trident/7.0)
  against a local server, 2026-10-13: a request every 10 seconds, a planned
  and a cancelled maintenance shown within that time, and the server
  stopped and started again with the page staying as it was meanwhile.

A release changes the ZIP, the patch list and the revision together: the
ZIP is put in place while srp is stopped, and srp's start takes the new
game data and setting (`release/release.sh`; `release/lib.sh`,
`swap_server`). An updater never sees the new list and revision beside the
old ZIP.

The updater (`client/updater`), at every start, under a lock of the game
folder (`updater_skhid.lock`):

On a new installation, unpack the Skhid ZIP into an empty folder and start
`Skhid.bat`. After contacting the update server, the updater downloads the
original 0.26g game from the Google Drive mirror if `survarium.exe` or
`resources.db` is missing. It unpacks it with `7zr.exe`, 7-Zip's standalone
extractor, which is a file of the Skhid ZIP beside the updater and is not
downloaded. The archive and the extractor both have
compiled-in SHA-256 pins; neither is used until verified. Extraction happens
in a fresh staging folder, and the extracted original exe is checked against
its known SHA-256 too. The updater adds missing original files and then runs
the overlay update below. Allow about 10 GB free. Mirror configuration and
verification details: [updater README](../../../client/updater/README.md#original-game-download-and-verification).

The overlay (the Skhid files of the ZIP):

1. Reads `updater_skhid.url` beside it (else the address it was built with)
   and asks `/api/client-files`, three times at most. HTTPS only, through
   Windows' own WinHTTP; plain HTTP is taken for `127.0.0.1` and
   `localhost` alone. No answer: the offline case below.
2. Compares the answer with `updater_skhid.state` beside it: the checksum of
   the ZIP it installed last, and the size and checksum of every file of it.
   An install is due when the ZIP is another one, when a file is missing, or
   when a program file (exe, dll, certificate) has other bytes. A text file
   a player may have edited (`.bat`, `.sh`, `.txt`, `.url`) is no reason.
3. Downloads `/download` into memory, and goes on only if its size and
   SHA-256 are what the server announced.
4. Unpacks **every** file in memory and checks its CRC before anything is
   written. A name that is not a plain relative path of ordinary characters
   refuses the whole archive (no `..`, no drive, no backslash).
5. Writes each file that differs beside its place as `<name>.skhid-new`,
   reads it back, and only when all are written renames them into place
   (the launcher and the updater last); the file that was there becomes
   `<name>.skhid-old`. Windows allows renaming a running program's file,
   so the launcher and the updater are replaced the same way while they
   run. If one rename fails, those done are undone: the game folder is as
   it was.
6. Files the ZIP before had and this one has not are deleted, if they still
   are what was installed.
7. Writes `updater_skhid.state`, and marks the patches unchecked
   (`updater_skhid.patches.status`: `blocked overlay changed`): new files
   may need other patches, so only step 10 with the server's list makes the
   game playable again. Leftover `.skhid-old` files are deleted at the next
   start.

Where files go: an entry `game/...` goes into the game folder (two levels
above `binaries\x86`, whatever its name), any other entry beside the game
folder when that is named `game` (as the game is unpacked), else into it.

When the launcher or the updater was among the files, the run ends here
("Updating itself" below): steps 8 to 11 are the new updater's.

8. The revision gate: the revision the installed `survarium_skhid.exe`
   says in its sign-in (the byte the client patch writes at 0x380862, read
   from the exe file) is compared with the answer's `min-revision`. Lower:
   PLAY stays off with "The server lets in game revision n and up; this game
   is revision r, and the update to it is not offered yet ..." (a server
   whose ZIP is older than its setting: a release half put in place), and
   the watch (11) starts the launcher anew once the server offers another
   ZIP. An exe whose revision cannot be read is left to the login server.
9. The restore, best effort, once: the `resources.db` regions an earlier
   updater (before the patches) rewrote in place get their stock bytes
   back, from the copies it kept, else from `/resources/<sha256>`. A failure
   is logged and tried again at the next start; it never keeps PLAY off.
   While the game runs it holds `resources.db`, and the step changes
   nothing ([updater README](../../../client/updater/README.md#restoring-resourcesdb)).
10. The patch step, required: `GET /patches`; every listed patch file in
    the game folder is checked (hashed when it changed since its last hash,
    or that was more than a week ago), a patch file the list has not is
    deleted, and a listed patch that is missing or damaged is downloaded
    from the first of its addresses that gives it, checked, and only then
    renamed into place. PLAY stays off until every listed patch is in place
    and no other is ([updater README](../../../client/updater/README.md#patches)).
11. "The latest version of the game is installed", PLAY on. The updater
    stays connected (a closed pipe would turn the line into "Error") and
    asks `/api/client-files` every 10 s until the launcher closes: another
    ZIP or `min-revision` starts the launcher anew, whose updater updates;
    two answers missed in a row turn PLAY off ("Server maintenance...",
    state 22) until the server answers again, which starts the launcher
    anew too ([updater README](../../../client/updater/README.md#play-and-the-servers-revision)).

The offline case: when `/api/client-files` does not answer (three tries),
the original game is present, and the files the updater installed last are
whole, the box says "No update check: <why>. ..." and the restore and the
patch step run against the list kept from the last run: PLAY is on only
when that run ended with every patch in place and the folder still is so
(a first start with the server down, or one after a new overlay, keeps PLAY
off). The watch then leaves PLAY on while the server stays away, and starts
the launcher anew once it answers with a ZIP that is not the one installed,
or with a `min-revision`.

Updating itself: the launcher was replaced → the updater tells the launcher
it is done (state 15) and closes the pipe, on which the launcher closes; the
updater waits until the launcher's "only one of me" mutex is gone and starts
the new `launcher_skhid.exe`. Only the updater was replaced → the same: the
launcher is closed and started again, and that start runs the new updater,
which does steps 8 to 11 (the old one does none of them after its install:
so updater 0.2.0 never reads the `/patches` of 0.3.0).

`updater_skhid.exe check` does the same by hand, without a launcher, and
hashes every patch file whole where the launcher's run takes the quick
check (the repair; `check quiet`: no message box; the exit code is 0 or 1).
When it replaces the updater itself, it runs the new one (`check`) at once
for steps 8 to 10 and ends with its exit code. It does not watch.

## When something goes wrong

Every failure is said in words, in a box titled "Skhid", and written
to the run's `game\logs\updater_skhid.<date>.log`. Nothing is half installed: the
ZIP's files go in all or none, and a patch file is a checked file or none.

| What happened | What the player gets |
|---|---|
| The server cannot be reached (no internet, server down, a provider that blocks it, a firewall), three tries | The updater's own box, in the launcher's language, with Retry and Cancel: "Blocked on this network: skhid.link did not answer (timed out)" when Windows' own test site answers (what to do: a VPN with the three programs in its split tunnelling, or none, `VPN.txt`), else "No connection: skhid.link did not answer (timed out)" (`Skhid-network-setup.cmd`); "name not found", "connection refused", "connection cut off" for the other ways ([updater README](../../../client/updater/README.md#the-players-boxes)). The same boxes, and "Download failed: <file>, damaged at every address" (`Skhid.bat` again), when every address of a patch or of the game's archive failed. Retry runs the update again; Cancel: PLAY if the original game is present, the installed Skhid files are whole (or were never installed by this updater) and the last patch step ended with every patch in place, else status "Error" and PLAY stays off |
| The original game is missing and Drive returns an error, a sign-in page, a redirect or bytes that do not match the pinned SHA-256 | Installation stops, PLAY stays off, and no files from that download are extracted or run. Retry or configure another mirror; verification cannot be skipped |
| ...and a file the updater installed is missing or damaged | The same box; the log says "x86/survarium_skhid.exe is missing or damaged and cannot be repaired now: <reason>"; status "Error", PLAY stays off |
| The answer is not the server's (a Wi-Fi sign-in page, a proxy) | "The answer from <host> is not the update server's ..." |
| The server has no ZIP to offer | "<host> offers no client files now" |
| The server's download slots are taken (it answers 503) | Waits 15 s and tries again, four times |
| The download breaks off, or its checksum is wrong | Tried again, four times; then "The files downloaded from <host> are damaged (the checksum is wrong). Nothing was installed." |
| The ZIP is not usable (not a ZIP, a bad name, a damaged entry) | "The downloaded archive cannot be used (<why>). Nothing was installed. Tell the server's operator." |
| Not enough disk space | "Not enough free disk space for the update in <folder>: N MB needed, M MB free." |
| Less than 5 GB free on the Documents drive (at every start, before PLAY) | Status "Error", PLAY stays off, a box in the game's language: "The game requires at least 5 GB of free disk space on drive C: to run smoothly. PLAY stays off until then." Retry asks again |
| The game folder cannot be written (Program Files, read-only) | "Cannot write <file>: Access is denied (error 5). Move the game out of Program Files or start the launcher as administrator; an antivirus can block it too." |
| A file is held by another program (the game is running) | "Cannot replace <file>: ... (error 32). Nothing was changed. Another program holds the file: close the game and try again." |
| Windows Security blocks a Skhid program file (error 225 or 226 at its read, write or rename), or a program holds it from every reader (a start script or text file is left alone) | Status "Error", PLAY stays off, nothing downloaded; the updater's own box, in the launcher's language, with Retry and Cancel: "Game files removed by antivirus: survarium_skhid.exe", then `Skhid-defender-setup.cmd` to run (with another antivirus, `game\binaries\x86` as an exclusion, or the game folder for a patch file), and the log's path (or "... is in use by another program. Close the game, then try again."). A file gone or denied right after the updater wrote it is the same box; a program file gone at the start is put back, then the same box with OK. Retry runs the update again ([updater README](../../../client/updater/README.md#a-file-that-cannot-be-read)) |
| A Skhid file cannot be read for another reason (access denied) | Repaired: the ZIP downloaded and installed again; when that fails too, the install's error naming the file, PLAY off. Never "up to date" |
| The server lets in a newer game than its ZIP (a release half put in place) | "The server lets in game revision n and up; this game is revision r, and the update to it is not offered yet (the server is being updated). Click OK: the launcher keeps looking and starts anew with the update. PLAY stays off until this is done."; the window starts anew by itself once the server offers the new ZIP (after OK: a launcher with a box open cannot close, and a second one is never started beside it) |
| A patch cannot be had or put in place (the list cannot be fetched or read, every address of a patch fails, a wrong checksum, the game is running, an antivirus holds the file, the disk is full) | Status "Error", PLAY stays off, a box that says which and what to do, ending "PLAY stays off until this is done." The next start tries again (the cases: [updater README](../../../client/updater/README.md#patches)) |
| The restore of `resources.db` fails (the game is running, a blob is missing on the server) | Nothing: no box, PLAY as usual. The reason is in the log after `resources:`; it is tried again at the next start |
| The server goes away while the window is open (a release, a restart) | "Server maintenance...", PLAY off; once it answers, the window starts anew by itself and updates |
| `updater_skhid.exe` is missing | The launcher's own box: "Cannot start updater_skhid.exe. Copy the files again." |
| The launcher is closed during an update | The updater stops; nothing was changed, or everything was |
| The updater is started from another folder than `game\binaries\x86` | "The updater must be in the game's folder game\binaries\x86 ..." |

A game started some other way (`play_skhid.bat` runs `updater_skhid.exe
ready`, which checks the patches against the last list, but not the server)
can still be one the server refuses: the game then says "You're using an
outdated version of the game." Start the launcher: it updates.

The texts are English only (the launcher shows them in an ANSI message box,
so only ASCII is safe), but the box of a file Windows Security blocks or a
program holds, which is the updater's own. The launcher's own lines are in
its five languages.

## What each patch does

Addresses are virtual addresses in the original exe (image base 0x10000;
`.text` file offset = address - 0x10C00, `.rdata` and `.data` = address -
0x11000). Every site is compared with the original bytes first. How each was
found: `../../docs/notes/launcher.md`.

### 1. Run where it is (no copy in `temp`)

- **Where:** `jne` at 0x1474A in `launcher::run_as_copy` becomes `jmp`
  (`0F 85 58 06 00 00` → `E9 59 06 00 00 90`); `je` at 0x14442 in the
  constructor becomes six `nop`s.
- **Why:** the original launcher shows no window at all when started from
  `game\binaries\x86`. It asks its updater to copy both of them to
  `<install>\temp`, starts the copy and ends; only a launcher whose path
  ends with `\temp\survarium_launcher.exe` makes a window. That was how the
  old updater could overwrite the launcher. The new updater renames instead
  (above), so no copy is needed.

### 2. No hardware warnings

- **Where:** `launcher::check_hardware` 0x145C5 returns 0 at once
  (`81 EC 04` → `33 C0 C3`).
- **Why:** before its window the launcher measured RAM, the CPU (registry)
  and video memory (WMI and DirectX) and warned below 1920 MB / 448 MB. The
  game does its own checks; under Wine these probes are slow and wrong.

### 3. Never through the Windows service

- **Where:** `je` at 0x15521 becomes `jmp` (`74` → `EB`).
- **Why:** the launcher first asks "Survarium Update Service" to start the
  updater. A PC that once had the real game installed may still have that
  service, which would start the old updater as SYSTEM. Now the updater is
  always started directly, as the player.

### 4. The installer's registry values are not needed

- **Where:** `registry_key::get_string` 0x14027 starts with a jump into the
  new section (`55 8B EC 51 8D 04 36` → `E9 <rel32> 90 90`). The four
  functions that read the installer's key open none: each jumps from the
  call that gives the key's path straight to its first read (0x15223,
  0x15997, 0x162F4, 0x1709F: `E8 <rel32>` → `EB <rel8> 90 90 90`), past
  `RegOpenKeyExW` and its failure branch, and each `RegCloseKey` is jumped
  over with the push of its handle (0x153FE, 0x1541E, 0x15A4D, 0x17123,
  0x1714F: `FF 75 xx` → `EB 07 90`; 0x16338, the two of
  `restart_launcher`: `test al,al / jne 0x163AD / jmp 0x16345`).
- **Why:** the launcher read five strings that only the game's installer
  wrote, from `HKLM\Software\Vostok Games\Survarium`: what PLAY starts
  (`file to execute`, `arguments`, `working folder`), where the updater asks
  (`update server`) and its own path (`launcher`). Without them it shows
  "Error" and PLAY never works. The new code answers for those five names:

  | Name | Answer |
  |---|---|
  | `file to execute` | `<the launcher's folder>\survarium_skhid.exe` |
  | `working folder` | `<the launcher's folder>` |
  | `launcher` | the launcher's own path |
  | `arguments` | `-minimum_resources_memory_size=768` |
  | `update server` | the site's host (the new updater does not use it) |

  Every other name goes on to the registry as before; these four functions
  read none, so they touch no key at all. Revision 39 still opened one (its
  patch 4 made the four opens `HKCU\Software\Vostok Games\Survarium`,
  which launchers before it made for their language): since 39 the language
  is in user.cfg, nothing makes the key, and a player who never ran an older
  launcher got "Can't open survarium registry key" from `start_updater`, no
  updater and no PLAY. `check_registry_026g.py` runs the four under Unicorn
  with no key, the key in HKCU and the key in HKLM, sees no registry call,
  and sees revision 39's box.
- **The new code:** `launcher_values_026g.asm`, in a new section `.skhid`
  after `.reloc` (VA 0x49A000). It finds where it was loaded and adds that
  to the two image addresses it uses, so it also works if the launcher is
  loaded at another base. The game's version shown under the logo is read
  from the file `file to execute` names, as before.

### 5. The updater starts even when the game exe is missing

- **Where:** 0x1577A in `launcher::start_updater`: the three pushes of the
  message "Can't retrieve first install version or local version" become
  `and word [ebp-0ECh],0` / `jmp 0x157A4` (the version is an empty string,
  go on). Two relocations under them are disabled.
- **Why:** with no `survarium_skhid.exe` to read a version from, the
  original stops there, and the one program that could bring the file back
  is never started.

### 6. The updater is `updater_skhid.exe`

- **Where:** the string `survarium_updater.exe` at 0x3DF6C.
- **Why:** the game's own updater is not touched and not used. It asked for
  administrator rights at every start, ran BitTorrent with DHT and UPnP
  port mapping, and uploaded crash reports (`../../docs/notes/launcher.md`,
  "What the original updater did").

### 7. One server: the page, the button

- **Where:** two `push <address>` operands point at new strings in
  `.skhid`: 0x1AFA2 (the page: `<site>/launcher/v2/`, to which the launcher
  adds `survarium/?lang=<code>`) and 0x1BCBA (PROJECT WEBSITE:
  `<site>/?lang=`). The old texts are emptied, SHOP's
  (`https://shop.survarium.com/` at 0x3F58C, pushed at 0x1BBFC) too: its
  button is gone (8).
- **The local pages:** the string `\..\game\launcher\index-` at 0x3F514
  becomes `\launcher_skhid\index-`: while the page loads, and when it does
  not, the launcher shows the pages of the bundle, not the game's own
  `game\launcher\index-*.html` with their links to the old sites.

### 8. The buttons that are gone

- **Where:** the click handlers of SHOP, CREATE ACCOUNT, Facebook and VK
  return at once (first byte → `C3` at 0x1BBEF, 0x1BC45, 0x1BD16, 0x1BD3C;
  YouTube's control is the "Allow torrent" box and Twitter's lies over its
  text since patch 15); their rectangles get a left edge of 30000 (0x45844,
  0x45854, 0x458B4, 0x458D4), which is outside the window, and so does the
  settings gear's (0x45814: its one setting that applies, "Allow torrent",
  is on the main window, patch 15; its port is the updater's); PROJECT
  WEBSITE moves from 478 to 750, SHOP's place (0x45864). Their addresses
  are emptied.
- **Why SHOP:** it opened the site's silver and gold exchange, which is in
  the lobby now (`../README.md`, "Lobby exchange"). Until 2026-10-16 it
  stayed, at `<site>/exchange?lang=`, and PROJECT WEBSITE stood beside it
  at 614.

### 9. Nothing of the old sites is left

- **Where:** the crash-report address, the support address and e-mail
  (0x3DBB4, 0x3DB94, 0x3DB6C, handed to `bugtrap.dll` only) are emptied.
  The self-test searches the result for `survarium.com`, `vostokgames`,
  `facebook.com`, `twitter.com`, `vk.com` and `youtube.com`, in ASCII and
  UTF-16, and finds none.

### 10. Two texts

- "Can't run survarium_updater from service nor directly" (0x3EA90) becomes
  "Cannot start updater_skhid.exe. Copy the files again."; the title of the
  updater's error box "Updater error:" (0x3EF80) becomes "Skhid updater".

### 11. Enter presses PLAY

- **Where:** the message loop of `launcher::execute`: its first
  instruction, `cmp dword [ebp-1Ch],113h` at 0x14EF0, becomes a jump to
  `enter_key` in the new section and two `nop`s
  (`81 7D E4 13 01 00 00` → `E9 <rel32> 90 90`).
- **Why:** the original window takes no keys; PLAY was mouse only.
- **How:** for a `WM_KEYDOWN` of `VK_RETURN` that is not a repeat, the new
  code reads the form shown (`[launcher+2DC4h]`) and PLAY's click callback
  (`form+638h`, with its argument at `form+63Ch`). `set_play_button_active`
  (0x1B5E9) sets them to `launch_survarium` and the form while PLAY is
  enabled and to 0 while it is not, and a click calls exactly that pair
  (0x1275B). So Enter does what a click on PLAY does, and nothing while PLAY
  is off. The key is looked at before `TranslateMessage`, so it works
  wherever the focus is, the server's page included; the Enter that pressed
  PLAY is not handed on. Every other message runs the replaced `cmp` and
  goes back to 0x14EF7.

### 12. The file itself

- The Authenticode signature is taken off: it cannot be right for a changed
  file, and the launcher never reads it (unlike the game exe). The `.skhid`
  section takes its place at the end of the file, the section table gets a
  sixth entry, `SizeOfImage` grows by its pages (two at most) and the PE checksum is
  recomputed.

### 13. Install-status transition crash

The 2026-10-18 launcher dump stops in the invalid-parameter handler with
`ERANGE` (34), called by `wcscat_s` at `0x25DC9` from `refresh_status`
(`0x16A94`). The 128-character status buffer contains a Russian CPU warning
followed by part of the remaining-time suffix.

The cause is an unchecked string-table index at `0x16D2C`. The launcher
keeps the last pipe message ID at `+0x51EC`, including `2` for a state
message. If an earlier install left a filename at `+0x4FE4`, entering
INSTALLING can render that filename before a new install-info message
arrives. The formatter expects IDs 6..9 and otherwise reads an unrelated
stack local as a resource ID. This can select long warning text and abort
while appending the ETA.

The six bytes at `0x16D06` now jump to `install_status` at offset 560 in
`.skhid`. IDs 6..9 retain the native format; other IDs take the native
generic installation message (`0xA9`). The stub uses relative branches
and calls, so rebasing needs no new relocations.

`check_install_status_026g.py ORIGINAL_survarium_launcher.exe` executes the
original and patched selector plus the native `wcscat_s` under Unicorn.
It reproduces the Russian overflow, then checks 2,580 cases across all
five original languages and two image bases, plus the empty-filename
path. Resource strings come from the original executable. The test stubs
resource loading and the single `%s` label formatter; it does not run the
launcher UI or perform an update. `selftest_launcher_026g.py` also checks
the assembly, patch boundaries, PE layout, checksum and relocations.

### 14. The language is the game's

The owner (2026-10-29), after a player's report (the launcher in Russian,
the game in Polish, his Windows' language): the launcher's language choice
is the game's, in one place, user.cfg, and the launcher offers every
language the game has. `docs/notes/language-option.md` has the rule.

- **One list.** `launcher_languages_026g.inc` is generated at every build
  from `localization_026g.py`'s `LANGUAGES` (the game's own list:
  resource name, own name, Windows locale rule, two-letter code), as
  the game's table is.
  Exact Croatian LANGIDs take precedence over the Serbian primary-id fallback,
  as in the game. The launcher's own string tables and pages stay its five (`LAUNCHER_UI`
  there): another language shows them in English.
- **The store.** `settings.language` is the index in that list. The start
  (`settings::reload_language` 0x1D852) reads the `localization` line of
  `<Documents>\survarium\user.cfg` (SHGetFolderPathW(CSIDL_PERSONAL), the
  game's folder, client_patch.md; UTF-16 paths and the W file functions, so a
  Documents folder outside the ANSI code page works); without a registered one, the
  Windows user locale's language (patch 79's rule), else English; it writes
  nothing. `settings::save_language` (0x1D8DA) writes that line: replaced
  where it is, added at the end when missing, the file (and its folder) made
  when missing; written to `user.cfg.new`, then renamed over (MoveFileExW), so
  the game never finds it half written. A user.cfg it cannot read whole
  (64 KiB or more, or locked) is left alone.
- **The menu.** The button's click (`language_menu::expand_button_click`
  0x19A08), which opened the five-row drop-down, opens a Windows menu
  (CreatePopupMenu, TrackPopupMenu, by GetProcAddress) under the button;
  a pick goes on as a drop-down row did (`language_selected` 0x19A20: saved,
  the window made again in that language). The five drop-down rows are
  still made, never shown.
- **Every place that read the index by five.** The button's label
  (0x19663, the table 0x48280), the launcher's LANGID (0x150B3, 0x3E098) and
  `get_language_code` (0x1A84B: the page's `?lang=` and `index-<code>.html`)
  call or jump to the new code by `language_entries` (`LANGUAGE_HOOKS` in
  `patch_launcher_026g.py`); their three relocations are disabled. PLAY's
  `-localization=` word (0x1718D) is computed but unused (patch 10).
- **The updater** writes its boxes in the same language: user.cfg's line
  (`client/updater`, `replay::language_of_user_cfg`); it reads no registry
  value any more.
- Checked: `check_language_026g.py ORIGINAL_survarium_launcher.exe` runs the
  new code in the patched launcher under Unicorn at two image bases, with
  Windows' functions stubbed and user.cfg in memory: the start (the line,
  the locale, English; nothing written), the save (replaced, added, made,
  through a new file; a file it cannot read whole left alone), the menu
  (every language by its own name, the current one checked, the pick handed
  on), and every language's label, LANGID and page; each with a Documents
  folder of ASCII and one of Cyrillic and Chinese. Not yet run in a real
  launcher window.

### 15. "Allow torrent" on the main window, kept in user.cfg

The owner (2026-11-15): BitTorrent is off by default; a player turns it on
with an "Allow torrent" box right on the launcher's main screen ("no need
for settings"), and it stays as they left it, kept as the language is.

- **What it looks like:** the settings window's own box and text, at the
  left of the bottom panel where the social buttons were: the box (PNG 347
  unticked, 345 under the pointer, 346 pressed, 344 ticked, 20 by 20) at 28,
  20 of the panel (32 before patch 16 put the relay row under it), in line
  with the page's left column, the two boxes centred on the status line and
  PLAY; "Allow torrent" (string 0xD3, the launcher's
  language) 25 pixels right of it and 1 below its top, Calibri bold 18,
  white, as the settings window has it beside its box (0x1C38E, 0x3A96C).
  The widest text, Russian, ends at x 186, before the status line (x 200).
  Previews: `private/renders/launcher-allow-torrent/` (not in git).
- **Where:** YouTube's control (form+0x648) is the box: its three pictures
  (0x1AAFC, 0x1AB0F, 0x1AB22) and Facebook's first (0x1AB35, the ticked
  one; Facebook stays out of sight) are loaded from the settings window's
  PNGs, its rectangle (0x458A4) moves to 28, 20 and its size initialisers
  (0x3A715, 0x3A722) say 20. Twitter's control (form+0x698) makes the text
  clickable: its rectangle (0x458C4) is 53, 20 to 187, 40, the text in every
  language (Russian, the widest, is 134 pixels; its initialiser 0x3A746,
  which could add at most 127, returns at once and right and bottom are
  static), with the hand cursor as the box's; its click (0x1BD29) jumps to
  YouTube's. Five sites call or jump to new code (`TORRENT_HOOKS`, entries
  after the language's):
  - 0x1AD78, the bottom panel's `image_window` (`panel_made`): the text is
    painted into the panel's picture (form+0xEC, JPG 301) as the status
    texts are, and the box's four pictures are made opaque over the
    panel's picture where the box is (`bake_box`: a transparent control is
    drawn over what its window last showed; only Windows composes the
    launcher's windows, WS_EX_COMPOSITED, so under Wine an unticked box
    kept the check of a ticked one); Twitter's three pictures become the
    panel's picture where the text is, text included (`bake_label`);
  - 0x1AEA3, Twitter's `transparent_control` (`label_made`): made as
    before, then opaque (+24h: BitBlt, not AlphaBlend), so it shows exactly
    the panel's pixels and no visible change (text drawn by GDI may leave
    their alpha 0);
  - 0x1AE1F, YouTube's `transparent_control` (`box_made`): ticked
    (settings+0), its picture and the one under the pointer are the ticked
    one;
  - 0x1B4D9, `set_status_texts`' copy of the panel's picture from its
    resource (`panel_restored`): the copy, then the text again (each status
    change copies the picture anew);
  - 0x1BD03, YouTube's click (`torrent_click`): user.cfg's line written
    at once with the other value; only once it is written, settings+0 and
    +1 toggled, the callback settings::save fires for a changed box
    (`function0` at settings+0x28: 1A to the updater while the pipe is
    connected, 0x172FA), the pictures as
    `settings_window::show_torrent_checkbox` (0x1CE0E) shows them. A click
    on the text comes here the same way.
- **The store:** user.cfg (`<Documents>\survarium\user.cfg`, patch 14),
  the game's own saved command `enable_torrent_seeding on` / `off` (exe
  0x1EE580; the game keeps it when it rewrites the file), written as the
  language is (one `save_setting` for both: the line replaced or added,
  through `user.cfg.new` renamed over). The start (`reload_torrent`, from
  `settings::reload_language` right after `settings::reload` at 0x14200):
  ticked for `on`; no line, another value or no user.cfg: off. The
  registry's `download_type` is neither read nor written any more
  (settings::reload still reads it; its result is replaced at once).
- **The one-time reset:** the original game saved its default,
  `enable_torrent_seeding on`, in every player's user.cfg. Before the line
  is read, a Windows user without `launcher_skhid.torrent_reset` beside
  user.cfg gets that empty file made, then the line written `off`; from then
  on the line is the player's alone. A user.cfg that cannot be written
  takes the file away again (the next start tries again), and without the
  file the box is off, whatever the line. The updater takes the line
  only with that file there (`client/updater`, `win::launcher_torrent_allowed`).
- **Why it is the one place:** every reader takes `settings+0`:
  `start_updater`'s 4th argument (0x1580B: `torrent` or `direct`) and
  `send_switch_download_type_message` (1A, 0x17367).
- Checked: `check_torrent_box_026g.py` runs the five sites, the start
  (0x141D9..0x1420C: settings::reload, then reload_language) and the
  box's and the text's window procedure under Unicorn at two bases: the
  start reads the line whatever `download_type` says; the text and the
  opaque pictures; the box ticked or not; the text's control at its rect in
  the panel, painting the panel's crop with BitBlt; five clicks, on the box
  and on the text, each writing the line (others kept), firing the callback
  once, no registry call, and a fresh start reading the box as it was left;
  a click (box or text) whose line cannot be written changing nothing (box,
  callback, file); the reset (`on` becomes `off` once, the file made; a
  locked user.cfg makes no file; a tick after it stays across restarts).
  `check_language_026g.py` still passes (the shared user.cfg code). Run in
  the real launcher under Wine (headless, with a stand-in updater on its
  pipe, 2026-11-15): the box drawn in all five languages, clicks writing
  the line and sending `1A 00` / `1A 01`, a restart ticked, the legacy `on`
  reset once with the file made. The clickable text is not yet run in a
  real launcher. Not yet run on Windows.

### 16. "Relay: auto / on / off" under "Allow torrent"

The owner (2026-11-20): players see their route and pick it: automatic, the
relay, or direct, and the choice stays. First it was a tick box that the
updater ticked; the owner, the same day: a tick box has two states and this
has three. So it is a row that says its state in words, with a menu of the
three. The route is the updater's (`client/updater`, `updater_logic::route`,
its README's "The route"): it keeps the choice, probes, and says what the row
shows; the launcher only shows it and passes the pick on.

- **What it looks like:** under "Allow torrent", a button with the settings
  gear's pictures (the launcher's own art: PNG 308, under the pointer 309,
  pressed 310; its settings gear left the top panel with patch 8) at 28, 46
  of the panel, and beside it, at 53, 47, in the torrent box's font, the
  state: "Relay: on", "Relay: off", and in auto what the updater's check
  (run when the launcher opens, so that PLAY never waits on one) does or
  found: "Auto: checking...", "Auto: direct", "Auto: via relay", "Auto: no
  answer" (the owner, 2026-11-23: a wait the player cannot see is no good;
  until then "Relay: auto", "auto (on)", "auto (off)"). Five languages
  ("Авто: через релей", "Авто: без відповіді", "Auto: über Relay", "Auto:
  brak odpowiedzi"; Ukrainian's 19 letters, the widest, about 140 pixels,
  end before the status line at x 200). Hidden until the updater says what
  it shows: without `updater_skhid.relay`, or with an older updater, there
  is no row. A tooltip (comctl32's, 30 s) says in a few lines what the
  relay is, the three choices, and what the row is now (in auto, the route
  the check found, or that it is checking the connection). It says what the
  relay costs: "On: always through the relay (higher ping)", and for auto
  on the relay "auto: through the relay (higher ping): direct did not
  answer."
- **The views** (message `0A <view>` of the updater, its one byte
  `updater_logic::route::Shown::byte`): 1 auto, checking; 2 auto, direct;
  3 auto, the relay; 4 auto, neither answered; 5 on; 6 off. The original launcher's dispatch ignored 0A..0D (their table
  entries are the handlers' end, 0x16093): 0A's (0x160C2, a relocated dword)
  is now `route_message`; any other byte is ignored, and an older launcher
  ignores the message altogether.
- **A click** on the gear opens a menu under it, as the language's is
  (`pick_language`: CreatePopupMenu, TrackPopupMenu): "Auto: only when
  blocked", "On: always through the relay", "Off: never", the chosen one
  checked. A pick sends `22`, `23` or `24` (auto, on, off;
  `send_simple_message` 0x1749F, while the pipe is connected); the updater
  saves it, runs its update again by the new route and answers with the new
  view. The launcher keeps nothing but the last view. The updater also
  moves it by itself, to auto, when it finds the direct route blocked while
  the relay answers, and says so in its box.
- **A server that restarts** (the owner, the same day: "The server is
  restarting", retried every few seconds with a count the player sees):
  the updater says "Server maintenance..." (its state 16h: PLAY off) and
  then, each second, message `13 <seconds>` to its next try, which the
  original launcher keeps (launcher+0x2BA0) and paints the status line anew
  for, showing nothing of it (its only reader is the text of updater state
  23, which this updater never says). The status line's `load_string` call
  for the launcher's states 6 and 7 (0x1687C) is `restart_text`: asked for
  "Server maintenance..." (string 0xB2) while that byte is not 0, it gives
  "The server is restarting. Next try in 5 s." with the byte, in the row's
  five languages; `13 00` (the server answers, or the wait ends) and every
  other string are `load_string`'s own. PLAY comes on with the updater's
  "latest" as ever.
- **Where:** VK's control (form+0x6C0) is the button: its three pictures
  (0x1ABA7, 0x1ABBA, 0x1ABCD) are the gear's PNGs, made opaque over the
  panel where the button is (`bake_relay_box`, as the torrent box's); its
  rectangle (0x458D4) is 28, 46, its size initialisers (0x3A766, 0x3A773)
  say 20. Its making (0x1AEE5, `relay_made`) shows or hides it as the last
  view says, so a window made again in another language keeps it; its click
  (0x1BD3C, `relay_click`) is the menu. The text is painted with the torrent
  box's (`paint_label`); a new view calls `refresh_status` (0x16811), which
  paints the panel anew.
- **Its state:** the launcher (for the pipe), the view, the tooltip and the
  button's window it is for: 16 bytes at 0x4A800, in `.data` past its last
  variable; `.data`'s VirtualSize grows from 0x67FC to 0x6810, inside the
  page the loader maps already. The new code is 3 pages now (its texts), 4
  at most.
- Checked: `check_relay_box_026g.py` runs message 0A from the dispatch to
  the handlers' end (every view, and 0, 7, 255 ignored; the registers and
  the stack as they were), the button's making (hidden, or shown, its three
  pictures the gear's), the tooltip (made once, its text updated in place,
  made again for a new button), the panel's text for every view in every
  language, the click (the menu's three rows in each language, the chosen
  one checked, its pick 22, 23 or 24; no menu while not connected or
  hidden), and the status line (the count in each language,
  1 to 255 seconds; no count and state 6's text passed on to
  `load_string`), under Unicorn at two bases.
  `check_torrent_box_026g.py` and `check_language_026g.py` still pass. The
  tick box before it, the tooltip and the status line were run in a real
  launcher window under Wine (2026-11-20, a headless display, the test
  folder of the PR); the gear and its menu not yet.

### 17. "(5/69)": which file of how many is being downloaded

The owner (2026-11-22): a download of many patches says which file of which
it is at, "like (5/69)", so that 64 more are understood to follow. The
updater knows (`client/updater`, its patch step's files to fetch) and says
it; the launcher adds it to its downloading line.

- **What it looks like:** the downloading line's middle text, "120 MB
  left", is "120 MB left (5/69)" while the updater downloads the 5th of 69
  patches. Numbers only: the same in every language. The Skhid ZIP and the
  original game are one file each and have no count; a patch the torrent
  gave is counted and not shown (nothing is downloaded for it).
- **The message** `0B <file> <of>` of the updater (two 16-bit numbers; `0B
  00 00 00 00`: no count, sent when the downloads end, however they end).
  The original launcher's dispatch ignored 0B as it did 0A: its table entry
  (0x160C6, a relocated dword) is now `count_message`, which keeps the two
  numbers, or none when the file is 0 or past the last. An older launcher
  ignores the message.
- **The line:** in the updater's state 9 `refresh_status` loads "%s left"
  (string 0x8D, its `load_string` call at 0x16ADF, a buffer of 128
  characters) and formats what is left into it. That call is `count_text`:
  `load_string`'s own, then " (5/69)" after the text while a count is kept
  and 16 characters are free. The line is painted anew with every progress
  message (four a second), so the count needs no repaint of its own. The
  text is only there while the total is known (the original's rule for
  "... left").
- **Its state:** 4 bytes at 0x4A810, after the relay row's; `.data`'s
  VirtualSize is 0x6814 now.
- Checked: `check_file_count_026g.py` runs message 0B from the dispatch to
  the handlers' end (kept, none for 0 of 0, file 0 and a file past the last;
  the registers and the stack as they were) and the line's site (the count
  after `load_string`'s text, 1 of 1 to 65535 of 65535; none without a
  count or without room; the registers as they were), under Unicorn at two
  bases. The other checks still pass. Not yet seen in a real launcher
  window.

## The pipe, in short

The launcher makes `\\.\pipe\vostok_updater_pipe-Survarium`, then starts the
updater with that name as its second argument. A message is a 16-bit length
and that many bytes, the first being its type.

| Updater says | Launcher shows |
|---|---|
| `02 04` | "Checking game version..." |
| `02 05` | "An error has occurred during checking current version, retrying..." |
| `02 09`, then `01 <done> <total> <speed> <time left>` (four 64-bit numbers) | the bar, "... left", "Speed ..." |
| `02 0A` | "An error has occurred during downloading, retrying..." |
| `02 0C` | "Download complete" |
| `02 10`, then `07 <name>` and `01 ...` | "Extracting file <name>", the bar |
| `02 0E` | "The latest version of the game is installed", PLAY enabled |
| `02 16` | "Server maintenance...", PLAY (and Enter) off (`../../docs/notes/launcher.md` 5.4; said by the updater while the server stops answering) |
| `02 01` | "Error" |
| `05 <text>` | a message box with the text (at most 255 ASCII characters) |
| `02 0F` and the pipe closed | the launcher closes |
| `0A <view>` | the "Relay" row (patch 16): hidden until then |
| `0B <file> <of>` (two 16-bit numbers) | "(5/69)" after "... left" while downloading (patch 17); `0B 00 00 00 00`: no count |
| `13 <seconds>` | after `02 16`: "The server is restarting. Next try in <seconds> s." (patch 16); `13 00`: "Server maintenance..." again |

The launcher says `1B` (stop) when its window is closed, `22`, `23` or `24`
(auto, on, off) when a row of the "Relay" menu is picked. The updater stays
connected after `02 0E` until then (a closed pipe would turn the line into
"Error"), asking the server every 10 s; to have the launcher start anew it
says `02 0F` and closes the pipe, then starts `launcher_skhid.exe`. Types 0, 24 and above 25 would crash the launcher (its table has no
bounds check), so the updater's tests check that none is ever sent. The
whole protocol: `../../docs/notes/launcher.md`.

## What was run (2026-10-13, Windows 11)

- `client/updater/logic`: 28 unit tests (SHA-256 vectors, DEFLATE against
  zlib's output and every cut and bit flip of a stream, ZIP reading and
  hostile archives, the two text formats, the pipe messages).
- `client/updater/tests/test_updater.py`: 18 scenarios with the real
  `updater_skhid.exe`, a web server on 127.0.0.1 and the test in the
  launcher's place on a real named pipe: first install, up to date, a new
  archive, repair, the launcher and updater replaced while running and the
  restart, no server (three cases), a wrong checksum, a broken download, a
  hostile archive, a locked file with the undo, closing during a download,
  `check` by hand. One of them installs a real players' ZIP and compares
  every file with Python's unpacking of it.
- The real patched launcher with the real updater and a local srp server
  (`cargo run -p srp`), in a test game folder under another product name
  (`-product_name=SkhidTest`, so that its registry key, mutex and pipe are
  its own): the window and the server's page; the language switched to
  Ukrainian (texts, page, the registry value); PLAY with a stand-in for the
  game exe (it was started as `"<folder>\survarium_skhid.exe"
  -localization=ukrainian -mt_allocator=768
  -minimum_resources_memory_size=768` in `binaries\x86`, and the launcher
  closed); a changed game exe repaired at the next start; a new ZIP with
  another launcher in it (the launcher replaced itself and came up again);
  the server stopped (the local page, the message, PLAY enabled).
- `selftest_launcher_026g.py`: all checks. `test_release_overlay.py`: 10
  tests; one runs the real `Skhid.bat` in a folder with a space and a
  bracket in its name, with a stand-in for the launcher (it was started by
  its full path, in its own folder), and without the launcher (the text
  that says where the files belong). `test_linux_launcher.py`: 25 tests
  (in WSL, with a stand-in for Wine); one runs `Skhid.sh` from another
  folder, with and without `launcher`, and without the script it looks
  for.

- Later that day, with the servers on the internet (HTTPS, Windows 11). The
  test server, a fresh copy of the game with its ZIP unpacked over it: the
  first check (the ZIP taken once, no file changed, the state written);
  `Skhid.bat`, the window with the server's page and its top 10 with a
  player in it, PLAY enabled; a second ZIP offered, with another launcher
  in it (13 files installed in two seconds, the launcher replaced itself
  and came up again; every other file of the folder was left as it was,
  compared by checksum before and after); the language changed in the
  window (saved at once; the updater is not told and stays); PLAY started
  the real game, which signed in and reached the lobby. The players'
  server: its ZIP taken from the site, unpacked, `updater_skhid.exe check`
  run in it.

**Not run:** Wine (`bash play_skhid.sh launcher`); another Windows than 11;
a download slow enough to watch the bar move in the launcher's window (6 MB
came in under a second each time); a PC that still has the real game's
installation and service.

## Limits, and what is not done

- **The ZIP is trusted because HTTPS is.** The updater checks that what it
  downloaded is what the server announced; that the server is the server
  rests on its certificate. A signature of the ZIP with a key kept off the
  server would also cover a taken-over server. Not done.
- The launcher has no line for "could not check, playing with what is
  there": after the message box it says "The latest version of the game is
  installed".
- While the launcher stays open the updater asks the server every 10 s:
  a release reaches an open launcher within about that time (it starts
  anew), and PLAY is off while the server is down. "Server maintenance..."
  (state 22) is the original launcher's line and behaviour, read from its
  code (`../../docs/notes/launcher.md` 5.4) and run with the stand-in
  launcher of the updater's tests; not yet seen in the real launcher's
  window.
- The login server's refusal of an older game is the game's own "You're
  using an outdated version of the game." (login answer 0x22): the login
  server has no answer with words of its own. The launcher keeps PLAY from
  starting such a game; a game started otherwise gets that line.
- One launcher at a time (its mutex), as before.
- The launcher keeps no language of its own: its registry value
  (`HKCU\Software\Vostok Games\Survarium`, `language`) is no longer read or
  written; the game's user.cfg holds it (patch 14). Its own texts and page
  are in five languages (English for the other five).
- `play_skhid.bat` starts the game without the server: it runs
  `updater_skhid.exe ready`, which checks the patches against the last
  list, and looks for no update.

## Putting it on a server

1. The server must run a build with `/api/client-files` and `/launcher/`
   **before** a ZIP with the launcher is offered: a launcher asking a server
   without them gets "offers no client files now" at every start.
2. Build the ZIP with `make_client_026g.py --pack-zip` for that server and
   put it where `SRP_DOWNLOAD_FILE` points. Players who have the launcher
   get it at their next start; the others take it from the page once.
   The site's "How to play" follows the file: while the ZIP it offers holds
   `launcher_skhid.exe`, the steps say to unpack it into the game's folder
   once, double-click `Skhid.bat` and press PLAY, with the start script and
   the command line folded under "Without the launcher"; for a ZIP without
   it they are the steps as before.
3. From then on a release is `release/release.sh` (the `skhid-release`
   skill): it builds the ZIP and the server data, and at the swap puts the
   ZIP, the server data and `SRP_MIN_PATCH_REVISION` in place **while srp
   is stopped**, then starts srp. Never replace the ZIP alone on a running
   server: its exe revision must be the one srp lets in, and its patches
   the ones srp lists (the updater then keeps PLAY off, "lets in game
   revision n and up"). An open launcher sees the new ZIP within 10 s and
   starts anew.
4. Game content is not in the ZIP: it ships as `resources.db.patch_<n>`
   files on a patches server, listed by the game server at `/patches`
   (`../../../client/updater/README.md`, "Patches"). Keep
   `download/resources/` with the originals' blobs of the regions earlier
   releases changed in place: the updater puts the stock bytes back from
   the player's own copy, or from `/resources/<sha256>` when that copy is
   gone.

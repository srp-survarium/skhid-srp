# Download mirrors: the game's archive and the patches from many addresses

The incident: on 2026-11-01 Google Drive's download quota for the 2.9 GB game archive ran out.
Every whole download answered 200 with a 2040-byte "Google Drive - Quota exceeded" page; the
updater (one built-in address) saved it and said "size or SHA-256 mismatch",
Skhid-without-updater.bat called it "damaged". New players could not install, and nothing said why.
The owner's design (approved 2026-11-01), built in one PR.

## Pinned per release, links dynamic

- **The bytes are pinned, per release.** Each file's name, size and sha256: the patches' rows of
  `release/patches.tsv`, and the row of the game's archive, `survarium-026g.7z`, in
  `release/game.tsv` (2906544103 bytes, the sha256 of `client/updater/logic/src/game.rs`; a file
  of its own, so that every reader of the patch list stays as it is). The server game data names them
  (`srp-game-data build --game "<row>"`, a `game` line of `srp/provenance`).
- **The links are not.** srp offers each file's release URLs, then the rows of `mirrors.tsv` in
  its data folder (`<file> <url>` a line, tab or spaces, `#` comments), in their order. It reads
  the file again when its time or size changed, at a request: no restart.
- **A bad row is skipped and logged**, never an error: a file the release has not, a URL the
  updater would refuse (`server::is_url`), one the file has already, past 8 URLs a file. `/patches`
  is never a 404 for it (that would keep PLAY off for everyone).
- **`/game`**, beside `/patches`: `{"sha256":"<hex>","size":<bytes>,"urls":[...]}`. Not a member
  of `/patches`: the .bat takes every member there for a patch, and the updaters take an entry of
  exactly `sha256` and `urls`. 404 when the data names no archive.

## Adding a link

- `release/add-mirror [--test] [--first | --at <n>] <file> <url>`: downloads it as the updater
  does (whole, redirects, not a web page, size, sha256), then, under the release lock and after a
  y/N, writes the row into the target's `mirrors.tsv` (srp26's volume, through the jump host),
  and checks `/patches` or `/game` lists it. `--remove <file> <url>` takes a row out.
- Into the next release too (the built-in addresses): add the URL to the file's row of the list,
  in a PR.
- Google Drive: `https://drive.usercontent.google.com/download?id=<id>&export=download&confirm=t`.

## The updater

- **Every failure moves on to the next address**, in their order: a timeout (WinHTTP: 10 s name
  lookup, 10 s connect, 30 s send and receive), host not found, unreachable, a reset, another
  status than 200/206, a web page (its `<title>` read from its first 8 KB, any host), too slow
  (under 50 KB/s over a 2-minute stretch), cut short, larger than the file, wrong size or sha256.
- **The game's archive goes on from the bytes it has** at the next address (`Range: bytes=<n>-`):
  a 206 from byte n is appended, a 206 of another range is a failure, a 200 starts from byte 0. The
  whole file is checked; a mismatch deletes it. A part stays for the next start. Patches are
  fetched whole (unchanged).
- **Built-in addresses** (`SKHID_BUILTIN_GAME`, `SKHID_BUILTIN_PATCHES`: the list's `/game` and
  `/patches`, passed by `release/build-zip`) are used when the server cannot be reached (some
  Russian providers block skhid.link: `VPN.txt`), offers no `/game` or another archive. A server
  that answers always wins.
- **The log**, `game\binaries\x86\updater_skhid.log`: a line a try, a failure (the reason, the
  bytes received) and the success:

  ```text
  skhid-game  try      drive.usercontent.google.com: address 3 of 3, https://..., from byte 0
  skhid-game  failed   drive.usercontent.google.com: web page "Google Drive - Quota exceeded", not the file (2040 bytes received)
  skhid-game  ok       archive.org: 2906544103 bytes, SHA-256 verified
  ```

- **The player** is told each address's host and reason, then, after every failure the run ends
  on: `Send game\binaries\x86\updater_skhid.log to #bugreports in Discord (tag: install):
  discord.gg/Pxa4prXfwu` (the crash reporter's words; the launcher's line is 255 ASCII characters,
  so the reason is cut, the log has it whole).

## Skhid-without-updater.bat

- The same in PowerShell (the code at the end of the .bat, `:ps`), with curl: `--connect-timeout
  10`, `--speed-limit 51200 --speed-time 120`, `-r <n>-` to go on, the status, the type and the
  `Content-Range` looked at before the bytes are kept.
- The same lines in `skhid-download.log` beside it; at a failure, where to send that file.
- The release's rows written in at build time (`make_client_026g.py --downloads`, `#skhid-file
  <name> <sha256> <size> <urls>` lines): used when the server cannot be reached, instead of
  stopping (when the Skhid files are there; else it stops as before).

## Not done (the owner's choice)

- No report of failures to srp, no admin-page tab, no health-check cron.

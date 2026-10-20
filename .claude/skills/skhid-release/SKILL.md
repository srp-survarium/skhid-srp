---
name: skhid-release
description: Make a release of skhid-srp (the Survarium 0.26g server "Skhid") — a new players' download (exe, launcher, updater, crash reporter), new game content as a resources.db patch, server code, or any mix — with release/release.sh, which at the owner's go plans the maintenance at once to start 30 seconds after the last running match ends, prepares and stages everything during the countdown and swaps it all in at once, keeps a folder per release on the host and rolls back by itself when the new server does not come up. Covers the questions to ask first, the revision (each release sets it and tags itself), the offered-patch list, the dry run, the test environment, what to watch, rollback, and the step scripts in release/. Use when asked to make, ship, test or roll out a release of skhid-srp.
---

# Making a Skhid 0.26g release

## Private values

Every host, address and local path is a setting. Their format is
`release.env.example` next to this file; the filled-in copy is
`private/release/release.env` in the main checkout (never published). The
scripts load it themselves (`SKHID_RELEASE_ENV=<file>` names another); if
it's missing, ask the owner. Any PC can run a release: what is particular
to it is in its own `release.env`.

The hosts, by role: the **jump host** (`$SKHID_JUMP`: it holds the live
host's key, runs the patches servers, asks the admin page, holds the
release lock and the copies of srp.db), the **live host** (`$SKHID_LIVE`,
the srp26 compose project in `$SKHID_LIVE_DIR`, the site `$SKHID_SITE`),
the **test environment** (`$SKHID_TEST` in `$SKHID_TEST_DIR`, compose
project `srptest`, its site `$SKHID_TEST_SITE`, its own patches server
`$SKHID_TEST_PATCHES_*`; today it lives on the jump host itself). Never print
a `.env`; read a single setting with `grep -nE '^(NAME)=' .env`. One release
at a time per target: `release.sh` and `deploy` take a lock on the jump
host and refuse while another run holds it.

## When the owner says "make a release"

Ask, in this order:

1. **Does it include new content?** If yes: **the path to the patch
   database**, `resources.db.patch_<rev>` (a standalone archive in the
   game's format, named for the release's revision, below; how it was made
   doesn't matter). A folder of files at the paths the game reads them from
   (`gameplay/items/...`) is packed into one by
   `release/build-patch <folder> <rev>`. **The lobby movie**
   (`flash_movies/lobby_menu.swf`), when any lobby change is new since the
   last release, is built by `srp-026g/client-patches/build_lobby_026g.py`
   alone (every lobby change, in order, on revision 23's movie; README
   "Building the lobby movie"), never by running one lobby step on some
   movie: a patch replaces the movie whole, so a movie missing a step drops
   that change for the players.
2. **Any server-code change?** `release.sh` compares main's `srp-026g`
   build inputs with the target's tree and builds the image when they
   differ; the owner may know better (`--server-code yes|no`).

**The revision is the release's own.** Every release has a new patch
revision, content or not: it is how the server refuses games without the
new files. It is the live exe's + 1 (`--test`: the test exe's + 1), which
`release.sh` reads from the target's ZIP and passes to the build
(`build-zip --revision <n>`, `make_client_026g.py`/`patch_026g.py
--revision=<n>`: the revision byte alone; nothing is edited first).
`--revision <n>` names another, above the live exe's (a number once
released is never used again: after a rollback the preflight says which).
After a live release went out, the release itself records it (step 7):
`REVISION = <n>` in `patch_026g.py`, the row of the revision table in
`srp-026g/client-patches/README.md`, a changelog entry to word, the list,
and the tag `026g-rev<n>`. So `REVISION` on main is always the last
release's, and **a feature PR never changes `REVISION` or the revision
table**, even when it changes the exe; a server rule for its game asks for
the next release's revision, `REVISION + 1`. There is no bump PR. From
revision 12 on the exe expects the menus of `resources.db.patch_12`: a
release of revision 12 or later is refused unless patch_12 is offered (in
the list, or the `--resources` file).

**Replays, and a change of what a match computes.** A game is given a
replay only when every game of its recording was of the oldest replayable
revision or later: the last row of
`srp-026g/crates/session/src/v026g/simulation_changes.tsv` (a recording of
older games plays out otherwise; one with no revision kept, from before
2026-10-15, is refused too). The player sees the movie's "This replay was
recorded by a game version that yours cannot play." and a message box in
the game's language naming the revisions and the release tag to watch it
with. A PR that changes what a match computes (an exe patch of the
game's rules, a kit or a respawn the game computes otherwise, ...) cannot
know its revision: it adds the row `next<TAB><what, the PR>` at the end of
that list. The release that ships it is that revision without anyone
typing it: its server takes `next` for its `SRP_MIN_PATCH_REVISION` (the
release's own, set at the swap), and its record writes the number in place
of `next`. The preflight says which (`replays: this release changes what a
match computes (…)` or `replays: no change … is marked`), refuses `next`
with `--server-code no` (and `--server-code no` when the running server was
built with `next`: made anew with this release's revision, it would take
that for it), and notes when the exe's patches changed since the
last tag with nothing marked: **read that line to the owner**; if the
change does alter a match, stop and have the row added by a PR first.

The test environment's revision is its own exe's + 1, which can be another
number than live's next: a patch tried there is named for the test's
(a copy of the file under that name), then for live's.

Then, first on the test environment, then live:

```sh
release/release.sh --test --dry-run [--resources <path>/resources.db.patch_<rev>]
release/release.sh --test            [--resources ...]   # then play it there
release/release.sh --dry-run         [--resources ...]
release/release.sh                   [--resources ...]
```

Show the owner each dry run's output (every read-only check runs; every
other command is printed; nothing is planned or written). Then the same
without `--dry-run`: after its preflight it waits for **the go**, and the
go is the owner's to give (below); before the swap it asks one y/N
(`--yes --i-know` answers it, never the go).

## What is offered: release/patches.tsv

The patches players get are an explicit, committed list:
`release/patches.tsv` (live) and `release/patches-test.tsv` (test), one
`<name> <sha256> <size> <urls>` row per patch, tabs between, in revision
order. `<urls>` is the patch's URLs, one space apart, in the order the
players' updater tries them: the patches server's (`$SKHID_PATCHES_URL/<name>`,
always first), then mirrors; 1 to 8, none twice. The server data is built
of exactly those rows and `/patches` offers exactly those; a file on the
patches server that is not listed is offered to no one.
`release/upload-patch` (and `release.sh --resources`) adds the row when it
publishes: **commit the list with the release's record**. A row's patch is
never changed (a published patch is never replaced; a fix is the next
revision); only its mirrors may be. The preflight checks every row against
the patches server (size, sha256) and every URL of it (below).

**Mirrors.** `release/add-mirror [--test] <resources.db.patch_<n>> <url>`
downloads the file from `<url>` as the updater does (up to 5 redirects,
http and https only), checks it is not a web page and has the row's size
and sha256, and appends the URL to the row; commit the list. Players get a
mirror with the next release (the server data names the URLs; srp's
`/patches` offers them). A mirror is removed by hand (the first URL stays).
For **Google Drive**, share the file with "anyone with the link" and add
`https://drive.usercontent.google.com/download?id=<id>&export=download&confirm=t`
(the id of the link `.../file/d/<id>/view`): the `drive.google.com/uc`
form answers a large file with a "cannot scan for viruses" HTML page,
which the updater and the scripts refuse. The updater tries the URLs in
order and moves on at any failure (no connection, another status than 200,
a web page, cut short, another sha256); PLAY stays off only when every URL
failed. There is no download resume: a broken download is fetched again
whole.

The checks of every URL (release.sh's preflight, build-server-db for the
mirrors, add-mirror) download it whole once and keep the result per (URL,
sha256) in `~/.cache/skhid-release/patch-urls.checked` (`$XDG_CACHE_HOME`
if set): later runs ask such a URL for its first bytes only (200 or 206,
not a web page, the row's size when the answer gives the whole file's
size, the archive's `little-endian` header). A mirror whose file was
replaced since by one of the same size is not seen so (the updater's
sha256 check still refuses it, and moves on to the next URL). Delete a
line, or the file, to have a URL downloaded whole again.

## release/release.sh

```
release/release.sh [--resources <resources.db.patch_<rev>>] [--test]
                   [--revision <n>] [--server-code auto|yes|no]
                   [--minutes <n> | --no-maintenance] [--now]
                   [--match-wait <minutes>] [--label <word>] [--ref <branch>]
                   [--dry-run] [--yes --i-know]
```

After the preflight it waits for the owner's go; at the go it plans the
maintenance at once, to start **30 seconds after the last running match
ends**, so Play locks and no new match starts; **during the countdown** it
prepares and stages everything, and at the later of the planned time and
the end of the staging, once no match runs, swaps it all in at once (the
downtime is the restart). The countdown hides the build: players wait for
the matches that run, or for the build when it takes longer.

The order, as it runs:

1. **Preflight**, nothing players see. It says exactly what's missing: ssh
   to the jump host, through it to the live host, the patches server; the
   release lock free; nix; `$SKHID_GAME`; `$SKHID_STOCK_DB` with the stock
   sha256 (cached by size and time); free disk here and there;
   srp-game-data builds; the release's revision `<n>` (the target exe's + 1,
   or `--revision <n>` above it; not below main's `REVISION`); live: main's
   `REVISION` (the last release's) is the live exe's or above and below
   `<n>` (below the live exe's: the last record's PR is not merged; `<n>` or
   above: that revision went out before and was rolled back), the tag
   `026g-rev<n>` and the branch `release/record-rev<n>` are not on origin,
   git has a user here (`user.name`, `user.email`: the tag and the commit
   are made as that user), and `gh` is signed in; the
   admin page answers the jump host (not with `--no-maintenance` or `--now`); no
   maintenance planned already; no
   release folder left unfinished; `download/` on the file system of the
   target's folder; `download/resources/` keeps the original menus' blobs
   (main_menu `0ff118a8…`, lobby_menu `ab6f22d8…`, login_menu `805ea5f8…`:
   the updater's one-time restore to the stock resources.db fetches them);
   srp26's container and its image ID, which must be what `srp-026g:latest`
   names (else a restart would run an image no release made); the list's
   rows on the patches server, and every URL of every row giving the file
   (as the updater fetches it, not a web page, the size and sha256; whole
   the first time, then from the cache above); patch_12 offered from
   revision 12 on; with
   `--resources`: the name is `resources.db.patch_<n>` (the release's revision),
   srp-game-data reads it over the stock db, it is neither published nor
   listed yet. Live: the website's pages made from the game data (the
   Maps tab, the calculator): when main's `built-from.tsv` names other
   patches than the list (and the `--resources` file), it rebuilds both
   with `release/build-site` in a worktree of the commit (about 2 min; the
   patches cached in `~/.cache/skhid-release/patches/`, FFDec 26.0.0
   downloaded there, dbgrab built there from the commit's `tools/re-kit` with the
   unpacker checked out as `tools/vostok-resources-db` in the main checkout,
   or `$SKHID_DBGRAB`). Nobody rebuilds them by hand before a release. The
   rebuild comes after the other checks and is skipped when one of them
   failed. A failed rebuild is a problem like any other: the run stops before the go.
   The server's source carries the rebuilt pages, so the server code is
   built (`--server-code no` is refused then, by the dry run too). Last, which server-code
   files differ, which settings
   main's compose file newly passes that the target's `.env` sets (names
   only: their values take effect at the recreate), and whether main
   changes what a match computes (a `next` row of `simulation_changes.tsv`,
   above: then the image must be built; with none, a note when the exe's
   patches changed since the last tag).
2. **The lock, then the go.** Right after the preflight it stops and
   waits: y at its prompt (n stops), or `touch` the go file it prints
   (`~/.cache/skhid-release/go-<release folder>`); `--yes` does not give
   it. **The go is the owner's**: it is their word that players may feel
   the release from now on, and the maintenance is planned the moment it
   is given. Ask the owner, and give it only when they say so
   (`SKHID_RELEASE_GO=1` gives it beforehand, for the tests). Nothing is
   planned, built or staged before it; a Ctrl-C there only frees the lock.
3. **The maintenance, at once after the go**: a POST to the admin page
   (`tab=maint`, `minutes`) from the jump host, whose address must be in
   the target's `SRP_ADMIN_IPS` (behind Caddy with
   `SRP_WEB_TRUST_PROXY=on`). The admin path and token are read from the
   target's `.env` into variables and handed to curl as its config on
   stdin: never printed, in no command line. It checks the countdown on the
   launcher page and prints the planned time. The minutes (the admin page
   takes whole minutes, 1 to 10080):
   - `ceil((last_match_ends_in + 30) / 60)`, at least 1, where
     `last_match_ends_in` is what `/api/stats` says: the seconds until the
     last running match ends at the latest (its time left; a match still
     loading counts its whole wait, countdown and 15 min), 0 when none
     runs. A match ending in 5 min (300 s, or 299): 6; none: 1.
   - An srp from before that field (revision 15 and older): 1 with
     `"matches":0`; else the most "Time left" (`≤ <m> min`) of the launcher
     page's matches + 1 (30 s, rounded up), when the page gives one for
     every running match (≤ 9 min: 10); else a whole match and 30 s, 16.
     The log says which.
   - `--minutes <n>`: `<n>`, planned at the same point (right after the go).
   - `--no-maintenance` or `--now`: nothing is planned (below).
4. **Prepared and staged during the countdown**: `upload-patch` (with `--resources`; it adds
   the row), `build-server-db` (of the list), `build-zip` (of the commit
   checked in step 1); the checks of `deploy` on both files; then the
   **release folder** on the target (below) with both files staged in it
   (sha256 checked there, `chmod 644`) and `changed.txt`. With server
   code: the running image tagged `srp-026g:before-rev<rev>-<stamp>` by
   its ID, main's source (with the rebuilt pages) unpacked in the folder's `src/` and
   `docker compose build srp26` run on it (the running container is not
   touched). The local builds run at nice 10.
5. **The swap, at the later of the planned time and the end of the
   staging**: staged before the planned time, it waits for it; staged
   after it, it goes on at once. Then, once the launcher page says the
   server is down and `/api/stats` says `"matches":0` (it waits up to `--match-wait`,
   default `$SKHID_MATCH_WAIT_MINUTES`, else 15, more): it shows what changes,
   **y/N** (no answer in `$SKHID_SWAP_ANSWER_MINUTES`, default 10, is a
   no), then: notes srp's newest backup of srp.db, stops srp26, keeps the
   old files in the folder, puts the folder's rollback in, puts the server
   data in place (a hard link renamed over the old file: atomic), sets
   `SRP_MIN_PATCH_REVISION=<rev>` (that line only; it must be at least the
   highest patch, or srp refuses to start) and runs
   `docker compose up -d --no-deps --force-recreate srp26` (after any
   `.env` change the container is made anew, never just restarted).
6. **Checks, then the ZIP**: the container running, healthy, not
   restarting; the log's `game data:` line; `/patches` exactly the list's.
   Then srp.db: at its start srp copies it with `VACUUM INTO` to
   `backups/srp-<unix>.db` before it opens it, so that copy is the state
   just before the release; it is copied (`docker exec ... cat`) into the
   folder and, from live, to the jump host (`$SKHID_BACKUP_DIR`, default
   `/srv/skhid-backups/srp-db`: folder 700, files 600, a line in
   `SHA256SUMS` there), its sha256 compared on both ends. Only then the
   ZIP goes in place and `/api/client-files` must name it.
7. **The record, live only, once it all went out** (never after a stop or
   a rollback: those leave no commit and no tag; `--test` makes neither):
   - an annotated tag `026g-rev<n>` on the commit that was built (main as
     the preflight fetched it; its `REVISION` is still the one before, and
     `--revision=<n>` makes the exe again), its message the UTC time, the
     ZIP's, the server data's and the exe's sha256, pushed;
   - the commit `Release revision <n>` on that same commit, as the git
     user: `REVISION = <n>`, the revision table's row (after the highest
     one: the UTC time, the tag, the commit, the exe's sha256 without and
     with the server, the ZIP's size and sha256, the server data's, the
     content patch, and what it shipped: the merged PRs since the tag
     before, the record PRs left out), a `CHANGELOG-SKHID-026G.md` entry
     under the rule, `TODO` for the owner to word, with those PRs, and
     `release/patches.tsv` as the server offers it (the row
     `upload-patch` added), the Maps tab and the calculator when
     step 1 rebuilt them (so main's pages match its list, as the web-server
     test wants), and `<n>` in place of a `next` row of
     `simulation_changes.tsv` (said in the commit); pushed as
     `release/record-rev<n>`, a PR opened
     (`gh pr create`) and merged as a merge commit (`gh pr merge --merge`).
   A step gh or git cannot do is said at the end, loudly (`RELEASED, BUT
   THE RECORD IS NOT COMPLETE`), with what to do, and the run ends with 1:
   players have the release. The usual one: a rule of the repository (a
   review required) lets the token open the PR but not merge it; the PR
   stays open, and the next release's preflight refuses until it is
   merged (a merge commit).

   Why the release records itself after the fact, from the built commit,
   and not a bump commit made first and built: what is built is exactly
   main as reviewed, no commit nobody reviewed; a release that stops or is
   rolled back leaves nothing to undo on main; and the tag names a commit
   that is on main. The cost: the tagged commit's `REVISION` is the one
   before, so the tag's message says the `--revision` it was built with.

A **failed check** (srp refusing to start, `/patches` wrong, no backup of
srp.db, the ZIP not handed out) prints srp's last log lines and the lines
where it says why — `srp: the game's files: <data> lacks what srp's code
names`, `srp: SRP_MIN_PATCH_REVISION is …`, `srp: cannot load the 0.26g
accounts: … lacks …` — and runs the folder's rollback at once, then checks
srp26 is up and `/patches` is as before.

A **stop after the go and before the swap** (a failure, a no, Ctrl-C,
during the countdown's build too) cancels the maintenance when it was
planned, removes the staged files (the folder stays, state `stopped`, with
its release.log) and puts the image tag back: the server is as it was (a
published patch stays with its row; the next run goes without
`--resources`). A stop before the go asks the admin page nothing and only
frees the lock. A stop in the swap before anything moved starts srp26
again as it was.

`--yes --i-know` (both or neither) answers the y/N before the swap; only
for a run the owner watches anyway. It never gives the go.

Every wait (the go, the y/N, the wait for the planned time, for
`"matches":0`) answers Ctrl-C within a second or two: its sleeps are
`sleep & wait`, which a trap interrupts.

### A quick release: --no-maintenance

When the owner doesn't want a countdown (a small fix, few players on),
`--no-maintenance` skips it: no maintenance is planned and the admin page is
never asked (the preflight doesn't check it, nor `SRP_ADMIN_*` and
`SRP_WEB_TRUST_PROXY`). After the lock and the go, step 4 (publish
the patch, build the server data and the ZIP, stage them, build the image)
runs at once; then it waits for `"matches":0` on `$TARGET_SITE/api/stats`
(`--match-wait`, default 120 min here; past it, a clean stop: nothing
swapped, the staged files removed, state `stopped`), shows what changes and
asks the y/N (no answer in `$SKHID_SWAP_ANSWER_MINUTES` is a no). Right
before it stops srp26 it asks `/api/stats` once more: a match begun during
the question is waited for (within the same limit), and the stop follows a
`"matches":0` by one ssh. The swap, the checks and the rollback are the same.

What players see: those in a running match lose nothing (the swap waits for
`matches:0`); those in the lobby get a ~30 s "server sync" while srp26 is
made anew and reconnect by themselves; the launcher shows no countdown.
The progress lines show the time since the go instead of the time to
the planned end. `--minutes` and `--no-maintenance` are refused together.

### Right now: --now

`--now` means now: **no maintenance is planned and no notice given**.
After the go it prepares and stages, then swaps at once, **matches running
or not**: they end without results. Right before it stops srp26 it says
how many matches run (`--now: <n> match(es) running: the stop ends them
now, WITHOUT RESULTS`), and its last lines say how many it ended; both are
in the folder's release.log. It still needs the go, still asks the y/N
before the swap, and still rolls back by itself when the new srp doesn't
come up. `--now --no-maintenance` is the same thing (accepted, with a
note); `--now --minutes` is refused; `--match-wait` is ignored. Use it only
when the owner asks for it: every player in a match loses that match.

### The release folder

Every release keeps a folder on its target:
`$TARGET_DIR/release-rev<rev>-<UTC YYYYMMDD-HHMM>[-<label>]/` (live:
`$SKHID_LIVE_DIR`, test: `$SKHID_TEST_DIR`). It holds:

| File | What |
|---|---|
| `state` | `staging`, `swapping`, `released`, `stopped`, `rolled-back` or `rollback-refused` |
| `release.log` | what the run printed (no secret is ever printed) |
| `Survarium-0.26g-srp-files.zip`, `server-game-data.db` | the new files, staged here; the swap hard-links them into `download/` |
| `new.sha`, `old.sha` | sha256sum of the new files, and of `download/`'s before |
| `old-Survarium-0.26g-srp-files.zip`, `old-server-game-data.db` | `download/`'s files before the swap (hard links) |
| `srp-<unix>.db`, `srp-db.name` | srp.db as it was before the release (srp's own start backup) |
| `image.old`, `image.new`, `min.old` | the image IDs before and after, `SRP_MIN_PATCH_REVISION` before |
| `changed.txt` | the server source files that changed, the ZIP's changed entries |
| `src.tar`, `old-src.tgz` | with server code: main's source, the tree's files it replaced |
| `rollback.sh`, `rollback.env` | the rollback (`release/host-rollback.sh`) and what it puts back |

A folder left in `staging` or `swapping` (a run that died) blocks the next
preflight: look at its `release.log`, finish or roll it back, then write
`stopped` in its `state`.

### What to watch

- The preflight: every line `ok`.
- After the go: the plan's line at once, with its minutes and where they
  come from (`/api/stats`, the launcher page, or a whole match), and the
  launcher countdown (none with `--no-maintenance` or `--now`); then the
  builds and the staging, during the countdown; then `5. staged before
  the planned time` (it waits for it) or `5. staged after the planned
  time: the swap at once`.
- The ZIP's entries against the current ZIP (build-zip's list, shown again
  at the y/N before the swap and in `changed.txt`): `survarium_skhid.exe`
  always differs (the revision); `updater_skhid.exe` only if the updater
  changed (players get it at their next launcher start); anything else
  must be explained by a merged change. Unexplained: answer no (that
  cancels cleanly).
- After the swap: the checks, the srp.db copy on both ends; then a game
  updates, downloads the patches, signs in; an old game is refused. Ask
  before starting any game window (Wine: unset `WAYLAND_DISPLAY`, run the
  exe directly).

## Rollback

```sh
release/rollback [--test]                                # the release folders and their state
release/rollback [--test] [--keep-db] release-rev<rev>-<stamp>[-<label>]
```

It runs the folder's `rollback.sh <folder>` on the target (by hand there:
`sh <folder>/rollback.sh <folder> [--keep-db]`, as root): the ZIP and the
server data from before (renamed into place), `SRP_MIN_PATCH_REVISION`
back (or its line removed), the server source back, the image that ran
before tagged `srp-026g:latest`, and srp.db as it was before the release
(the folder's copy; or, when the run did not get to keep one, srp's own
backup in the volume made by the new start; none at all means the new srp
never opened srp.db, and it is kept as it is). Every change to srp.db since
the release is lost; `--keep-db` keeps today's srp.db, only when the
release changed no schema and no player got an item of its new patch. It
makes srp26 anew and waits for it.

When srp refuses the old files (the accounts name items the old data
lacks), the rollback stops srp26 instead of letting it loop, says **roll
forward with a fixed patch** (the next revision), and prints the command
that puts the release back from its folder. The updater deletes local
patches the server's list no longer has; published patches stay on the
patches server.

## The test environment

`--test` does everything the live release does, against the test
environment: `$SKHID_TEST` in `$SKHID_TEST_DIR` (compose project
`srptest`, `$SKHID_TEST_COMPOSE` to change it), its site and game server
(`build-zip --test` makes a ZIP for them, compared with the test site's),
its own patches server and list (`$SKHID_TEST_PATCHES_SSH/_DIR/_URL`,
`release/patches-test.tsv`; the scripts refuse settings that are the
production folder or URL), its own release folders and lock. The admin
page there allows the jump host. srp.db is not copied to the jump host for
it (it is there). The host also has `/opt/srp-test/redeploy` (a branch on
it, built and recreated) and `/opt/srp-test/teardown`; read
`/opt/srp-test/README-test.txt` there.

## The steps it runs

Each is a script of its own (`--help`; it asks y/N before every write and
shows the command, unless release.sh, already given the go-ahead, runs it):

| Script | Does | Writes |
|---|---|---|
| `release/build-patch <folder> <rev> [--out <file>]` | packs a folder into `resources.db.patch_<rev>` (read back and compared) | a local file |
| `release/upload-patch [--test] [--dry-run] <file>` | publishes one patch: `.tmp`, sha256 there, `chmod 644`, `mv -n`, GET must be 200 with the sha256, then its row in the list; never replaces one | the patches server, the list |
| `release/add-mirror [--test] <name> <url>` | another URL of a listed patch: downloaded as the updater does, not a web page, the row's size and sha256, then appended to the row | the list (and the URL cache) |
| `release/build-server-db [--test] [--out <file>]` | `server-game-data.db` from the stock db and the list's patches (each first URL GET 200 with the row's size and sha256; each mirror checked as above), `srp-game-data build --patch "<file>=<urls>"` | a local file |
| `release/build-site --tree <checkout> [--with <patch>]` | the Maps tab and the calculator from the stock db and the list's patches (cached, each sha256 checked), FFDec 26.0.0, dbgrab; both `built-from.tsv` must name exactly those; release.sh runs it in its preflight, by hand it is a local preview (`--tree .`) | the checkout's `servers/web-server/{maps,damage}/`, the cache |
| `release/build-zip [--test] [--ref <commit>] [--revision <n>] [--work <dir>]` | the players' ZIP from a fresh worktree of origin/main (never this checkout, never `git stash`): `--server`, `--site`, `--localization`, `--revision` (default: the site's exe's + 1; above it), no resources.db regions; the exe without a server too, both sha256 in `<work>/exe.sha256`; entries against the site's ZIP | a worktree, the ZIP |
| `release/deploy --zip --server-data --revision [--label] [--dry-run]` | the swap on its own, in a window planned by hand: the release folder, the checks, the rollback on failure | the live host |
| `release/rollback [--test] [--keep-db] [<folder>]` | a release folder's rollback | the target |
| `release/setup-patches-server [--test] [--check]` | once: the patches folder and Caddy's site block for it (validate and reload; either failing puts the backup back) | the jump host's Caddy |

`release/test.sh` runs them all, release.sh and the rollback included,
against stand-ins on this machine (`nix shell nixpkgs#python3
nixpkgs#shellcheck -c release/test.sh`); it sends nothing anywhere.

## How the game gets its content

- The game mounts the stock `game/resources.db`, then every
  `game/resources.db.patch_<n>` that exists, n = 1…255 ascending (gaps are
  normal). A later patch overrides files of earlier ones and adds new files;
  a patch can't delete. `n` is the **revision** the patch was released in.
- The updater asks the game server for `GET /patches`
  (`{"patch_12": {"sha256": "…", "urls": ["…", "…"]}, …}`), downloads the
  missing ones from the first URL that gives each, checks every patch's
  sha256, deletes local patches not in the list, and keeps PLAY off until
  all match. It hashes a patch file only when it changed (its size, time
  or file ID), the list changed for it, or its last hash is a week old;
  `updater_skhid.exe check` hashes them all (the repair).
- Updater 0.3.0 reads `urls`; 0.2.0 reads only `url`. The release that
  ships 0.3.0 ships the ZIP and the server (which lists `urls`) together:
  a 0.2.0 asks for the overlay first, installs 0.3.0, and leaves the patch
  step to it (the launcher restarts it at once); it never reads the new
  list. A rollback past it puts 0.2.0 back the same way (0.2.0 cannot
  read the list 0.3.0 kept beside it, so a start without the server keeps
  PLAY off until 0.2.0 has run once with it).
- The **patches server** is plain file storage: `$SKHID_PATCHES_URL`
  (files in `$SKHID_PATCHES_DIR` on `$SKHID_PATCHES_SSH`). Patch files there
  are never deleted or changed.
- The **game server** runs on `server-game-data.db`: the files it reads,
  built by `srp-game-data build` from the stock `resources.db` plus the
  list's patches with their URLs, on the release PC (the live host never
  stores the stock db). It serves `/patches` from what that file records,
  so it can only advertise the patches it runs on; it refuses to start when
  `SRP_MIN_PATCH_REVISION` is below its highest patch, or when srp.db or
  its code name what the data lacks. Every map, its modes, respawn points,
  battery containers and names come from that file (format 3: the gameplay
  files, each project's `server_objects`, the en/ua texts); srp and the file
  go out together (a format the image does not read stops its start, and
  the rollback puts back both).
- Login, lobby, match and web are one process (container `srp26-srp26-1`):
  a restart ends every match and drops every lobby link; games reconnect by
  themselves. A planned maintenance lives in its memory: a restart ends it.

## Record

The release makes it (step 7): the tag, and the merged commit `Release
revision <n>`. What is left to the owner: the changelog entry's wording
(its `TODO` heading and lines: players' words, "update required"), in a
PR of its own. If the run ended with `THE RECORD IS NOT COMPLETE`, do
what it says (merge the open PR as a merge commit; push the tag) before
the next release. The release folder has the hashes in `new.sha` too.

The token `gh` uses on the release PC must be allowed to push branches and
tags and to open and merge PRs (`repo`, or a fine-grained token with
Contents and Pull requests: read and write); a branch rule that requires a
review keeps the record's PR open until someone merges it.

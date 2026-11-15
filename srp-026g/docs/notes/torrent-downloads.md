# Torrent downloads (spec)

The owner's rules for downloading and sharing the game by BitTorrent. Short on purpose:
corrections from the owner go here.

## What players get

- An optional way to download the game, and to share it: no one has to use it.
- Every download is hash-correct. A wrong byte is never kept, installed or shared.
- The hashes are in the `.torrent` and the magnet link themselves, so the bytes can come from
  anyone; they do not have to come from our servers.
- Two ways to use it:
  - their own client (qBittorrent, Transmission...) with the `.torrent` or the magnet of the
    site's download page;
  - the updater, when "Allow torrent" is ticked: a checkbox right on the launcher's main screen,
    at the left of its bottom panel (the owner, 2026-11-15: "no need for settings"; the
    original settings window, the gear, stays hidden). Off: the updater downloads as now, by
    HTTPS.
- **Off by default** (the owner, 2026-11-15): a player turns it on. **It stays ticked until they
  untick it**, across launcher and PC restarts, the updater replacing the launcher and itself,
  and new releases: it is **stored in user.cfg like the language** (the owner: "just like
  language"), as the game's own saved command `enable_torrent_seeding on` / `off`, written at
  once on a click (launcher patch 15). No line: off. The registry's `download_type` is not used.
- **The one-time reset**: the original game saved its default, `enable_torrent_seeding on`, in
  every player's user.cfg. So the launcher, at its first start in an install, writes the line
  `off` before it shows the box and makes `launcher_skhid.torrent_reset` beside itself; from
  then on the line is what the player's tick says. The updater takes the line only once that
  file is there (`check`); `Skhid-get.bat` says `torrent` (whoever runs it chose BitTorrent).
- Turned on, it is used at once, without a restart; turned off, it stops at once (below).
- The port: no setting. The updater picks a free port the first time it listens and keeps it
  (`updater_skhid.port` beside it), the same at every start after; if another program holds it,
  it takes another and keeps that. A port the launcher names (its 13th argument, then `21`)
  still wins; the Skhid launcher names none.
- The firewall: `Skhid-network-setup.cmd` lets `updater_skhid.exe` in too (TCP, every port: the
  port is the updater's pick), so a player who ran it gets no firewall question when they
  turn torrent on; `--undo` removes that rule with the others.

## The torrent

- One torrent per released revision: `skhid-rev<n>.torrent`, built by the release
  (`release.sh`), never by a feature PR (`AGENTS.md`, revisions).
- It holds the whole game folder, ready to play: the original game's files the game needs, the
  Skhid files of that revision's ZIP, and every `resources.db.patch_<n>` the revision lists. Same
  layout as a player's folder, so a client pointed at an existing install checks it and fetches
  only what differs.
- Nothing the game or the updater writes (settings, logs, `updater_skhid.state`, caches,
  replays) is in it.
- Hybrid v1 + v2 (BEP 52): every file piece-aligned, SHA-256 piece hashes. Files that did not
  change between revisions keep the same v2 hashes, so their seeders keep sharing them.
- Trackers: a few public ones, so the torrent works without our servers. No web seed for now.
- It travels inside that revision's ZIP, as `game/binaries/x86/skhid.torrent` beside the
  updater (the torrent itself leaves that one file out: it cannot hold its own hash). The ZIP is
  checked (its SHA-256 in `/api/client-files`), so the updater has the revision's `.torrent` with
  it. A ZIP without the file (an older one, `release/deploy`'s) means no torrent: HTTPS.
- The site's download page shows the `.torrent` and the magnet (v1 and v2 hashes, name,
  trackers) next to the ZIP, for people's own clients: the web server hands out the ZIP's file.
- Seeding a new revision: the first players download by HTTPS (there are no peers yet), and every
  player with the launcher's "Allow torrent" on then shares what their updater has checked,
  including the files it got by HTTPS. That is one way a revision gets seeded; the others stay
  open: people seeding with their own client, and a host of ours seeding (the owner's call; none
  is built).

## Bootstrap: the game without our site

- `Skhid-get.bat`: one plain-text script (a `.bat` with PowerShell inside; Windows 10's
  PowerShell 5.1 and later, no administrator rights) that anyone can pass around. Put beside an
  empty place and run, it gets the Skhid updater, its `skhid_torrent.dll` and `7zr.exe`, lays them
  out in a `Survarium` folder beside
  itself (`game/binaries/x86/`), runs the updater (`updater_skhid.exe check torrent`), which
  installs the original game and the Skhid files, and starts `Skhid.bat`.
- It gets the three files by BitTorrent first, from the bootstrap torrent: those three files at
  one version, in the players' layout, hybrid like the revision's (`release/bootstrap.torrent`). It
  announces the torrent's v1 info hash to the public UDP trackers (BEP 15) and fetches from the
  peers with the plain BEP 3 wire: no metadata exchange, the script pins every piece's SHA-1 and
  each file's SHA-256 and size. A piece that is not its pin drops that peer; a message over a block
  and its header drops it too; so does a handshake not in 10 s or no new checked piece in 60 s
  (keep-alives, chokes, trickled bytes count for nothing); 40 peers at most, 3 minutes for them
  all.
- No peer gives them: the files' URLs in their order (`release/bootstrap.tsv`, the form of
  `release/patches.tsv`: Google Drive's first, the patches server's among them, 8 at most), each
  answer checked by its size and SHA-256 (HTTPS by TLS 1.2). Our server is never needed; peers
  come first.
- Who seeds it: updaters with "Allow torrent" on, and anyone with the torrent. The release puts
  `release/bootstrap.torrent` in the players' ZIP too, as `game/binaries/x86/skhid-bootstrap.torrent`
  (left out of the revision's torrent, as `skhid.torrent`); the updater takes it as the checked
  ZIP installed it (its SHA-256 in `updater_skhid.state`; it is not signed, the script pins it),
  keeps a copy of each of its three files in `updater_skhid.bootstrap\` beside itself while the file
  in its place is the bootstrap's (every piece checked), and shares them from there while it
  shares, after it has updated itself too.
- `release/build-bootstrap <players' ZIP>` makes it, once for an updater version: the three
  files from the ZIP (7zr.exe the pinned one), each published on the patches server at
  `bootstrap/<sha256>/<name>`, its row in `release/bootstrap.tsv`, the torrent and the script
  (`srp-026g/servers/web-server/bootstrap/Skhid-get.bat`); the same files make the same bytes.
  More URLs: `release/add-mirror <name> <url>` (checked as for a patch; it makes the script again).
  Commit the three.
- The site serves the script at `GET /Skhid-get.bat` and links it on the home page under the
  ZIP's button, when the build has one.

## The updater

`client/updater/README.md` ("BitTorrent") has the details; `client/updater/exe/src/torrent.rs`
(the session's owner) and `client/updater/exe/src/libtorrent.rs` (the DLL's) are the code.

- **The engine is libtorrent** (the owner, 2026-11-15): libtorrent-rasterbar 2.0.12 in
  `skhid_torrent.dll` beside the updater (`client/updater/torrent-dll`: a small C API, built from
  the repo by nix, pinned and reproducible), a file of the players' ZIP and so of the torrent.
  The updater loads it at run time; no DLL, one that does not load (Windows before 10: Windows 10
  is the requirement) or lacks a function: no torrent, HTTPS as without it.
- It answers the launcher's torrent messages instead of ignoring them: "Allow torrent" (its 4th
  argument, then `1A`), port (its 13th argument, then `21`, answered port ok `15` or busy `14`;
  none named: the kept one, above),
  upload limit (`1E`, which may only lower the cap below; the 0.26g launcher sends none), and
  shows the launcher's states (`08` checking the pieces already on disk, `09` downloading;
  `srp-026g/docs/notes/launcher_updater.md`).
- `1A` on takes effect at once: while it watches (PLAY shown) it starts sharing; while a
  download by HTTPS runs (the original game's archive, a patch) and the torrent has that file,
  the rest of the download moves to the torrent, as the original updater's `restart_downloading`
  did: what HTTPS got stays (a patch's part is the torrent's part, its pieces checked first; the
  archive's bytes stay for HTTPS to go on from if the torrent gives nothing). `1A` off stops the
  engine at once: sharing ends, a download by torrent goes on by HTTPS.
- The torrent is trusted by its signature alone: the release's Ed25519 key is built into the
  updater ("Signed releases"), and a torrent whose signature does not verify, or whose revision
  is lower than the highest it has taken (`updater_skhid.torrent.revision` beside it), than the
  installed game's or than the updater's own, is never taken. Where it came from does not
  matter: the ZIP's `game/binaries/x86/skhid.torrent` beside the updater, or the newest one the
  DHT named (below). One function decides it (`torrent::trusted`).
- No torrent (an older ZIP, one released without it), or none it takes: it downloads by HTTPS
  exactly as today, switch on or off. Torrent is only ever an extra source, never a
  requirement.
- With torrent on, it gets what it would otherwise download from the torrent: the original
  game's files (in place of the archive from the mirrors; the Skhid files come first then, as
  they bring the torrent) and missing or damaged patches. The Skhid ZIP stays HTTPS.
- Every piece is checked by libtorrent against the torrent's hashes when it is added (what the
  folder has) or downloaded, before it counts or is written; a piece is not checked again before
  each upload. Each finished file is also checked against the pins the updater has (the original
  files' build-2777 SHA-256, `logic/src/game.rs` `FILES`; the patches' SHA-256 in `/patches`)
  before it is installed, and is fetched into a part beside its place, renamed only after that.
- **What was checked is not hashed again**: the updater keeps beside itself
  (`updater_skhid.torrent.checked`) each file libtorrent found whole, with what Windows said of
  it then (size, last-write time, file ID). A file still so at a later start is shared as checked
  (15.5 GB are not read again at every start); any other is checked again (libtorrent hashes from
  its first piece on). A wrong file is still caught by its pin before it is installed.
- Players are the seeders (no host of ours seeds): with torrent on it shares every file of the
  torrent the folder has, however it came (the torrent, HTTPS, the ZIP), so the first players of
  a revision, who get it by HTTPS, seed it. Only checked pieces, only while it runs (while the
  launcher is open, or the game shares: below), within the upload cap, on the kept port.
- **The upload cap** (the owner, 2026-11-15): 1 MB/s (1 MiB a second) for all the sharing of an
  updater's run: the libtorrent session's upload limit, every peer's (those of the local
  network too). The launcher's `1E` may only lower it; the game's own limit is ignored.
- If the torrent gives nothing (no peer in 20 s, no piece for 60 s), it downloads by HTTPS as
  now. PLAY never waits on the torrent.
- Peers from the updater's own trackers (the release's list, "Top level"; never a torrent's:
  they are outside the signature, and whoever hands a torrent over could name its own trackers
  and be the only one to give peers; its web seeds neither) and from the DHT (on). No LSD, PEX,
  UPnP, NAT-PMP or uTP; of libtorrent's extensions only `smart_ban` and `ut_metadata`; its own
  connection limits. A torrent is kept and passed on as its `info`, `piece layers` and `skhid`
  only.
- A piece that fails its hash: the peer that sent it all, or the one `smart_ban` finds lied, is
  banned for the updater's run (every session of it).

## Sharing from the game

The owner (2026-11-15): with "Allow torrent" on, the game shares too, but never in a match: on the
login screen and in the lobby, where the original developers had it (the game's own
`start_seeding`, which never worked: it asked a Windows service).

- "Allow torrent" is the game's own setting `enable_torrent_seeding`, a line of its `user.cfg`
  as the language is (the launcher's box writes it); off without the line (the game's default
  was on: exe patch 150 makes it off). The game's menu never shows it, nor its upload rates (the
  game's limit is ignored, below). The game writes every setting into user.cfg at its end, so
  every player who played before has `enable_torrent_seeding on` there, from the old default:
  the launcher's one-time reset (above) writes it `off` at its first start in an install. A
  game started without the launcher having run once (`play_skhid.bat`) still sees that `on`.
- Entering the login screen or the lobby with it on (or as it is turned on there), the game makes the pipe
  `\\.\pipe\skhid_seed-<its process id>` and starts `updater_skhid.exe seed <that pipe>` beside
  itself, with no window. Entering a match closes the pipe (so do the game's end and the setting
  turned off); a replay never shares.
- The updater (`seed`) shares the folder's revision torrent as the launcher's updater does
  while it watches (the same trust, the bootstrap, the DHT item; nothing fetched or installed),
  on the kept port (a free one, then kept, when none is), within the cap, until the pipe closes. A port another updater holds
  is tried for 10 s, then it ends. No box, no lock; its own log (`updater_skhid.seed.log`).
- On the pipe the game sends its limit (`1E`, ignored) and its stats switch (`22`: the console's
  `lobby_torrent_stats`); while that is on, the updater sends a tick a second (`18`: `FF`, the
  bytes a second sent, the cap, the peers connected), and nothing else ever: the game reads
  every message as that tick.

## The newest release

How an updater finds a newer release without our site: the release's signed item on the DHT
(BEP 46; `client/updater/logic/src/newest.rs`).

- The item: a mutable BEP 44 item under the release's Ed25519 key (the torrents' key), salt
  `skhid-newest 1`, sequence number the revision, value `d1:i32:<v2 info hash>1:ri<revision>e1:s64:<the
  torrent's signature>e`: the facts of the torrent's own signature. The release signs it with
  the torrent (`skhid.dht`, "The signature") and puts it (`release/publish-dht`, from
  `release.sh` once the release went out; the release's own libtorrent, `skhid-dht`).
- Every updater that shares puts the same item again (its torrent's `skhid.dht`: no private key
  anywhere but the release's), at once and every 30 minutes: the DHT forgets an item in hours.
  A newer release's item replaces it (a higher sequence number); the nodes refuse an older one.
- An updater that cannot reach the update server (a blocked network, our servers down, a bare
  folder) asks the DHT for the item for 20 s. It takes the item only when its signature is the
  release's and its revision is higher than the torrent it has (or, having none, not lower than
  what the folder has: the installed game's, the highest taken, the updater's own). Then it
  fetches that torrent's `info` from its swarm (BEP 9, `ut_metadata`; the swarm's peers from the
  trackers and the DHT; 60 s at most), checks its SHA-256 is the item's info hash, and makes the
  signed torrent again (`info` and `skhid`; no piece layers, libtorrent gets them from peers),
  kept beside itself as `updater_skhid.found.torrent`. It is trusted as any torrent is, by its
  signature; the highest revision of it and the ZIP's is the torrent of the run.
- No server of ours needed: when the update server cannot be asked and the run's torrent is
  newer than what is installed (or nothing is: `Skhid-get.bat`'s bare folder, whose
  `updater_skhid.exe check torrent`),
  the updater installs every file of it from it: the Skhid files (all or none, the torrent's info
  hash recorded as the installed archive), the original game, the patches. A patch known only
  from a verified torrent is kept in the patch list with no addresses (`"urls": []`, its SHA-256
  from the checked file) and gets the `/patches` addresses when the site answers again.

## Signed releases

- Trust comes from our signature, not from our site: every revision's `.torrent` is signed by
  the release with one Ed25519 key, and the updater takes only a torrent that verifies with the
  public key it is built with.
- What is signed: a fixed domain string, the revision and the SHA-256 of the torrent's `info`
  (so every file's hashes); the signature sits outside `info`, so the info hashes, the swarm and
  the magnet do not change with it ("The format").
- The key: the private key only in `private/release/torrent-signing.pem` beside `release.env`
  (mode 600, never committed, never printed, never on a host); its public key committed as
  `release/signing.pub` (64 hex digits), which the updater build reads. `release/make-signing-key`
  makes both, once, by the owner (it never replaces a key). Without the key, or with a public key
  it does not verify with, the release builds no torrent and stops (the preflight compares the
  key's public key with the commit's). The players' updater carries that public key: `build-zip`
  checks it is in the ZIP's updater.
- The test environment has its own pair (`torrent-signing-test.pem`, `release/signing-test.pub`;
  `release/make-signing-key --test`): `release.sh --test` signs with it and its updater is built
  with it (`SKHID_TEST_PUBLIC_KEY`), so a test torrent is never one a player's updater takes.
- No rollback: an updater never takes a torrent of a lower revision than the highest it has
  taken, nor than its own. And a revision's number never names other files: once srp came up on
  a release's files (its folder's `served`), a later release of that number must build the same
  ZIP, or it stops after its build (`--revision` the next).

## The format (fixed; the updater reads exactly this)

Built by `release/build-torrent` (`release/build_torrent.py`, Python's standard library only),
from `release.sh` step 2 after `build-zip`. The same inputs make the same bytes.

**Contents.** `info.name` is `Survarium`: the folder a player unpacks the ZIP into (the 7z's own
top folder). In it, at their paths (`/` between parts):

- the original game's five files the updater extracts from the pinned archive
  (`client/updater/logic/src/game.rs` `FILES`), under `game/`; `game/resources.db` is
  `$SKHID_STOCK_DB`, the other four come from `$SKHID_GAME`; each is read against the sha256 of
  build 2777 (`release/build-torrent` `GAME_FILES`, `release/lib.sh` `STOCK_DB_SHA256`):

  | path | size | sha256 |
  |---|---|---|
  | `game/binaries/x86/survarium.exe` | 12894840 | `ac33d993…3bf153` (`game.rs` `ORIGINAL_EXE`) |
  | `game/binaries/x86/bugtrap.dll` | 278648 | `5bb81aea…9a8aac` (the ZIP's replaces it) |
  | `game/resources/ssl/survarium_login_server.crt` | 757 | `f29e488d…8c7b68` (the ZIP's replaces it) |
  | `game/resources/sources/splash_screens/development_engine.bmp` | 536056 | `f81222a3…aac00f` |
  | `game/resources.db` | 6285533906 | `c74078be…75492c` (stock) |

- every file of that revision's players' ZIP at its ZIP path (`game/binaries/x86/…`, `Skhid.bat`,
  `README.txt`...), over an original of the same path; but `game/binaries/x86/skhid.torrent`,
  which is the torrent itself;
- every patch of the offered-patch list (`release/patches.tsv`) as `game/resources.db.patch_<n>`,
  read against its row's sha256.

No empty file, nothing else (no state, logs, settings). A trial build (a ZIP of 7 November,
41 patches): 77 files, 15.5 GB, 3747 pieces, a 212 KB `.torrent`.

**Hashes.** Hybrid v1 + v2 (BEP 52, `meta version` 2), `piece length` 4194304 (4 MiB). The v2
`file tree` gives each file `length` and `pieces root` (SHA-256 Merkle tree of 16 KiB blocks,
zero leaves to a power of two); `piece layers` holds the layer of each file over 4 MiB (keyed by
its root). The v1 `files` list is the same files in the same order (by path, part by part, bytes
compared: the order of the v2 tree), each followed by a pad file `{"attr": "p", "length": <n>,
"path": [".pad", "<n>"]}` that fills its last piece, except the last file: every file starts a
piece, and a v1 piece holds one file (and zeros). `pieces` is the SHA-1 of each such piece.

**Top level.** `announce` (the first tracker), `announce-list` (one tracker a tier, in this
order), `info`, `piece layers`, `skhid` (the signature, below); no `creation date`, `created by`, `comment` or `url-list` (no web
seed). Trackers (`build_torrent.py` `TRACKERS`):

```text
udp://tracker.opentrackr.org:1337/announce
http://tracker.opentrackr.org:1337/announce
udp://open.stealth.si:80/announce
udp://tracker.torrent.eu.org:451/announce
udp://exodus.desync.com:6969/announce
```

The second is the one plain-HTTP tracker (BEP 3 announce); the others are UDP (BEP 15).

**The info hashes** are of the bencoded `info` exactly as the file has it: v1 (`btih`) its SHA-1,
v2 (`btmh`) its SHA-256 (multihash `1220<64 hex>`).

**The signature.** The top-level `skhid` is a dictionary of three keys: `dht` (a 64-byte string:
the BEP 44 signature of the release's DHT item, "The newest release": of
`4:salt14:skhid-newest 13:seqi<revision>e1:v<the item's value>`), `revision` (an integer,
1 to 255: the release's) and `signature` (a 64-byte string): the Ed25519 signature (RFC 8032,
pure Ed25519, no prehash; `openssl pkeyutl -sign -rawin`) of these 59 bytes:

```text
"skhid torrent signature 1" 00     26 bytes: the ASCII text and a zero byte
<revision>                          1 byte
SHA-256 of the bencoded info        32 bytes, exactly as the file has info
```

The public key is `release/signing.pub`: one line, the 32-byte Ed25519 public key in lower-case
hex (64 digits). Ed25519 is deterministic, so the same inputs still make the same bytes.

**In the ZIP.** The release (`release.sh` step 2) builds the ZIP (`build-zip`), then the torrent
over its files, then appends the torrent to it as `game/binaries/x86/skhid.torrent`, stored (not
compressed), mode 644; that ZIP is the one shipped, and its SHA-256 is the one `/api/client-files`
names (`sha256`; the format there is unchanged: no torrent line). The tag of the release names the
info hash.

**`GET /torrent`** (and `HEAD`; srp's web server, `servers/web-server/src/torrent.rs`): that
entry of the ZIP at `/download`, `Content-Type: application/x-bittorrent`,
`Content-Disposition: attachment; filename="skhid-rev<n>.torrent"` (n: the server's
`SRP_MIN_PATCH_REVISION`, the release's). "Not found" when the ZIP has none, or it is compressed,
over 4 MiB or not one whole torrent (one bencoded dictionary with a text `info.name`).

**The bootstrap torrent** (`release/bootstrap.torrent`): `info.name` `Survarium`, three files,
`game/binaries/x86/7zr.exe`, `game/binaries/x86/skhid_torrent.dll` then
`game/binaries/x86/updater_skhid.exe`, the same hybrid layout and
piece length (4 MiB) as above, the same trackers, no `skhid` signature (the script pins it). The
script's pins, written by `build_torrent.py bootstrap`: `$InfoHash` (v1, 40 hex), `$PieceLength`,
`$Trackers` (the UDP ones), `$Files` (each `Path`, `Size`, `Sha256`, `Pieces`: SHA-1 of each v1
piece, its last one with the pad's zeros but the last file's, and `Urls`).

**The site's home page** shows under the ZIP's button: "Or the whole game by BitTorrent:
.torrent · magnet", the magnet being
`magnet:?xt=urn:btih:<40 hex>&xt=urn:btmh:1220<64 hex>&dn=Survarium&tr=<tracker>...` (each value
percent-encoded).

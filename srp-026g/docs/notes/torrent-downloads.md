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
  - the updater, when the launcher's own "Allow torrent" switch is on (the original launcher's
    setting, with its port). Off: the updater downloads as now, by HTTPS.

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
- The site's download page shows the `.torrent` and the magnet (v1 and v2 hashes, name,
  trackers) next to the ZIP. The web server serves the current `.torrent`, and
  `/api/client-files` names its info hash (over HTTPS, so the updater can check it).
- Seeding a new revision: the first players download by HTTPS (there are no peers yet), and every
  player with the launcher's "Allow torrent" on then shares what their updater has checked,
  including the files it got by HTTPS. That is one way a revision gets seeded; the others stay
  open: people seeding with their own client, and a host of ours seeding (the owner's call; none
  is built).

## The updater

- It answers the launcher's torrent messages instead of ignoring them: download type (`1A`),
  port (`21`), upload limit (`1E`), and reports port ok / busy and the torrent states the
  launcher already shows (`srp-026g/docs/notes/launcher_updater.md`).
- With torrent on, it gets what it would otherwise download from this revision's torrent: the
  original game's files (in place of the archive from Google Drive) and missing or damaged
  patches. The Skhid ZIP stays HTTPS.
- It takes the `.torrent` only if its info hash is the one `/api/client-files` names.
- Every piece is checked against its SHA-256 before it counts. Each finished file is also checked
  against the pins the updater already has (the game exe, the patches' SHA-256 in `/patches`)
  before it is installed.
- It shares only pieces it has checked, and only while it runs (while the launcher is open),
  within the launcher's upload limit, on the launcher's port.
- If the torrent gives nothing (no peers, port busy), it downloads by HTTPS as now. PLAY never
  waits on the torrent.
- First version: peers from the torrent's trackers only. No DHT, PEX, UPnP or uTP.
- Peers are strangers: everything they send is parsed once, with fixed limits, and refused
  otherwise.

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
  `README.txt`...), over an original of the same path;
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
order), `info`, `piece layers`; no `creation date`, `created by`, `comment` or `url-list` (no web
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

**On the host.** The release stages it in its release folder as `skhid.torrent` and, at the swap,
puts it beside the ZIP: `download/skhid.torrent` (with the ZIP; kept and put back by the
rollback). `release/deploy`, which ships a ZIP without one, takes it away.

**`GET /torrent`** (and `HEAD`; srp's web server, `servers/web-server/src/torrent.rs`): that
file, `Content-Type: application/x-bittorrent`, `Content-Disposition: attachment;
filename="skhid-rev<n>.torrent"` (n: the server's `SRP_MIN_PATCH_REVISION`, the release's).
"Not found" when there is none, it is over 4 MiB, or it is not one whole torrent (one bencoded
dictionary with a text `info.name`).

**`GET /api/client-files`** gains one line after the others when there is a torrent:

```text
skhid-client-files 1
sha256 <64 hex>        the ZIP
size <bytes>
min-revision <0-255>
torrent <64 hex>       SHA-256 of the torrent's bencoded info: its v2 info hash (btmh without 1220)
```

The format stays `skhid-client-files 1`: updaters before it skip a word they do not know
(`client/updater/logic/src/manifest.rs`). No `torrent` line: no torrent now (download by
HTTPS). The updater takes `/torrent`'s bytes only when the SHA-256 of their `info` is this value
(and the v1 hash, for v1 peers, is the SHA-1 of the same bytes).

**The site's home page** shows under the ZIP's button: "Or the whole game by BitTorrent:
.torrent · magnet", the magnet being
`magnet:?xt=urn:btih:<40 hex>&xt=urn:btmh:1220<64 hex>&dn=Survarium&tr=<tracker>...` (each value
percent-encoded).

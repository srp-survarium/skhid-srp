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

- `Skhid-get.bat`: one plain-text script (a `.bat` with PowerShell inside; Windows 7's PowerShell 2
  and later, no administrator rights) that anyone can pass around. Put beside an empty place and
  run, it gets the Skhid updater and `7zr.exe`, lays them out in a `Survarium` folder beside
  itself (`game/binaries/x86/`), runs the updater (`updater_skhid.exe check`), which installs the
  original game and the Skhid files, and starts `Skhid.bat`.
- It gets the two files by BitTorrent first, from the bootstrap torrent: those two files at one
  version, in the players' layout, hybrid like the revision's (`release/bootstrap.torrent`). It
  announces the torrent's v1 info hash to the public UDP trackers (BEP 15) and fetches from the
  peers with the plain BEP 3 wire: no metadata exchange, the script pins every piece's SHA-1 and
  each file's SHA-256 and size. A piece that is not its pin drops that peer; a message over a block
  and its header drops it too; 40 peers at most.
- No peer gives them: the files' URLs in their order (`release/bootstrap.tsv`, the form of
  `release/patches.tsv`: Google Drive's first, the patches server's among them, 8 at most), each
  answer checked by its size and SHA-256. Our server is never needed; peers come first.
- Who seeds it: updaters with "Allow torrent" on, and anyone with the torrent. The release puts
  `release/bootstrap.torrent` in the players' ZIP too, as `game/binaries/x86/skhid-bootstrap.torrent`
  (left out of the revision's torrent, as `skhid.torrent`); the updater takes it as the checked
  ZIP installed it (its SHA-256 in `updater_skhid.state`; it is not signed, the script pins it),
  keeps a copy of each of its two files in `updater_skhid.bootstrap\` beside itself while the file
  in its place is the bootstrap's (every piece checked), and shares them from there while it
  shares, after it has updated itself too.
- `release/build-bootstrap <players' ZIP>` makes it, once for an updater version: the two files
  from the ZIP (7zr.exe the pinned one), each published on the patches server at
  `bootstrap/<sha256>/<name>`, its row in `release/bootstrap.tsv`, the torrent and the script
  (`srp-026g/servers/web-server/bootstrap/Skhid-get.bat`); the same files make the same bytes.
  More URLs: `release/add-mirror <name> <url>` (checked as for a patch; it makes the script again).
  Commit the three.
- The site serves the script at `GET /Skhid-get.bat` and links it on the home page under the
  ZIP's button, when the build has one.

## The updater

`client/updater/README.md` ("BitTorrent") has the details; `client/updater/exe/src/torrent.rs`
and `client/updater/logic/src/swarm.rs` are the code.

- It answers the launcher's torrent messages instead of ignoring them: "Allow torrent" (its 4th
  argument, then `1A`), port (its 13th argument, then `21`, answered port ok `15` or busy `14`),
  upload limit (`1E`; the 0.26g launcher sends none: no limit), and shows the launcher's states
  (`08` checking the pieces already on disk, `09` downloading;
  `srp-026g/docs/notes/launcher_updater.md`).
- The torrent is trusted by its signature alone: the release's Ed25519 key is built into the
  updater ("Signed releases"), and a torrent whose signature does not verify, or whose revision
  is lower than the highest it has taken (`updater_skhid.torrent.revision` beside it) or than the
  installed game's, is never taken. Where it came from does not matter: today the ZIP's
  `game/binaries/x86/skhid.torrent` beside the updater, or the newest one the rendezvous gave
  (below). One function decides it (`torrent::trusted`).
- No torrent (an older ZIP, one released without it), or none it takes: it downloads by HTTPS
  exactly as today, switch on or off. Torrent is only ever an extra source, never a
  requirement.
- With torrent on, it gets what it would otherwise download from the torrent: the original
  game's files (in place of the archive from the mirrors; the Skhid files come first then, as
  they bring the torrent) and missing or damaged patches. The Skhid ZIP stays HTTPS.
- Every piece is checked against its SHA-256 (the torrent's v2 hashes) before it counts, is
  written or is sent. Each finished file is also checked against the pins the updater has (the
  original files' build-2777 SHA-256, `logic/src/game.rs` `FILES`; the patches' SHA-256 in
  `/patches`) before it is installed.
- Players are the seeders (no host of ours seeds): with torrent on it shares every file of the
  torrent the folder has, however it came (the torrent, HTTPS, the ZIP), so the first players of
  a revision, who get it by HTTPS, seed it. Only checked pieces, only while it runs (while the
  launcher is open), within the launcher's upload limit, on the launcher's port.
- If the torrent gives nothing (no peer in 20 s, no piece for 60 s), it downloads by HTTPS as
  now. PLAY never waits on the torrent.
- Peers from the trackers only (the torrent's; the release's list for the rendezvous). No DHT,
  PEX, UPnP or uTP.
- Peers are strangers: everything they send is parsed once, with fixed limits (40 connections,
  4 of an address; 16 KiB blocks; 16 requests in flight, 256 queued; 32 MiB of pieces held until
  checked; a 4 MiB torrent at the rendezvous), and refused otherwise. A peer that sent a block of
  a piece that fails its hash is dropped and banned for the run.

## The rendezvous

How an updater finds a newer release without our site (`client/updater/logic/src/rendezvous.rs`):

- Every updater with torrent on and a signed torrent is a peer of one fixed swarm on the
  trackers, whose info hash is SHA-1 of `skhid-rendezvous 1\n` and the release's 32-byte public
  key. Peers there have no pieces.
- Its handshake sets the extension bit (BEP 10). The extension handshake says
  `m: {skhid_torrent: <id>}` and `skhid_revision: <n>`, the revision of the newest signed torrent
  it has (no `skhid_revision`: none). An end whose revision is higher sends its torrent, whole,
  as one `skhid_torrent` message (at most 4 MiB) to an end with an older one or none; an end that
  said the higher revision is waited for; then the connection ends. Anything else ends it.
- A torrent that comes is taken only when its signature is the release's and its revision is
  higher than the one this end has (or, having none, not lower than the installed game's). It is
  kept beside the updater as `updater_skhid.found.torrent`; the highest revision of it and the
  ZIP's is the torrent of the run.
- When: an updater that cannot reach the update server asks the rendezvous for 20 s before it
  goes on (a blocked network, our servers down); one that shares serves it the whole time.

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
  it does not verify with, the release builds no torrent and stops.
- No rollback: an updater never takes a torrent of a lower revision than the highest it has taken.

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

**The signature.** The top-level `skhid` is a dictionary of two keys: `revision` (an integer,
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

**The bootstrap torrent** (`release/bootstrap.torrent`): `info.name` `Survarium`, two files,
`game/binaries/x86/7zr.exe` then `game/binaries/x86/updater_skhid.exe`, the same hybrid layout and
piece length (4 MiB) as above, the same trackers, no `skhid` signature (the script pins it). The
script's pins, written by `build_torrent.py bootstrap`: `$InfoHash` (v1, 40 hex), `$PieceLength`,
`$Trackers` (the UDP ones), `$Files` (each `Path`, `Size`, `Sha256`, `Pieces`: SHA-1 of each v1
piece, its last one with the pad's zeros but the last file's, and `Urls`).

**The site's home page** shows under the ZIP's button: "Or the whole game by BitTorrent:
.torrent · magnet", the magnet being
`magnet:?xt=urn:btih:<40 hex>&xt=urn:btmh:1220<64 hex>&dn=Survarium&tr=<tracker>...` (each value
percent-encoded).

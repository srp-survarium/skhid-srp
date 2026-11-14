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
- Someone has to seed each new revision from the start: which host does that, and with how much
  bandwidth, is the owner's call (open).

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

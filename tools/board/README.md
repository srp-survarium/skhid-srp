# skhid-board

A read-only status page for the Skhid project: short notices, then the work items grouped by state.
One small Rust program (std, tiny_http, serde) renders one JSON file as one HTML page. Nobody edits
anything through the web: changes are edits of the data file, published with `push.sh`.

- The page lives at `http://<host>:<port>/<secret>/`. Every other path is a plain 404, whatever the
  method; only GET and HEAD are answered on the page.
- Plain HTTP, no login, no script, nothing loaded from elsewhere. Every response carries
  `Cache-Control: no-store`, `X-Robots-Tag: noindex`, `Referrer-Policy: no-referrer` (links to
  GitHub or Discord do not carry the secret URL) and a strict Content-Security-Policy.
- The data file is re-read when it changes on disk (mtime, size or inode), so a pushed file shows on
  the next page load. A file that does not parse or validate is rejected: the page keeps showing the
  last good version, with a banner saying when the file was rejected and why (with field paths).
- The secret is never logged; requests are not logged at all.

## Running

```
BOARD_SECRET=<at least 16 of [A-Za-z0-9_-]> skhid-board [--bind ADDR] [--port N] [--data FILE]
skhid-board --check FILE      # validate a data file, print the problems, exit 1 if any
```

Options fall back to `BOARD_BIND` (default `0.0.0.0`), `BOARD_PORT` (`8095`) and `BOARD_DATA`
(`board.json`). The secret comes from the environment only, so it never shows in the process list.

Checks: `cargo clippy --all-targets -- -D warnings` and `cargo test` in this folder (outside the
srp-026g dev shell: `nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc nixpkgs#clippy`).

## Page

Notices first (newest first), then one section per state, in this order:

| state         | section           | note                                                       |
|---------------|-------------------|------------------------------------------------------------|
| `waiting`     | Waiting on owner  | `next` says what the owner is asked                        |
| `in-progress` | In progress       |                                                            |
| `review`      | In review         |                                                            |
| `next`        | Next              | planned, not started                                       |
| `requested`   | Requests          | not started, not planned; owner's first, then players'     |
| `done`        | Recently done     | only items updated in the last 14 days                     |
| `parked`      | Parked            | collapsed                                                  |

Each item shows its number (`T12`), title, source and area chips, PR links, the date of its last
update and the `next` line; `detail` and `links` sit under a "Details" toggle.

## Data file

One JSON document. Unknown fields are rejected, so a typo does not vanish silently.

```
{
  "updated": string,                 shown as "Updated ..." (free text, e.g. "2026-10-23 15:00 UTC")
  "notices": [                       optional
    {
      "date":  "YYYY-MM-DD",
      "text":  string,               one line
      "level": "info" | "warn" | "done"     optional, default "info"
    }
  ],
  "items": [                         optional
    {
      "n":       integer >= 1,       the ticket number, shown as T<n>; unique, never reused
      "id":      slug,               [a-z0-9-]{1,64}, unique
      "title":   string,
      "state":   "waiting" | "in-progress" | "review" | "next" | "requested" | "done" | "parked",
      "source":  "owner" | "player" | "dev"      optional, default "dev"
      "area":    "launcher" | "updater" | "defender" | "lobby" | "match" | "weapons" | "economy"
               | "maps" | "server" | "linux" | "tooling" | "release" | "site" | "other"   optional
      "next":    string,             one line: where it stands / what it waits on
      "detail":  string,             optional, a few lines (newlines kept)
      "requested_by": [string],      optional, for requests: who asked (short list)
      "prs":     [integer],          optional, links to github.com/srp-survarium/skhid-srp/pull/<n>
      "links":   [{ "label": string, "url": "http(s)://..." }],   optional (Discord messages, ...)
      "updated": "YYYY-MM-DD"        last change of the item; orders sections, ages out done items
    }
  ]
}
```

Rules kept by whoever edits the file:

- Items are never deleted, so numbers are never reused: a finished item moves to `done` (it leaves
  the page 14 days later), a dropped one to `parked`. The next new item gets `max(n) + 1`.
- Bump the item's `updated` date on every change of it, and the top-level `updated` on every push.

`example.json` is a complete example with made-up data (the tests use it).

## Deployment

Hosted on the jump host (`root@2.28.121.133`), port 8095:

- `/opt/skhid-board/skhid-board`: a static musl build;
- `/opt/skhid-board/board.json`: the data file (mode 644);
- `/etc/skhid-board/env`: `BOARD_SECRET=...` (mode 600);
- `skhid-board.service` (this folder): `DynamicUser`, read-only file system, restart on failure.

The master copy of the data is `private/board/board.json` in the main checkout (not in git); the full
URL is in `private/board/url.txt`.

Publish the data: `tools/board/push.sh` (copies, checks it on the host with `--check`, renames
into place; a rejected file is not swapped in).

Update the program:

```
# static build (rust-overlay's toolchain with the musl target, e.g. from the srp-026g flake inputs)
cargo build --release --target x86_64-unknown-linux-musl
scp target/x86_64-unknown-linux-musl/release/skhid-board root@2.28.121.133:/opt/skhid-board/skhid-board.new
ssh root@2.28.121.133 'mv -f /opt/skhid-board/skhid-board.new /opt/skhid-board/skhid-board && systemctl restart skhid-board'
```

First install, once: copy the binary and data as above, then

```
install -m 644 skhid-board.service /etc/systemd/system/
install -d -m 700 /etc/skhid-board
(umask 077; printf 'BOARD_SECRET=%s\n' "$(openssl rand -hex 32)" > /etc/skhid-board/env)
systemctl daemon-reload && systemctl enable --now skhid-board
```

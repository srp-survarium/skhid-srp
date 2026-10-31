# srp-026g: the server for Survarium 0.26g

The server side for the 0.26g client (build 2777, January 2015, no PDB). Live
on [026g.skhid.link](https://026g.skhid.link) since 2026-10-12.

No 0.21d message is wire-compatible with 0.26g: every message id changed and
the lobby moved from TCP to UDP. One process, `srp`, is every server a client
talks to:

```
survarium.exe -server=<host>:1234
      |
      1. login   TCP 1234   the sign-in in the clear, then TLS; the answer is a
      |                     session id and the lobby's IPv4 address
      2. lobby   UDP 1235   one reliable-UDP link a client: account, inventory, shop,
      |                     items, skills, quests, chat, friends, squads, clans, the
      |                     queue, results
      3. match   UDP 1236   lockstep: the server relays inputs and compares world
                            hashes; the world itself comes from a patched client
      web page   HTTP 8088  home, download, the launcher's page, the admin page
```

0.26g has no browser server and no presence pings. A match is **lockstep**, as
in 0.21d: the server does not simulate the game, so a client must be able to
send its world, which the client patch adds (`client-patches/`). An unpatched
0.26g client does not even start: its code-signing certificate ran out in
2015.

This folder began as a copy of `srp-021d`. Its unused browser-server crate
has been removed; the remaining 0.21d server code still compiles and is
tested (the shared crates and several tests use it). `srp` here starts only
the 0.26g servers. The hosted 0.21d server is built from `srp-021d`.

## Build, run, test

A recent Rust toolchain and OpenSSL (`flake.nix` gives a dev shell with both).

```bash
cargo run -p srp
cargo test --workspace
```

On Windows, point cargo at OpenSSL first:

```bash
export OPENSSL_DIR="C:/Program Files/OpenSSL-Win64-3" OPENSSL_STATIC=true OPENSSL_LIB_DIR="C:/Program Files/OpenSSL-Win64-3/lib/VC/x64/MD"
```

By default everything listens on 127.0.0.1 and the data goes to `./data`.
Every setting is an `SRP_*` environment variable with a default
(`docs/SRP-026G.md`, "Settings").

The game: the original 0.26g with the patched exe and the certificate of
`client-patches/`, started from `game\binaries\x86` with

```
survarium_skhid.exe -localization=english -server=127.0.0.1:1234 -mt_allocator=768 -minimum_resources_memory_size=768
```

A match needs two players who did not come as one squad. Alone on one PC, the
test bot stands in for the second (`docs/SRP-026G.md`, "A match alone").

## Where things are

| Path | What |
|---|---|
| `docs/SRP-026G.md` | The server: its parts, how a player gets into a match, every setting, the web page and the admin, what is implemented |
| `docs/MATCH-SYNC-026G.md` | How a match stays in step: uploads, hash phases, resync |
| `docs/CLIENT-FACTS-026G.md` | Proven facts about the 0.26g client, with addresses |
| `docs/OPEN-ISSUES.md` | What is unverified, broken or not done |
| `docs/DEPLOY.md` | Hosting in Docker: the files, the ports, the settings, updating |
| `docs/notes/` | The protocol and client research, message by message (`README.md` first) |
| `client-patches/` | The patch tools for the original exe, the login certificate, and what each patch does |
| `Dockerfile`, `docker-compose.yml`, `deploy/env-026g.example` | The Docker files |
| `crates/udp-transport` | The reliable UDP transport of the lobby and the match server |
| `crates/survarium/src/v026g/`, `crates/session/src/v026g/` | Game data and wire formats; the account, the rules, the rewards |
| `servers/*/src/v026g*` | The 0.26g login server, lobby, match server and web page; `servers/srp` joins them |
| `crates/resources-db`, `resources/game-026g/` | The players' `resources.db` and the server's game data of a release (`srp-game-data`), read at start; the test copy of it |
| `scripts/packet-coverage/` | Which messages the tests cover |
| `certs/` | The login server's certificate and key |

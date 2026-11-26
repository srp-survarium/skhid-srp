# srp (0.69d): the Skhid server program

`srp` is the server program of **Skhid (Схід)**, a private server for Survarium 0.69d (guides: the root `README.md`,
`docs/guide-server.md`, `docs/guide-player.md`).

One binary that starts the **login server** (TCP, `servers/login-server`), the **lobby** (UDP, `servers/lobby-server`) and the
**match server** (UDP, `servers/match-server`) on the shared account database (`crates/accounts`, SQLite), and the **world host
daemon** (`servers/world-host`, the game's own world code for every online match) when none is running yet. Every client that
signs in gets its own account and profile (inventory, slots, money, upgrades, modules, factions, xp); nothing is shared between
players but the game tables.

## Settings: one config file

All settings are in one TOML file: `config/srp.example.toml` lists every key with its default and meaning (copy it to
`srp.toml`). Every feature is always on; there are no on/off switches. Details, the start command and the event schedule:
`docs/notes/config.md`.

```
srp [--config FILE] setup [--game DIR] [--address IP]   check this computer; write the config (if missing), the database
                                   and the login key; print the players' launch line (setup.rs)
srp [--config FILE] [run]          world host (if none runs yet) + login + lobby + match server
srp [--config FILE] check          check the config and the files it names, print the effective settings
srp extract [--game DIR] --out DIR      build the game tables from a game install into DIR (setup does this too)
srp [--config FILE] selftest [PROJECT]   a world host of its own builds one start world and plays a bot match: OK or
                                   the failing part
srp [--config FILE] world-host     the world host daemon alone, in the foreground
srp [--config FILE] account add|password <name> <password>
srp [--config FILE] ban <account> <ban|suspend> <reason 0..15> <minutes|0> | unban <account> | comments [account [limit]]
scripts/srp-host.sh setup|start|run|stop|status|log|check|selftest [FILE]
```

Without `--config`: the file named by `SRP_CONFIG`, else `./srp.toml`, else the defaults. `RUST_LOG` sets the log level (default
`info`). The only other environment variable is the debug switch `SRP_RECORD_DIR` (record every online match's input stream).

## How a client gets from login to lobby
1. The client signs in over TCP (`-server=host:port`). The login server decrypts the secure data with its RSA key, takes the
   account name from the request and the 40-hex digest from the secure data and asks `AccountAuth`: a known name needs the same
   digest as the stored password; an unknown name is **registered by its first sign-in** (the digest becomes the password) unless
   `accounts.auto_register = false`.
2. The `0B` success answer carries a random session id and the lobby address (`network.lobby_public`).
3. The client opens the UDP lobby connection and sends `2E sign-in` = `u32 session, ff, ff`. The lobby redeems the session in the
   shared `SessionTable` (valid one hour, may be used again after a lost connection) and then pushes the account (91, 8F, 8B, 9D,
   89, 87, ...). An unknown or expired session gets no answer.

## Running it with the real client

The login key (`files.login_key`) is an RSA-1024 private key (the client's buffer is 160 bytes, a 2048-bit key fails). The
login server sends its public half in `0D` at every sign-in and the client keeps it, so the client needs no key argument (the
0.69d game has no such argument and stops at an unknown one). Start it with the server and localisation arguments:

```
srp --config srp.toml
survarium.exe -product_name=Survarium-Steam -server=<host>:65000 -localization=english
```

Log in with any account name and password (first sign-in creates the account). For a play over a network bind `network.login`,
`network.lobby` and `network.match` to `0.0.0.0`, set `network.lobby_public` / `network.match_public` to the address players reach
and use that address in `-server=`.

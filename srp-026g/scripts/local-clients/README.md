# Multiple local 0.26g clients (Linux / Wine)

Start a local server and several native game windows with one command. Each
window has its own Wine prefix and prefilled account name and password.
`stop` closes only this launcher's prefixes and server; profiles and accounts
survive a restart.

Requires Python 3.10+, Wine, GNU `cp`, a working game installation with
`survarium_skhid.exe` (including the multiple-instance and Linux VRAM patches),
and an **inactive** Wine prefix with working DXVK. The launcher copies this
prefix once per client, keeping Documents inside each copy. It does not download
game files or modify the source game/prefix.

From the repository root, configure once:

```sh
python3 srp-026g/scripts/local-clients/run.py init \
  --game-dir /path/to/game \
  --prefix-template /path/to/working-wine-prefix \
  --wine /path/to/wine/bin/wine \
  --server-bin /path/to/srp-026g/target/debug/srp \
  --server-workdir /path/to/srp-026g \
  --accounts 4
```

`--game-dir` contains `resources.db`, `resources/ssl/`, and `binaries/x86/`.
The server's working directory must contain its `certs/` directory. Build the
server with `cargo build --locked -p srp` from `srp-026g` before starting it.
The local server is given the sign-in cipher's secret key (`SRP_SIGN_IN_KEY`,
`docs/notes/sign-in-cipher.md`) from `~/.config/skhid/sign_in_cipher.key`,
unless the configuration's `server.env` or the environment sets it; the key
is no file of the repository.

`"remote_login": "host:port"` in the configuration points the games at a
login server that is not this machine's (`start --clients-only`).
If the Wine executable needs a different wineserver path, edit the configuration.

Edit `private/local-clients/config.json` to choose names, passwords, ports,
resolution, and optional client/server environment settings (for example a GPU
ICD or `LD_LIBRARY_PATH`). It initially contains `Local1` through `Local4`, each
with `LocalTest123`. These are disposable local test credentials. The existing
`/private/` ignore rule keeps configuration, game logs and account data out of Git.

```sh
python3 srp-026g/scripts/local-clients/run.py start --count 2
python3 srp-026g/scripts/local-clients/run.py status
python3 srp-026g/scripts/local-clients/run.py restart --count 4
python3 srp-026g/scripts/local-clients/run.py stop
```

Click **Enter** in each game window. A new account registers on its first login;
existing accounts require their existing password. Normal tutorials are enabled
for new accounts. The launcher deliberately keeps `SRP_TUTORIAL=on`: disabling
it prevents the native client's normal UI action handlers from being installed.

The managed server binds only to `127.0.0.1`. Its accounts live in
`private/local-clients/data`; logs and copied Wine prefixes live in
`private/local-clients/runtime`. Default ports are login TCP 1234, lobby UDP
1235, match UDP 1236, web TCP 8088. Occupied ports cause startup to fail without
stopping another server. You can change the four ports together in the JSON.
The server binary is used as configured; `restart` does not build new code.

The managed server records every match, from one player on, into the world
corpus: `private/world-corpus/local/<the configuration's folder>/`
(`SRP_RECORD_DIR`, `SRP_RECORD_MIN_PLAYERS=1`, at most 2000 MB a folder, the
oldest deleted first). `tools/check`'s `world-check` and `world-drift` read
them, so a feature's own test games are checked with the live ones
(`srp-026g/docs/notes/packet-validation.md`, "The world corpus"). It is on by
default; `"SRP_RECORD_DIR": "off"` in the configuration's server `env` turns it
off, and the env may change the other two.

Each game window starts directly under Wine. Set `"virtual_desktop": true` to
put every window in its own Wine virtual desktop of `resolution` instead; on
some Wayland sessions (niri) the game exits inside one and only a blue desktop
is left, so it is off by default.

For comparisons between client patches, an account entry may set
`"executable": "survarium_skhid.fov65-backup.exe"`. The filename is relative
to the game's `binaries/x86/` directory; omitted entries use
`survarium_skhid.exe`. This lets two windows use different executables while
sharing game resources and retaining separate profiles.

`start` reads the patch revision of each window's executable and prints it. When
the server's env sets `SRP_MIN_PATCH_REVISION`, an account whose executable is
older (or unpatched) stops the start before anything launches, naming the
account and the file: the login server would otherwise refuse it only at its
sign-in. A configuration shared between tests drifts; point the accounts at the
build under test.

To use a server that is already running, use `start --clients-only --count 2`.
It must listen on the configured local login port. This launcher will not stop
that server. A different setup can use `--config /path/to/config.json` **before**
the subcommand; its runtime and data live beside that configuration.

Passwords are passed as literal native `-account_name` / `-account_password`
arguments, never through a shell. The launcher enables `store_user_password`
in each isolated profile; the native login screen otherwise leaves the password
field empty even when its command-line argument is present. The game can include them in its local logs.
The script clears copied `account_digest` values: that saved field contains a
machine-key-encrypted digest and cannot be populated with plain SHA-1.

Tests (no game or Wine required):

```sh
python3 -m unittest discover -s srp-026g/scripts/local-clients -p 'test_*.py'
```

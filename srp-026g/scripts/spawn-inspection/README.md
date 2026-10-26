# Inspect all player spawns locally

The native `srp-bot` signs in as nineteen real test accounts, joins your room,
and sends no movement, aiming or firing inputs. Only your normal game window
renders the twenty players. No game executable or server rules are changed.

Build `cargo build -p srp --bin srp-bot --bin srp-worlds` in `srp-026g`.
Use the same local configuration as `scripts/local-clients/run.py`:

```sh
python3 srp-026g/scripts/spawn-inspection/run.py --config /path/to/local/config.json configure
# Restart the local server through the usual local-clients launcher.
```

Create a **private room**, **10 players per side**, with a PIN, and select
only the map/mode you want to inspect. Then:

```sh
python3 srp-026g/scripts/spawn-inspection/run.py \
  --config /path/to/local/config.json --bot-bin /path/to/srp-bot run ROOM_CODE --pin PIN
```

Wait for all **20/20** names. The room owner presses **Start**. Leave the first
spawn untouched briefly, then walk around; the other nineteen stay still.
They are not the moving Player-vs-Bots opponents. Ctrl+C disconnects them.
The command always connects to loopback; configure registers only their names
and backs up your config. Existing accounts and inventories stay unchanged.

The original game selects distinct eligible spawn pointers for a simultaneous team
spawn batch (0x316820 → 0x3171D0), then assigns each selected pointer once. London
has exactly ten points per team. Staggered arrivals still depend on occupancy,
so filling ten places on each team does not
by itself prove that every configured point was selected (a blocked point can
be excluded). For London, verify the actual initial world saved by the local
server, rather than assuming coverage:

```sh
srp-worlds /path/to/local/runtime/snapshots /tmp/spawn-positions
python3 srp-026g/scripts/spawn-inspection/run.py coverage /tmp/spawn-positions/positions.csv --match MATCH_ID
```

It checks the first snapshot with twenty alive players against all twenty
London positions, within one metre, and lists missing or shared points. All four
London modes use the same positions. Move only after the server has saved the
initial complete world; otherwise your own point may appear missing.

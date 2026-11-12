# bot-routes

Tools for `bot-routes.json`, the human-routes table of play 17 (`bots/routes.rs`). The table and the
tables it is made from are private (they come from the players' saved worlds); nothing here holds
data.

- `perches.py`: adds the human perches (`perches`, `perch_maps`) to the table. Format, method,
  checks and numbers: `docs/notes/player_bots_mapdata.md`, "Human perches". Inputs are the routes
  tool's `matches.json` and `positions.csv`.
- `test_perches.py`: its tests on made-up data (`python -m unittest test_perches` in this folder).
- `walked_floors.py`: puts every route point on a floor the players stood on (the table's points, from the verdicts of the
  ignored test `route_points_stand_where_the_players_stood`, `bots/routes.rs`). Method and numbers:
  `docs/notes/player_bots_mapdata.md`, "Route points on floors the players walked". The players' samples are private.
- `test_walked_floors.py`: its tests on made-up data (`python -m unittest test_walked_floors`).

```
python perches.py matches.json positions.csv --routes routes.json --out routes-perches.json --print

# the verdicts, then the table with them applied (see the test's doc comment for the variables)
SRP_MAPDATA_DIR=... SRP_BOT_ROUTES=routes.json SRP_ROUTES_HUMAN_DIR=... SRP_ROUTES_WALKED_OUT=verdicts.tsv   cargo test --release -p match-server route_points_stand_where_the_players_stood -- --ignored --nocapture
python walked_floors.py routes.json verdicts.tsv routes-walked.json changes.txt
```

## The checks on the real maps (ignored tests, `bots/routes.rs`)

They build each map's grid as a match has it (the default tuning's body, off-mesh links and extras), so they speak of
the grid the bots walk. Run them in release with the private map files.

- `how_walkable_the_real_routes_are`: every route of a table from its side's spawns through its points (legs with no
  way, jumps, the way's length against the straight one, the leg out of the base apart, every leg's way over its
  straight line, points lost); `SRP_ROUTES_POINTS_OUT` adds each point's floor.
- `real_routes_and_the_damage_zones`: the table against the zones a match has; `SRP_ZONE_FLOORS_DIR` gets each map's
  zone floors.
- `how_reachable_the_real_positions_are`: a corpus of places players stood at (`SRP_POSITIONS`, a TSV with `project`,
  `x`, `y`, `z`: the feet) against the grid: the floor there, whether a walk from each side's spawns gets to it, zones,
  pockets, the walker's surface where the grid has no floor, and how far under it the next floor lies (`under_m`: an
  upper level). A place players use that no bot can reach is a builder bug to look at.

```
SRP_MAPDATA_DIR=... SRP_POSITIONS=positions.tsv SRP_POSITIONS_OUT=reach.tsv   cargo test --release -p match-server how_reachable_the_real_positions_are -- --ignored --nocapture
```

The bench (`real_bot_cost`) prints the bots' own seconds on upper levels by the same rule (its `upper:` line), to set
beside the players' share from that check.

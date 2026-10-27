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

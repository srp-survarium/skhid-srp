# bot-routes

Tools for `bot-routes.json`, the human-routes table of play 17 (`bots/routes.rs`). The table and the
tables it is made from are private (they come from the players' saved worlds); nothing here holds
data.

- `perches.py`: adds the human perches (`perches`, `perch_maps`) to the table. Format, method,
  checks and numbers: `docs/notes/player_bots_mapdata.md`, "Human perches". Inputs are the routes
  tool's `matches.json` and `positions.csv`.
- `test_perches.py`: its tests on made-up data (`python -m unittest test_perches` in this folder).

```
python perches.py matches.json positions.csv --routes routes.json --out routes-perches.json --print
```

# foliage-volumes

`foliage_volumes.py` makes `bot-foliage.json`: for each level project of the game's `resources.db`, a box round every
bush the map draws and gives no collision object. The bots' sight and the bullets know the bullet mesh only, so a bot
sees through such a bush and a player does not (`servers/match-server/src/v026g/bots/foliage.rs` reads the file).

```
cargo build -p resources-db --bin srp-game-data
python -I scripts/foliage-volumes/foliage_volumes.py target/debug/srp-game-data <resources.db> bot-foliage.json
```

Put the file in the mapdata folder (`SRP_MAPDATA_DIR`) with the other private data; `SRP_BOT_FOLIAGE` names another
file, empty or `off` reads none.

What it counts (the stock 0.26g `resources.db`, day_2 of each level; the other weather variants share the placements
but for a few): Rudnya 168 bushes, School 91, Vostok Radar Station 0, Chemical Plant 5, Tarakanovsky Fort 174,
Mamayev Kurgan 195, Vector Laboratory 198, Cologne 0. A bush that has a collision object (`bush_wide` and others of
Vector Laboratory and Tarakanovsky Fort, `grey_kruch`) is in the sight tree already and left out.

A box is the bounds of the model's `hq_collision` hull (its simplified mesh), or of its LOD0 render mesh where it has
none (`bush_thin`, `grey_kust`), through the placement's scale and rotation. A box holds more than the leaves do: a
line through its corner is counted as inside though it may pass between branches.

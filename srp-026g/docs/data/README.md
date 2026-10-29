# Item tables

`equipment.tsv` and `weapons.tsv`: every armour piece and weapon of the game
data a release ships, with what the wiki shows of it (protection, weight,
built-in bonuses, what it can roll, whether a helmet flies off; damage,
handling, recoil, spread). Never edited by hand: `srp-item-tables`
(`servers/web-server/src/item_tables.rs`, the wiki's own code) writes them
from a server game data file, only where an item changed, and says each
change (`item: column old -> new`).

- `release/build-content <revision>` writes them into `<out>/item-tables/`
  from the server data it makes, against these, so the item changes show
  before any release (never into a checkout: the release commits them).
- `release/release.sh` writes them from the server data it ships into the
  release's record commit, so each revision's balance changes are a diff here.

Tab-separated, a header row, a row per item keyed by its `items_dict` name;
armour and isolation in hundredths as the lobby prints them, weights in kg,
ranges in metres, damage and penetration out of 100.

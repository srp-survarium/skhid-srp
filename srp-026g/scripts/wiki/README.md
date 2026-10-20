# Wiki

The website's `/wiki` (`srp-026g/servers/web-server/src/wiki.rs`, spec in
`docs/notes/wiki.md`): every item, map, mode and rule of the game as Skhid's
players have it, in English and Ukrainian.

## Build: the release does it

`build.py` reads the game as the players have it (through
`srp-026g/scripts/released-game/released.py`, shared with the Maps tab and the
calculator): the stock `resources.db` with exactly the patches of
`release/patches.tsv` over it, each checked by its sha256. srp's own rules
come from `session::v026g::wiki::rules`, run by the example `wiki_rules`
over the server data `srp-game-data` makes of the same archives.

A live release builds it: when main's `built-from.tsv` names other patches
than the release offers, `release/release.sh` rebuilds the Maps tab, the
calculator and the wiki in its preflight with `release/build-site`. By hand,
for a local preview or after a change of this folder or of srp's rules:

```
release/build-site --tree . [--with <resources.db.patch_<n>>]
```

or this page on its own (after the calculator's build, for its icons):

```
cargo build --release --locked -p resources-db --bin srp-game-data
cargo build --release --locked -p session --example wiki_rules
target/release/srp-game-data build --db "$SKHID_STOCK_DB" --patch "<patch>=<url>" ... --out data.db
SRP_GAME_DATA=data.db target/release/examples/wiki_rules > rules.json
target/release/examples/wiki_rules --test-data > rules-test.json
python3 -I srp-026g/scripts/wiki/build.py --stock "$SKHID_STOCK_DB" --patches <folder with the list's patches> \
  --dbgrab tools/re-kit/common/dbgrab/target/release/dbgrab --icons <the calculator's icons> \
  --rules rules.json --rules-test rules-test.json
```

It writes `srp-026g/servers/web-server/wiki/`:

- `pages/<page>.<en|uk>.html` (the site's header goes in at
  `<!--SITE-HEADER-->`), `wiki.js` (sortable tables, search), `icons/`;
- `pages.rs` and `files.rs` (what the server includes);
- `built-from.tsv` (the patches) and `rules-from.tsv` (the SHA-256 of
  `wiki::rules` over srp's test data).

Two web-server tests keep it fresh on main: the patches of `built-from.tsv`
must be the offered ones, and `rules-from.tsv` must match the code's rules,
so a PR that changes a price rule, a modification roll or a constant the wiki
shows builds the wiki again. A third follows every link and file.

## Files

- `build.py`: the command; facts and tables for the written pages; the main
  page and the A to Z.
- `model.py`: the game data and srp's rules as items, factions, compatibility.
- `pages_items.py`: weapons, ammo, equipment, scopes, supplies, cosmetics.
- `pages_world.py`: maps, modes, anomalies, artefacts, factions, skills,
  modifications, shop, drops, a new account.
- `pages_text.py` and `text/`: the written pages, `<page>.en.md` and
  `<page>.uk.md` (the format is in `pages_text.py`). A number on them is a
  fact, `{{name}}`, of `build.py`'s `facts_of`; a generated table is
  `{{table:name}}`. An unknown one, or a link to no page, stops the build.
- `web.py`: the page shell (the calculator's look) and its pieces.
- `wiki.js`: the pages' script.

## What comes from where

| what | where |
|---|---|
| items, their names, texts, card lines, stats | the item's config (`gameplay/items/...`), `db_static_dictionaries`, the localizations, `static_game_parameters` `ui_properties` |
| icons | the calculator's lobby icons (`damage-calc/assets/tools/build_icons.py`) |
| sellers, ranks, prices, sale, repair | `rules.json`: srp's price list and formulas |
| bonuses and defects an item can get, their chances | `rules.json`: srp's candidates and weights; the steps' values from `modifications.options` |
| ammo of a weapon | `items_compatibility` |
| damage near / far | `engine.js`'s `bulletAt` over the weapon and ammo configs |
| scopes | `rules.json`: srp's scope pairs |
| maps, modes, hazards, artefacts | `gameplay/game_modes/`, the projects' `server_objects` and `client_objects` |
| healing, injuries, fall | `human_hit_params`, `default.player` |
| shop, drops, gifts, kits, rooms | `rules.json`: srp's constants |

The fandom wiki (a local copy in `/mnt/hdd/Projects/survarium-fandom-wiki-20261020/`)
gave the structure only: none of its text or images is used.

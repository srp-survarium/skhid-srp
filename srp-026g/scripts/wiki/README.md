# Wiki: weapons

The wiki is `/wiki` (its index) and four sections: `/wiki/weapons`, built here;
`/wiki/modifiers`; and the damage calculator (`/wiki/calculator`,
`scripts/damage-calc/`) and the maps (`/wiki/maps`, `scripts/maps-tab/`), each a
page of its own look under a "Wiki / <page>" line. Their old addresses `/damage`
and `/maps` send to them.

The modifiers page has no build step: the web server makes it from the server's
own game data (`servers/web-server/src/modifiers.rs`), with the tables and the
roll rule the lobby gives modifications by (`Modification::rolls`). Each kind
lists its strengths, commonest first, and the item groups that roll it (or, for
the kinds that never roll, the items that come with it); a group only some of
whose items take it lists those items. Reliability is left out: items wear
nothing.

The weapons section is a searchable weapon catalogue with class filters,
sorting and expandable weapon stats, in EN/UK.
Two selected weapons share a stat comparison and recoil simulation. Comparison
colors follow each stat's direction, stay neutral for equal displayed values,
unknown values and neutral traits, and can be turned off. Range comparisons
require both endpoints to be no worse; crossed ranges stay neutral.

`stats.py` defines the common display schema for the individual cards and the
comparison. Config data includes damage, handling, dispersion, recoil and sway;
missing values are shown as absent, except for the verified native 1× hip-fire
default. Wear rates and reliability remain omitted. Horizontal recoil is the
right draw minus the left draw, never their average. Its variability is the
standard deviation of that difference for independent uniform draws.

`recoil.js` models the native shot, ramp and recovery operations at 50 ms ticks,
matching the research reference in `match-server/.../bots/recoil.rs`. Weapon
parameters all come from the dataset. The fixed animation normalization factors
63.735 / 21.48 come from the executable. Random samples use a repeatable xorshift
seed, not the native generator. This is a recoil model, not a bullet-impact
prediction: no mouse compensation, spread, camera shake or confirmed horizontal
screen projection. The UI states those limits. Next shot, reset and timeline
scrubbing update the same simulation position. Changing weapons or the shot
count resets the position; changing the random seed rebuilds both traces while
preserving the current position.

Both comparison slots use the calculator-style image chooser (`picker.js`),
with class groups, level ordering, search and keyboard navigation. The same
selection drives stats and recoil and is restored from the page URL.

The weapon names, images and numbers come from the damage calculator's generated
JSON literal in `servers/web-server/damage/calc.js`. `build.py` decodes JSON; it
does not execute JavaScript. Classes use the game's item-dictionary category,
not the weapon implementation type (several rifles use the SMG implementation).
Damage and penetration are converted to the same 100-point scale as the game UI.
Ammo, modifications and character effects are excluded explicitly on the page.

`damage-calc/build_site.py` builds this catalogue immediately after the calculator,
including during `release/build-site`. A web-server test checks the SHA-256 of
the calculator against `wiki/source.sha256`, preventing stale weapon data.

After changing the wiki template or JavaScript:

```
python3 srp-026g/scripts/wiki/build.py
```

Local browser preview (under `nix develop`, from `srp-026g`):

```
cargo test -p web-server serve_wiki_preview -- --ignored --nocapture
```

Open `http://127.0.0.1:24583/wiki`. This uses isolated in-memory accounts and
does not start or connect to the game servers. Stop the command to close it.
The modifiers and equipment pages come from the tests' game data (an old fixed
copy) unless `SRP_GAME_DATA` names a server data file. To preview a release,
run `release/build-content <revision> --out <folder> --site` (add
`--tree <worktree>` to build in a worktree): one run makes the patch,
`<folder>/server-game-data.db`, `<folder>/item-tables/` and, in the tree, the
site's pages with their icons, all from the same patches; then start the
preview there with `SRP_GAME_DATA=<folder>/server-game-data.db`. The rebuilt
pages are for the preview only: keep them out of a PR (the release rebuilds
and commits them).

`check_browser.mjs` exercises the real HTTP preview in an isolated Firefox
WebDriver BiDi session (port 9457 by default), including filtering, sorting,
EN/UK, expanded rows, mobile sizing, images and the site's CSP. It saves review
screenshots to `/tmp/wiki-weapons-review`.

Focused model checks: `node scripts/wiki/test_recoil.mjs servers/web-server/damage/calc.js`
and `python3 -m unittest discover -s scripts/wiki -p 'test_*.py'` from `srp-026g`.

## Icons

`/wiki/icons/` holds the lobby's own icons (`lobby_menu.swf`), made by
`build_icons.py` each time `damage-calc/build_site.py` runs (so every
`release/build-site`) into `servers/web-server/wiki/icons/`, with `files.rs`
listing them for the server (`servers/web-server/src/icons.rs`, `icons::url`
gives an address with its `?v=` hash):

- `items/<items_dict name>.webp`: the item icons the calculator's build cuts
  (171x83), for every category of the shop's equipment and weapon filters;
- `mods/<ui_desc.icon>.webp`: every cell of the `modifiers` atlas (character
  3279, 38x38, column-major); each icon a modification shows must be one;
- `groups/<item_category>.webp`: the shop filter buttons' icons
  (FilterItemCatRenderer's `itemIcon`, sprite 1125, frame 300 + the icon of
  that category's filter in the lobby's table
  `flash/lobby_menu/tables/survarium/data/ShopFilters.tsv`, the one the movie
  is built with).

The modification and group icons are written white, with the glyph's
brightness as alpha (their baked drop shadow drops out), for the page to
tint in its ink with CSS masks. The build fails if an item of those
categories has no lobby icon, a modification's icon is an empty cell, or a
filter frame does not hold exactly one bitmap.

## Equipment

`/wiki/equipment` is the web server's (`servers/web-server/src/equipment.rs`),
like the modifiers page: every armour piece from the server's own game data
(`crates/survarium/src/v026g/armour.rs`, read at srp's start) with its icon,
protection per body part (Armour and Isolation when a part's physical types
and its anomaly types each share one value, as in every config so far; else a
column per type), the slots its model takes, an oxygen tank's time and gas
filter (`data.amount_time_sec`, `influences`), level, matchmaking level when it
differs, faction (`factions_dict`), and the modifiers page's "Can roll" /
"Comes with" (`modifiers::item_lists`). What it shows and skips follows
`docs/notes/armour-params.md`. A helmet "flies off" when the headwear edit list
(`scripts/content-edit/headwear-1.json`, compiled in) rebinds its model.
`equipment.js` (copied by `build.py`) filters by slot and faction (both kept
in the address: `?slot=helmet&faction=3`), level and name, sorts, and groups
by set (a faction's tier: `<family>_<slot>_<tier>` configs) or slot.

## Stats

`/wiki/stats` is a frame the web server gives (`servers/web-server/src/wiki.rs`,
its policy lets the script ask the site for JSON) and `history.js` (copied by
`build.py`) fills from the match history's JSON (`/api/history/...`,
`docs/notes/match_history.md`) when it is viewed. The address says what it
shows: `?player=<name>`, `?match=<number>`, `?clan=<tag>`, else the
leaderboards (`?board=players|clans|best|latest&by=<key>`). Each view is
drawn once empty in its final shape, then with its data, so nothing moves
while it loads; match lists grow by "Older matches" (`before=`). The page is
`aria-busy` while an answer is on its way.

`check_history.mjs` exercises it in an isolated Firefox, as
`check_browser.mjs` does, against `history_fixture.mjs` (fake answers in the
contract's shapes, each 300 ms late): the page, script and policy from the
real preview (`serve_wiki_preview`, port 24583), served again on port 24584
with `/api/history/...` from the fixture. Screenshots go to
`/tmp/wiki-stats-review`.

```
node scripts/wiki/check_history.mjs http://127.0.0.1:24583 9457
```

With a fifth argument `real`, `/api/history/...` is passed on to the server
of the first argument instead: a web server over an `srp.db` that holds the
fixture's players and clans and 23 public matches (Yasen in the newest 15,
one a draw), recorded as srp records them (`Results::settle`, then the
accounts' flush). The checks are the same; the ratings and the match numbers
are taken from the answers.

```
node scripts/wiki/check_history.mjs http://127.0.0.1:24583 9457 /tmp/wiki-stats-review 24584 real
```

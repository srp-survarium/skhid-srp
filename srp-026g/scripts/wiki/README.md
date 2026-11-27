# Wiki: weapons and simulators

The wiki is `/wiki` (its index) and its sections: `/wiki/weapons` and
`/wiki/simulators`, built here;
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

The artefacts page (`/wiki/artefacts`) has no build step either: the web server
makes it from the server's game data (`servers/web-server/src/artefacts.rs`,
the configs read by `survarium::v026g::artefacts`): each artefact's passive and
active with its numbers, active time, cooldown, weight and skill, the rules of
carrying several (exe patches 54 and 109, `docs/notes/artefacts.md`) and the
artefact tree's skills. Its icons are the lobby's, made with the wiki's
(`build_icons.py`).

The weapons section is a searchable weapon catalogue with class filters,
sorting and expandable weapon stats. Both pages are one copy for every language
(`weapons.html`, `simulators.html`, each with its `.texts.json`): their words
are the site's (`servers/web-server/words/`), filled in by the server, and the
names of weapons, rounds, skills and modifications the game's in each language
(`scripts/site-texts/README.md`).
Two selected weapons share a stat comparison; it, and each weapon card, link to
the simulators with the same weapons and rounds. Comparison
colors follow each stat's direction, stay neutral for equal displayed values,
unknown values and neutral traits, and can be turned off. Range comparisons
require both endpoints to be no worse; crossed ranges stay neutral.

`stats.py` defines the common display schema for the individual cards and the
comparison. Config data includes damage, handling, dispersion, recoil and sway;
missing values are shown as absent, except for the verified native 1× hip-fire
default. Wear rates and reliability remain omitted. Horizontal recoil is the
right draw minus the left draw, never their average. Its variability is the
standard deviation of that difference for independent uniform draws.

The simulators section (`simulators.js`) is the recoil and spread simulation of one
or two weapons (A, B, the same one twice if wanted), each with a round and its own
setup: a panel of the damage calculator's rows (`damage-calc/hud.js`, `hud.css`), each
with its card (hover, keyboard focus or a tap). A setup is the aim (from the hip,
aiming in, fully aimed), the stance (standing, walking, crouching, crouch-walking,
sprinting, jumping), broken arms, the ranks of the skills that change the spread or
the kick, and a row per modification kind and piece that can have it: the weapon's
(the cook's type 2 on its own values: Weapon recoil its kick ranges, Dispersion its
`dispersion/base_dispersion`, the base every bullet's spread has, all of it fully
aimed; `gen_calc_data.py` `WEAPON`), and each armour slot whose items roll it or come
with it, the pieces adding up in the kit's player modifier (as `apply_modifier` takes
the sum). A Skhid item has no defects: srp rolls none. The kit's recoil modifier (18)
scales each axis's step a shot, the camera shake's too (`recoil.js`). `spread.js`
models the spread from the exe (its header has the formula and addresses): the
stance's part held at its target, the shots' part run at the ticks; fully aimed, only
the base counts. Every value comes from the calculator data's `simulators`
(`gen_calc_data.py`: `default.player`'s stance spread and skill weights, the skills
and modifications carrying the player modifiers the spread and the kick read, each
skill rank the sum of its levels, the slots that roll or come with each
modification) and each weapon's `dispersion` and `double_handed`; the skills' names
and icons are the skills page's, the modifications' the calculator's. Low stamina's
floor is left out. A table gives each weapon's spread parts in degrees.

A shot moves three things, and the simulation shows each by its own switch:

- **Recoil** (`recoil.js`): the kick and, from the hip, the shake. It turns the
  camera by the weapon's recoil numbers, at random within their ranges; the
  next bullet leaves where it points.
- **Camera wobble** (`motions.js`): the hands' shot clip turns the camera bone
  `Item` on some weapons (of the automatic ones only the H&K 416 and the M4A1).
  One fixed curve a shot; no number scales it. It turns the bullets too: a
  bullet leaves along the eye as the simulation's step before its shot left
  it (steps end every 50 ms), so from a burst's second shot on a bullet takes
  the curve late in the pass before (the H&K 416: 0.29 degrees on average, at
  most 0.67, mostly sideways); the first leaves from rest. The hits are drawn
  so.
- **Gun kick** (`motions.js`): the gun's own shot clip moves its body bone
  `offset` in front of the camera, which the aimed view shows on every gun
  since patch 172. One fixed curve a shot; the bullet leaves from the camera,
  so this moves only where the sights point.

The view's cross is the camera's centre, where a bullet goes; a ring is the
gun's sight while aimed. Under each view the same from the side: the eye, the
view's line, the sight's line and each bullet from the eye to the wall.
`shot_motions.py` reads both curves from each weapon's clips in the site build
(`damage-calc/build_site.py` extracts `animations/single/weapons/`), so a
release's page has that release's clips: `shot` in the calculator's data, a
weapon without it (a page built from older data) moving nothing but by its
recoil. Its header has the bones, the axes' signs and what was checked.

A side's **custom gun** (the panel under its setup) is its weapon with any of
those values the visitor's own: the recoil numbers by the config's keys, and
each motion's peak, which scales the weapon's own curve or, where it has none,
a standard one (to its peak in the pass's first fifth, back by its end). The
same weapon in both slots compares it with its custom gun.

`recoil.js` models the native shot, ramp and recovery operations at 50 ms ticks,
matching the research reference in `match-server/.../bots/recoil.rs`. Weapon
parameters all come from the dataset. The fixed animation normalization factors
63.735 / 21.48 come from the executable. Random samples use a repeatable xorshift
seed, not the native generator. This is a recoil model, not a bullet-impact
prediction: no mouse compensation or confirmed horizontal screen projection. Next shot, reset and timeline
scrubbing update the same simulation position. Changing weapons or the shot
count resets the position; changing the random seed rebuilds both traces while
preserving the current position.

Both slots, on either page, are `slots.js`'s `WikiWeaponSlots` and use the calculator-style image chooser (`picker.js`),
with class groups, level ordering, search and keyboard navigation; the chosen item's level shows on the right
of its button (not on phones). The
selection is kept in and restored from the page URL.

The weapon names, images and numbers come from the damage calculator's generated
JSON literal in `servers/web-server/damage/calc.js`. `build.py` decodes JSON; it
does not execute JavaScript. Classes use the game's item-dictionary category,
not the weapon implementation type (several rifles use the SMG implementation).
Damage and penetration are converted to the same 100-point scale as the game UI.
Modifications and character effects are excluded explicitly on the page.

Every value is with a round (`stats.with_round`), the weapon's default
(`default_ammo`) unless one is chosen: each card and each comparison slot has a
Round chooser (`picker.js`, narrowed with `offer` to the weapon's rounds,
`compat`; each with its lobby icon and level, from the calculator's data, in
the page once as `#round-options`), the slot's in the address (`ra`, `rb`)
when it is not the default. A round scales what the exe scales by
its `data` keys (ammo_data::load 0x332440): damage (`k_damage`), penetration
(`k_arp`), minimum damage retained (`k_min_damage_amount`), both damage
distances, stopping power (`k_power`), stamina damage (`k_stamina_damage`) and
base dispersion (`dispersion`, also in the recoil simulation's spread). A
pellet round's damage reads "per pellet × pellets" and compares as the whole
shot. `build.py` renders the default round and gives each weapon row
`data-rounds`: every round's rows that differ from the default's, which
`weapons.js` swaps in. The list's quick stats and damage sort stay on the
default round. The game's card (rev50+, `WeaponDetails.bestRoundFactor`)
shows the highest damage and the highest penetration of all the weapon's
rounds, often from two different rounds, so the page names that difference.

`damage-calc/build_site.py` builds this catalogue immediately after the calculator,
including during `release/build-site`, so every release's catalogue is made from
its own calculator.

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
and `python3 -m unittest discover -s scripts/wiki -p 'test_*.py'` from `srp-026g`. `tools/check`
runs them as `wiki-model` (the models and the released data's weapons and modification rows) and
`wiki-stats`, and the browser check as `wiki-browser` (`tools/checks/wiki_browser.sh`: its own
preview, Firefox and ports).

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

## Modes

`/wiki/modes` is the web server's (`servers/web-server/src/modes.rs`), like the
quests page: one page, a section a mode in the lobby's order, each with its
anchor (`#team-deathmatch`, `#protective-device`, `#chargers`, `#blackout`,
`#research`, `#free-for-all`, `#freeze-tag`, `#artifact-hunt`) and the free-for-all's
versions under it (`#racket`, `#saloon`, `#saloon-winchester`, `#roulette`). A
section has the mode's loading-screen icon (`icons/modes/`, above), its game
name, the rooms that offer it, its rules a short line each, and its maps with
their weathers, each linked to the Maps tab (`#<project folder>/<mode>`) when
the tab has it. Nothing in it is typed in that the server has:

- each number of a map's config (time limit, respawn, batteries, chargers and
  their timing, tickets, Research's points, the kills to win, Artifact Hunt's
  goal, waits and sudden death) is read from the playable maps of the mode
  (`maps::playable_in`), a span where they differ;
- Freeze Tag's thaw time, reach, round wins and pause are its rule's
  constants (`srp_validate::world::freeze`);
- a free-for-all's versions: their shares (`twist::SHARES`), Racket's guns and
  sight (`racket`), Saloon's (`cowboy`), Roulette's kits, bands and pistol share
  (`roulette`);
- the rooms: the running lobby's pools (`SRP_MATCH_MAPS`,
  `SRP_ROOMS_EXTRA_MAPS`; srp hands them to the page as `web_server::Pools`):
  a mode is public with a map of the match pool, private only with an
  experimental one; a version is a public room's alone.

The texts are the site's (`words/`), the mode, map, weather and item names the
game's own in the page's language.

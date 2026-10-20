# Maps tab

The website's `/maps` (`srp-026g/servers/web-server/src/maps.rs`, spec in
`docs/notes/maps_tab.md`). It shows every playable map, by mode, with these
points, each kind toggled on its own:

- respawn points, by team;
- battery spawns (the battery mode) and chargers (the mode with chargers:
  its batteries come out of them, so it has no battery spawns to show);
- the teams' battery containers;
- the teams' ammo stashes (resupply zones);
- artefact spawns, one toggle per artefact.

Player spawns are drawn as the HUD's player triangle, turned the way the
player faces at spawn. The markers are the game's own HUD minimap icons,
tinted by team.

The map underneath is either the top-down render or the game's own minimap.

## Build: the release does it

`build.py` reads it all from the game as the players have it (through
`srp-026g/scripts/released-game/released.py`, shared with the calculator's build): the stock
`resources.db` with exactly the patches of `release/patches.tsv` over it,
each checked by its sha256. Nothing about a map is typed in by hand: a patch
that adds a map, moves a spawn or redraws a minimap shows up at the next build.

A live release builds it, so no one has to remember to. When main's
`built-from.tsv` names other patches than the release offers (a release with
`--resources` always), `release/release.sh` rebuilds this tab and the
calculator in its preflight, before the go, with `release/build-site`. That
takes about 2 minutes and reads the stock db (`$SKHID_STOCK_DB`) and the
patches (cached in `~/.cache/skhid-release/patches/`). The server's source
carries the rebuilt pages, and the release's record commits them with the
list. A failed rebuild stops the release before players see anything.

By hand, for a local preview, on your own checkout (the same steps; it
reads the owner's `private/release/release.env`):

```
release/build-site --tree . [--with <resources.db.patch_<n>>]
```

or, without that file, this tab on its own:

```
nix build --impure -o py --expr 'with import <nixpkgs>{}; python3.withPackages(p:[p.pillow])'
nix shell nixpkgs#jdk21 -c py/bin/python3 -I srp-026g/scripts/maps-tab/build.py \
  --stock "$SKHID_STOCK_DB" --patches <folder with the list's patches> \
  --dbgrab tools/re-kit/common/dbgrab/target/release/dbgrab --ffdec <ffdec-cli.jar> \
  --icons <the lobby icons, as the damage calculator's build exports them>
# a patch not in the list yet: add --with <its resources.db.patch_<n>>
```

It writes `srp-026g/servers/web-server/maps/`:

- `maps.html`, `maps.js` (the data and `ui.js`);
- `minimap/` and `icons/`;
- `files.rs` (what the server includes);
- `built-from.tsv`.

The patches in `built-from.tsv` are exactly the offered ones on main. The
web-server test `the_maps_and_the_calculator_are_built_from_the_offered_patches`
fails a PR otherwise. A PR that changes the page's code commits its rebuild
(`release/build-site --tree .`, or the commands above). FFDec is 26.0.0: it makes the committed
icons byte for byte.

What comes from where:

| what | where in the game data |
|---|---|
| maps, modes, names | `maps_dict` ids whose `gameplay/game_modes/` config is mode 2, 3 or 4 and whose project exists (the server's rule, `crates/survarium/src/v026g/maps.rs`); names from the localization |
| respawn points, battery spawns, chargers, containers | the project's `server_objects`, shown only where the mode's `rules` read them (88 gather, 89 timed with chargers); a container where its collision zone is (its own position can be a pivot far off: level_04, level_06 and four more) |
| ammo stashes | `client_objects`: each `resupply_zone`'s collision zone, by team |
| marker icons | `hud.swf` `Minimap._icons` (`PlayerBlue`, `Base`, `SpawnIcon`, `ResupplyIcon`), as SVG (FFDec), with each one's lightness and opacity so the page paints them in one blue and one red; the battery (the game's electrobox) is drawn flat after the real one, as the game has no flat icon of it (only a 96x64 photo-like one and an 11x9 glyph) |
| spawn facing | the respawn point's rotation: yaw = y, or pi - y when x and z are pi; forward (-sin yaw, cos yaw) in x/z (on every map the points face the enemy's side) |
| artefact spawns | `client_objects`: each `artefact_container`'s collision zone, its artefacts from the `generic_anomaly_core` that lists it; names from `st_<artefact>_pickup_hint` |
| minimap and its frame | `flash_movies/hud.swf`, `survarium.controls.Minimap` (`_minimapProps`, `_levels`) by the project's `project_name`, as the HUD picks it (exported with FFDec) |
| top-down render | `srp-026g/scripts/damage-calc/assets/maps/` (its README) and the site copy in `servers/web-server/damage/maps/`: the one input not read from the game |

A map without a render stops the build. Render it first, add it to
`damage/maps/` and `damage.rs`, then build again.

The build also stops in these cases, rather than guess:

- the weathers of one map and mode hold different objects;
- the modes of one map hold different artefact spawns.

`--work <dir>` keeps the extracted files between runs while you work on the
page. `--allow-missing-renders` is for the same purpose: a map without a
render then shows its minimap only.

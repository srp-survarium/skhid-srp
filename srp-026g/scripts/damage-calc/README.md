# Damage calculator

A single-page shooter-vs-target damage calculator for 0.26g, in English and Ukrainian. The website serves it at `/damage` (`srp-026g/servers/web-server/src/damage.rs`, spec in `docs/notes/damage_calculator.md`). It covers:

- every weapon, ammo and armour piece, and every body part and distance
- weapon and armour modifications (bonuses only)
- Onyx (passive and active), and drugs (the anesthetic)

The rules follow the client's hit code as the reverse-engineering notes record it (`tools/re-kit/v026g/damage/`, not part of this folder): `formula.txt` for the pipeline, the other `*.md` files for pain, limbs, drugs, artefacts, modifications and coverage, checked against the original exe in unicorn.

## Files

- `dump_json.py`: decodes raw configs to JSON.
- `gen_calc_data.py`: builds `calc-data.json` from the JSON configs.
- `engine.js`: the hit pipeline. Pure functions; also runs under node.
- `rules.js`: modifications, Onyx and drugs.
- `ui.js`: the page logic.
- `template.html`: the page shell.
- `build_page.py`: inlines everything into one HTML file, or with `--site` writes the website's copy into `srp-026g/servers/web-server/damage/`.
- `assets/`:
  - `doll/`: the HUD doll, `hud.swf` sprite 119, split into hit locations by the bones in `base_character_full.skinned_model/hit_targets`.
  - `tools/`: regenerates the doll and the lobby item icons from `resources.db`.
  - `fonts/`: PT Sans, PT Sans Narrow and PT Mono (SIL OFL, `OFL-PT.txt`) and Carlito Bold, subset to Latin and Cyrillic. The website serves them itself: its Content-Security-Policy allows no outside font.

## Build

The page follows the game as Skhid's players have it: the stock `resources.db` with exactly the patches of `release/patches.tsv` over it, each checked by its sha256.

A live release builds it, with the Maps tab: when main's `built-from.tsv` names other patches than the release offers (a release with `--resources` always), `release/release.sh` rebuilds both pages in its preflight, before the go, with `release/build-site` (about 2 minutes, from `$SKHID_STOCK_DB` and the patches, cached in `~/.cache/skhid-release/patches/`). The server's source carries them, and the release's record commits them with the list. A failed rebuild stops the release before players see anything. For a local preview, `release/build-site --tree . [--with <resources.db.patch_<n>>]` runs the same steps on your checkout (it reads the owner's `private/release/release.env`).

Without that file, one command does every step of this page, after the Maps tab's build (`srp-026g/scripts/maps-tab/README.md`, which gives the maps' names, order and minimaps):

```
nix shell nixpkgs#jdk21 -c <python with pillow and numpy> -I srp-026g/scripts/damage-calc/build_site.py \
  --stock "$SKHID_STOCK_DB" --patches <folder with the list's patches> \
  --dbgrab tools/re-kit/common/dbgrab/target/release/dbgrab --ffdec <ffdec-cli.jar>
# a patch not in the list yet: add --with <its resources.db.patch_<n>>
```

It extracts `gameplay/`, the English and Ukrainian texts and `lobby_menu.swf` (newest archive wins), then runs `dump_json.py`, `gen_calc_data.py`, the icons (FFDec, `assets/tools/build_icons.py`) and `build_page.py --site` into `srp-026g/servers/web-server/damage/`, and writes `built-from.tsv` there. On main the patches in `built-from.tsv` are exactly the offered ones: the web-server test `the_maps_and_the_calculator_are_built_from_the_offered_patches` fails a PR otherwise. `python -I build_page.py calc-data.json damage-calc.html icons/icons 22` still makes one standalone file.

The server binary includes `srp-026g/servers/web-server/damage/`: a content patch reaches it through the release's rebuild, and a PR that changes the page's code commits its own (`release/build-site --tree .`, or the command above). The page names each file it loads with `?v=<its hash>`.

The item dictionary (`gameplay/db_static_dictionaries`) decides which items exist: a patch can leave a retired config behind (the one-piece Duty suit in the back slot). Items are keyed by their config's file name, or by the dictionary's name where file names clash (the exo pieces are all `duty_exo_026` / `freedom_exo_026`, one per slot).

The standalone page loads `maps/<key>.webp` from beside itself. `assets/maps/maps.json` holds each map's pixel↔world affine (from the HUD minimap frame), a 64×64 ground-height grid, and how the scale was checked. `assets/hud-doll/` holds the HUD doll's bitmaps and placement; `FINDINGS.md` there records how the client tints it (0x2370e0). Without an icons dir the page has no item pictures.

Text: `T(en, uk)` in `ui.js` and `rules.js`, `data-uk` on the template's own text. Item, map and modification names come from the game's localization (`name_uk`, `label_uk`). The page takes its language from `?lang=`, else from the document (the website sets it).

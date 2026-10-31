# Backpacks from 0.69d

`equipment-069@1` adds the two 0.69d-only backpacks that equipment-046 left out:
the Medic's bag (1357) and the Soldier's backpack (1359). What ships, the balance
proposals and the open questions are in
[docs/notes/backpacks-069.md](../../docs/notes/backpacks-069.md). `items.tsv` is the
balance source. The icon cells, the catalog reader and the asset checks are
equipment-046's (`common.py` loads them).

The models, materials and textures are resource-porter's: `vostok-port
build-armour-069d` (`src/armour_069d.rs`, manifest
`ports/survarium-069d-backpacks-to-026g.json`) converts the 0.69d surfaces, materials
and maps and measures the fit against the stock torsos. These scripts write the items
around that bundle.

## Rebuild

Run from the repository root with Python 3 and Pillow. `inputs.sha256` pins every
input; `base-patches.tsv` pins the published patches through 38. Neither a server
database nor a release working directory is an input.

The 0.69d files are extracted at their archive paths from the 0.69d volumes (a
`.split` list of `resources.00.db` .. `resources.75.db`), one at a time:
`vostok-port extract --db donor-069d.split --path <path> --output <folder>/<path>`
for the two models' folders, their materials and maps, and the icons
`flash_movies/textures/ui_icons/items/equipment/equipment_13_1.png` and
`equipment_13_3.png`.

```sh
vostok-port build-armour-069d --manifest ports/survarium-069d-backpacks-to-026g.json \
  --source-root <0.69d files> --target-db /path/to/stock-026g/resources.db \
  --patch ~/.cache/skhid-release/patches/resources.db.patch_12 ... (base-patches.tsv) \
  --output <bundle>
python srp-026g/scripts/equipment-069/prepare_inputs.py --bundle <bundle> \
  --source069 <0.69d files> --stock /path/to/stock-026g/resources.db \
  --patches ~/.cache/skhid-release/patches \
  --game-data-bin srp-026g/target/release/srp-game-data --out <inputs folder>
python srp-026g/scripts/equipment-069/make_pack.py \
  --inputs <inputs folder> --out private/content-packs/equipment-069-1
python srp-026g/scripts/equipment-069/check_pack.py \
  private/content-packs/equipment-069-1 <inputs folder>
python srp-026g/scripts/equipment-069/balance_audit.py \
  private/content-packs/equipment-069-1 <inputs folder>/base --out <report.json>
```

`prepare_inputs.py --pin` computes the inputs from `items.tsv` and the bundle
instead, and writes `inputs.sha256`. Output folders must be new. A changed pack is a
new version: its folder, `VERSION` in `make_pack.py` and the `pack.json` hash in
`release/content.tsv`. The release patch is made only by
`release/build-content <revision>`.

## Checks

- `check_pack.py` checks:
  - the two configs: the template's parameters, level, faction, price, weight,
    drop weight, the card, the surface name, the sound, and that the built-in
    bonuses are at the promoted strength;
  - ids, names and icon cells clear of the released game and of equipment-046;
  - the six dictionaries are the catalog's;
  - equipment-046's asset checks: surface chunks, LOD0 only, no 16-byte
    config, every shader combination one a released character draws, every
    map shipped;
  - the porter's fit record.
- `balance_audit.py`: no strict stat dominance against released back pieces
  of the same level.
- `client-patches/check_localization_026g.py` for the catalog rows.
- The composed content: `release/build-content`, then `srp-check-game-data` on
  its server data and the porter's `validate-overlay`.

None of these shows the fit or the materials in the game; see the notes for the
in-game checks.

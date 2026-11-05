# Armour from 0.69d

`equipment-069@2` adds 0.69d-only armour: the two backpacks that equipment-046 left out,
the Medic's bag (1357) and the Soldier's backpack (1359), the Snow-20 vest, pants, gloves,
boots and helmet (1878-1881, 1883) and the Delta mask (1882). What ships, the balance
proposals and the open questions are in
[docs/notes/equipment-069.md](../../docs/notes/equipment-069.md). `items.tsv` is the
balance source. The icon cells, the catalog reader and the asset checks are
equipment-046's (`common.py` loads them).

A row's `template` is what its config starts from:

- `native:<id>`: a released piece of the same kind (a back piece or a mask): its parameters,
  defect and modification masks, repair, card layout, regions and sounds, with the row's
  level, faction, protection, weight, drop weight and built-in bonuses;
- `equipment-046:<id>`: an equipment-046 piece's config whole (the Snow-20 copies the Sniper
  set), drawn with the bundle's recolours of its models, with the row's built-in bonuses. The
  row states the copied piece's other numbers, and the build refuses any other.

The models, materials and textures are resource-porter's: `vostok-port
build-armour-069d` (`src/armour_069d.rs`, manifest
`ports/survarium-069d-armour-to-026g.json`) converts the 0.69d surfaces, materials
and maps, and measures each piece: a back piece against the stock torsos, a recolour against
the 0.69d model it recolours (the same vertices and triangles), a mask fitted onto the native
head (`face_items_069d::fit_surface`). These scripts write the items around that bundle.

## Rebuild

Run from the repository root with Python 3 and Pillow. `inputs.sha256` pins every
input; `base-patches.tsv` pins the published patches through 38. Neither a server
database nor a release working directory is an input.

The 0.69d files are extracted at their archive paths from the 0.69d volumes (a
`.split` list of `resources.00.db` .. `resources.75.db`), one at a time:
`vostok-port extract --db donor-069d.split --path <path> --output <folder>/<path>`
for the pieces' model folders (and those of 0.69d's Sniper set, which the Snow-20
recolours), their materials and maps, and the icons
`flash_movies/textures/ui_icons/items/equipment/equipment_13_1.png`, `equipment_13_3.png`
and `equipment_12_85.png` .. `equipment_12_90.png`.

```sh
vostok-port build-armour-069d --manifest ports/survarium-069d-armour-to-026g.json \
  --source-root <0.69d files> --target-db /path/to/stock-026g/resources.db \
  --patch ~/.cache/skhid-release/patches/resources.db.patch_12 ... (base-patches.tsv) \
  --output <bundle>
python srp-026g/scripts/equipment-069/prepare_inputs.py --bundle <bundle> \
  --source069 <0.69d files> --stock /path/to/stock-026g/resources.db \
  --patches ~/.cache/skhid-release/patches \
  --game-data-bin srp-026g/target/release/srp-game-data \
  --equipment046 private/content-packs/equipment-046-4 --out <inputs folder>
python srp-026g/scripts/equipment-069/make_pack.py \
  --inputs <inputs folder> --out private/content-packs/equipment-069-2
python srp-026g/scripts/equipment-069/check_pack.py \
  private/content-packs/equipment-069-2 <inputs folder>
python srp-026g/scripts/equipment-069/balance_audit.py \
  private/content-packs/equipment-069-2 <inputs folder> --out <report.json>
```

`--equipment046` is equipment-046@4's built pack (its make_pack.py's folder): the configs
of the pieces copied whole and the surfaces they draw are inputs. `prepare_inputs.py --pin`
computes the inputs from `items.tsv` and the bundle instead, and writes `inputs.sha256`.
Output folders must be new. A changed pack is a new version: its folder, `VERSION` in
`make_pack.py` and the `pack.json` hash in `release/content.tsv`. The release patch is made
only by `release/build-content <revision>`.

## Checks

- `check_pack.py` checks:
  - each config: the template's parameters, level, faction, price, weight, drop weight,
    built-ins, the card, the surface names, the sounds, and that the built-in bonuses are at
    the promoted strength; a mask's protection on every hit type of its region, worn in the
    mask slot alone over the native head; a copy equal to the equipment-046 piece but for
    its models, built-ins, icon and texts, each of its surfaces the copied piece's mesh
    (positions and indices);
  - ids, names and icon cells clear of the released game and of equipment-046;
  - the six dictionaries are the catalog's;
  - equipment-046's asset checks: surface chunks, LOD0 only, no 16-byte
    config, every shader combination one a released character draws, every
    map shipped;
  - the porter's fit record.
- `balance_audit.py`: no strict stat dominance against released pieces of the same level,
  kind, regions and slots (the copied equipment-046 pieces included).
- `client-patches/check_localization_026g.py` for the catalog rows.
- The composed content: `release/build-content`, then `srp-check-game-data` on
  its server data and the porter's `validate-overlay`.

None of these shows the fit or the materials in the game; see the notes for the
pictures and the in-game checks.

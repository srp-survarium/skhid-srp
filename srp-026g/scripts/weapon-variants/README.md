# Skins of live guns from 0.69d0

`weapon-variants@1` adds the owner's pick of 0.69d0's skins of guns 0.26g has: sixteen golden
copies of their guns (`variants.tsv`: ids, guns, cells, gold prices, 0.69d0's models). What ships
and why: [docs/notes/weapon-variants.md](../../docs/notes/weapon-variants.md).

- `make_pack.py`: the pack (configs, dictionary and compatibility rows, models, materials,
  textures, camouflage masks, scope fits, icons) and the lobby movie's mirror `ui/WeaponVariants1.as`;
- `paint.py`: 0.69d0's paint on 0.26g's diffuse of the gun;
- `mesh.py`: the TOZ-122 "Leather"'s model (0.69d0's mesh and wrap in 0.26g's layout);
- `icons.py`, `swf.py`: the lobby icons from the guns' live icons;
- `names.py`, `names.tsv`: the names in the ten localization tables;
- `check_pack.py`, `check_ui.sh`: the checks below.

## Rebuild

Run from the repository root with Python 3, numpy, scipy and Pillow
(`nix-shell -p 'python3.withPackages(p: [p.numpy p.scipy p.pillow])'`). `inputs.sha256` pins every
input (prepare_inputs.py says which). 0.69d0's files are read with a vostok-port that reads its
volumes (the weapon survey's, `/mnt/hdd/Projects/weapon-survey-20261016/work/target/release/vostok-port`)
from a `.split` list of `resources.00.db` .. `resources.75.db`.

```sh
python3 srp-026g/scripts/weapon-variants/prepare_inputs.py --stock $SKHID_STOCK_DB \
  --patches ~/.cache/skhid-release/patches \
  --game-data-bin srp-026g/target/release/srp-game-data \
  --donor full069.split --donor-port <0.69d0 vostok-port> --out <inputs folder>
python3 srp-026g/scripts/weapon-variants/make_pack.py --inputs <inputs folder> \
  --out $SKHID_PACKS_DIR/weapon-variants-1 [--previews <folder>]
cmp $SKHID_PACKS_DIR/weapon-variants-1/ui/WeaponVariants1.as \
  srp-026g/flash/lobby_menu/src/survarium/constants/WeaponVariants1.as
python3 srp-026g/scripts/weapon-variants/check_pack.py $SKHID_PACKS_DIR/weapon-variants-1 <inputs folder>
python3 srp-026g/scripts/weapon-variants/names.py --inputs <inputs folder> --check
```

`prepare_inputs.py --pin` computes the inputs from variants.tsv and writes `inputs.sha256`; the
players' lobby movie is one of them (its free cells and the guns' icons), so after a release that
changes it the inputs are pinned again. Output folders must be new. A changed pack is a new
version: its folder, `VERSION` in make_pack.py and the pack.json hash in `release/content.tsv`.
The release patch is made only by `release/build-content <revision>`.

A new skin: its row in variants.tsv and names.tsv (claim the dict id and the cell first), pin the
inputs, build, `names.py` (without `--check`) writes its names into the tables, copy the mirror.

## Checks

- `check_pack.py` (its docstring lists them): each skin is its gun but for its model, icon, name,
  no drop and srp_premium; each edit list the release applies names the skin as often as its gun;
  models, materials and textures are the gun's but for the paint (the same shader switches and
  texture formats; the gun's camouflage masks byte for byte); the UV fit; the scope fits; free ids
  and cells; the lobby mirror; the names; paint.py against the premium textures both builds ship.
- `check_ui.sh <lobby_menu.swf> <work folder>` (CPU Ruffle on a virtual display, `VariantsHarness.as`):
  in the built lobby movie each skin is a golden weapon of its gun (the gold background, craft
  donors both ways, the silver bonus first) and its icon is drawn in its cell.
- `client-patches/check_localization_026g.py` for the names.
- `cargo test -p survarium golden`: golden weapons a content pack declares.
- The composed content: `release/build-content`, then `srp-check-game-data` on its server data.

# German mountain rucksack

`mountain-rucksack@1` adds the Bundeswehr mountain troops' rucksack (1877), modelled for 0.26g.
What ships, the proposals and what needs a look in game are in
[docs/notes/mountain-rucksack.md](../../docs/notes/mountain-rucksack.md). `items.tsv` is the
balance source. The icon cell, the catalog reader and the asset checks are equipment-046's
(`common.py` loads them through equipment-069's).

- `model.py` (Blender) builds the mesh, its baked maps and the icon's picture from nothing but
  itself: the glTF the porter reads, four PNG maps, `rucksack_icon.png`.
- resource-porter's `vostok-port build-armour-model` (`src/armour_model.rs`, manifest
  `ports/mountain-rucksack-model-to-026g.json`) converts them to a 0.26g model, material and
  textures and measures the fit against the stock torsos.
- These scripts write the item around that bundle.

## Rebuild

Run from the repository root with Python 3 and Pillow, and Blender (`nix-shell -p blender`).
`inputs.sha256` pins every input; `base-patches.tsv` pins the published patches through 39.
Neither a server database nor a release working directory is an input.

```sh
blender -b -noaudio -P srp-026g/scripts/mountain-rucksack/model.py -- --out <model>
vostok-port build-armour-model --manifest ports/mountain-rucksack-model-to-026g.json \
  --source-root <model> --target-db /path/to/stock-026g/resources.db \
  --patch ~/.cache/skhid-release/patches/resources.db.patch_12 ... (base-patches.tsv) \
  --output <bundle>
cd srp-026g/scripts/mountain-rucksack
python3 prepare_inputs.py --model <model> --bundle <bundle> \
  --stock /path/to/stock-026g/resources.db --patches ~/.cache/skhid-release/patches \
  --game-data-bin ../../target/release/srp-game-data --out <inputs folder>
python3 make_pack.py --inputs <inputs folder> --out <packs>/mountain-rucksack-1
python3 check_pack.py <packs>/mountain-rucksack-1 <inputs folder>
```

`prepare_inputs.py --pin` takes the inputs from the model and the bundle instead and writes
`inputs.sha256` (Blender's bakes are not bit-exact from machine to machine: the pinned files are
the pack's source). Output folders must be new. A changed pack is a new version: its folder,
`VERSION` in `make_pack.py` and the `pack.json` hash in `release/content.tsv`. The release patch
is made only by `release/build-content <revision>`.

## Checks

- `check_pack.py`:
  - the config: the template's parameters, level, faction, price, weight, drop weight, built-in
    bonuses, the card, the surface name, the sound;
  - the id, names and icon cell clear of the released game;
  - the six dictionaries are the catalog's;
  - equipment-046's asset checks: surface chunks, LOD0 only, no 16-byte config, the material's
    shader combination one a released character draws, every map shipped;
  - the porter's fit record, and the bones the model is skinned to.
- `client-patches/check_localization_026g.py` for the catalog rows.
- The composed content: `release/build-content`.

None of these shows the model in the game; see the notes for the in-game checks.

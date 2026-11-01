# weapons-046: the guns of 0.46c2

The rules, the balance and what ships: `../../docs/notes/weapons-046.md`. The pack is made by
resource-porter's weapon core (`port-weapons`, branch `feat/weapons-046c2`) from the port manifest
`ports/weapons-046c2.json` there; this folder holds what skhid-srp adds to it.

| File | What |
|---|---|
| `camo.tsv` | the guns' textures that get an `_aoac` (the camouflage masks, `../weapon-camo/guns.tsv`'s format), for the pack `weapons-046-camo` |
| `parts.tsv` | the textures whose mask paints nothing (sights, barrels, muzzle brakes, rounds); the others are suggested |
| `../kill-feed-icons/make_weapon_icons_046.py` | the hud's kill-feed icons 75, 77-80, 82-84 (0.46c2's), 95 (0.69d0's, the Glock 17) and 99 (the Colt M1911's, drawn from its lobby cell) |

## Rebuild

```bash
# the players' files: the stock resources.db and every offered patch, in order
L=; B=; for n in $(cut -f1 release/patches.tsv | grep -o '[0-9]*$'); do
  L="$L --target-patch ~/.cache/skhid-release/patches/resources.db.patch_$n"
  B="$B --base-patch ~/.cache/skhid-release/patches/resources.db.patch_$n"; done
# 1. the pack (resource-porter checkout of feat/weapons-046c2)
vostok-port port-weapons --manifest ports/weapons-046c2.json --donor <0.46c2 game folder> \
  --target-db $SKHID_STOCK_DB $L --output BUNDLE
vostok-port pack from-bundle --base-db $SKHID_STOCK_DB $B --bundle BUNDLE \
  --output $SKHID_PACKS_DIR/weapons-046-<n> --id weapons-046 --version <n>
# 2. the camouflage masks: a game folder of BUNDLE/overlay's models and textures and
#    textures/character/human/black_market_01/black_market_01_torso_aoac.options (the players')
python3 -I srp-026g/scripts/weapon-camo/weapon_camo.py srp-026g/scripts/weapons-046/camo.tsv \
  <game folder> <empty folder> $SKHID_PACKS_DIR/weapons-046-camo-<n> --pack weapons-046-camo@<n> \
  --bare-masks   # (#949's weapon_camo.py) every surface gets an _aoac, the parts.tsv ones bare
# 3. the kill-feed icons, then the lobby's copies of them
python3 -I srp-026g/scripts/kill-feed-icons/make_weapon_icons_046.py --log-046c2 <0.46c2 log icons> \
  --log-069d0 <0.69d0 log icons> --cells $SKHID_PACKS_DIR/weapons-046-<n>/icons --out srp-026g/flash/hud
python3 -I srp-026g/scripts/kill-feed-icons/make_lobby_weapon_icons.py --patches ~/.cache/skhid-release/patches \
  --ffdec <ffdec.jar> --hud-bitmaps srp-026g/flash/hud/bitmaps --lobby srp-026g/flash/lobby_menu
```

Each pack's row in `release/content.tsv` names its version and its `pack.json`'s sha256; the release
patch is made only by `release/build-content <revision>`.

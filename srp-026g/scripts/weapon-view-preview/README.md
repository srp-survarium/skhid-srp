# The Weapon view window's preview pictures

The 27 bitmaps of `flash/main_menu/bitmaps/SrpPreview*` that `SrpWeaponPreview.as` draws (patch 218,
`docs/notes/weapon-view-settings.md`, "The preview"). Made once, offline; nothing here runs in a
build or a check. NumPy and Pillow, `scripts/sight-alignment`'s renderer, the weapon-modules pack
`release/content.tsv` ships (`private/content-packs/weapon-modules-<version>/files`), and the stock
character models (`srp-game-data get` of `models/character/human/...` and the lobby's animation).

1. The AK-12's eyes as the crate resolves them:

       cargo run -p weapon-modules --example preview_eyes -- <pack>/gameplay/weapon_modules.srpm 363 eotech_552 zenit_1p69 > ak12.json

2. The gun's layers (`SrpPreviewIron<cm><tier>`, `SrpPreviewRedDot<cm><tier>`, `SrpPreviewScope`; each a JPEG and its `.alpha.png`):

       python layers.py ../sight-alignment <pack> ak12.json <out>

3. The world: Vostok (map 123) from `vostok.json`'s two poses (one view at 100 and 20 degrees), headless (`scripts/capture`):

       ~/.cache/skhid-heavy/run --gpu python3 ../capture/capture.py --map 123 --poses vostok.json \
           --out <shots> --game <game folder> --exe <exe with patch 176> --game-data <server-game-data.db>

4. A soldier standing at the poses' target, and the two world JPEGs (`SrpPreviewWorld`, the frame at
   100 degrees; `SrpPreviewWorldScope`, the 20-degree one):

       python world.py ../sight-alignment <character assets> <shots> vostok.json <out>

Copy the PNGs and the JPEGs into `flash/main_menu/bitmaps/` under their names.

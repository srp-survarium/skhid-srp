# The Weapon view window's preview pictures

The 44 pictures of `flash/main_menu/pictures/SrpPreview*` that `SrpWeaponPreview.as` draws (patch 219,
`docs/notes/weapon-view-settings.md`, "The preview"). Made once, offline; nothing here runs in a
build or a check. NumPy and Pillow, `scripts/sight-alignment`'s renderer, the weapon-modules pack
`release/content.tsv` ships (`private/content-packs/weapon-modules-<version>/files`), and the stock
character models (`srp-game-data get` of `models/character/human/...`, the skeleton and the AK's third
person clips), the decal packs (`private/content-packs`) and Vostok's walk grid (`crates/mapdata`).

1. The AK-12's eyes as the crate resolves them:

       cargo run -p weapon-modules --example preview_eyes -- <pack>/gameplay/weapon_modules.srpm 363 eotech_552 zenit_1p69 > ak12.json

2. The gun's layers (`SrpPreviewIron<cm><tier>`, `SrpPreviewRedDot<cm><tier>`, `SrpPreviewScope`; each a JPEG and its `.alpha.png`):

       python layers.py ../sight-alignment <pack> ak12.json <out> <character assets>

3. The world: Vostok (map 123) from `vostok.json`'s three poses (one view at 100, 60 and 20 degrees), headless (`scripts/capture`);
   lane-60 at 1920x1080 (the poses' file says 2560x1440 for the others):

       ~/.cache/skhid-heavy/run --gpu python3 ../capture/capture.py --map 123 --poses vostok.json \
           --out <shots> --game <game folder> --exe <exe with patch 176> --game-data <server-game-data.db>

4. The soldiers of `vostok.json` (`soldiers.py`; their places the walk grid's floor) laid over the three,
   the world JPEGs (`SrpPreviewWorld`, `SrpPreviewWorld60`, `SrpPreviewWorldScope`):

       python world.py ../sight-alignment <character assets> <shots> vostok.json <out> <pack> <content packs> ak12.json

Copy the JPEGs and their `.alpha.png` files into `flash/main_menu/pictures/` under their names.

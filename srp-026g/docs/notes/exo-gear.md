# Exoskeleton boots and gloves: the black areas painted over (pack exo-gear)

The owner (2026-10-29): the Duty and Freedom boots show dark where the socks
should be (the Firefighter boots show them); the gloves a bit of the same.

- Cause: the STK2 donor paints parts of the boots and gloves solid black,
  areas STK2 hid under the trousers and sleeves: the boots' upper shafts and a
  side patch (`legs_diffuse`), the gloves' wrists and a patch on the palm
  (`torso_diffuse`). 0.26g's kits show them. The lobby icons, rendered from
  the meshes (resource-porter `scripts/render-duty-026-icons.py`; the Freedom
  ones recoloured from the Duty ones), show them as a black lid on each boot
  and a black stump at the glove's wrist. The meshes are shared by both sets;
  each set has its own four textures.
- Fix: `srp-026g/scripts/exo-gear/make_pack.py` (pack `exo-gear@1`). In the four
  textures (Duty and Freedom, legs and torso), the flat black areas under the
  boots' and gloves' faces (darker than 0.04, no grain, at least 400 pixels;
  not the soles' undersides, which face down) are painted over with the
  fabric beside them, reflected across the edge so the weave goes on. Dark
  leather with grain stays. The DDS stay DXT1 2048x2048 with 10 mips; only the
  4x4 blocks with painted pixels are encoded again, each mip from the painted
  top level. The four lobby icons are rendered again from the painted
  textures, posed as the old ones, over their own cells (`replaces_sha256`).
- Not changed: the helmets and bodies (their icons have no such area), the
  meshes, the first-person hands' mesh (it uses the same painted texture).
- Check in the game: Duty and Freedom boots with short trousers (the shaft
  tops, the side patch), the gloves' wrists in third person and first person.

## Version 2: the boots' icons posed as the stock boots'

The owner (2026-10-29): the boots need a different angle, as the Zubr boots'
lobby icon (Zubr UM-4, `ren_boots_6`).

- `exo-gear@2` renders the two boot icons as the stock boots are shown: the
  pair seen nearly level from the side and front (yaw 75, pitch 6), toes to
  the left, one boot ahead of the other, fitted to the cell's height (77 px)
  around x 85, with a soft dark shadow to the left (`BOOTS_VIEW`,
  `stock_framed`). The exo boots are squat, so the pair is wider than the
  Zubr's.
- The textures and the glove icons are the same as in `@1`.

## Version 3: the gloves keep their stock icons

The owner (2026-10-29), on the rev 38 local test: the re-rendered glove icon (one
glove alone, fingers down) read as an arm pointing down; "just use old ones".

- `exo-gear@3` renders only the two boot icons (as in `@2`); the Duty and
  Freedom gloves keep their stock lobby icons. The textures are `@1`'s.

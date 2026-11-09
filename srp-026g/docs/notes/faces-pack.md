# The faces pack (face choice: the content)

The content half of the face choice: the faces players choose from, their
helmet and mask variants, the picker's icons and the list the game reads. The
plumbing (the account's face, the exe's cook, the lobby picker) is its own
work; this note is what the pack gives it. The port is resource-porter's
`faces_026` (branch `feat/faces-pack`, `docs/faces-026.md` there); the pack is
`faces@2`.

## What the game reads

`gameplay/bodyparts/default`, `body_parts.heads`, in 0.31a2's form (the
contract with the cook and the server):

```
heads[i].data         {base_model, part_name, additional_models}   the bare face, its hair or scalp an additional model
heads[i].data_helmet  {base_model, part_name}                      worn under a helmet without a mask
heads[i].data_mask    {base_model, part_name}                      worn under a mask (a helmet over it or not)
heads[i].icon         [row, col]                                   the picker's icon, LobbyMenu_EquipmentIcons
```

- A missing `data_helmet` or `data_mask` is the bare face, as in 0.69d0.
- `heads[0]` is today's head (`character/human/base/base_character_head`,
  `black_market_01_head_s`). `body_parts.head` is unchanged.
- `heads[i].icon` is an addition to the spec's format: cell `[i + 1, 16]`
  of the equipment atlas (column 16: the first no released or open pack uses).
- The server reads the count of `heads` from its game data.

| i | face | from | variants |
|---|---|---|---|
| 0 | today's head | 0.26g | none |
| 1 | `069d_head_default` | 0.69d0 `head_default` | helmet, mask |
| 2-4 | `069d_head_01` .. `_03` | 0.69d0 `head_01` .. `head_03` | none |
| 5 | `069d_head_04` | 0.69d0 `head_04` | helmet |
| 6 | `069d_head_05` | 0.69d0 `head_05`, with its hair | helmet, mask |
| 7 | `069d_head_06` | 0.69d0 `head_06` | helmet, mask |
| 8 | `069d_head_07` | 0.69d0 `head_07`, with its hair | helmet, mask |
| 9 | `stk2_neutral_01_head` | STK2, the Novice: his knit cap off, a scalp grafted on | none |

The STK2 loner's face (`neutral_03_head`) and the STK2 bandit's (`bandit_01_head_03`) are left
out (the owner, 2026-11-09).

## The faces

- The 0.69d0 faces are a straight conversion: 0.69d0's vertices, all its bone weights (Chest,
  Neck and Head, at most three a vertex; 0.26g's four-bone skinned layout takes them), normals,
  tangents and UVs, its own helmet and mask variants and hair. Nothing is cut, moved or
  re-weighted. Their teeth, mouth and eyes are meshes of their own inside each face.
- Their materials are 0.69d0's, input for input: albedo, normal map, smoothness (the map baked
  to 0.26g's gloss so that the light's GGX roughness is 0.69d0's: 0.69d0's alpha is
  (1 - s)^2, 0.26g's 0.5 r^6 + 0.1 r), metalness (0.26g's Fresnel from 0.04 to the albedo, the
  diffuse less it), the hair's alpha test at 0.69d0's 0.2 and its two sides.
- No subsurface scattering. 0.69d0 names it in its materials but never reads it: the key is not
  in its exe and no shader of it has one. 0.26g's screen-space pass blurs the diffuse light
  (red widest) over the face, and the first pack had it on.
- The Novice: his model has his knit cap modelled on and no scalp under it. The cap is taken
  off and today's head's scalp grafted on, cut along his skin's open top (over the cap's
  painted shadow) and lifted over his skin. It keeps today's head's skin weights and UVs, so it
  fits every item as today's head does. Its colour map is today's, with the forehead painted
  over by today's short side hair and toned to his skin.

## Why the first pack (faces@1) looked wrong

- **The fit reshaped the faces.** The bare faces were pulled in to stay inside today's head under
  every torso collar in every sampled pose, the lobby idles included.
  - It moved their chins and lower lips back by up to 9 to 16 mm (`069d_head_05` 12.5 mm,
    `069d_head_06` 16.5 mm) and the jaw's sides by up to 15 mm.
  - The teeth are a mesh of their own and the pull never moved them, so they stood in front of
    the lips: the teeth through the mouth.
  - The faces kept today's head's chin and jaw line under 0.69d0's features: the mask-like look.
- **The materials were today's skin, not 0.69d0's:**
  - screen-space subsurface scattering on every surface (0.69d0 has none);
  - the gloss scaled to today's head's 90th percentile, which left the eyes and lips at 0.13 of
    their gloss (dull);
  - a constant Fresnel;
  - the hair's alpha test at 0.5 (0.69d0's is 0.2) and one-sided (0.69d0's hair is two-sided).
- **Ruled out, with evidence:**
  - The native head drawn with the face: the exe's hook replaces the cook's only head block
    (0x205659..0x2056E6: record 0's model and tag), and no other code adds a head record.
  - Face animation:
    - no face bone moves in any clip of either build (1326 clips of 0.26g, 672 of 0.69d0; Jaw,
      LeftEye and RightEye are keyed constant at rest);
    - neither exe has blink, lip-sync or morph code;
    - the 0.69d0 faces are weighted to Chest, Neck and Head only, so the lobby idle moves a face
      with its head (its neck as today's).
  - The tangent frames and normal maps: both builds' normal maps are DXT5 with x in alpha and y
    in green, both meshes' tangent is dP/du and binormal dP/dv, and the texture options (sRGB
    albedo, linear maps) are the same.
  - The bone weights: the conversion keeps all of them.

## The fit

Nothing reshapes the faces. `face_fit`, run by the pack's build, measures every face in every
state and writes the report `faces-fit.json`; the build does not stop on it. The items: 111
torso and back models, 29 helmets, 18 masks and the helmet-over-mask pairs that can be worn, in
the bind pose and 456 poses, against today's head on the same rays.

Under that strict rule (no deeper through an item than today's head on the same ray), today:

- bare: each 0.69d0 face shows through 71 to 89 of 111 torsos, mostly at the neck, which reaches
  4 to 5 cm below today's head inside the collar. The worst is 17 to 18 mm, and 39 mm for
  `head_05`'s mohawk under the STK2 loner's hood.
- helmet: 80 to 104 of 140 worn sets. The worst is about 20 to 22 mm under
  `scavenger_helmet_3`'s neck flap, and 34 mm for `head_05`'s mohawk.
- mask: 482 to 578 of 615.

Making the faces fit is the next step, done 0.69d0's way: its per-face item models
(`models.head_N`) and whatever our live items need beyond them. The owner decides.

## Pictures and checks

- In `private/renders/face-choice/` of the main checkout: before (faces@1) | after (faces@2) |
  0.69d0's own picture of the face (`ui_icons/heads/head_N.png`), labelled.
- They are software renders (resource-porter `render-faces-026`): the colour map under a flat
  light, without the game's shading.
- Not looked at in the game yet: everything of faces@2.

## Release

- One `pack` row in `release/content.tsv`, `faces@2`, built as resource-porter's
  `docs/faces-026.md` says over the stock db and patches 12 .. 53. The release PC keeps it in
  `$SKHID_PACKS_DIR/faces-2`.
- It ships with the server and the exe of the face choice, the server first (the spec). Players'
  clients without the exe's cook ignore `heads` and draw today's head.
- Cosmetic only: no simulation change.

# The faces pack (face choice: the content)

The content half of the face choice: the faces players choose from, how they sit
under the game's helmets, masks and collars, the picker's icons and the list the
game reads. The plumbing (the account's face, the exe's cook, the lobby picker) is
its own work (`face-choice.md`); this note is what the pack gives it and what the
cook reads of it. The port is resource-porter's `faces_026`,
`face_shape` and `face_fit` (branch `feat/faces-pack`, `docs/faces-026.md` there);
the pack is `faces@4`.

## What the game reads

`gameplay/bodyparts/default`, `body_parts.heads`, in 0.31a2's form (the contract
with the cook and the server):

```
heads[i].data         {base_model, part_name, additional_models}   the bare face, its hair or scalp an additional model
heads[i].data_helmet  {base_model, part_name, additional_models}   worn under a helmet without a mask
heads[i].data_mask    {base_model, part_name, additional_models}   worn under a mask (a helmet over it or not)
heads[i].icon         [row, col]                                   the picker's icon, LobbyMenu_EquipmentIcons
```

- A missing `data_helmet` or `data_mask` is the bare face, as in 0.69d0.
- `heads[0]` is today's head (`character/human/base/base_character_head`,
  `black_market_01_head_s`). `body_parts.head` is unchanged.
- `heads[i].icon` is an addition to the spec's format: cell `[i + 1, 16]`
  of the equipment atlas (column 16: free in what players have on 2026-11-18:
  `vostok-port pack check` passes over the stock db and patches 12 .. 64, and no
  other pack of `release/content.tsv` whose folder is at hand draws into it; the
  pack's compose stops on a taken cell).
- The server reads the count of `heads` from its game data.

| i | face | from |
|---|---|---|
| 0 | today's head | 0.26g |
| 1 | `069d_head_default` | 0.69d0 `head_default` |
| 2-4 | `069d_head_01` .. `_03` | 0.69d0 `head_01` .. `head_03` |
| 5 | `069d_head_04` | 0.69d0 `head_04` |
| 6 | `069d_head_05` | 0.69d0 `head_05`, with its hair |
| 7 | `069d_head_06` | 0.69d0 `head_06` |
| 8 | `069d_head_07` | 0.69d0 `head_07`, with its hair |
| 9 | `stk2_neutral_01_head` | STK2, the Novice: his knit cap and hair off, 0.69d0 `head_01`'s scalp joined on |

The STK2 loner's face (`neutral_03_head`) and the STK2 bandit's (`bandit_01_head_03`) are left
out (the owner, 2026-11-09).

## A mask's model for a face

The faces fit the live masks only with a model of the mask for the face (0.69d0's own way for
what is worn on the face, below). `faces@4` ships them; the cook reads them since patch 123's
lookup on `feat/face-choice` (b5c88345a).

In the item's config, `gameplay/items/armour/mask/<item>.options`, beside `data`:

```
data_head_<i>         {base_model, part_name, additional_models, type, ...}   the item as face i wears it
```

- `<i>` is the face: the index into `body_parts.heads` (1 to 9; face 0 has none).
- The entry is the item's `data` with other models (`ported/faces/items/<face>/<the item's
  model>`): the same fields, the same parts in the same order.
- **The rule, 0.69d0's:** for a worn mask, the entry is `data_head_<face>` if the config has it,
  else `data` (0.69d0: `has(models, "head_<n>") ? models["head_<n>"] : models["base"]`,
  0x200150126). The native 0.26g cook has no such check (the key goes straight into
  `operator[]`, which stops on a missing one), so the patch tests that the key is there.
- Everything else the cook does with the entry is as for `data` (its model and part, its
  additional models, `remove_head_model`, the record's tag).
- The skin's name (the cache key) already carries the face (`_f<face>`, patch 123), so two faces
  in the same gear stay two skins.
- **Masks only**, as in 0.69d0 (which looks `head_<n>` up for the mask and face-wear slots only;
  its helmets are keyed by the worn mask's `model_type`). No helmet has a `data_head_<i>`.
- **More than the form said before: `remove_head_model` and an additional model in a per-face
  entry.** 0.69d0's face-wear models for a face are the cloth and the face it shows in the eye
  holes (`head_s`: that face's mask head there), drawn with `remove_head_model` (no head from
  `heads`). Their entries here are the same: `remove_head_model: true`, the cloth as
  `base_model`/`part_name`, and `head_s` of the same model as an additional model, drawn with
  the face's own material (`ported/faces/<face>`, in the model's `settings`). The face's head
  model is not drawn under them.

### The items that have one

155 models, over the stock db and patches 12 .. 65:

- **0.69d0's own** (32): `face_069d_balaclava`, `face_069d_balaclava_3_hole`, `face_069d_keffiyeh`
  and `face_069d_buff`, each for the eight 0.69d0 faces (1 to 8).
- **Carried** (123): `army_026`, `bandit_026`, `bm_mask_1`, `bm_mask_8`, `edge_resp_1`,
  `edge_resp_3`, `eq046_edge_mask_6`, `eq046_edge_mask_7`, `eq069_league_shadow_mask`,
  `renesanse_mask_3`, `renesanse_mask_5`, `scavenger_mask_9`, `scavenger_resp_1` for all nine
  faces, `face_069d_protective_mask` for six.
- The Novice wears the game's four balaclavas (0.69d0 has no model for him; carried, their
  rolled insides came through the cloth).

## How 0.69d0 does it

Read from the retail 0.69d0 x64 exe (sha256 1e1ae04d..., preferred base 0x200000000; names in
`tools/re-kit/v069d/functions.tsv`) and its data (`gameplay/bodyparts/default`, the game options'
`equipment.<item>.models`).

- **The cook** is `profile_skin_visual_cook::on_configs_loaded` 0x20014fa00. It goes through nine
  slots in the order of the table at 0x20095e540: gloves, torso, boots, pants, face-wear (slot 22),
  mask (1), helmet-look (21), helmet (0), back. An item in slot 22 hides slot 1, one in 21 hides 0.
- **One lookup a slot, one fallback:** `has(models, key) ? models[key] : models["base"]`
  (0x200150126..0x200150163). The key is fixed by the slot (the switch at 0x2001500e5):

  | slot | key | from |
  |---|---|---|
  | mask (1), face-wear (22) | `head_<n>` | `sprintf("head_%d", face)`, 0x20014fdd4; `face` is the profile's byte +2F6h, the index into `heads` |
  | helmet (0, 21) | the mask's type | `model_type` of the entry chosen for slot 22, else for slot 1 |
  | torso | the gloves' type | `model_type` of the gloves' entry (`long`, `medium`, `short`) |
  | pants | the boots' type | `model_type` of the boots' entry |
  | back, gloves, boots | `base` | |

- **The head** is `body_parts.heads[i].models.<key>`, key `mask` when a mask or face-wear is worn,
  else `helmet` when a helmet is, else `base` (0x2001507f8..0x20015085d; masks are gone through
  first, so a mask wins); a missing key is `base` (0x200150c9a..0x200150de8). 0.69d0 has eight
  heads; heads 1 to 3 (`head_01` .. `head_03`) have only `base`, head 4's `mask` is its `base`.
  0.31a2 and 0.46c2 spell the same three `data`, `data_helmet`, `data_mask`, which is the form the
  pack uses.
- **`remove_head_model`** is a field of the chosen entry, of any slot (0x200150893..0x200150a17):
  the head is not made from `heads`, and the entry's own model takes the head's record. Only such
  an entry's `additional_models` are drawn (0x20014f380).
- **What has `head_<n>` in the data:** twelve items, all face-wear (category 57): the four
  balaclavas and their versions with glasses, `paintball_mask_1` (each entry a model of the
  balaclava with that face's eyes in it, `remove_head_model`, the glasses an additional model) and
  the three `medical_mask_4` (a mask's model for each face, the head drawn). No helmet, no armour
  mask and no torso has one.
- **What has mask-type keys:** nine event helmets (`victoryday2019_pilotka_helmet`, four
  `victoryday2019_ssh40_helmet_*`, four `victoryday2020_rkka_*_helmet`), a model for each of
  fourteen mask types (`edge10`, `bm10`, `shadow`, ...), which 33 masks declare as `model_type`.
- **0.69d0's own helmet and mask heads are not cuts.** Each has every vertex and triangle of the
  bare head; what differs is where vertices are. The helmet heads press the hair and scalp
  (`head_06`: 244 vertices, up to 27 mm; `head_default`: 19 vertices, up to 2.5 mm; `head_05`: none).
  The mask heads push the face itself in under 0.69d0's masks: about 2,000 of 2,500 vertices,
  the nose and cheeks by up to 7 to 15 mm, the mouth and chin by up to 7 to 20 mm.
- 0.26g's own cook (0x204560) has the same keying for the torso and the pants only
  (`data_<model_type>` in place of `data`, 0x204B1D and 0x2052C6) and no fallback: the key goes
  straight into `operator[]`.

## The faces

- The 0.69d0 faces are a straight conversion: 0.69d0's vertices, all its bone weights (Chest,
  Neck and Head, at most three a vertex; 0.26g's four-bone skinned layout takes them), normals,
  tangents and UVs. Their teeth, mouth and eyes are meshes of their own inside each face.
- Their materials are 0.69d0's, input for input: albedo, normal map, smoothness (the map baked
  to 0.26g's gloss so that the light's GGX roughness is 0.69d0's: 0.69d0's alpha is
  (1 - s)^2, 0.26g's 0.5 r^6 + 0.1 r), metalness (0.26g's Fresnel from 0.04 to the albedo, the
  diffuse less it), the hair's alpha test at 0.69d0's 0.2 and its two sides.
- No subsurface scattering. 0.69d0 names it in its materials but never reads it: the key is not
  in its exe and no shader of it has one.
- The Novice: his model has his knit cap modelled on and no scalp under it. The cap and his own
  hair are taken off at his hairline and a 0.69d0 face's short scalp joined on (below).

## How the faces fit

What 0.69d0 modelled is the reference: a face is never reshaped to fit an item.

1. **Seen skin never moves.** Every vertex that follows the Head (a Head weight of 0.9 or more:
   the face from the hairline to under the chin, the ears, the teeth, eyes and mouth, the skull)
   is 0.69d0's, bit for bit; the build compares each with the donor's and stops on a difference
   (1,589 to 2,496 vertices a face).
2. **The neck joins the body where today's head does.** Every body was made for today's neck: its
   collars stand round it and its neck hole ends on its open edge. A 0.69d0 neck reaches 39 to
   49 mm below that edge and is up to 21 mm wider there (the base of a neck made for 0.69d0's
   bodies). Each face's neck is cut along today's edge, its last ring laid on that edge, and
   the skin that bends with the neck (the neck, the nape, the throat under the jaw) laid onto
   today's neck with today's neck's skin weights: wholly where it follows the Head by half or
   less, not at all from 0.9 up (63 to 334 vertices a face, none above 1.64 m; the chin's
   lowest point is at about 1.55 m).
3. **Under a helmet** a face is 0.69d0's own helmet head where it has one (`head_default` and
   `head_04` to `head_07`: the hair and scalp pressed, the face as it is), else the bare face
   without its hair. **Under a mask** it is 0.69d0's own mask head where it has one
   (`head_default`, `head_05` to `head_07`), else the helmet head.
   - **0.69d0's own helmet and mask heads are not cuts.** Each has every vertex and triangle of
     the bare head; what differs is where vertices are. Its helmet heads move hair and scalp
     only (0 to 225 vertices, up to 14 mm). Its mask heads push the face itself in under 0.69d0's
     masks (1,054 to 1,789 vertices by more than half a millimetre, up to 8 to 18 mm): 0.69d0's
     face-wear is made over them (the face it shows in its eye holes is the mask head's skin).
   - **Skin no one can see is not drawn**, but there is little of it: the live helmets include
     ones that cover almost nothing (`eq046_blackmarket_helmet_7`, `novice_026`) and the masks a
     pair of goggles (`eq046_edge_mask_6`), so what every helmet hides is 4 to 58 triangles of
     the scalp's top, and what every mask hides nothing.
4. **A mask for the face.** Two kinds of `data_head_<i>` (above):
   - **0.69d0's own face-wear**, for each 0.69d0 face, whatever the fit: `face_069d_balaclava`,
     `_balaclava_3_hole`, `_keffiyeh` and `_buff` are 0.69d0's `renesanse_balaclava`,
     `scavenger_balaclava`, `edge_balaclava` and `bm_balaclava`, and 0.69d0 has a model of each
     for each of its eight faces (`<kind>_balaclava_head_0<n>`). Face n gets it as 0.69d0 draws
     it: its cloth (0.69d0's vertices, skin, basis and UVs) and the face it shows in the eye
     holes, no head. One change: the part of the cloth that bends with the neck is laid where
     the game's model of the item has it (tucked into the game's collars; by UV, each vertex as
     much as a face's neck is laid onto today's), as each face's neck is joined to the body.
     0.69d0's other face-wear with `head_<n>` (the balaclavas with glasses, `paintball_mask_1`,
     the three `medical_mask_4`) is not live. The Novice, who has no 0.69d0 model, gets them
     carried as the other masks.
   - **Carried**, for a mask a face shows through or whose seen edge stands off it: the game's
     mask moved from today's head onto the face. Each vertex moves along today's head's normal
     at the point of it nearest the vertex, by how far along that normal the face's skin is
     from that point: an eye hole stays on the eyes (the eye bones, where `mask_fit` put it),
     an edge on the same part of the head, and every vertex keeps the distance from the skin
     its maker gave it. The moves are smoothed (6 rounds), but on the open edges. Then it is
     settled on the face in the bind pose (where the face is still nearer its skin than the
     gate allows, it moves out, at most 16 mm a vertex: 0.69d0's ears are farther back than
     today's, under straps). A mask that fits a face as the game's gets no model for it.
   - No helmet is made for a face.
5. **The gate.** `face_fit` measures every face in every state against every live worn set (111
   torso and back models, 29 helmets, 18 masks and the 486 helmet-over-mask pairs that can be
   worn), in the bind pose and 456 poses, and the pack's build stops on a misfit its open list
   does not name:
   - **showing through**: where an item covers a ray out of the head, the face stays 1 mm (bind
     pose; 0.2 mm in a pose) under its skin, or as far under it as today's head is under the
     game's item on the same ray; where today's head itself is through the item, skin is seen
     there today and a face may show there too;
   - **standing off**: an item's seen edges that lie on today's head stand no more than 4 mm
     farther off the face;
   - **contact** is recorded and is not a misfit: skin a torso does not cover in the bind pose
     (the chin, the jaw) that comes down into its collar when the head bows, where today's chin
     only touches it.
6. **The Novice's scalp.** His cap, the band it shadowed and his own longer hair are taken off
   along his hairline (across the forehead under the band, over the ears, behind them down to the
   nape; his ears whole), and 0.69d0 `head_01`'s scalp above the same line is joined on: its cut
   edge laid on his, the rest moved with it, and a strip of its triangles closing the seam (no
   gap). `head_01` is the short cut whose edge moves least onto his (4.0 mm on average; the
   others 4.1 to 9.4). Its colour map is toned to his skin (his forehead just under the seam over
   `head_01`'s just above it, a gain a channel); its normal map is `head_01`'s, its gloss and
   Fresnel his face's. `faces@2` and `faces@3` grafted today's head's scalp; seen in the game it
   left a pale band across his forehead, a dark gap behind the ears and his own hair below a
   shaved scalp (a skullcap).

### The numbers

Under a mask, `faces@3` -> `faces@4`, over the stock db and the released patches (the gate's
masks worn alone):

| face | masks it shows through as it sits (deepest) | in a pose only | seen edges standing off | models: 0.69d0's own, carried |
|---|---|---|---|---|
| `069d_head_default` | 12 (3.5 mm) -> 0 | 4 (6.6 mm) -> 8 (2.1 mm) | 5 (4.8 mm) -> 1 (9.9 mm) | 4, 13 |
| `069d_head_01` | 13 (5.3 mm) -> 0 | 5 (6.5 mm) -> 6 (4.1 mm) | 9 (6.8 mm) -> 4 (9.7 mm) | 4, 14 |
| `069d_head_02` | 9 (4.9 mm) -> 0 | 5 (5.7 mm) -> 5 (5.2 mm) | 8 (9.0 mm) -> 3 (8.3 mm) | 4, 13 |
| `069d_head_03` | 15 (6.2 mm) -> 0 | 3 (7.8 mm) -> 10 (6.6 mm) | 6 (6.4 mm) -> 4 (7.6 mm) | 4, 14 |
| `069d_head_04` | 8 (10.3 mm) -> 0 | 9 (11.1 mm) -> 8 (8.2 mm) | 11 (13.2 mm) -> 3 (7.7 mm) | 4, 13 |
| `069d_head_05` | 10 (2.5 mm) -> 0 | 3 (2.7 mm) -> 9 (4.9 mm) | 10 (9.2 mm) -> 2 (7.5 mm) | 4, 14 |
| `069d_head_06` | 11 (8.8 mm) -> 0 | 7 (12.4 mm) -> 11 (13.3 mm) | 10 (8.2 mm) -> 2 (11.4 mm) | 4, 14 |
| `069d_head_07` | 8 (4.0 mm) -> 0 | 6 (3.7 mm) -> 10 (5.2 mm) | 9 (9.3 mm) -> 1 (4.0 mm) | 4, 14 |
| `stk2_neutral_01_head` | 10 (4.8 mm) -> 4 (0.7 mm) | 4 (7.4 mm) -> 5 (3.0 mm) | 10 (43.9 mm) -> 9 (43.9 mm) | 0, 14 |

- **`face_069d_balaclava`** (the owner's report): `faces@3` showed `head_default`, `head_01` ..
  `head_07` through it by 1.0 to 9.5 mm as they sit (`head_04` 9.5, `head_06` 7.8, `head_07` 3.7)
  and its edges stood off five of them by 4.2 to 10 mm. In `faces@4` they wear 0.69d0's own
  model of it: no head is drawn under it, nothing shows through.
- Worn sets a face shows through in the mask state (masks, helmets over masks, torsos), of 615:
  417, 431, 288, 398, 456, 229, 471, 351, 376 -> 199, 177, 142, 266, 219, 220, 290, 261, 252
  (`head_default` .. `head_07`, the Novice); the 0.69d0 faces bare and under helmets are as in `faces@3`.
- **In a pose only**: the jaw or throat against the part of a mask that follows the neck when the
  head bows (the models are settled in the bind pose only).
- **Standing off** (20 lines): straps of `edge_resp_1`, `renesanse_mask_5`, `bm_mask_1`,
  `scavenger_resp_1` beside the ears: 0.69d0's ears stand farther back than today's, the strap
  is settled out over the ear and its edge beside it lifts (4 to 11 mm). The Novice's nine are
  his nape: a mask's lower edge lies on today's neck there, his own neck ends higher (as in
  `faces@3`; the bare faces are not changed).

### What is open

`ports/faces-026-open.tsv` (resource-porter) names every misfit left, with the most it may be;
a new one, or a deeper one, stops the build. 398 lines, a face and an item each:

- masks: 130 in a pose only, 20 seen edges standing off beside the ears, 9 the Novice's nape, 4
  the Novice under the game's balaclavas by under a millimetre (no model for him), and no mask
  a face shows through as it sits where it has a model;
- helmets (no model for a face): 39 in a pose only, 21 as they sit, 10 standing off, and 36 a
  helmet over a mask, deeper than under either alone (the ear or the crown under the helmet, the
  face being the mask head there);
- torsos: 129 in a pose only (the throat behind the chin against a collar), 0 as they sit.

- **The torsos**: the throat right behind the chin follows the Head, so it is 0.69d0's and is not moved; it is
  fuller than today's head's, and a high collar that touches today's throat when the head bows in a lobby idle is
  entered by up to 14 mm (the deepest of a face: 1 to 14 mm). The cures are a collar for each face (0.69d0 has
  none either: its torsos are keyed by the gloves' sleeves only) or the owner's word to lay that ring of the
  throat onto today's too.
- **Hair and a hood that is a torso** (`head_05`'s mohawk by up to 33 mm, `head_07`'s hair by up to 28 mm) is in the
  contact record, not the list: the hood does not cover the hair in the bind pose, the hair goes into it in a pose.

## Why the first pack (faces@1) looked wrong

- **The fit reshaped the faces.** The bare faces were pulled in to stay inside today's head under
  every torso collar in every sampled pose, the lobby idles included: chins and lower lips back
  by up to 9 to 16 mm, behind the teeth (a mesh of their own, which the pull never moved), and
  today's head's chin and jaw line under 0.69d0's features.
- **The materials were today's skin, not 0.69d0's:** screen-space subsurface scattering on every
  surface, the gloss scaled to today's head's (eyes and lips dull), a constant Fresnel, the
  hair's alpha test at 0.5 and one-sided.
- **Ruled out, with evidence:** the native head drawn with the face (the exe's hook replaces the
  cook's only head block, 0x205659..0x2056E6); face animation (no face bone moves in any clip of
  either build, neither exe has blink, lip-sync or morph code); the tangent frames and normal
  maps (the same conventions in both builds); the bone weights (the conversion keeps all).

## Pictures and checks

`private/renders/face-choice/` of the main checkout (its `README.md` is the index):

- `v4/ingame/masks/<outfit>.png`: in the game (the lobby's character, a headless client, one
  camera, one exposure): `faces@3` | `faces@4`, a row a face (with 0.69d0's own picture of it),
  front, side and back, under `face_069d_balaclava`, `_keffiyeh`, `_balaclava_3_hole`, `_buff`,
  `bandit_026`, `scavenger_resp_1`, `eq069_league_shadow_mask`, `edge_resp_1` and two helmets
  over masks. The exe: `feat/face-choice` d75dc43a0 (patch 123 reading `data_head_<face>`) as
  the rig's capture build; the server and content: that tree's, `faces@3` or `faces@4` in the
  manifest (a quick build).
- `v4/ingame/novice-bare.png`: the Novice bare, front, both sides and back.
- `v3/`: `faces@2` | `faces@3`, and the software renders of the items made for faces then.

What is checked, and how:

- **By the build** (every time): each face's vertices that follow the Head against the donor's, bit
  for bit; the gate over every worn set and pose; the bundle's models (`validate_overlay_on`).
- **By `vostok-port pack check`**: the pack with the packs it requires.
- **By `tools/check build-content`**: the manifest with `faces@4` composes over what players have.
- **In the game** (the lobby captures above): the ten faces bare and under the masks above, over
  `scavenger_torso_1`, standing straight; the masks' models for faces drawn by patch 123.
  Not in the game: a match, the idles, other torsos, the helmets (not changed).
- **Not built with `release/build-content --full`**: that is the release's step.

## Release

- One `pack` row in `release/content.tsv`, `faces@4`, built as resource-porter's
  `docs/faces-026.md` says over the stock db and the patches of `release/patches.tsv` (12 .. 65
  on 2026-11-19). The release PC keeps it in `$SKHID_PACKS_DIR/faces-4`.
- It ships with the server and the exe of the face choice (patch 123 with its `data_head_<face>`
  lookup), the server first (the spec). Players' clients without the exe's cook ignore `heads`
  and `data_head_<i>` and draw today's head and the game's items.
- Cosmetic only: no simulation change.

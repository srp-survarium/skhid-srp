# The faces pack (face choice: the content)

The content half of the face choice: the faces players choose from, how they sit
under the game's helmets, masks and collars, the picker's icons and the list the
game reads. The plumbing (the account's face, the exe's cook, the lobby picker) is
its own work (`face-choice.md`); this note is what the pack gives it and what the
cook must still learn to read. The port is resource-porter's `faces_026`,
`face_shape` and `face_fit` (branch `feat/faces-pack`, `docs/faces-026.md` there);
the pack is `faces@3`.

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
| 9 | `stk2_neutral_01_head` | STK2, the Novice: his knit cap off, a scalp joined on |

The STK2 loner's face (`neutral_03_head`) and the STK2 bandit's (`bandit_01_head_03`) are left
out (the owner, 2026-11-09).

## What the cook must still read: an item's model for a face

The faces fit the live masks, and some helmets, only with a model of the item for the face
(below: 0.69d0's own way for what is worn on the face). The pack's build makes those models,
but `faces@3` ships without them: today's cook (exe patch 123) draws every item's `data` and
could not draw them, and they have not been seen in the game. This is the data form they come
in, for the exe's change to follow.

In the item's config, `gameplay/items/armour/<helmet|mask>/<item>.options`, beside `data`:

```
data_head_<i>         {base_model, part_name, additional_models, type, ...}   the item as face i wears it
```

- `<i>` is the face: the index into `body_parts.heads` (1 to 9; face 0 has none).
- The entry is the item's `data` with other models (`ported/faces/items/<face>/<the item's
  model>`): the same fields, the same parts in the same order.
- **The rule, 0.69d0's:** for a worn helmet or mask, the entry is `data_head_<face>` if the config
  has it, else `data` (0.69d0: `has(models, "head_<n>") ? models["head_<n>"] : models["base"]`,
  0x200150126). The native 0.26g cook has no such check (the key goes straight into
  `operator[]`, which stops on a missing one), so the patch must test that the key is there.
- Everything else the cook does with the entry is as for `data` (its model and part, its
  additional models, the record's tag).
- The skin's name (the cache key) already carries the face (`_f<face>`, patch 123), so two faces
  in the same gear stay two skins.
- 0.69d0 looks `head_<n>` up for the mask and face-wear slots only; its helmets are keyed by the
  worn mask's `model_type` instead (below). Here helmets have `data_head_<i>` too: an addition
  in 0.69d0's spelling, which the measurements ask for (5 to 11 of the 29 helmets a face, 16 helmets in all).
- Not in the pack, not asked of the cook: 0.69d0's mask-type keys of helmets (no live helmet
  has a model for a mask) and `remove_head_model` in a per-face entry (0.69d0's balaclavas carry
  the face's eyes in the balaclava's model; ours are worn over the face).

### The items that need one

By the gate, over the stock db and patches 12 .. 64: 229 models, of 16 helmets and 18 masks.

- **Masks** (all 18 that draw the head, nearly each for every face): `army_026` (9), `bandit_026` (9), `bm_mask_1` (9), `bm_mask_8` (8), `edge_resp_1` (9), `edge_resp_3` (8), `eq046_edge_mask_6` (8), `eq046_edge_mask_7` (9), `eq069_league_shadow_mask` (9), `face_069d_balaclava` (9), `face_069d_balaclava_3_hole` (9), `face_069d_buff` (9), `face_069d_keffiyeh` (9), `face_069d_protective_mask` (7), `renesanse_mask_3` (9), `renesanse_mask_5` (9), `scavenger_mask_9` (8), `scavenger_resp_1` (9).
- **Helmets** (16 of 29): `blackmarket_helmet_1` (6), `blackmarket_helmet_5` (1), `blackmarket_helmet_9` (3), `edge_helmet_1` (8), `edge_helmet_4` (7), `edge_helmet_6` (7), `eq046_blackmarket_helmet_10` (3), `eq046_edge_helmet_6` (3), `eq046_edge_helmet_7` (7), `novice_026` (4), `renesanse_helmet_1` (2), `renesanse_helmet_2` (2), `renesanse_helmet_5` (8), `scavenger_helmet_10` (1), `scavenger_helmet_3` (9), `stk2_novice_026_hood` (2).
- In brackets: the faces (of 9) the item has a model for. The other 13 helmets fit every face as they are.

**They are not in `faces@3`.** The build makes them (resource-porter `face_shape::Carry`, `face_fit::settle`) and the
gate measures them, but they have only been looked at in software renders, and they are not right yet:

- with them a face shows through a helmet or mask as it sits (the bind pose) in 12 cases of a face and an item; without them, over the face as it is, in 135, and over 0.69d0's mask heads as the pack ships in 117;
- 92 of them stand off the face at a seen edge by more than 4 mm (the carry is smoothed and never under what the skin needs, and the settling only moves out);
- 88 still meet the jaw or throat in a pose (a hood's drape, a balaclava's neck: cloth that follows the neck), as the collars do;
- settling in the sampled poses too pushed such cloth out by up to 48 mm; it is done in the bind pose only, at most 6 mm a vertex.

So the pack ships the faces without them, the open list names every helmet and mask that waits for its model, and
the models come with the cook's change, after the owner has seen them (`v3/offline-items/`).

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
- The Novice: his model has his knit cap modelled on and no scalp under it. The cap is taken
  off and today's head's scalp joined on (below).

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
   without its hair. **Under a mask** it is, for now, 0.69d0's own mask head where it has one
   (`head_default`, `head_05` to `head_07`), else the helmet head.
   - **0.69d0's own helmet and mask heads are not cuts.** Each has every vertex and triangle of
     the bare head; what differs is where vertices are. Its helmet heads move hair and scalp
     only (0 to 225 vertices, up to 14 mm; 0 to 156 of them are in sight under some live helmet, by up to 11 mm: 0.69d0's own look of that face in a helmet). Its mask heads push the face itself in under 0.69d0's masks
     (1,054 to 1,789 vertices by more than half a millimetre, up to 8 to 18 mm), and every one of those vertices is in sight under at least one live mask:
     under a respirator the forehead and cheeks are not quite the bare face's.
   - **They stay in `faces@3` all the same**, as 0.69d0 draws a masked face. A build with the
     face as it is under the masks was looked at in the game (`v3/ingame/trial-masks-over-the-face-as-it-is/`):
     without the masks made for faces it shows through the live masks far more than `faces@2`
     did (the nose and cheeks through `face_069d_balaclava`). When the masks made for faces
     ship, the mask state becomes the helmet head, the face as it is.
   - **Skin no one can see is not drawn**, but there is little of it: the live helmets include
     ones that cover almost nothing (`eq046_blackmarket_helmet_7`, `novice_026`) and the masks a
     pair of goggles (`eq046_edge_mask_6`), so what every helmet hides is 4 to 58 triangles of
     the scalp's top, and what every mask hides nothing. The cut is in the helmet models; it
     cannot do the fitting.
4. **The item follows the face** (built and measured, not in `faces@3` yet: above). A helmet or mask a
   face shows through, or whose edge stands off it, gets a model for that face: the item carried
   from today's head onto the face (each vertex
   keeps the distance from the skin its maker gave it; the moves smoothed so a hard shell keeps
   its shape), then settled on the face in the bind pose. That is 0.69d0's `models.head_<n>`
   (above), and the measurements ask for it widely: 0.69d0's faces are larger than today's head
   at the nose, the jaw and the chin, and the live masks were made close on today's head.
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
6. **The Novice's scalp** is cut along his skin's open top (where his cap sat), its cut edge laid
   on that open edge and the rest of it moved with the edge (`faces@2` hung it 2 mm over his
   skin: a ledge, and facets of it over his forehead). **Not right yet**, seen in the game: the
   ledge and the facets are gone, but the band his cap shadowed stays a pale strip across his
   forehead and temples (his own colour map), and behind the ears, where his skin's open edge
   runs lower than the cut follows, a dark gap stays between scalp and skin down to the nape. A
   trial with the scalp's band laid on his skin under the edge closed the gap and left flaps at
   the ears' tops (`v3/offline/novice-scalp-closeup-shipped-and-band-trial.png`). He needs his
   cap line and colour map worked by hand, or another scalp.

### The numbers

Worn sets a face shows through, of 111 (bare), 140 (helmet) and 615 (mask), over the stock db and
patches 12 .. 64:

| face | bare | helmet | mask | with the items made for the face (bare, helmet, mask) | chin and jaw meet a collar in a pose: collars, deepest |
|---|---|---|---|---|---|
| `069d_head_default` | 43 -> 13 | 57 -> 19 | 495 -> 417 | 13, 19, 252 (7 helmets and 18 masks made) | 83, 18 mm -> 62, 8 mm |
| `069d_head_01` | 38 -> 13 | 50 -> 18 | 489 -> 431 | 13, 15, 203 (5 helmets and 18 masks made) | 78, 18 mm -> 68, 9 mm |
| `069d_head_02` | 47 -> 12 | 58 -> 18 | 427 -> 288 | 12, 16, 125 (6 helmets and 17 masks made) | 79, 17 mm -> 56, 5 mm |
| `069d_head_03` | 42 -> 12 | 52 -> 20 | 447 -> 398 | 12, 16, 207 (10 helmets and 18 masks made) | 83, 18 mm -> 67, 13 mm |
| `069d_head_04` | 41 -> 14 | 53 -> 23 | 526 -> 456 | 14, 19, 234 (11 helmets and 17 masks made) | 86, 18 mm -> 81, 12 mm |
| `069d_head_05` | 22 -> 5 | 31 -> 9 | 431 -> 229 | 5, 10, 163 (6 helmets and 17 masks made) | 61, 34 mm -> 34, 33 mm |
| `069d_head_06` | 49 -> 19 | 63 -> 29 | 528 -> 471 | 19, 23, 237 (11 helmets and 18 masks made) | 86, 18 mm -> 83, 15 mm |
| `069d_head_07` | 44 -> 14 | 58 -> 22 | 474 -> 351 | 14, 18, 254 (11 helmets and 18 masks made) | 89, 28 mm -> 78, 28 mm |
| `stk2_neutral_01_head` | 27 -> 22 | 32 -> 26 | 458 -> 376 | 22, 26, 202 (6 helmets and 15 masks made) | 76, 19 mm -> 73, 13 mm |

Each cell of the first three: `faces@2` -> `faces@3` as it ships. A helmet over a mask counts as a set of its own, so one mask a face
shows through is up to 28 sets. `faces@2` is its shapes measured by this gate (its own, stricter report said
71 to 89, 80 to 104 and 482 to 578).

### What is open

`ports/faces-026-open.tsv` (resource-porter) names every misfit left, with the most it may be;
a new one, or a deeper one, stops the build.

493 lines, a face and an item each:

- 129: a torso or back piece, in a pose only (the throat behind the chin against a collar);
- 117: a helmet or mask shown through as it sits (the bind pose);
- 88: a helmet's or mask's seen edge standing off;
- 85: a helmet or mask, in a pose only (the jaw or throat against cloth that follows the neck);
- 74: a helmet over a mask that is deeper than under either alone;

- **The helmets and masks** wait for their models for the face (above). Until then a face shows through them as
  `faces@2`'s did: by 2 to 20 mm at the nose, the jaw and the chin under the masks made close on today's head.
- **The torsos**: the throat right behind the chin follows the Head, so it is 0.69d0's and is not moved; it is
  fuller than today's head's, and a high collar that touches today's throat when the head bows in a lobby idle is
  entered by up to 14 mm (the deepest of a face: 1 to 14 mm). The cures are a collar for each face (0.69d0 has
  none either: its torsos are keyed by the gloves' sleeves only) or the owner's word to lay that ring of the
  throat onto today's too.
- **Hair and a hood that is a torso** (`head_05`'s mohawk by up to 33 mm, `head_07`'s hair by up to 28 mm; by the
  first measure under `stk2_loner_026_torso`, `renesanse_torso_2` and `scavenger_torso_7`) is in the contact
  record, not the list: the hood does not cover the hair in the bind pose (nothing of it within 2 cm), the hair
  goes into it in a pose. 0.69d0's cook knows no torso that hides hair either.

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

- `v3/ingame/pairs/`: in the game (the lobby's character, a headless client, one camera, one
  scene): `faces@2` | `faces@3`, each face bare, under four helmets, four masks and two helmets
  over masks, from the front, the side and the back, with 0.69d0's own picture of the face.
  The test exe is patch 123's cook as it is today: it draws the faces' new necks, helmet and
  mask models and the Novice's scalp, and every item as the game's own model.
- `v3/offline/`: software renders (`render-faces-026-pairs.py`: the colour map under a flat
  light, the bind pose) of the same sets, `faces@3` as it ships.
- `v3/offline-items/`: the same with the items made for each face on the right: the only
  pictures of those until the cook reads `data_head_<i>`.

What is checked, and how:

- **By the build** (every time): each face's vertices that follow the Head against the donor's, bit
  for bit; the gate over every worn set and pose; the bundle's models (`validate_overlay_on`).
- **By `vostok-port pack check`**: the pack over the stock db and patches 12 .. 64 (232 operations).
- **By `tools/check build-content`**: the manifest with `faces@3` composes over what players have.
- **In the game** (the lobby captures above): the ten faces bare, under `renesanse_helmet_2`,
  `scavenger_helmet_3`, `edge_helmet_4`, `novice_026`, `face_069d_balaclava`, `bandit_026`,
  `scavenger_resp_1`, `eq069_league_shadow_mask` and two helmets over masks, over
  `scavenger_torso_1`, standing straight. Not in the game: a match, the idles, other torsos.
- **Only in software renders**: the items made for faces.
- **Not built with `release/build-content --full`**: that is the release's step.

## Release

- One `pack` row in `release/content.tsv`, `faces@3`, built as resource-porter's
  `docs/faces-026.md` says over the stock db and the patches of `release/patches.tsv` (12 .. 64
  on 2026-11-18). The release PC keeps it in `$SKHID_PACKS_DIR/faces-3`.
- It ships with the server and the exe of the face choice, the server first (the spec). Players'
  clients without the exe's cook ignore `heads` and draw today's head.
- The items' models for faces come as a later version of the pack, with the exe that reads
  `data_head_<i>`; a client without that exe ignores the key.
- Cosmetic only: no simulation change.

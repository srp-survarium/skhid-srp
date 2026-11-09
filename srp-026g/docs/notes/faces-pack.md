# The faces pack (face choice: the content)

The content half of the face choice: the faces players choose from, their
helmet and mask variants, the picker's icons and the list the game reads. The
plumbing (the account's face, the exe's cook, the lobby picker) is its own
work; this note is what the pack gives it. The port is resource-porter's
`faces_026` and `face_fit` (branch `feat/faces-pack`, `docs/faces-026.md`
there); the pack is `faces@1`.

## What the game reads

`gameplay/bodyparts/default`, `body_parts.heads`, in 0.31a2's form (the
contract with the cook and the server):

```
heads[i].data         {base_model, part_name, additional_models}   the bare face, its hair an additional model
heads[i].data_helmet  {base_model, part_name}                      worn under a helmet without a mask
heads[i].data_mask    {base_model, part_name}                      worn under a mask (a helmet over it or not)
heads[i].icon         [row, col]                                   the picker's icon, LobbyMenu_EquipmentIcons
```

- A missing `data_helmet` or `data_mask` is the bare face.
- `heads[0]` is today's head (`character/human/base/base_character_head`,
  `black_market_01_head_s`). `body_parts.head` is unchanged.
- `heads[i].icon` is an addition to the spec's format: cell `[i + 1, 16]`
  of the equipment atlas (column 16: the first no released or open pack uses).
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
| 9 | `stk2_bandit_01_head_03` | STK2, the bandit's bare face |
| 10 | `stk2_neutral_01_head` | STK2, the Novice's face under his knit cap |

The STK2 loner's face (`neutral_03_head`) is left out (the owner, 2026-11-09).

## The fit

- The gate (resource-porter `face_fit`, run by the pack's build; a face through anything stops
  it): every face's bare model against the 111 torso and back models (each torso's distinct
  sleeve variants), its helmet variant under 29 helmets (and the torsos), its mask variant under
  18 masks, the 486 helmet-over-mask pairs that can be worn (and the torsos): 866 worn sets, in
  the bind pose and 456 poses (24 across each of 19 lobby and match clips), every vertex and
  triangle middle. The 8 items with `remove_head_model` draw no face and are left out. The items
  are the live dictionary's, as the stock db and patches 12 .. 53 draw them.
- The rule: where an item covers a ray out of the skull (or, for the neck, out of its axis),
  the face stays 1 mm (bind pose) / 0.2 mm (poses) under the item's skin, or no farther out than
  today's head on the same ray; where today's head itself shows through an item in a pose (up to
  10.9 mm, at the collars in the deepest crouches), a face may show through as deep in that
  region of the head.
- The 0.69d0 necks hung 4 to 5 cm lower than today's head: every face's neck now ends 3 mm above
  today's head's slanted neck edge (1.480 m under the chin, 1.524 m at the nape) and turns with
  the head as today's neck does.
- Where a face showed through, its variant was pulled in (never the item): no per-face item
  model was needed. Every face now has helmet and mask variants. Result, all 30 states passing:

| face | state | vertices pulled | deepest pull (mm) | least margin (mm) | under |
|---|---|---|---|---|---|
| 069d_head_default | bare | 817 | 12.7 | 0.14 | torso/blackmarket_torso_5 |
| 069d_head_default | helmet | 907 | 21.9 | 0.08 | helmet/scavenger_helmet_3 |
| 069d_head_default | mask | 1427 | 21.5 | 0.1 | mask/eq069_league_shadow_mask |
| 069d_head_01 | bare | 741 | 13.9 | 0.02 | torso/duty_exo_026 |
| 069d_head_01 | helmet | 364 | 13.5 | 0.12 | helmet/eq046_edge_helmet_7 |
| 069d_head_01 | mask | 1202 | 13.5 | 0.01 | mask/face_069d_balaclava_3_hole |
| 069d_head_02 | bare | 698 | 14.2 | 0.25 | torso/scavenger_torso_2 |
| 069d_head_02 | helmet | 490 | 14.9 | 0.01 | helmet/edge_helmet_1 |
| 069d_head_02 | mask | 721 | 15 | 0.19 | helmet/eq046_edge_helmet_7 + mask/edge_resp_1 |
| 069d_head_03 | bare | 698 | 13.9 | 0.02 | torso/duty_exo_026 |
| 069d_head_03 | helmet | 574 | 13.5 | 0.09 | helmet/eq046_edge_helmet_7 |
| 069d_head_03 | mask | 838 | 13.5 | 0.08 | mask/edge_resp_1 |
| 069d_head_04 | bare | 742 | 13.3 | 0.1 | torso/duty_exo_026 |
| 069d_head_04 | helmet | 835 | 20.8 | 0.03 | helmet/scavenger_helmet_3 |
| 069d_head_04 | mask | 929 | 15.4 | 0.02 | mask/face_069d_keffiyeh |
| 069d_head_05 | bare | 1163 | 39.5 | 0.04 | torso/scavenger_torso_7 |
| 069d_head_05 | helmet | 1164 | 23.5 | 0.09 | torso/blackmarket_torso_2 |
| 069d_head_05 | mask | 1642 | 24 | 0.02 | helmet/scavenger_helmet_3 + mask/bm_mask_1 |
| 069d_head_06 | bare | 1121 | 17.1 | 0.1 | torso/duty_exo_026 |
| 069d_head_06 | helmet | 1490 | 21.9 | 0.1 | torso/blackmarket_torso_3 |
| 069d_head_06 | mask | 1530 | 22 | 0.1 | mask/edge_resp_1 |
| 069d_head_07 | bare | 1602 | 28 | 0 | torso/eq046_renesanse_torso_3 |
| 069d_head_07 | helmet | 1328 | 29.7 | 0 | torso/eq046_renesanse_torso_3 |
| 069d_head_07 | mask | 1679 | 26.1 | 0.1 | mask/face_069d_balaclava_3_hole |
| stk2_bandit_01_head_03 | bare | 578 | 26.6 | 0.21 | torso/scavenger_torso_5 |
| stk2_bandit_01_head_03 | helmet | 367 | 13.4 | 0.17 | helmet/edge_helmet_6 |
| stk2_bandit_01_head_03 | mask | 1121 | 14 | 0.07 | mask/face_069d_balaclava_3_hole |
| stk2_neutral_01_head | bare | 696 | 12.9 | 0.07 | torso/eq046_renesanse_torso_3 |
| stk2_neutral_01_head | helmet | 364 | 4.2 | 0.07 | torso/eq046_renesanse_torso_3 |
| stk2_neutral_01_head | mask | 1164 | 8.7 | 0 | mask/edge_resp_3 |

- The deepest pulls: `069d_head_05`'s mohawk (39.5 mm) and `069d_head_07`'s hair (28 mm) are
  pulled in under the hoods of the STK2 loner's jacket and the Scavengers' hooded torso 7, which
  are torsos (so the bare face is drawn under them, hair and all); the 0.69d0 napes are up to
  2 cm broader than today's head under helmets with neck flaps (`scavenger_helmet_3`).
- `stk2_neutral_01_head` is the Novice's head with his knit cap modelled on it (its scalp ends
  under the cap, so the cap cannot be taken off): the face wears the cap in every state, pulled in
  under helmets and masks.

## Pictures and checks

- Front and back of every face (today's head first) bare, under six helmets, four masks and three
  helmet-over-mask pairs, over one torso, in the bind pose: `private/renders/face-choice/` of the
  main checkout (`<nn>-<face>-bare-helmets.png`, `<nn>-<face>-masks.png`, and
  `all-<nn>-<item>.png`, every face under one item), from resource-porter's
  `scripts/render-faces-026.py` (a software render, not the game's shading).
- Not looked at in the game yet: the materials (0.69d0's and STK2's maps on today's head's skin
  material), the hair's alpha test, the pulled napes and hair, and the faces in motion.

## Release

- One `pack` row in `release/content.tsv`, `faces@1`, built as resource-porter's
  `docs/faces-026.md` says over the stock db and patches 12 .. 53; the release PC keeps it in
  `$SKHID_PACKS_DIR/faces-1` (a copy of the built folder: `/mnt/hdd/Projects/faces-069d/packs/faces-1`).
- It ships with the server and the exe of the face choice, the server first
  (the spec). Players' clients without the exe's cook ignore `heads` and draw
  today's head.
- Cosmetic only: no simulation change.

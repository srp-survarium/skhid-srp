# Exoskeleton joins: the neck under the helmets, the legs into boots (pack exo-joins)

The owner (2026-11-27): "there is a hole when I wear exoskeleon helment and some armor", "the
helmet is floating in the air"; "there is a hole between legs and boots" (an exo body with other
boots). Live since revision 17; the owner's choice (2026-11-27, of the options below): fix the
pieces (C, parts 1 and 2), not forbid the mix.

## Why

The STK2 exoskeleton was one character: its helmet, body, legs and boots were made to meet one
another only. It was ported as one suit in the back slot covering every other slot (patch 12);
Skhid split it into pieces worn with anything (resource-porter 6598c20, 2026-10-16: body and
helmet; 0e8251f, 2026-10-17: gloves and boots), first shipped in patch 17. Each piece still ends
where its exo partner took over, not where the native body's seams are, which every stock piece
meets:

- **The neck.** The cook draws the native head (`gameplay/bodyparts/default` `body_parts.head`, or
  the face's `heads[n]`) unless an item's `remove_head_model` takes its place
  (`client_patch.md`, how 0.26g draws a character). That head carries the neck every stock collar is
  made around: down to 1.480 m at the throat, 1.524 m at the nape. Both exo helmets
  (`helmet/duty_exo_026`, `freedom_exo_026`: one mesh, `duty_03_head` `helmet_s`, all 1182 vertices
  on Head) set `remove_head_model` and end at 1.523 m in front, 1.56-1.59 m at the sides and back.
  The exo body's own collar (`duty_03_torso_s1` `torso_s`, 86 vertices on Neck, 1.525-1.578 m) was
  their neck. Over any other torso the band between is empty: horizontal rays from the neck's axis
  passed through 26 of the 59 other torsos at 10 degrees and 1 cm (31 at 5 degrees and 5 mm),
  mostly at the throat; from below (the lobby camera's height) the gap shows on every torso. The
  helmet's bind pose is the native one (0.0 mm): it sits where it always did, with no neck under
  it. Stock's two helmets with `remove_head_model` (`renesanse_helmet_4`, `_6`) each bring a
  replacement head with a neck (`renesanse_novice_combat_head`).
- **The ankle.** The exo body covers the trousers' slot and draws its own legs
  (`duty_03_legs_s1`, Freedom's `freedom_exo_legs_s1`, one mesh), so the boots' `model_type` never
  picks a trousers cut for them (stock trousers have three: `_s1` over low boots, down to 6-13 cm;
  `_s2`, `_s3` narrowed into medium and tall shafts). The legs end in a closed, rounded bottom from
  0.245 m to 0.210 m, inside the exo boot's 0.223 m shaft. Every low boot whose top is under 0.21 m
  left the ankle empty: Cossacks (`blackmarket_boots_1`, 0.173 m), Mocassins (`edge_boots_0`,
  0.166), Sorcerer (`eq046_edge_boots_7`, 0.178), Pathfinder (`eq046_scavenger_boots_7`, 0.172),
  Leather boots (`scavenger_boots_1`, 0.179), Nomad (`scavenger_boots_10`, 0.202), Sneakers
  (`stk2_bandit_026_boots`, 0.151), Loner's boots (`stk2_loner_026_boots`, 0.155); Firefighter
  (`scavenger_boots_9`, 0.213) by a few millimetres. In the lobby's idle pose the exo boot itself
  showed a sliver.
- Gloves: the same test at the wrists found no hole either way (exo gloves on stock jackets, stock
  gloves on the exo body).

## The fix (pack `exo-joins@1`, `scripts/exo-joins/make_pack.py`, geometry in `joins.py`)

- **A neck for each helmet.** An additional model of the helmet's `data`
  (`ported/<suit>_026/exo_helmet_neck`, surface `neck_s`; `remove_head_model` stays): the native
  head's neck triangles from its bottom (1.48 m) up to 4 cm above the helmet's rim (its lowest point in each
  5-degree sector about the neck's axis), without the eyes; a vertex the helmet's shell (or a strap)
  passes within 5 mm of, or that is outside it, is drawn in to 5 mm inside (faces between
  vertices can still cross a strap at the left side by at most 1.9 mm). 95 vertices, 139
  triangles. It has the native neck's shape and skin (Neck, Head), so it sits inside every stock
  collar as the native neck does, and follows the head in every pose. In the helmet's own texture
  and material: the dark ribbed rubber of the gas mask's side hood (2048 px texture, u 50-380,
  v 372-500), u round the neck mirrored front to back (no seam), v down it; tangents from those
  UVs. On the exo body it is inside the body's collar (renders: no change). An additional model of
  a helmet is hidden in first person with it.
- **The legs into the boots.** Positions only, in both legs meshes: Duty's file in place
  (replace_file, checked against the released bytes); Freedom's, which pack exo-camo added (a later
  pack may not replace an earlier one's file: vostok-port refuses it), as a model of its own,
  `ported/freedom_026/freedom_exo_legs_s2`, which edit list `exo-joins-1.json` points the Freedom
  body's four sleeve tables at, after the packs: exo-camo@2's `camo_available` on that additional
  model stays, and patch 217 bakes the legs with the body's camouflage under the new file's name,
  `<camo>freedom_exo_legs_s2` (Duty's keeps `duty_03_legs_s1`). The material and UVs are the same,
  so the legs' `_aoac` masks still apply. Both: the closed bottom (below 0.245 m) moved 0.105 m down (to 0.105-0.14 m, under
  the lowest boot's top, 0.151 m), the trousers from there up to 0.42 m stretched to meet it (1.6
  times), and below 0.24 m narrowed over 5 cm towards the native tucked trousers (`_s3`) plus 4 mm,
  along each shin's axis (the bind pose's Leg to Foot): they go into tall shafts as stock trousers
  do, and the hem meets a low boot's collar. The same vertices, triangles and UVs (the camouflage
  masks and decals of `exo-camo` and `exo-decals` stay where they are), normals and tangents of the
  moved band from the new triangles, the skin below 0.24 m the native full trousers' Leg/Foot blend
  (Foot 0.5 at 0.10 m, 0.1 at 0.20 m, none at 0.24 m). The models' and surfaces' bounding boxes and
  counts in `export_properties` follow.
- A first try kept the full width down to the hem (trousers over every boot, as `_s1`): the low
  boots closed as well, but tall shafts' buckles poked through the trousers and the exo boot was
  covered to 0.14 m. The narrowed version looks as before over tall boots and the exo boot.

## Checks

- `scripts/clothing-fit/check_joins.py` (and `test_joins.py`, the `clothing-fit` check): every
  helmet or mask with `remove_head_model` with each torso, horizontal rays from the native neck's
  axis over 10 cm above its bottom; every torso that covers the trousers' slot with each pair of
  boots, rays from each shin's axis from 0.10 to 0.32 m; a ray through everything is a hole. Live
  (revision 74): 68 of 568 pairs have one (26 torsos with each exo helmet, 8 boots with each exo
  body); with the pack and the edit list: none. At 5 degrees and 5 mm: 78 (31 torsos, 8 boots)
  and none. About 45 s on the whole game data. Not in build-content's gate yet
  (it needs numpy, which build-content's Python is not given).
- It does not see clipping, nor gaps that open only in a pose: renders in the lobby idle
  (`ak/player/stand/3rd_view/on_site/idle_lobby` at 0.5, resource-porter's
  `pose-stk2-outfits-026`) and the bind pose, back faces culled as the game draws one-sided
  materials, a magenta background so a hole shows.

## Not changed

The exo frame's foot brackets (`exoskeleton_s`, on the Foot bones down to 3.9 cm) still sit where
the exo boot is; over other boots they stand on or in them (the owner: not now).

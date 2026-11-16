# Mamayev Kurgan: the tunnel top (level_06)

The owner, 2026-11-16, on level_06/evening_8 (map 155) with the `collision` view, standing in the
hollow under the fallen trees at the tunnel's east mouth (`where` 88.098 29.702 136.981): "this
collision stops players from getting on top of that tunnel", "let's remove it", "in 0.69 I think
they did", "so this whole block should be removed of collision". Edit list
`scripts/content-edit/level06-tunnel-top-1.json`.

- **What:** the tunnel is `level/06/object_4` (x 49-99, z 129-148): a floor at y 31-33 and a
  roof at y 34-38, holes in its roof drawn and in its collision alike (a 2-3 m drop into the
  tunnel). The roof has walker and bullet collision of its own, the same surface the model draws.
- **The block:** a ring of 15 invisible walker-only boxes (`editor/walker_collision`, cgroup 2,
  cmask 4, 9-10 m tall from the ground) round the roof: east over the tunnel's mouth (878, 905,
  1047, 1092 in evening_8's `collision_objects`), south to the hill (846, 868, 910, 861, 869, 935,
  1023, 1073, 1044), west (1031, 941). All 15 are taken out. Bullets, grenades and the camera never
  met them.
- **Kept:** the map's outer boundary north of the roof (1060, 1052, 946, 1004, 914 and the rest
  of the chain); everything else of the map is byte for byte the same.
- **Where:** all 15 level_06 projects (day, evening, rain; modes 2, 3, 4, 5, 8): the ring is the
  same boxes in each. Not `v021d_level_06` (the 0.21d port): it has an older fence of its own
  there, left as it is.
- **0.69d:** its level_06 (`default`, `day_tdm`) uses the same level models and has none of the 15
  boxes; it keeps the outer boundary north of the roof. It also added things on top that are not
  ported here: a plank bridge (`generic/favella/bridge_small_003`, two `column_big` poles and a
  walker slab) from the ravine up to the mouth, two concrete wrecks on the roof for cover and a
  walker slab over part of the roof.
- **Ways up** (bot walk grid, steps 0.6 m, jumps 1.3-1.7 m): on foot from the hill south of the
  roof and the hill west of it. The grid gets onto those hills from Free for All's spawns on them
  and, in modes 2 and 3, up the slope from the battery spawn at their south-west foot (55.8 32.4
  115.3); from modes 4 and 5's spawns it finds no way up (it may miss a jump a player makes). From
  the ravine or the hollow: none, a 4-5 m wall.
- **Nothing broken:** the floors newly reached (evening_8, from its spawns) are the roof alone,
  y 32-40, inside the outer boundary (still 9-10 m above the roof). Off the roof a player drops
  into the ravine or into the tunnel through its holes.
- **Simulation:** walker collision is what a player walks on, so it is a `next` row in
  `crates/session/src/v026g/simulation_changes.tsv`.

# Mamayev Kurgan: the tunnel top (level_06)

The owner, 2026-11-16, on level_06/evening_8 (map 155) with the `collision` view, standing in the
hollow under the fallen trees at the tunnel's east mouth (`where` 88.098 29.702 136.981): "this
collision stops players from getting on top of that tunnel", "let's remove it", "in 0.69 I think
they did", "so this whole block should be removed of collision". Having played it (rev63 quick
build), two more: the green over the rock ledge seen from the hollow (`where` 87.445 29.987
137.977), and the walker collision by the tree on the slope east (95.102 35.215 142.673) that keeps
players from getting onto the roof from there ("keep the white/magenta of the tree and rocks"). Edit list
`scripts/content-edit/level06-tunnel-top-1.json`.

- **What:** the tunnel is `level/06/object_4` (x 49-99, z 129-148): a floor at y 31-33 and a
  roof at y 34-38, holes in its roof drawn and in its collision alike (a 2-3 m drop into the
  tunnel). The roof has walker and bullet collision of its own, the same surface the model draws.
- **The block:** a ring of 15 invisible walker-only boxes (`editor/walker_collision`, cgroup 2,
  cmask 4, 9-10 m tall from the ground) round the roof: east over the tunnel's mouth (878, 905,
  1047, 1092 in evening_8's `collision_objects`), south to the hill (846, 868, 910, 861, 869, 935,
  1023, 1073, 1044), west (1031, 941). All 15 are taken out. Bullets, grenades and the camera never
  met them.
- **The slope's way up:** the three boxes by the tree on the slope north-east of the mouth (994,
  1004, 1078: x 85.6-93.8, z 141.5-146.5, up to y 43) go too; 0.69d moved its copies of them about
  1 m north instead.
- **The wall over the ledge:** 17 triangles of the tunnel's own walker mesh
  (`models/level/06/object_4.model/walker_collision`, which every level_06 project places): a thin
  wall over the rock ledge south of the hollow (x 85.8-88.6, z 134.5-135.2, y 33.4-37 in the map)
  that neither the drawn model nor its bullet mesh has. Taken out of the mesh (`remove_faces`:
  indices and face_data lose them, the vertices stay); the ledge below keeps its walker and bullet
  collision. 0.69d's mesh still has the wall.
- **Kept:** the map's outer boundary north of the roof (1060, 1052, 946, 914 and the rest of
  the chain); everything else of the map is byte for byte the same.
- **Where:** all 15 level_06 projects (day, evening, rain; modes 2, 3, 4, 5, 8): the ring is the
  same boxes in each. Not `v021d_level_06` (the 0.21d port): it has an older fence of its own
  there, left as it is.
- **0.69d:** its level_06 (`default`, `day_tdm`) uses the same level models and has none of the 15
  boxes; it keeps the outer boundary north of the roof. It also added things on top that are not
  ported here: a plank bridge (`generic/favella/bridge_small_003`, two `column_big` poles and a
  walker slab) from the ravine up to the mouth, two concrete wrecks on the roof for cover and a
  walker slab over part of the roof.
- **Ways up** (bot walk grid, steps 0.6 m, jumps 1.3-1.7 m): on foot from the slope north-east
  of the mouth (994, 1004 and 1078 out), which the ravine and the hollow reach, so in every mode;
  and from the hills south and west of the roof. The ravine's side under the roof itself stays a
  4-5 m wall.
- **Nothing broken:** the floors newly reached are the roof, the slope's way onto it and, in the
  modes with no spawn on it (Team Deathmatch's day_4: x 46-93, z 120-147), the hill south of the
  roof, y 32-41, all inside the outer boundary (still 9-10 m above the roof). Off the roof a player
  drops into the ravine or into the tunnel through its holes.
- **Spawns:** Free for All's three projects get six more of the owner's picks, three of them on
  the roof (`ffa-mamayev-tunnel-picks-1`, after this list; `ffa.md`).
- **Simulation:** walker collision is what a player walks on, so it is a `next` row in
  `crates/session/src/v026g/simulation_changes.tsv`.

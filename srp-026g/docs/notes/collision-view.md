# Collision view (0.26g)

The owner, 2026-11-16: "can we add a command to console which would show bbs of different invisible
collisions and collisions in general in the game?", "different groups with different colors,
right?". After playing the first build (boxes): "i am not sure showing bbs like that is correct,
since this doesn't give people idea on what is collision and where": draw the collision itself.
2026-11-17, after playing revision 63: "can we make variation which fills in collision? so it is
easier to find holes", and "just slightly bigger buffers for all meshes".

## The command

- **`collision`** in the console (`~`) turns the view on, every group, and prints the options:

  ```text
  collision: on, all (walk green, shot magenta, solid white, anomaly red)
    collision walk    - what stops walking (green + white)
    collision shot    - what stops bullets and the camera (magenta + white)
    collision solid   - what stops both (white)
    collision anomaly - anomaly damage zones (red, dim when off)
    ... fill          - after any of these (or alone): surfaces filled, to find holes
    collision         - again to turn off
  ```

  `collision` again turns it off (`collision: off`).
- **`collision walk`** shows what stops walking: the walk and solid groups. **`collision shot`**
  what stops bullets and the camera: shot and solid. **`collision solid`** the solid group alone.
  **`collision anomaly`** the anomalies' damage zones alone.
  **`fill`** after any of them (`collision fill`, `collision walk fill`, ...) shows the same with
  the surfaces filled (`collision: on, walk (green + white), filled`).
  Typing what is shown turns the view off; anything else of these switches to it. Anything else
  prints `collision: collision [walk | shot | solid | anomaly] [fill]` and the options, and changes
  nothing.
- It works in a match, a replay and while spectating: the lines are those of the camera's player's
  physics world, around the camera. In the lobby it draws nothing.
- Never saved: a new game starts with it off.

## What is drawn

- **The collision geometry itself, as lines**, where the game collides with it:
  - a mesh (walker meshes, hittable meshes, most of a map): the edges of its triangles near the
    camera; an edge a triangle shares with the one drawn just before it (a strip's) is drawn once,
    other shared edges twice, on top of themselves;
  - a box: its twelve edges, turned as the object is;
  - a sphere: three circles; a cylinder: its two rims and four sides; a capsule: its rims, four sides
    and its caps' half circles (12 segments a circle);
  - a compound (a model's primitives, with or without a mesh): each part where it sits.
- **Filled** (with `fill`): a mesh's triangles and a box's six faces are filled too, under their
  lines, in the group's colour, translucent (alpha 77 of 255, about 30 %), brighter the more a
  surface faces up or down (full colour on a floor or a ceiling, 40 % on a wall), so a gap between
  two pieces shows as a hole in the colour. Both faces of a triangle are filled (it is seen from
  either side). Spheres, cylinders and capsules keep their outlines only.
- **The level's collision only:** the static objects of the map's `collision_objects`, by their
  filter group (`cgroup`), each its own colour:

  | name | colour | what it stops | what it is |
  |---|---|---|---|
  | walk (`cgroup` 2) | green | walkers only (bullets and the camera pass) | walker meshes, invisible walls and map boundaries, walkable lids (level_02's ceiling at 48 m) |
  | shot (8) | magenta | bullets and the camera only (walkers pass) | the hittable meshes, camera walls, foliage |
  | solid (0Ah) | white | walkers, bullets and the camera | solid props |

- **Anomaly damage zones** (the owner: "can we draw anomaly damage zones too?"): each damage zone
  near the camera (as below), its real shapes (spheres, boxes, cylinders, capsules) where
  they stand: **red while it damages** (its anomaly has it on), **dim red while it is off**. A zone
  going on or off is read again at once.
- **Near the camera**: within 20 m of where the eye was when the lines were read (a mesh's
  triangles, and each box, sphere, capsule or cylinder whose bounds come that near). The room is
  **262144 vertices** (4 MiB): a line takes two, a filled triangle three, so it holds **131072
  lines**, or fewer with the fill (the lines fill it from its start, the triangles from its end,
  and it is full where they meet). Where that does not fit, the read takes 10 m, then 5 m; where
  even 5 m does not fit, what was found first is kept and the rest is not (the damage zones are
  read first, so they are always kept; which part of the rest is missing is the world's order, not
  the distance). The next read starts at the radius that fitted, one larger when it used under a
  quarter of the room. A filled mesh takes about 1.5 times the room of its lines (a triangle's three
  corners beside its two or three edges' six), so the fill reaches less far where the room is tight.
- **Read when needed, drawn every frame**: the lines are read again when the view is turned on or
  its filter changes, when the eye has moved more than 0.5 m since the last read, and in another
  match's world; between reads every frame draws the same lines (a read is finished before it is
  shown, so a frame never shows half of one). Nothing is read or drawn while no match's world is
  shown (the lobby, loading); a view turned on there starts, with its legend, in the next match.
- **What to expect** (the edges of the maps' collision triangles around every spawn, three a
  triangle, counted on the local map data of level_01, 07 and 09, 2026-11-16; medians, so a little
  less with the strips' shared edges; filled, 1.5 times the room):

  | shown | lines within 20 m | within 10 m | the view | the view filled |
  |---|---|---|---|---|
  | all | 142 000 to 239 000 | 52 000 to 71 000 | 10 m | 10 m |
  | walk (walk + solid) | 49 000 to 72 000 | 18 000 to 22 000 | 20 m | 20 m |
  | shot (shot + solid) | 97 000 to 206 000 | 35 000 to 56 000 | 20 m (10 m where over 131 000) | 10 m |
  | solid | 3 900 to 29 000 | 1 300 to 7 500 | 20 m | 20 m |

- **Behind walls hidden, on walls shown**: the lines are depth-tested, drawn with the game's own
  debug lines, so what stands in front of a surface hides its lines; each end is drawn 1 cm or 0.5 %
  of its distance nearer the eye where it was read, the more, so a line lying on a visible surface
  is drawn over it, not half into it. A filled corner is drawn half as much nearer: over the
  surface, under its lines. The fill is the game's own translucent debug triangles, depth-tested
  and writing depth, drawn before the lines: a filled surface hides the fill and lines behind it
  (the scene, drawn before, shows through).
- **Never drawn**: a player's bodies (the hit model, group 40h, and the character controller, 824h),
  and anything that moves (a battery 0A18h, a grenade 818h). The view shows nothing the map does
  not, so it is no help through walls.

## The legend

- The first frame after it is turned on, the console gets the head
  `collision: the level's static collision within 20 m (less where that is over 131072 lines):` and a
  line for each group shown, for example `collision: walk (green) 541 in the world: stops walkers
  only`, the count of objects over the whole map. The lines go to `srp_patches.<date>.log` too.

## Cost

- Off: one lock and one compare a frame on the game's thread and on the render thread.
- The two rooms (the lines shown and the next read's), 4 MiB each (and 192 KiB of indices),
  are taken from Windows the first time the view is turned on (pages of its own, not an engine arena) and kept until the game closes. If Windows has
  no room, the console says `collision: no room for its lines, still off`.
- A read (on a start, a filter's change, 0.5 m of movement): a walk of the world's objects (750 to
  2500) and, for each mesh near the camera, one query of its BVH a radius tried (at most three).
- Every frame while on: at most eight draw calls of lines (32768 vertices each) and, filled, of
  triangles (32766 vertices each; the game draws each twice, filled and as its own wireframe).

## Not done

- No lines of moving things; no other radius or cap; no merging of shared edges; round
  primitives are not filled.
- Nothing in the lobby's own world.

## How

- Exe patch 156 (`client-patches/rust/patches/src/collision_view.rs`, `client_patch.md`
  section 3.82, `check_collision_view_026g.py`). Nothing a match computes changes.

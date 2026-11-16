# Collision view (0.26g)

The owner, 2026-11-16: "can we add a command to console which would show bbs of different invisible
collisions and collisions in general in the game?", "different groups with different colors,
right?". After playing the first build (boxes): "i am not sure showing bbs like that is correct,
since this doesn't give people idea on what is collision and where": draw the collision itself.

## The command

- **`collision`** in the console (`~`) turns the view on, every group, and prints the options:

  ```text
  collision: on, all (walk green, shot magenta, solid white)
    collision walk  - what stops walking (green + white)
    collision shot  - what stops bullets and the camera (magenta + white)
    collision solid - what stops both (white)
    collision       - again to turn off
  ```

  `collision` again turns it off (`collision: off`).
- **`collision walk`** shows what stops walking: the walk and solid groups. **`collision shot`**
  what stops bullets and the camera: shot and solid. **`collision solid`** the solid group alone.
  Typing the name that is shown turns the view off; another name switches to it. Anything else
  prints `collision: collision [walk | shot | solid]` and the options, and changes nothing.
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
- **The level's collision only:** the static objects of the map's `collision_objects`, by their
  filter group (`cgroup`), each its own colour:

  | name | colour | what it stops | what it is |
  |---|---|---|---|
  | walk (`cgroup` 2) | green | walkers only (bullets and the camera pass) | walker meshes, invisible walls and map boundaries, walkable lids (level_02's ceiling at 48 m) |
  | shot (8) | magenta | bullets and the camera only (walkers pass) | the hittable meshes, camera walls, foliage |
  | solid (0Ah) | white | walkers, bullets and the camera | solid props |

- **Near the camera**: within 20 m of the eye. Where that would be more than **65536 lines** (the
  cap), the frame takes 10 m, then 5 m; where even 5 m is over the cap, the first 65536 lines found
  are drawn and the rest of that frame is not (which part is missing is the world's order, not the
  distance). Each frame chooses again, so the radius follows the camera.
- **What to expect** (the edges of the maps' collision triangles around every spawn, three a
  triangle, counted on the local map data of level_01, 07 and 09, 2026-11-16; medians, so a little
  less with the strips' shared edges):

  | shown | within 20 m | within 10 m | the view |
  |---|---|---|---|
  | all | 142 000 to 239 000 | 52 000 to 71 000 | 10 m (5 m on level_07) |
  | walk (walk + solid) | 49 000 to 72 000 | 18 000 to 22 000 | 20 m (10 m on level_07) |
  | shot (shot + solid) | 97 000 to 206 000 | 35 000 to 56 000 | 10 m |
  | solid | 3 900 to 29 000 | 1 300 to 7 500 | 20 m |

- **Behind walls hidden**: the lines are depth-tested, drawn with the game's own debug lines, so
  what stands in front of a surface hides its lines. Lines lying on a visible surface may flicker
  with it.
- **Never drawn**: a player's bodies (the hit model, group 40h, and the character controller, 824h),
  and anything that moves (a battery 0A18h, a grenade 818h). The view shows nothing the map does
  not, so it is no help through walls.

## The legend

- The first frame after it is turned on, the console gets the head
  `collision: the level's static collision within 20 m (less where that is over 65536 lines):` and a
  line for each group shown, for example `collision: walk (green) 541 in the world: stops walkers
  only`, the count of objects over the whole map. The lines go to `srp_patches.<date>.log` too.

## Cost

- Off: one lock and one compare a frame on the game's thread and on the render thread.
- The lines' room, 2 MiB (and 64 KiB of indices), is taken from Windows the first time the view is
  turned on (pages of its own, not an engine arena) and kept until the game closes. If Windows has
  no room, the console says `collision: no room for its lines, still off`.
- On: a walk of the world's objects (750 to 2500) and, for each mesh near the camera, one query of
  its BVH a radius tried (at most three); at most four draw calls (32768 vertices each).

## Not done

- No lines of moving things; no other radius or cap; no merging of shared edges.
- Nothing in the lobby's own world.

## How

- Exe patch 156 (`client-patches/rust/patches/src/collision_view.rs`, `client_patch.md`
  section 3.82, `check_collision_view_026g.py`). Nothing a match computes changes.

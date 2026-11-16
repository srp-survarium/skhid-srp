# Collision view (0.26g)

The owner, 2026-11-16: "can we add a command to console which would show bbs of different invisible
collisions and collisions in general in the game?", "different groups with different colors,
right?". After playing the first build (boxes): "i am not sure showing bbs like that is correct,
since this doesn't give people idea on what is collision and where": draw the collision itself.

## The command

- **`collision`** in the console (`~`) turns the view on, and off again. The console says
  `collision: on` or `collision: off`.
- **`collision 2`**, **`collision 8`** or **`collision a`** (`0x2`, `0x8`, `0xa` too) turns it on
  showing that group alone; another such line switches the group. Anything else prints the usage and
  changes nothing.
- It works in a match, a replay and while spectating: the lines are those of the camera's player's
  physics world, around the camera. In the lobby it draws nothing.
- Never saved: a new game starts with it off.

## What is drawn

- **The collision geometry itself, as lines**, where the game collides with it:
  - a mesh (walker meshes, hittable meshes, most of a map): the edges of its triangles near the
    camera (an edge two triangles share is drawn twice, on top of itself);
  - a box: its twelve edges, turned as the object is;
  - a sphere: three circles; a cylinder: its two rims and four sides; a capsule: its rims, four sides
    and its caps' half circles (12 segments a circle);
  - a compound (a model's primitives, with or without a mesh): each part where it sits.
- **The level's collision only:** the static objects of the map's `collision_objects`, by their
  filter group (`cgroup`). Each group value is its own colour:

  | group | colour | what meets it | what it is |
  |---|---|---|---|
  | `0x2` | green | walkers only (bullets and the camera pass) | walker meshes, invisible walls and map boundaries, walkable lids (level_02's ceiling at 48 m) |
  | `0x8` | magenta | bullets and the camera only (walkers pass) | the hittable meshes, camera walls, foliage |
  | `0xa` | white | walkers, bullets and the camera | solid props |

- **Near the camera**: within 10 m of the eye. Where that would be more than **6144 lines** (the
  cap), the frame takes 5 m, then 2.5 m; where even 2.5 m is over the cap, the first 6144 lines
  found are drawn and the rest of that frame is not (which part is missing is the world's order, not
  the distance). Each frame chooses again, so the radius follows the camera.
- **What to expect** (the edges of the maps' collision triangles around every spawn, counted on the
  local map data of level_01, 07 and 09, 2026-11-16; medians): the meshes are dense. Within 10 m:
  9 000 to 17 000 walker edges (`0x2`), 34 000 to 49 000 hittable ones (`0x8`, up to 85 000),
  1 000 to 7 500 of both (`0xa`); within 5 m: 2 000 to 4 700, 7 700 to 11 700, 0 to 1 400. So with
  every group the view draws about 2.5 m around the camera; `collision 2` (the walkers' invisible
  walls and lids) about 5 m; `collision a` 10 m on level_01, 5 m on the big maps; `collision 8`
  2.5 m.
- **Behind walls hidden**: the lines are depth-tested, drawn with the game's own debug lines, so
  what stands in front of a surface hides its lines. Lines lying on a visible surface may flicker
  with it.
- **Never drawn**: a player's bodies (the hit model, group `0x40`, and the character controller,
  `0x824`), and anything that moves (a battery `0xa18`, a grenade `0x818`). The view shows nothing
  the map does not, so it is no help through walls.

## The legend

- The first frame after it is turned on, the console gets a line a group: the head
  `collision: the level's static collision within 10 m (less where that is over 6144 lines), a colour
  a group:`, then for example `collision: 0x2 green, 541 in the world (walkers only)`, the count of
  objects over the whole map. The lines go to `srp_patches.<date>.log` too.

## Cost

- Off: one lock and one compare a frame on the game's thread and on the render thread.
- On: a walk of the world's objects (750 to 2500) and, for each mesh near the camera, one query of
  its BVH a radius tried (at most three); up to twelve draw calls of 512 lines.

## Not done

- No lines of moving things; no other radius or cap; no merging of shared edges.
- Nothing in the lobby's own world.

## How

- Exe patch 156 (`client-patches/rust/patches/src/collision_view.rs`, `client_patch.md`
  section 3.82, `check_collision_view_026g.py`). Nothing a match computes changes.

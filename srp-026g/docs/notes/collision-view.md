# Collision view (0.26g)

The owner, 2026-11-16: "can we add a command to console which would show bbs of different invisible
collisions and collisions in general in the game?", "different groups with different colors,
right?", "what if two bbs overlap?".

## The command

- **`collision`** in the console (`~`) turns the view on, and off again. The console says
  `collision: on` or `collision: off`.
- **`collision 2`**, **`collision 8`** or **`collision a`** (`0x2`, `0x8`, `0xa` too) turns it on
  showing that group alone; another such line switches the group. Anything else prints the usage and
  changes nothing.
- It works in a match, a replay and while spectating: the boxes are those of the camera's player's
  physics world, around the camera. In the lobby it draws nothing.
- Never saved: a new game starts with it off.

## What is drawn

- **The level's collision only:** the static objects of the map's `collision_objects`, by their
  filter group (`cgroup`). Each group value is its own colour:

  | group | colour | what meets it | what it is |
  |---|---|---|---|
  | `0x2` | green | walkers only (bullets and the camera pass) | walker meshes, invisible walls and map boundaries, walkable lids (level_02's ceiling at 48 m) |
  | `0x8` | magenta | bullets and the camera only (walkers pass) | the hittable meshes, camera walls, foliage |
  | `0xa` | white | walkers, bullets and the camera | solid props |

- **A box an object**: the box Bullet's broadphase holds for it (its shape's bounds and Bullet's
  small margin), its twelve edges as lines, no faces. Not the shape itself.
- **Within 50 m of the camera**: a box whose nearest point is at most 50 m from the eye. At most
  1024 boxes a frame.
- **Behind walls hidden**: the lines are depth-tested, drawn with the game's own debug lines, so
  what stands in front of a box hides it. A box in a wall shows where it comes out of it.
- **Overlapping boxes stay apart**: each group's boxes are moved in by 1 cm times its row above
  (`0x2` none, `0x8` 1 cm, `0xa` 2 cm), so the same box in two groups shows as two nested outlines,
  not one line over the other. Two boxes of the same group that coincide are one outline.
- **Never drawn**: a player's bodies (the hit model, group `0x40`, and the character controller,
  `0x824`), and anything that moves (a battery `0xa18`, a grenade `0x818`). The
  view shows nothing the map does not, so it is no help through walls.

## The legend

- The first frame after it is turned on, the console gets a line a group: the head
  `collision: the level's static collision within 50 m, a colour a group:`, then for example
  `collision: 0x2 green, 541 in the world (walkers only)`, the count over the whole map, not only
  near the camera. The lines go to `srp_patches.<date>.log` too.

## Cost

- Off: one lock and one compare a frame on the game's thread and on the render thread.
- On: a walk of the world's objects a frame (a map has 750 to 2500), and one draw call a box.

## Not done

- No boxes of moving things, no shapes (meshes, capsules) inside the boxes, no other radius.
- Nothing in the lobby's own world.

## How

- Exe patch 156 (`client-patches/rust/patches/src/collision_view.rs`, `client_patch.md`
  section 3.82, `check_collision_view_026g.py`). Nothing a match computes changes.

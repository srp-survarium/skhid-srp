# The first-person eye stays out of surfaces (patch 158)

The owner, 2026-11-17, on level_06/day_8 (`where` pos 40.512 35.923 170.722, eye 40.529 36.981
170.849, crouched under a fallen log): "this is location where the head (and camera) can very
easily clip into textures", "there are many places like this", "so the fix should be general to
not let camera clip in in similar situations".

- **Why it clips:** the first-person view is the eyes' transform (bone Item): 1.06 m over the
  feet crouched and 1.5 m standing, 0.13 m ahead of the controller's axis. The crouched capsule is
  1.1 m tall with a round top, so under anything a player crouches beneath the eye is within
  2-4 cm of the surface. The near plane is 0.05 m ahead and its corners reach about 0.07 m out, so
  it goes through the surface and the screen shows the inside. At the owner's spot the log's
  underside (`level/06/object_5`) is 1.18 m over the ground; its drawn mesh, bullet mesh and
  walker mesh are the same surface. Where an object has shot collision but no walker collision,
  the capsule itself walks into it, and the eye goes in with it.
- **Where it happens:** wherever a player can crouch under something, on every map, plus every
  object with no walker collision underneath that a capsule fits into.
- **0.69d:** its player has the same capsule (1.8 m standing, 1.1 m crouched, 0.76 m wide). The
  only camera-collision switch in its exe (`disable_camera_collision`) belongs to the scripting
  scene's free camera. Its first-person eye was not traced.
- **The fix (view only):** at update_camera's eye, for a living player's own eye (the first
  person, mode 0, and a spectator's, mode 4):
  - One ray is cast from 0.4 m under the eye, over the body's axis (inside the capsule), to the
    eye and 0.12 m past it. It tests the level's collision that bullets hit, which is what is
    drawn (the lobby camera's filter: not players, not invisible walk-only walls).
  - A hit puts the eye 0.12 m short of it on that way, at once, so the inside is never shown.
  - When the way clears, the eye eases back (95 % in 0.5 s).
  - Then all around (the owner, 2026-11-17, after the first build: "this is still possible in
    other places"): rays from the eye in 14 directions (the six axes and the cube's eight
    corners), 0.12 m long. The nearest hit pushes the eye straight back from it to 0.12 m, at
    once and up to three times a frame, never through what is behind. This catches a bank, a
    slope or a wall beside or in front of the eye, which the way never meets.
  - The way's own surface, already 0.12 m off, never pushes, so the first spot plays as the
    first build (the owner: fixed).
  - A wall a player stands against never comes into it: the capsule keeps the eye 0.25 m from
    walls.
- **What it does not change:**
  - the eyes' transform, the shots (they leave from where they did), the hit models, the
    controller, the movement;
  - the warmup's orbit camera (patch 132's) and a dead player's ragdoll eye (patch 76's).
  - It changes nothing a match computes, so it needs no `simulation_changes.tsv` row.
- **What a player sees:**
  - Crouched under a low log, the view is a few centimetres lower and no longer shows the
    inside.
  - Inside an object with no walker collision, the view stays under the object's surface, down
    to 0.4 m below the eye.
  - The crosshair still aims from the real eye: a few centimetres of difference, too little to
    matter at any range.
- **Not covered:**
  - geometry with no shot collision at all (bushes, grass, foliage): they are meant to be walked
    through;
  - drawn surfaces that stand off their collision. At the owner's second spot (level_06/day_8,
    `where` 14.681 35.905 142.948, crouched in a wedge under a bank), the terrain's blend layer
    `ground_02_s_alpha_vc` of `level/06_v1/terain1` hangs up to 0.24 m off the terrain's
    collision. The eye was 0.20 m from that drawn lip and 0.43 m from any collision, so no
    collision test sees it. Elsewhere around that spot the layer lies on the collision (median
    0, 90 % within 1.3 cm). Fixing these needs content: collision where the drawn lip is.
  - a scope's zoom (the near plane is 0.05 m times the zoom, so up to about 0.2 m when aiming).
- **Not played:** run with Unicorn only (`check_head_clearance_026g.py`).

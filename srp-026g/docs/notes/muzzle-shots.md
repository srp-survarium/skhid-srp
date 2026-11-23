# Shots from the muzzle (patch 200)

The owner, 2026-11-23: "Bullets leave the barrel, not the eye. Full simulation of where the weapon
looks, there shots go, with simulation with recoil being the next thing on top." The proof is
`client_patch.md`; the aimed view that goes with it is `aim-pose.md` and patch 197.

## The rules

- **Where a bullet starts:** the gun's fire point (the model's `barrel_point`, the weapon's
  +6E0h), on the gun as the simulation poses it. Fully aimed that is 5 to 10 cm under the eye's
  line and 30 to 110 cm ahead of the eye.
- **Where it goes:** the way the gun looks.
  - From the hip: along the barrel (the fire point's own axis), wherever the hands hold it.
  - Fully aimed: along the eye's line. The aim's alignment lays the sights on that line, so the
    bullet goes where the sights show, as before; only its start is lower.
  - While the sight is raised or lowered: between the two, by the aim's progress.
- **The spread** is the game's, around that way, and the kick and the camera's own motions turn
  the eye and the hands as they did: the gun's pose is made from both.
- **A gun not held ready** (its barrel more than 5 degrees off the eye's line: a sprint's pose,
  the hands between two poses) fires as the game always did, from the eye along its line. The
  hands' clips were not made as ballistics; a gun held ready is 0.5 to 2.4 degrees off (below).
- **A wall between the eye and the muzzle:** the shot leaves the eye instead (along the same
  way), so it meets that wall. No bullet starts beyond what the barrel pokes through.
- **Who:** every player, in every game, computed in the simulation from the simulation's own
  state (the animation tree, the weapon's records, the aim's progress). A bot's shots are a
  player's.
- **What does not change:** the recoil's numbers, the spread's, the damage, the bullets' flight,
  the world's format (a bullet's start is a place, as before).

## Why the direction is not the barrel's axis when aimed

The fire point's axis is within 0.6 degrees of the sights' line on the guns measured (AK-74M
0.23, PM 0.58, the others 0 to 0.2): a modelling error, not a zero. A gun's sights must be true,
so fully aimed the bullet goes along the line they are laid on. The barrel's own axis is used
where no sight is looked through.

## What follows from it

- **From the hip the centre of the screen is not the bullet.** The HUD's crosshair is put where
  the own player's gun looks: the place its shot's line first meets the level (150 m along it
  otherwise), through the frame's camera (the hud movie's `srp_crosshair_at`). Drawing only. The
  movie's side is run in Ruffle (`scripts/muzzle-crosshair`, `tools/check muzzle-crosshair`): a
  place in parts of the screen is that pixel at 1920x1080 and 1280x720, and 0.5, 0.5 is where the
  movie's own layout had the crosshair.
- **The gun is posed at the shot's own time.** The game's copy was the eye of the step before (up
  to 50 ms old), which in a burst carried the hands clip's camera turn of the pass before (the
  H&K 416, the M4A1: up to 0.67 degrees). Those bullets now go where the sights show.
- **Bots** aim their view. From 10 m on they fire down the sights (`bots/reflex.rs`,
  `ads_near_m`), where a shot's way is the eye's line as before. Nearer (15 % of their near
  targets) or running for cover they fire from the hip: there a bot turns its view by the gun's
  measured line and muzzle place (`hip_line_deg`, `hip_muzzle_m`, `hip_turn`), so that the gun's
  line is on its aim.
- **Replays** of games before the revision that ships this play out otherwise: a row in
  `simulation_changes.tsv`.
- **Peeking:** a bullet no longer clears a ledge the eye sees over and the barrel does not.

## Measured on real matches' memory

Three full-memory dumps of the owner's game inside matches (2026-11-20 and 2026-11-22, 33 held
guns) and two in the lobby. The game's own `compute_bone_matrix` was run under Unicorn on each
dump's memory for every held gun's bones, and patch 200's rig made of them
(`private/scope-shot-045/muzzle-shots/dump_pose.py`).

- **The bone function as the patch calls it gives the game's own values.** Bone Item times the
  player's transform is the eye the game kept (+44Ch) to 0.0 mm and 0.000 degrees, for every
  player of a match.
- **The hip pose is what the game draws.** For the players drawn in the first person the rig's
  fire point is the one `weapon::draw` kept (the weapon's +620h): 0.0 cm apart in the lobby, 0.7
  and 1.8 cm in a match (a frame's motion between the two).
- **A gun held ready from the hip** (eleven guns): its barrel is 0.5 to 2.4 degrees off the eye's
  line, on average 1.0 right and 0.3 down; its muzzle 10 to 15 cm right of the eye, 8 to 16 cm
  under it and 44 to 92 cm ahead. So from the hip a bullet lands right of the screen's centre:
  about 13 cm, plus 1.7 cm a metre. The HUD's crosshair shows it.
- **A gun not held ready points elsewhere:** a sprinting player's 54 to 58 degrees left and down,
  some states' 73 degrees up, a dead player's 125. Hence the rule of a gun not held ready: a shot
  fired as the hands come out of such a pose is the game's own. **Not known:** how often that
  is (a trigger pressed in a sprint fires at once, by the bots' model of the cadence);
  `SKHID_SHOT_LOG` in a game tells.

### The built exe's shot on those matches

The same dumps, the built exe's own code and arena laid over the dump's memory
(`private/scope-shot-045/muzzle-shots/dump_shot.py`): `instant_fire` run under Unicorn from its copy
of the eye to its call of `get_dispersed_bullet_direction`, for every held gun, beside the original
exe's code on the same memory. The hook, the Rust, the engine's `compute_bone_matrix` and the
level's own `ray_test` on the match's collision all run for real; only GetCurrentThreadId is stood
in.

- **Eleven guns held ready** (from the hip, half raised, fully aimed): the shot leaves the muzzle,
  within 0.05 mm of the place and 0.005 degrees of the way this note's arithmetic gives from the
  same bones; one ray of the level's collision from the eye to the muzzle each (46 to 93 cm),
  nothing in its way.
- **Twenty-three guns not held ready** (sprinting, other states, the dead, the lobby's): the copy
  is the original's, bit for bit.
- The stack at the call is the original's in every case; no run failed.

## Recoil, on top (the owner: "simulation with recoil being the next thing")

The game's recoil is already a state of the weapon in the simulation: the kick (the recoil
calculator, the weapon's +468h; its top kick at +478h) is added to the player's look pitch, a shot
raises it, a downward turn of the mouse is taken off it first and a controller returns it
(`bots/recoil.rs` has the proof). The look pitch with the kick poses the eye, the hands and the gun
together, so in the stock game the kick is a turn of the screen.

With the bullet following the gun, the kick is the gun's motion and the eye need not take all of
it. Patch 197 draws it so, always (its trailing view alone is the prototype behind
`SKHID_CAMERA_TRAIL`): of the lift the kick gives the eye the view takes 40 % (`KICK_SHOWN`), from
the hip too; the gun stays on the
simulation's aim, so it rises over the screen as the kick grows and comes back as the player pulls
it down. Nothing the simulation computes changes for it: the kick, its numbers and where the
bullets go are the game's. Not while the scope's own view is on (its picture is the screen).

Not done: the side kick (about 0.1 degree a shot: it scrubs the `recoil_horiz` clip, whose turn
on the screen is not traced) stays a turn of the view.

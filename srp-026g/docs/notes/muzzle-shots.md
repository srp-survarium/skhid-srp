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
  otherwise), through the frame's camera (the hud movie's `srp_crosshair_at`). Drawing only.
- **The gun is posed at the shot's own time.** The game's copy was the eye of the step before (up
  to 50 ms old), which in a burst carried the hands clip's camera turn of the pass before (the
  H&K 416, the M4A1: up to 0.67 degrees). Those bullets now go where the sights show.
- **Bots** aim their view. From 10 m on they fire down the sights (`bots/reflex.rs`,
  `ads_near_m`), where a shot's way is the eye's line as before. Nearer, or running for cover,
  they fire from the hip: those shots now go where their gun looks, and so do the ones fired
  while the sight is still coming up. Their aim does not take the gun's line yet: how far the
  hip pose turns it from the view is not measured (`SKHID_SHOT_LOG` gives it in a game).
- **Replays** of games before the revision that ships this play out otherwise: a row in
  `simulation_changes.tsv`.
- **Peeking:** a bullet no longer clears a ledge the eye sees over and the barrel does not.

## Recoil, on top (the owner: "simulation with recoil being the next thing")

The game's recoil is already a state of the weapon in the simulation: the kick (the recoil
calculator, the weapon's +468h; its top kick at +478h) is added to the player's look pitch, a shot
raises it, a downward turn of the mouse is taken off it first and a controller returns it
(`bots/recoil.rs` has the proof). The look pitch with the kick poses the eye, the hands and the gun
together, so in the stock game the kick is a turn of the screen.

With the bullet following the gun, the kick is the gun's motion and the eye need not take all of
it. Patch 197 (the aimed view's prototype, `SKHID_CAMERA_TRAIL`) draws it so: of the lift the kick
gives the eye the view takes 40 % (`KICK_SHOWN`), from the hip too; the gun stays on the
simulation's aim, so it rises over the screen as the kick grows and comes back as the player pulls
it down. Nothing the simulation computes changes for it: the kick, its numbers and where the
bullets go are the game's. Not while the scope's own view is on (its picture is the screen).

Not done: the side kick (about 0.1 degree a shot: it scrubs the `recoil_horiz` clip, whose turn
on the screen is not traced) stays a turn of the view.

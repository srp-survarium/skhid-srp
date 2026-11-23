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

- **From the hip the centre of the screen is not the bullet.** The HUD's crosshair has to show
  where the barrel looks (to do, with the HUD movie).
- **Bots** fire from the hip (`key::FIRE` alone, `bots/aimpoint.rs`) and aim their view: their
  shots now go where their gun looks. Their aim has to take the gun's line (to do).
- **Replays** of games before the revision that ships this play out otherwise: a row in
  `simulation_changes.tsv`.
- **Peeking:** a bullet no longer clears a ledge the eye sees over and the barrel does not.

## Next, on top (the owner: "simulation with recoil being the next thing")

With the bullet following the gun, recoil can be made the gun's own motion instead of a turn of
the eye. Not in this patch.

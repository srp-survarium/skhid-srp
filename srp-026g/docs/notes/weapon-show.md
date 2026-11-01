# A weapon shown while aimed keeps its body (patch 91)

The owner's screenshot (2026-11-01): after a resync the held rifle's body was gone; only the
sight and the hands were drawn. The proof is `client_patch.md` section 3.60.

## The rule

- **A weapon shown is drawn whole:** its body and its idle sight, whatever its "scope view" byte
  (+181Eh) says when it is shown.
- **The next frame aims:** the stock draw puts the aimed sight in, and takes the body out (and
  hides the hands) only for an optic with `hide_weapon_on_aim` (the full-screen scopes: PSO-1,
  ACOG, Leupold, ...). A red dot (EOTech, Aimpoint, Cobra, Docter, DeltaPoint: 101 of the 188
  scope configs of revision 39's data) keeps the gun under its aimed sight, as it does without a
  resync.
- **Releasing the aim** gives the idle sight and the body back, for both kinds of optic.
- **When a weapon is shown aimed:** a world load (a resync, a late join, the first world) shows
  the held weapon with the world's +181Eh; that is where the body was lost.

## What it does not change

- **Drawing only.** +181Eh is the world's (serialized, in the world's hash): the patch never
  writes it, nor anything a match computes. No `simulation_changes.tsv` row; games with and
  without it play together.
- **Other players' weapons** and a weapon shown without its scope view were already drawn this
  way: the same as before.
- **Not fixed here:** weapon::on_hide while the aimed sight is in the scene removes the aimed
  sight and not the body (0x243FE0 tests +181Dh alone, where 0.10b also tested
  `hide_weapon_on_aim`), so for a red dot the body stays in the scene until the weapon is shown
  again (then the renderer refuses the second add). Not seen in a game; left as it was.

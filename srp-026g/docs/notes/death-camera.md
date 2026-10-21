# The camera on the death screen (patch 46)

The rules the code and the tests follow. Code: `client-patches/death_camera_026g.asm`
(section `.srpc`), README patch 46, `client_patch.md` section 3.43; checks:
`check_death_camera_026g.py`, `selftest_026g.py` check 46.

The owner (2026-10-21): "when player dies they can still move camera around, with holding a
mouse button? Scroll shouldn't work though".

- **Today (the original):** after a death the camera stays in first person on the corpse's
  eyes, the kits and the cursor are shown until the respawn (8 s by the modes' options), the
  cursor takes every mouse move, and the wheel does nothing.
- **When:** the death screen: the local player dead, the kits shown, the camera in first
  person. Ends at the respawn (the kits hide).
- **Button:** a drag, as the warmup camera before the start (patch 31) and the lobby's
  character view: the right button (and the 5th and 7th) turns at once; the left button
  turns once it has moved more than 8 units, so a click on a kit still picks it. During the
  drag the cursor is off the screen (a drag begun on a kit picks nothing) and it is back where
  the drag began at the release.
- **Effect (the owner, after the first test, 2026-10-21: "sometimes you can see inside of
  models"; "still orbit around eyes but take a pose into consideration"):** the camera stays
  at the corpse's eyes and the drag turns it as the head would turn, relative to the head's
  own pose, read every frame (the head bone's transform, player+44Ch): left/right about the
  head's up, then up/down about the turned right, the step of the player's own mouse look
  (sensitivity, invert). No per-pose tables: whatever pose the body falls in, the look stays
  in a cone in front of the face: at most **70 degrees to either side, 60 up, 40 down** of
  the face's forward (40 down stays short of the chest; to be tuned in the game). Releasing
  the button stops; the view stays where it was turned until the respawn. Not an orbit.
- **The head itself:** not drawn for the local player in first person (the game's
  set_head_visibility, every frame, dead too), so the eyes never show the inside of the face;
  no offset of the eye point.
- **Without a button:** mouse moves are the cursor's, as before.
- **The wheel:** does nothing on the death screen, with or without a button (no zoom, no
  distance).
- **Respawn:** the turn is cleared; the new life's camera is the original's. A button still
  held at the respawn does nothing (patch 31's warmup_keys).
- **Not changed:** the kill info, the HUD, the kit selector and its picks, the respawn time,
  spectating and replays (another mode, or no kits shown), the warmup camera before the start
  (a death before the start keeps patch 31's orbit and wheel).
- **Lockstep:** camera only. A dead local player sends no input (game_world::tick), and the
  drag's moves never reach the player's input handler; nothing a match computes changes, no
  row in `simulation_changes.tsv`, replays unchanged.

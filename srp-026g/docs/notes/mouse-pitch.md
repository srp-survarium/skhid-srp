# The mouse look's pitch at every window (patch 110)

The rules the code and the checks follow. Code: `MOUSE_PITCH` in
`client-patches/patch_026g.py`, `death_camera_026g.asm` (patch 46); README patch 110,
`client_patch.md` section 3.69; checks: `check_mouse_pitch_026g.py`,
`check_death_camera_026g.py`, `selftest_026g.py` checks 110 and 46.

- **Report:** Discord suggestions thread "Чувствительность мыши на широкоформатных
  разрешениях экрана" (2026-11-03): the mouse is right only at 4:3; at 16:9 looking up
  and down is slower than turning. Only in the character's look, not the menu cursor.
- **Stock 0.26g** (`player_input_handler::on_mouse_move` 0x240200; 0.10b's source and
  0.21d's 0x17ED3D are the same formula), s = the sensitivity option x 0.1 x the aim's
  fov factor (handler+94h, both axes):
  - yaw: x * pi/180 * s radians a count;
  - pitch: y * pi/180 * s * h/w * 3/pi look-pitch units a count, h/w the render
    window's height over its width; a unit is 60 degrees of view up and 67.5 down (the
    first-person camera rides the look clip, `abi.tsv` 0x2FBF90);
  - so pitch radians / yaw radians = h/w looking up, 1.125 h/w looking down: 0.75 /
    0.84 at 4:3, 0.80 at 5:4, 0.625 at 16:10, 0.5625 / 0.63 at 16:9, 0.42 / 0.47 at
    21:9. 4:3 is not 1:1 either: it is the aspect the game was tuned at.
- **Retail 0.69d0** (`player_input_handler::on_mouse_move` 0x20023ECE0,
  `tools/re-kit/v069d`) keeps the same h/w x 3/pi; it only adds a separate aim
  sensitivity. No later build made the pitch aspect-free.
- **The change:** h/w is 0.75 (a 4:3 window's) at every window. Every window turns as
  a 4:3 one did, bit for bit: 4:3 players feel no change; wider windows get the 4:3
  pitch (16:9 +33 %, 21:9 +78 % vertical).
- **Same path, so covered:** the aimed look and scopes (the fov factor scales both axes
  alike), the warmup camera before the start (patch 31: mode 3 of the same function).
- **The death camera** (patch 46) copied the look's step: its h/w is the same 0.75, so
  it still turns as the player's look does.
- **Not changed:** the menu cursor and the lobby's character drag (`lobby_camera`
  0x283D30, menus), the keyboard look actions (`process_first_person_mode`, not the
  mouse), the input journal (a developer tool). No setting, no option.
- **Lockstep:** the input carries the turn (two halves), not the mouse's counts, and
  replays hold the inputs: nothing a match computes changes, no row in
  `simulation_changes.tsv`.

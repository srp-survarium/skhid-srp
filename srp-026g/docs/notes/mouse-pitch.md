# The mouse look's pitch: 16:9's at every window, and "Lock vertical sensitivity" (patch 112)

The rules that the code and the checks follow. Code: the Rust patch `mouse_pitch`
(`client-patches/rust/patches/src/mouse_pitch.rs`, `vostok/mouse.rs`), `death_camera_026g.asm`
(patch 46), the main menu movie (`flash/main_menu/src`: `SrpMouseVertical`, `SrpCheckOption`,
`survarium/menu/Settings.as`). See README patch 112 and `client_patch.md` section 3.70. Checks:
`check_mouse_pitch_026g.py`, `check_death_camera_026g.py`, selftest checks 112 and 46,
`scripts/hud-options/check.sh`.

- **The report:** the Discord suggestions thread "Чувствительность мыши на широкоформатных
  разрешениях экрана" (2026-11-03). The mouse is right only at 4:3. At 16:9, looking up and down
  is slower than turning. This happens only in the character's look, not with the menu cursor.
- **Stock 0.26g** (`player_input_handler::on_mouse_move` 0x240200). 0.10b's source and 0.21d's
  0x17ED3D use the same formula. s is the sensitivity option x 0.1 x the aim's fov factor
  (handler+94h, on both axes):
  - yaw: x * pi/180 * s radians per count;
  - pitch: y * pi/180 * s * h/w * 3/pi look-pitch units per count, where h/w is the render
    window's height over its width. One unit is 60 degrees of view looking up and 67.5 looking
    down (the first-person camera follows the look clip, `abi.tsv` 0x2FBF90);
  - so pitch radians / yaw radians is h/w looking up and 1.125 h/w looking down: 0.75 / 0.84 at
    4:3, 0.80 at 5:4, 0.625 at 16:10, 0.5625 / 0.63 at 16:9, and 0.42 / 0.47 at 21:9. 4:3 is not
    1:1 either; it is simply the aspect the game was tuned at.
- **Retail 0.69d0** (`player_input_handler::on_mouse_move` 0x20023ECE0, `tools/re-kit/v069d`)
  keeps the same h/w x 3/pi and only adds a separate aim sensitivity. No later build made the
  pitch independent of the aspect.
- **The look clip, decoded again (2026-11-05)** from 0.26g's
  `animations/single/weapons/ak/player/stand/3rd_view/on_site/on_site_camera_look.b-spline`. The
  clip is 36 frames long; bone `Item` has its x angle as a clamped cubic B-spline.
  - The x angle runs from 67.5 degrees (control 1.17801) at time 0, through 0 at the middle, to
    -60.0 degrees (-1.04711) at the end. Its control points are evenly spaced on each half, so
    the angle is exactly linear on each side.
  - `look_time_factor_calculator` (0x2FBF90) scrubs the clip at min((look pitch + 1) x 0.5,
    0.99999) x its length.
  - So one unit of look pitch is 67.5 degrees while the pitch is below 0 (down) and 60 degrees
    above 0 (up): piecewise constant. The on-site idle, aimed, reload and look clips have the same
    map.
  - Two other kinds differ. The running aimed look clips are within 1 % (66.8 / 60.7 degrees).
    The grenade hold (`grenade_idle_look`) is not linear: 41.7 degrees at a quarter instead of
    33.75. They were not traced as the camera's layer.
- **The owner's design (2026-11-05):** a checkbox "Lock vertical sensitivity" in Settings,
  Controls, under Mouse sensitivity. It is ticked by default, and under it is a slider "Vertical
  sensitivity" with the same range and look as the main slider.
  - **Ticked (locked, `Same`):** the stock formula with h/w fixed at 16:9's 0.5625 at every
    window. A 16:9 window turns bit for bit as in the original game; most players are on 16:9.
    Every other window gets 16:9's feel: pitch / yaw is 0.5625 looking up and 0.63 looking down.
    The death camera (h/w x 0.9549) and the warmup camera use the same constant. The greyed
    vertical slider shows Mouse sensitivity x 0.5625 (1.25 -> 0.70), live as the main slider is
    dragged. That is the locked look's looking-up rate in the vertical slider's units.
  - **Unticked (`Split(v)`):** exactly 1:1. A count's pitch radians equal the yaw's at the
    vertical value v. The step's factor in place of h/w is 1 while the look pitch is up and
    60/67.5 while it is down; the side is keyed on the local player's look pitch, +48Ch. In
    the warmup camera's orbit, whose pitch is in radians, the factor is pi/3. The death camera,
    whose pitch is in radians too, turns its pitch by its yaw step at v. v equal to the main value
    means vertical turns exactly as fast as horizontal.
  - **Unticking** starts v at the value the greyed slider showed, so looking up feels as it did
    at that moment (to the slider's 0.01). **Ticking** goes back to the locked look, and the
    slider follows main x 0.5625 again.
  - Apply saves, as for every option on the tab. Cancel puts the saved state back.
  - Texts: `st_srp_mouse_lock_option` and `st_srp_mouse_vertical_option`, in all ten dictionaries.
- **What players notice, locked:**

  | window | before, pitch / yaw (up / down) | locked |
  | --- | --- | --- |
  | 16:9 | 0.5625 / 0.63 | unchanged |
  | 4:3 | 0.75 / 0.84 | 25 % slower vertically |
  | 16:10 | 0.625 / 0.70 | 10 % slower |
  | 21:9 | 0.42 / 0.47 | +33 % |
  | 32:9 | 0.28 / 0.32 | +100 % |

  A 4:3 player who wants the old feel unticks the box and sets the vertical slider to main x 0.75.
- **Stored in user.cfg:** `srp_mouse_vertical_locked on|off` (on by default) and
  `srp_mouse_vertical <0.01 to 10>` (in 1:1 units), both saved console variables. The menu sends
  the value in hundredths, as the sliders snap it. The two variables are read once per move into
  `Pitch` (`Same` or `Split(vertical)`), and the look takes only that.
- **The Controls tab's layout (still open with the owner):** the left column holds Mouse
  sensitivity, the checkbox, Vertical sensitivity, Invert mouse and the Lobby keys. The Chat keys
  move to the right column, under the game keys. The game-key list scrolls; it now ends above the
  Chat keys.
- **Same path, so covered:** the aimed look and scopes (the fov factor scales both axes alike) and
  the warmup camera before the start (patch 31: mode 3 of the same function).
- **Not changed:** the menu cursor and the lobby's character drag (`lobby_camera` 0x283D30, menus),
  the keyboard look actions (`process_first_person_mode`, not the mouse), and the input journal
  (a developer tool).
- **Lockstep:** the input carries the turn (two halves), not the mouse's counts, and replays hold
  the inputs. Nothing a match computes changes, so there is no row in `simulation_changes.tsv`.

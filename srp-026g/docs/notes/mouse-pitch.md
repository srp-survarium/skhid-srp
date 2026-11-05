# The mouse look's pitch at every window (patch 110)

The rules that the code and the checks follow. Code: the Rust patch `mouse_pitch`
(`client-patches/rust/patches/src/mouse_pitch.rs`, `vostok/mouse.rs`), `death_camera_026g.asm`
(patch 46), the main menu movie (`flash/main_menu/src`: `SrpMouseVertical`, `SrpCheckOption`,
`survarium/menu/Settings.as`). See README patch 110 and `client_patch.md` section 3.69. Checks:
`check_mouse_pitch_026g.py`, `check_death_camera_026g.py`, selftest checks 110 and 46,
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
- **The fix:** h/w is 0.75 (a 4:3 window's) at every window. Every window now turns as a 4:3
  window did, bit for bit. Players at 4:3 feel no change. Wider windows get the 4:3 pitch (+33 %
  vertical at 16:9, +78 % at 21:9).
- **The option (the owner, 2026-11-05):** Settings, Controls, under Mouse sensitivity: a checkbox
  "Separate vertical sensitivity", off by default, and under it a slider "Vertical sensitivity"
  with the same range and look as the main slider.
  - Off: one slider for both axes, as described above. The vertical slider is greyed and inactive,
    and it shows the main slider's value as the main slider is dragged.
  - On: the pitch uses the vertical slider's value in place of the sensitivity option, in the
    same formula. A vertical value equal to the main one gives exactly the off behaviour. Ticking
    the box continues from the value the slider showed, which is the main slider's.
  - Unticking makes the vertical slider follow the main slider again. The stored vertical value
    matters only while the option is on.
  - Apply saves, as for every option on the tab. Cancel puts the saved state back.
  - Texts: `st_srp_mouse_split_option` and `st_srp_mouse_vertical_option`, in all ten dictionaries.
- **Stored in user.cfg:** `srp_mouse_split on|off` and `srp_mouse_vertical <0.01 to 10>`, both
  saved console variables. The menu sends the value in hundredths, as the sliders snap it. The
  two variables are read once per move into `Pitch` (`Same` or `Split(vertical)`), and the look
  takes only that.
- **The Controls tab's layout:** the left column holds Mouse sensitivity, the checkbox, Vertical
  sensitivity, Invert mouse and the Lobby keys. The Chat keys move to the right column, under the
  game keys. The game-key list scrolls; it now ends above the Chat keys. The left column had no
  room for two more rows.
- **Same path, so covered:** the aimed look and scopes (the fov factor scales both axes alike) and
  the warmup camera before the start (patch 31: mode 3 of the same function).
- **The death camera** (patch 46) copied the look's step. It uses the same 0.75, and when the
  option is on it uses the vertical sensitivity for its pitch, as the player's look does.
- **Not changed:** the menu cursor and the lobby's character drag (`lobby_camera` 0x283D30, menus),
  the keyboard look actions (`process_first_person_mode`, not the mouse), and the input journal
  (a developer tool).
- **Lockstep:** the input carries the turn (two halves), not the mouse's counts, and replays hold
  the inputs. Nothing a match computes changes, so there is no row in `simulation_changes.tsv`.

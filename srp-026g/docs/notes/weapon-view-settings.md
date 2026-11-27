# The player's weapon view (patch 219)

The owner, 2026-11-27, after revision 74 (#2650 gave the sights 0.69d0's size): "one problem with
scopes is that some people would prefer the old version. How can we satisfy them?" Asked what he
meant: the size and the field of view of the sights, and "it shouldn't be old/new, it should be more
customizable". He approved one global value for each setting; per-type values may come later.

## What the player sets

Settings → Gameplay. The world's slider (patch 22's `uro_fov`, patch 83's slider) is now headed
"World FOV:". Under it, the tab's standard button (the Optimal and Default buttons' symbol and width),
"Weapon view", opens a window in the Settings window's own skin, as the Controls tab's Key bindings
window (its frame and title band, its title, its close button, the screen dimmed behind it; Esc, the
close button or a press beside it closes it). It holds the tab's own controls in their look: the stock
slider with its label in the headings' look and its value (as World FOV), the HUD group's checkbox, and
a note in the "Applies after restart." look. Above them a preview (below) shows the AK-12 aimed at the
window's values. Every value is a
console variable kept in `user.cfg`, so it can also be set from the console.

| Variable | In the window | Range | Default | What it does |
|---|---|---|---|---|
| `srp_weapon_fov` | Weapon FOV at the hip | 40 to 100, step 1 | 65 | A rifle's held angle at the hip, in degrees. Each gun keeps 0.69d0's ratio to it (a pistol 0.75 times it). Anything that is no gun is drawn at this value. |
| `srp_weapon_fov_aimed` | Weapon FOV over sights | 20 to 65, step 0.5 | 32.5 | A rifle's angle fully aimed over iron sights, a collimator or the rail iron sight. Each gun is scaled by its own aimed factor over 0.5 (a pistol 1.2 times it, the SKS 0.7). |
| `srp_weapon_fov_scope` | Weapon FOV in scopes | 16.25 to 32.5, step 0.25 | 32.5 | The same rule, through a magnified scope. |
| `srp_weapon_fov_world` | Weapon at the world FOV | on / off | off | On: everything held is drawn at the world's projection, as before revision 74. The three sliders above are greyed while it is ticked. |
| `srp_sight_distance` | Sight distance (its value "+10 cm", its note "+ farther, - closer") | -20 to +50 cm, step 1 | 0 | Aimed over iron sights and sights in the gun's eye plane: how much farther from the sight the eye stands than 0.69d0's place (revision 74), negative nearer, within the clamps below. Never a magnified scope's or an optic of its own. |

- A move or a toggle is a pending change of the tab: Apply saves it (user.cfg at once), Cancel puts
  the saved value back. A saved value takes effect at the next frame, also in a match.
- The world's angle and zoom are never changed. The angles change only how big the held gun, its
  sight and its reticle are drawn, around the screen's centre.
- The 14 guns with an optic of their own (the SVD, VSS, VSK-94, Rem 700, SV-98, VSSK, HK416, MSBS,
  Kriss and P90 with their stock optic) keep the world's projection and their own eye whatever the
  settings, as in both revisions.

### Revision 73 and revision 74

- **The defaults are revision 74 bit for bit.** Each angle is 0.69d0's times a scale of exactly 1.0
  (`x * 1.0` is `x` in floats), and the eye's move is not made at all at 0 cm.
- **The checkbox is revision 73's projection** for everything held (the lens off; revision 73 had
  none); magnified scopes, whose eye is their own in both revisions, are then revision 73's whole.
- **Sight distance** first was a blend toward revision 73's eye (0 to 100 %); the owner, 2026-11-27:
  "I would just have it as a simple distance slider". So it is centimetres along the eye's line, the
  same for every gun. Revision 73's eye was per gun (its iron sights' `aim_offset`, a collimator's eye at
  the plane of the gun's header): no one value gives it back, which the owner chose.
- **Scope FOV 16.25** gives revision 73's scope picture at full aim (the world's angle at x4 is 16.25
  whatever `uro_fov` is), without the checkbox.

## Visual only

Nothing a match computes changes; no `simulation_changes.tsv` row.

- **The shot.** The bullet's origin and direction are player+0x44C, the `Item` bone times the root,
  which `computed_head_transform` 0x2DA4B0 writes every simulation step. They go through
  `get_dispersed_bullet_direction` 0x2E1D50. The first-person camera is set from that same matrix
  (`update_camera` 0x1F3480; `pixel_correct_shooting_069.md` section 3). The kick lifts the view
  itself, camera and bullet together (`player_bots_behaviour.md`).
- **The angles** change only the two scale terms of the foreground pass's own copy of the projection,
  on the render thread (`weapon_view.foreground` 0x13E0C2 in `foreground_begin` 0x13E010). The lens
  rides in the draw command's padding (+14h). The world's projection, its zoom (`sights.zoom`) and
  the camera are not touched.
- **The eye.** `calculate_aim_alignment` 0x244D40 has one caller, `process_hands_ik` 0x2959D0, in
  `mt_draw`'s bone pass. It runs in first person with the aim's progress above 0 and overwrites the
  hands' `Weapon` bone (`client_patch.md` 3.92). The `Item` bone, the camera, is its input and is
  never written.
  - The patch moves the matrix that call returns, in the frame's own stack slot, before the blend
    and the copy to the `Weapon` bone.
  - No weapon field is written: neither the eye record +7A8h nor the aim offset +6A0h.
  - The fire point (+620h) is for the shot's effects alone.
- **The settings** are the player's own `user.cfg` variables. They reach no packet and no world.
- **Fairness**: the bounds are revision 73's. A wider weapon angle shows less of the gun, as a wide
  `uro_fov` did there. A scope angle down to 16.25 overfills the lens as revision 73 did, the world
  inside it at the same x4.

## How the exe applies it

- **The angles** (patch 161, `attachments/view.rs`).
  - At `weapon_view.camera` the camera reads the five values once (`ViewOptions::now`).
  - With the checkbox there is no lens. Otherwise the gun's two factors are each multiplied by
    their scale, the hip's and the sights' or the scopes' by what the gun is aimed through, before
    0.69d0's interpolation and angle (`Profile::focal_at`).
  - What a gun is aimed through: none (its iron sights), a fitted optic whose eye is in the gun's
    plane (a sight), one with its own eye (a magnified scope), or an optic of its own (the world's).
    It is read from the modules table's loadout, the same one patch 161 resolved (`Eye::relief`).
  - A second sight (patch 206) blends the main's and the second's angle and move by its switch, as
    the eye record does.
  - `vostok::trig::focal_length` now takes half-angles to 90 degrees (100 at the hip is 50). Up to
    45 its floats are the same as before.
- **The eye** (patch 219).
  - The move is the player's centimetres along the eye's line (an `aim_offset` that much less:
    farther), clamped per sight: `weapon_modules::Relief::shift` over the table's `along` (how far
    behind its reticle `Build::in_gun_plane` puts the eye) for a sight in the gun's plane; for iron
    sights the row's `aim_offset_026g` and the cook's +6A0h.
  - The camera keeps the move for the gun it sampled (a `Lock`).
  - `hands_ik.aim_alignment`, after patch 172, takes the camera's line: the `Item` bone's third row,
    from the call's pushed argument (`hands_ik_frame.item`).
  - `hands_ik.aim_aligned` (0x295AB2, the call's return) adds `-shift x line` to the alignment's
    translation.
  - That is calculate_aim_alignment's own result with an `aim_offset` `shift` larger, because
    `inverse(T(0,0,a+s) R L B) Item = inverse(T(0,0,a) R L B) Item^-1 T(0,0,-s) Item`. The eye moves
    along its own line, so the reticle stays at the screen's centre.
- **The clamps** (nearer only; farther is as asked, to the slider's 50 cm).
  - A sight in the gun's plane is never nearer than 6 cm to its reticle (`weapon_modules::LEAST_RELIEF`;
    the nearest a release drew, revision 73's M4A1 EOTech, was 7.99 cm), nor nearer than its 0.69d0
    eye where that is nearer already. On the shipped table every such sight is 6 cm or more behind its
    reticle (`the_shipped_table_and_the_sight_distance`): the M4A1's EOTech 20.78 cm, the A-545's 39.46.
  - Iron sights come no nearer than the nearer of the two eyes released builds drew: revision 73's
    `aim_offset` (the row's `aim_offset_026g`, iron-sights-1.json's `from`, 0 where it added the key)
    and the content's. Their rear sight's place is in no table, so the eyes players saw are the
    known-safe ones: the PM's may come 42.9 cm nearer, the MP5's not at all. A gun without a row may
    only go farther.
- **The menu** (`flash/main_menu`, `SrpWeaponView.as`, `Settings.as`). The exe answers each name in
  the Settings' call (`weapon_view_options.rs`). A slider's answer is `name, label, saved, least,
  most, steps a unit`: 1, 10, 100 and 1, so every default and end is whole and of four digits at
  most; a negative one (the distance's) has a minus sign.
- **The values' fields** are one line, each as wide as the widest value its slider shows in the
  dictionary's language ("100", "16.25", "-20 cm", "+50 cm", "см" in Cyrillic) plus 10 px: the owner's
  first test in the game showed "-16 cm" cut to "-16 cr" in a field Ruffle had found wide enough, the
  game's Scaleform measuring the text wider. Negative values take a plain hyphen, as the stock sliders'
  numbers do; the unit is the dictionary's `st_srp_unit_cm` ("%1$s cm", "%1$s см"), answered with the
  window's texts.
- **The texts**: thirteen keys in the twelve dictionaries (`st_srp_weapon_view_heading`,
  `st_srp_weapon_fov_option`, `_aimed_option`, `_scope_option`, `_world_option`,
  `st_srp_sight_distance_option`, `st_srp_sight_distance_note`, `st_srp_unit_cm`, `st_srp_world_fov_option`;
  the preview's `st_srp_weapon_view_hip`, `_iron`, `_red_dot` and `_scope`).

## The preview

The window has two columns (1180 px wide at 1x, inside the screen at 1280x720 and scaled 1.5): on the
left the Settings window's own tabs (Hip, Iron sights, Red dot, Scope; its tab bar's renderer, the
selected one lit with the orange line under it), centred over an 800x450 picture of the AK-12
(`SrpWeaponPreview.as`): at the hip, or fully aimed over its iron sights, an EOTech 552 or the 1P69
Hyperon (one gun in every view; the AK-12 takes no PSO-1); on the right the controls, the first label's
text at the picture's top (the owner: "text can be on the level of the image"). No note under the
picture (the owner had it taken out). Moving a slider shows its view: the hip slider the hip, the
scopes' slider the scope, the sights' and the distance's the iron sights (or the red dot when it is
shown); the checkbox draws it again. The World FOV slider's value, pending or not, draws the world.

- **The pictures** are offline renders and captures, 44 bitmaps of the movie, about 5.3 MB
  (`flash/main_menu/bitmaps/SrpPreview*`, made by `scripts/weapon-view-preview`):
  - the gun's layers, 1600x900 at JPEG quality 95, each with its alpha (`<class>.jpg` and `<class>.alpha.png`:
    one DefineBitsJPEG3, a tenth of a lossless bitmap), drawn at twice that and downsampled
    (anti-aliased edges). Each view at a few angles, its tiers: the widest its sliders reach (the hip's
    100; the iron sights' 94.8, the World FOV checkbox at World FOV 100; the EOTech's 77.7; the
    scope's 32.5), so the whole gun is there at every setting (the owner: raising the sights' angle
    had shown the gun "without its new parts"), and narrower ones down to the sliders' narrowest (hip
    100, 77.5, 56.78; iron 94.8, 74.16, 55.42, 40.11, 28.47; the EOTech 77.7, 57.71, 41.3, 28.91; the
    scope 32.5, 23.06: each at most 1.485 times the next's half-angle tangent), so the narrowest tier
    at or over the angle always shows at least 606 of its 900 rows: the pane's 450 rows are 600 on the
    screen at 1440p (UI scale 1.33), so nothing is enlarged there at any setting. Drawn as patch 161
    draws it: the iron sights at the gun's `aim` locator and aim_offset, the EOTech's eye from this
    crate's resolve, the 1P69's aimed model alone at its own eye; the iron sights and the EOTech at
    -20, 0, +25 and +50 cm of sight distance (the exe's clamps: the AK-12's iron eye no nearer than
    revision 73's, the EOTech's no nearer than 6 cm). Emissive materials are drawn in their constant
    emissive colour over the rest, as the game's additive pass shows them: the EOTech's reticle red
    (the owner: "red dot is white");
  - the hip (the quick version the owner chose): the gun at the skeleton's Weapon bone and the scavenger
    04's hands, both in the AK's `on_site_idle` at its first frame (the hip's idle; the aimed one is
    the same pose, the game's hand IK aims it), seen from the Head bone looking straight ahead. The
    game's first person camera is its computed head transform, not read for this, so a few cm or
    degrees may differ; the hands are the clip's, without the game's hand IK;
  - the world: three headless captures of Vostok (map 123, `scripts/capture`) at the game's highest
    settings (textures, geometry and draw distance Maximum, shadows 3, ambient occlusion 3, anisotropy
    16, anti-aliasing), from where a player stands (the owner: "location can be chosen in playable
    game area"): a lane between two walls, the eye 1.6 m over a floor the walk grid reaches from a
    spawn (`crates/mapdata`), out of every damage zone. At 100 degrees vertical for worlds past 60
    (stored 2250x1266: the narrowest such, 60.1 degrees, is 615 of its rows), at 60 for 60 and less
    (1920x1080: the narrowest, the iron sights' 45.9 at World FOV 50, is 792 rows) and at 20 for the
    scope (1440x810), JPEG at quality 95;
  - two stock soldiers in them (`soldiers.py`, the owner: "properly placed characters, holding weapon
    and with bright colorful decals"): the scavenger 04 outfit posed by the AK's third person aimed
    idle, the AK-12 at his shoulder, standing on the walk grid's floor (its height at that spot)
    facing the camera turned 25 to 35 degrees (so the rifle shows its side), with decals on the chest
    and the thighs (Snufkin, the bottled fumo, Neco-Arc, the number stencils: the content packs' own
    pictures, each a square 1.5 cm off the outfit moving with its bone; the renderer cannot bake them
    as the game does). One 30 m ahead where the sights point (in the reticle, in the scope at its
    zoom), one 10 m ahead beside the sights (beside the front sight and the EOTech, inside the scope's
    circle at 16.25).
- **The angles are exact.** A tier made at `t` degrees drawn at angle `a` is the tier scaled about
  the centre by tan(t / 2) / tan(a / 2): the game's held projection is that same scale of everything
  held. The world is scaled the same way to its angle at the sight's zoom (patch 22: 65 z + (uro_fov -
  65) max(0, (z - 0.25) / 0.75); the iron sights' zoom 0.909, the EOTech's 0.8, the 1P69's 0.25).
  "Weapon at the world FOV" draws the gun at that world angle.
- **The distance is approximate**: the two renders around it crossfaded (10 cm: the 0 cm one with
  the +25 cm one at 40 %).
- **Drawn at the screen's pixels.** The pictures are smoothed Bitmaps scaled on the GPU under a mask,
  so at any UI scale (MainMenu scales the movie above 1080p: 1.33 at 1440p, 2 at 4K) they are
  filtered from their own pixels. Revision 80 drew them into one BitmapData of the pane's size, which
  the game's Scaleform then enlarged without filtering at 1440p (the owner: "the drawing quality is
  atrocious"). A layer is made only while shown (5.8 MB decoded; the world up to 11.4 MB) and let go after.
- The window has no bottom band (the Settings' button bar is not part of its frame).
- **The buttons** (the owner: "default settings, new line, apply close"): "Default settings" on a line of
  its own under the controls (the tab's standard button; the game's own text, the Controls tab's
  `st_options_default_controls`, which the exe answers with the window's texts) sets the five values
  back to revision 74's (65, 32.5, 32.5, off, 0 cm), each a move pending until Apply as the player's,
  and draws the preview again. Under it Apply and Close side by side, the Settings window's own Apply
  and Cancel/Exit buttons (their symbols; Close the game's `st_close`). Close closes the window as its
  cross does, its changes still pending. Each button is wider only for its text (16 px), every
  dictionary's inside the window.
- **Apply** (its label the Settings' Apply's)
  saves this window's pending values only, as the Settings' Apply does them, and keeps the window
  open; the other tabs' pending changes stay pending (the Settings' Apply saves everything pending,
  every tab's, so the window's could not be it). Disabled while none of its values is pending.
- **Test builds:** `tools/test-patch` used to keep a setup's bitmaps of a name it had already, so a
  changed picture of the tree never reached the test (revision 80 showed revision 79's world); a
  movie tree's bitmaps now take the place of the setup's own.

## The log

So a test proves the settings at work, `srp_patches.log` says:

- `weapon view: srp_weapon_fov 59000, srp_weapon_fov_aimed 65000, ... (thousandths); srp_weapon_fov_world
  off: everything held at its own, the scales of 0.69d0's angles (thousandths) hip 907, sights 2000, ...`:
  at the first camera of a run, and again whenever a value changes (Apply, a console line);
- `weapon view: gun 220 fully aimed through its iron sights: drawn at 650 tenths of a degree, the eye
  moved 0 tenths of a mm`: whenever what the held gun is drawn as at the hip or fully aimed changes
  (another gun, another sight, another setting); "drawn at the world's projection" for an optic of its own
  or the checkbox;
- `weapon view: the foreground draws what is held at its own projection, focal length ...`: the first lens
  the render thread takes in a run.

A test whose log has the first two lines and not the third would show the lens made but never drawn.

The owner's first test of revision 79 (2026-11-27): the values saved, no change seen, and no line to tell
why. The exe of that test, run under Unicorn with those values set by the console's own setters, makes
the lens they ask for (`check_attachments_026g.py` now sets them so). The next test's log tells which of
the game's paths it was.

## The scopes' surround

The "Weapon FOV in scopes" slider stops at 32.5, the default: never wider than 0.69d0's.

From each magnified optic's eye (its `aim` locator in the table), every surface of its aimed model
was projected (`pso_1_scope_aim` and the rest, weapon-modules@9). The widest held angle at which the
model covers the whole screen, vertical degrees:

| Aimed model | 4:3 | 16:9 | 16:10 | 21:9 | 32:9 |
|---|---:|---:|---:|---:|---:|
| PSO-1 old | 32.8 | 27.3 | 29.3 | 21.6 | 15.2 |
| 1P69 Hyperon | 30.6 | 25.3 | 27.2 | 20.1 | 14.2 |
| PSO-1 | 30.4 | 24.9 | 26.9 | 19.6 | 13.6 |
| Elcan Specter DR | 25.2 | 20.8 | 22.4 | 16.6 | 11.6 |
| Leupold 3-7 | 22.7 | 18.6 | 20.1 | 14.8 | 10.3 |
| Leupold Mk4 | 21.7 | 17.8 | 19.2 | 14.2 | 9.9 |
| ACOG TA01 | 18.0 | 14.9 | 16.0 | 11.7 | 8.3 |
| Aperture | 12.3 | 10.1 | 10.9 | 8.1 | 5.7 |
| PU 3.5x22 | 9.4 | 7.7 | 8.3 | 6.1 | 5.7 |

No magnified scope's aimed model covers a 16:9 screen at 0.69d0's 32.5 degrees. 0.69d0 draws a
dark mask outside `scope_radius` (0.7 of the half height on the PSO-1), which 0.26g does not have.
So revision 74 already shows the world at x4 past the housing at the screen's sides and corners.

A wider angle would show more of it, so the range stops at the default. A narrower one, toward
revision 73's 16.25, covers more of the screen. A mask like 0.69d0's would be a change of its own.

## Checks

- `cargo test -p patches` (`tools/check patches-crate`):
  - the defaults' lens the same bits as revision 74's code for five profiles at five aims;
  - no move at 0 cm;
  - each angle scaled by what is held;
  - the checkbox gives no lens;
  - the distance moves the eye by its centimetres, within each clamp (iron sights with and without a
    nearer released eye, a gun without a row, a collimator at the least relief), never a magnified
    scope or an optic of its own;
  - the sliders' steps;
  - `focal_length` to 89 degrees.
- `cargo test -p weapon-modules` (`cargo:weapon-modules`, `weapon-modules-table`): `Relief::shift`
  and the shipped table's sights (every one at least the least relief behind its reticle).
- `check_attachments_026g.py` (`tools/check attachments`, Unicorn):
  - the five variables registered at `configs.start`;
  - the lens at 90 and 52 degrees, and with no gun;
  - the checkbox gives the world's projection;
  - a fitted PSO-1 at a scope angle of 20.
  - the values set by their own setters, as user.cfg's lines are;
  - process_hands_ik through calculate_aim_alignment at 0, +10, +50, -1 and -20 cm (the A-545's iron
    sights) equals what the original exe makes with the aim offset that much less, -20 clamped at
    revision 73's eye 1.3 cm nearer: bit for bit at 0, within 2e-6 otherwise.
  - A weapon the camera did not sample is not moved.
- `check_weapon_view_options_026g.py` (`tools/check hud-options`, Unicorn): the registration, each
  slider's steps within and out of its range, the checkbox, the heading; the original answers none.
- `scripts/hud-options/check.sh` (Ruffle; its harness has no row in `tools/checks.tsv`):
  - the tab's standard button under World FOV, the window in the Settings' skin (the Key bindings
    window's frame, title and close button) at 1280x720 and with the movie scaled
    1.5;
  - the sliders' labels in World FOV's look, the checkbox the HUD group's size, the distance -20 to
    50 and its value in cm with a sign;
  - every dictionary's labels and the distance's note inside the window;
  - Esc and the close button close it, its changes still pending;
  - Apply sends the steps, and Cancel and the exe's answers keep or restore the saved values;
  - the checkbox greys the angles;
  - the preview: the slider moved picks its view and selects its tab, a tab picks its view, the
    layer's scale is tan(16.25) / tan(a / 2) at the sights' and the scopes' angles and at the world's
    with the checkbox, the scope's world at 16.25 degrees, 10 cm crossfades two renders at 40 %;
    every dictionary's tab names inside their buttons with 10 px to spare; the two columns inside the
    frame, the first label at the picture's top, the tabs centred over it.
  - Screenshot `settings-weapon-view.png`.

**Not run:** a game. To look at in one (1280x720, `uro_fov 65`):

- the EOTech on the A-545 and the M4A1 at 0 cm, +20 cm and -15 cm (the M4A1's stops at 6 cm from its
  reticle);
- a PSO-1 at a scope angle of 16.25 (revision 73's full-screen lens), and the distance not moving it;
- the PM at -20 cm (20 cm nearer) and +30 cm;
- the hip at 90 with a pistol;
- the arms at each.

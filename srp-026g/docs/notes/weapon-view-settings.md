# The player's weapon view (patch 218)

The owner, 2026-11-27, after revision 74 (#2650 gave the sights 0.69d0's size): "one problem with
scopes is that some people would prefer the old version. How can we satisfy them?" Asked what he
meant: the size and the field of view of the sights, and "it shouldn't be old/new, it should be more
customizable". He approved one global value for each setting; per-type values may come later.

## What the player sets

Settings → Gameplay. The world's slider (patch 22's `uro_fov`, patch 83's slider) is now headed
"World FOV:". Under it, a "Weapon view" button opens a panel over the left column. Every value is a
console variable kept in `user.cfg`, so it can also be set from the console.

| Variable | In the panel | Range | Default | What it does |
|---|---|---|---|---|
| `srp_weapon_fov` | Weapon FOV at the hip | 40 to 100, step 1 | 65 | A rifle's held angle at the hip, in degrees. Each gun keeps 0.69d0's ratio to it (a pistol 0.75 times it). Anything that is no gun is drawn at this value. |
| `srp_weapon_fov_aimed` | Weapon FOV over sights | 20 to 65, step 0.5 | 32.5 | A rifle's angle fully aimed over iron sights, a collimator or the rail iron sight. Each gun is scaled by its own aimed factor over 0.5 (a pistol 1.2 times it, the SKS 0.7). |
| `srp_weapon_fov_scope` | Weapon FOV in scopes | 16.25 to 32.5, step 0.25 | 32.5 | The same rule, through a magnified scope. |
| `srp_weapon_fov_world` | Weapon at the world FOV | on / off | off | On: everything held is drawn at the world's projection, as before revision 74. The three sliders above are greyed while it is ticked. |
| `srp_sight_distance` | Sight distance (0.69 - 0.26) | 0 to 100 % | 0 | Aimed over iron sights and sights in the gun's eye plane: the eye's share of the way from 0.69d0's place (revision 74) to revision 73's. |

- A move or a toggle is a pending change of the tab: Apply saves it (user.cfg at once), Cancel puts
  the saved value back. A saved value takes effect at the next frame, also in a match.
- The world's angle and zoom are never changed. The angles change only how big the held gun, its
  sight and its reticle are drawn, around the screen's centre.
- The 14 guns with an optic of their own (the SVD, VSS, VSK-94, Rem 700, SV-98, VSSK, HK416, MSBS,
  Kriss and P90 with their stock optic) keep the world's projection and their own eye whatever the
  settings, as in both revisions.

### Revision 73 and revision 74

- **The defaults are revision 74 bit for bit.** Each angle is 0.69d0's times a scale of exactly 1.0
  (`x * 1.0` is `x` in floats), and the eye's move is not made at all at 0 %.
- **The checkbox and 100 % are revision 73 for every gun:**
  - the projection is the world's, the lens off (revision 73 had no lens);
  - iron sights: the eye moves by the gun's revision 73 `aim_offset` (iron-sights-1.json's `from`,
    0 where the edit added the key) minus the one the cook made (+6A0h);
  - collimators and the rail iron sight: the eye moves to the plane of the gun's own `aim` (the
    composed body's header), which revision 73's resolver used; where the line misses that plane,
    the eye stays at the sight's own locator, as revision 73 left it;
  - magnified scopes: their eye is their own in both revisions; the checkbox alone gives revision 73's
    picture (the world's x4 projection, the lens past the screen).
- **Checked on the shipped table** (weapon-modules@9, `the_shipped_table_at_the_026g_eye`):
  - the M4A1 with the EOTech 552: the reticle 20.78 cm ahead of the eye at 0 %, 7.99 cm at 100 %
    (revision 73's);
  - the A-545 with the same sight: 39.46 cm at 0 %, 53.16 cm at 100 %;
  - the PM's and the SKS's collimators move 43 to 49 cm and 16.7 cm;
  - a fitted PSO-1 keeps its own eye.
  - The clamp below never acts on any of the 835 sights in the gun's plane, so 100 % is revision
    73's on every one of them.
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
- **The eye** (patch 218).
  - Each sight's move comes from the table: `weapon_modules::Relief`, `along` and `along_026g` of
    `Build::in_gun_plane`. For iron sights it comes from the row's `aim_offset_026g` and the cook's
    +6A0h.
  - The camera keeps the move for the gun it sampled (a `Lock`).
  - `hands_ik.aim_alignment`, after patch 172, takes the camera's line: the `Item` bone's third row,
    from the call's pushed argument (`hands_ik_frame.item`).
  - `hands_ik.aim_aligned` (0x295AB2, the call's return) adds `-shift x line` to the alignment's
    translation.
  - That is calculate_aim_alignment's own result with an `aim_offset` `shift` larger, because
    `inverse(T(0,0,a+s) R L B) Item = inverse(T(0,0,a) R L B) Item^-1 T(0,0,-s) Item`. The eye moves
    along its own line, so the reticle stays at the screen's centre.
- **The clamp.** A sight in the gun's plane is never nearer than 6 cm to its reticle
  (`weapon_modules::LEAST_RELIEF`; revision 73's nearest, the M4A1's EOTech, was 7.99 cm), nor nearer
  than its 0.69d0 eye where that is nearer already. Iron sights move only between their two
  released eyes.
- **The menu** (`flash/main_menu`, `SrpWeaponView.as`, `Settings.as`). The exe answers each name in
  the Settings' call (`weapon_view_options.rs`). A slider's answer is `name, label, saved, least,
  most, steps a unit`: 1, 10, 100 and 1, so every default and end is whole and of four digits at
  most.
- **The texts**: seven keys in the twelve dictionaries (`st_srp_weapon_view_heading`,
  `st_srp_weapon_fov_option`, `_aimed_option`, `_scope_option`, `_world_option`,
  `st_srp_sight_distance_option`, `st_srp_world_fov_option`).

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
  - no move at 0 %;
  - each angle scaled by what is held;
  - the checkbox gives no lens;
  - revision 73's eye at 100 %;
  - the sliders' steps;
  - `focal_length` to 89 degrees.
- `cargo test -p weapon-modules` (`cargo:weapon-modules`, `weapon-modules-table`): `Relief::shift`
  and the shipped table at the 0.26g eye.
- `check_attachments_026g.py` (`tools/check attachments`, Unicorn):
  - the five variables registered at `configs.start`;
  - the lens at 90 and 52 degrees, and with no gun;
  - the checkbox gives the world's projection;
  - a fitted PSO-1 at a scope angle of 20.
  - process_hands_ik through calculate_aim_alignment at 0, 50 and 100 % equals what the original
    exe makes with the aim offset 0, half and all of the way to `aim_offset_026g`: bit for bit at
    0, within 2e-6 otherwise.
  - A weapon the camera did not sample is not moved.
- `check_weapon_view_options_026g.py` (`tools/check hud-options`, Unicorn): the registration, each
  slider's steps within and out of its range, the checkbox, the heading; the original answers none.
- `scripts/hud-options/check.sh` (Ruffle; its harness has no row in `tools/checks.tsv`):
  - the button under World FOV, the panel over the left column at 1280x720 and with the movie scaled
    1.5;
  - every dictionary's labels inside the panel;
  - Apply sends the steps, and Cancel and the exe's answers keep or restore the saved values;
  - the checkbox greys the angles.
  - Screenshot `settings-weapon-view.png`.

**Not run:** a game. To look at in one (1280x720, `uro_fov 65`):

- the EOTech on the A-545 and the M4A1 at 0 % and 100 % with the checkbox (revision 74's and 73's
  rings: 19.7/8.6 px, 37.1/56.9 px);
- a PSO-1 at a scope angle of 16.25 (revision 73's full-screen lens);
- the PM at 100 % (revision 73's place, 40 cm nearer);
- the hip at 90 with a pistol;
- the arms at each.

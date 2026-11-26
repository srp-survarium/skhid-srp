# Second sights: a sight on the gun's side, and switching to it

The owner, 2026-11-26: "allow setting second scopes ... what 0.69 did, as I understand it was on the
side of the weapon", switching between the sights "like retail 0.69"; then "maybe we should go as far
as have the whole system from there" (the aim-slot system as retail has it, simulated). Client 0.26g;
0.69d0 the reference. Tags: PROVEN (read in the data or the code named), LIKELY, OPEN.

## What 0.69d0 has (PROVEN)

Data: 0.69d0's `gameplay/game.options` (the weapon survey's dump `d069/game75.leaves`). Code: the
identity exe (`tools/re-kit/v069d/identity.json`), read with the full disassembly and Ghidra; every
function named below has its row in `tools/re-kit/v069d/functions.tsv`.

### The parts

- **The offset rail**: `ncstar_mpr45_picatinnyrail` (dict 1886, group `scope_rail`, cost 100), an NcStar
  MPR45 45-degree mount. It goes on a gun's `angled_picatinnyrail_l` (its `attach_params`: adds
  `angled_collim_short_l` at its locator `angled_collim_short_l_point`, closes the right rail) or
  `angled_picatinnyrail_r` (`attach_params_2`: `angled_collim_short_r`, closes the left one).
- **The sights that go on it**: only two modules name the angled slots, both collimators that are also
  top-rail optics, `leupold_dppro_collimator` (DeltaPoint Pro, ours 2112) and `hensoldt_rsas_collimator`
  (RSA-S, ours 2116): their `slots_to_attach_to[7]` `angled_collim_short_l` with `attach_params_2`
  (`slots_to_add` `aim_3` at their `aim` with `plane_slot: body_aim`, and `reticle_2`), `[8]`
  `angled_collim_short_r` with `attach_params_3` (`aim_4`, `reticle_3`). Nothing else goes on the rail.
- **The guns**: the angled rails are gun slots of the A-545, AK-12, HK416, MSBS, SCAR-H and VHS-2 and
  their premium and event copies (locators `angled_picatinnyrail_point`, `_inv_point`); the M4A1 and
  the L85A2 get them only from their tactical forends (`m4a1_tactical_forend`,
  `enfieldl85a2_tactical_forend`, `slots_to_add[4..5]`). The MPR45's `module_to_item_links` also name
  the FAMAS, which has no angled slot: a dead link.
- **The other aim slots** (not this change's first step): every magnified optic has a second aim slot of
  its own, `aim_2` at its own `aim`, with two zooms (`aim_zoom_factor [4, 8]`, `[3, 6]` on the aperture
  sight and the Elcan); the Docter on the Elcan's `elcan_collimator_mount` adds `aim_5`.

### How the player switches (`weapon` = the gun's weapon_core)

- The aim slots: `collect_aim_slots` 0x2006d6af0 finds `aim`, `aim_2` .. `aim_8` among the gun's slots
  into a list (weapon+0x43d0). A slot's owner is the module that added or overrode it.
- **The key**: action `scope_next` (row 0x2a of the action table 0x200aa1700), default **T**;
  `zoom_next` / `zoom_prev` (0x28, 0x29) the mouse wheel. The input apply
  `base_player::process_weapon_input` 0x200301940 calls `weapon::request_aim_slot_switch` 0x2006cd490 on
  input bits 45 (T), 42 and 43 (the wheel).
- `request_aim_slot_switch`: nothing with one aim slot or a switch pending. T picks the next aim slot,
  wrapping, whose owner differs from the current one's (from a scope to the side sight and back); the
  wheel the next or the previous slot of the same owner (a magnified optic's two zooms). It sets the
  next slot (+0x4484) and the end time (+0x4488) = now + `scope_switch_delay` or `zoom_switch_delay`.
- **Timing**: `scope_switch_delay` 500 ms, `zoom_switch_delay` 70 ms (console integers, 1..2000:
  0x200086630, 0x200086680 over 0x200a9000c, 0x200a90008). `start_aim_slot_switch` 0x2006d87d0 runs the
  switch's interpolator s (+0x44ec) from 0 to 1 over the delay, from the next tick after now.
  `update_timed_switches` 0x2006d8950 commits at the end time: the current slot (+0x4483) = the next,
  s back to 0; a step between one optic's zooms plays its zoom sound (vtable +0x1c8).
- **What it does while it runs**:
  - the gun's pose (`align_weapon_bone_to_aim` 0x2002b8cf0, first person only): the alignment to the
    current slot blended to the next one's by s;
  - the effective aim (`effective_aim_progress` 0x2006cc8a0) = aim progress - min(s, 1 - s), clamped:
    half way through, the aim is down by 0.5;
  - the world's FOV (`current_camera_fov_factor` 0x2006cdb40): the two slots' zoom factors blended by
    s, then from 1 by the effective aim; the weapon's own FOV (`first_person_view_camera_controller::
    update_camera` 0x2002c2d70) by the effective aim too;
  - which slot's module draws its reticle and hides parts: the next one's once s > 0.5
    (`current_aim_slot_index` 0x2006cdd10).
- **The eye**: the slot's frame, `get_slot_transform` 0x2006cf320 with its `plane_slot`: the side
  sight's `aim` moved along its own line to the gun's `body_aim` plane, as for every collimator on top.

### It is simulated (PROVEN)

- The switch is an input bit (the u64 input mask's bit 45), applied by every game for every player.
- Its state is the weapon_core's: `read_state` 0x2006cde80 and `write_state` 0x2006d7bc0 save the
  current slot (+0x4483, always), with more than one aim slot the end time (+0x4488, -1 none) and the
  next slot, and the interpolator (+0x44ec) beside the aim's (+0x44d4).
- **Shots**: `recoil::next_kick` 0x2004024d0 takes the aimed recoil only when the effective aim is 1,
  else the hip one; so a shot fired during a switch kicks as from the hip. It and
  `recoil::apply_shot_modifiers` 0x200402180 also scale by per-zoom lists
  (`zoom_factor_recoil_multiplier`, `zoom_factor_dispersion_multiplier` of `aim_recoil`, indexed by the
  current zoom, `round(1 / FOV factor)`).
- **Sway**: the breath vibration (0x2003dedd0) takes the current slot's module's
  `breath_vibration_factor` (`current_breath_vibration_factor` 0x2006ccb40).

## Here

The plan and its state: the PR's description. What is built is said in each part's section below as
it lands.

## The simulation (0.26g)

Client patch 206 (`rust/patches/src/second_sights.rs`, `second_sights.toml`; its reads of the game
`vostok/second_sight.rs`), the machine and its world bytes the crate `second-sight` (`no_std`,
`#![forbid(unsafe_code)]`, tested on the host; srp-validate and the match server's bot reader use it
too). Written 2026-11-26. PROVEN in the code of 0.26g and in the Unicorn check below; not run in a
game.

### The machine (`second_sight::State`)

- `Settled(Main | Second)` or `Switching { to, pressed }`: the press's time alone, from which the
  blend's start (the first 50 ms tick after it) and the end (`pressed + 500`, 0.69d0's
  `scope_switch_delay`) follow, so no start and end can disagree. One transition, `State::step`,
  total over `Press(t)`, `Tick(t)`, `Deactivate`: a press of a settled weapon switches to the other
  sight, a press while switching is nothing; a tick at or past the end settles on `to`; putting the
  weapon away (`reset_switches`) settles back on the sight switched from. A weapon of one sight
  (`Sights::One`) is always on its main. Game time is `GameTime(u32)`, its wrap defined there.
- `s = clamp((t - start) / 500, 0, 1)` while switching, 0 settled; the sight shown is `to` past 0.5;
  the effective aim `clamp(aim - min(s, 1 - s), 0, 1)` (`View::effective_aim`); a weapon counts as
  aimed only where `min(s, 1 - s)` is 0 (`State::aimed`). The commit comes before the blend's end:
  s reaches 0.9 at most (a press at 104520: the blend from 104550, the end at 105020).

### The sites (sites_026g.tsv, each re-read in the exe)

| site | VA | what |
|---|---|---|
| `input_builder.end` (order -1, first of the chain) | 0x2421B0 | scope_next (5Ah) down: bit 30 of the handler's mask (+33Ch), in that input only |
| `sights.press` | 0x2D877C | apply_new_input after process_quick_slots: bit 30 set in +330h and clear in the actions before ([esp+24h]), at the input's time ([ebp+8]): a press of the weapon held (+128h) when slot 7 or 10 holds it. Every game applies every player's input here |
| `sights.tick` | 0x2E0A45 | weapon_core::tick's join (EBP the weapon, EBX the time): the end. Only the active item ticks |
| `sights.deactivate` | 0x2E2DB0 | weapon_core::deactivate's entry (its user still set): the undo |
| `sights.made` | 0x2DF0D0 | weapon_core::weapon_core's entry: its config's root (the cook's, the one patch 161 reads) has `data/srp_second_sight` nonzero and `srp_second_side` 0 or 1: two sights |
| `sights.destroyed` | 0x2DF570 | ~weapon_core: forgotten (and a weapon made anew at an address is nothing of the one before) |
| `sights.bullet_aim` | 0x2E1D68 | get_dispersed_bullet_direction after its load of the aim progress: while switching, XMM1 is the effective aim, so the dynamic dispersion is zeroed only at an effective 1 |
| `sights.dispersion_aimed` | 0x331375 | dispersion_calculator::tick's `cmp byte [eax+5E8h],0`: while the aim dips, a Skip with ZF set (the hip's multiplier +3F0h) |
| `sights.skill_aimed` | 0x33149D | get_skill_influence's `mov cl,[ecx+5E8h]`: a Skip with CL 0 |
| `sights.target_aimed` | 0x331572 | get_target_koef's `mov dl,[eax+5E8h]`: a Skip with DL 0 (push edi after it runs natively) |
| `carry.save` (order 1, after patch 78's) | 0x2DB47E | the record after the carry snapshot |
| `carry.load` (order 1) | 0x2DBAB5 | the record after the carry snapshot, when the state has one |

- A weapon not switching takes every native path bit for bit: the four aim hooks change nothing
  unless the weapon's state is `Switching` (and the three m_aimed ones only while `min(s, 1 - s)`
  is over 0). The recoil has no aimed/hip difference in 0.26g (`weapon_recoil_calculator::on_shot`
  0x331C50 reads no aim): nothing there. The time a dispersion read is at is the weapon's last
  tick (+5A4h), set at the start of its tick (0x2E09F5).
- **Bit 30** was set and read by nothing in the stock exe (no test of 40000000h, no byte test of the
  mask's high byte; the relays and the replays strip bit 31 alone). srp-validate refused it
  (`UNUSED_ACTIONS`); it is now `messages::SCOPE_NEXT` and every bit of the mask is a client's. The
  bots never send it (drive.rs `key::NEVER`); afk counts a change of it as acting; the replay
  converter clears bit 31 only.
- **The key**: action `scope_next`, id 5Ah, kinds 5, row 77 of the moved action table
  (item_hotkeys_026g.asm), a controls row after the aim toggle (hide_ui_026g.asm, label
  `st_srp_scope_next_action`, the localization row the lobby's work adds). Default **Y**: 0.69d0's T is
  chat here (3Fh, kinds 5), and a default T would unbind chat (bind_key 0x207DC0 clears the key from
  the other bindings); Y is no kinds-5 row's. The player may rebind it.

### The state (who owns what)

- `second_sights::Table` behind a `Lock` (the game thread writes, patch 161's view reads from the
  render thread): the weapons that have two sights (by address, from their constructor to their
  destructor; 512 at most, a line in the log when full), and a player's life (carry's `Key`: place,
  kit, the player object, the world, the spawn time) with the state of each weapon slot (7, 10)
  and the gun (dict id) it was made for.
- **The view's reading** (patch 161): `vostok::second_sight::sight(thread, weapon) -> Option<View>` at
  the weapon's user's simulation time (+324h), `sight_at(thread, weapon, now)` at a frame's time
  (both `unsafe`: a live `raw::Weapon` of the caller's draw or hook); `None` for a weapon of one sight
  or in no weapon slot. `View { from, to, s }`, `View::shown()` (the sight past half way),
  `View::effective_aim(aim)`.
- A state is the slot's gun's: another gun in the slot (a pickup of another kind, patch 85) is on
  its main sight; a picked-up gun of the same kind keeps the slot's state. A respawn is a new life:
  both slots on their main sights. A gun on the ground has no state (the switch it had is undone at
  its deactivation; only a player's slots are saved).

### The world bytes

After patch 78's 32-byte carry snapshot at the end of a player's state (base_player::serialize's
epilogue), `second_sight::Record`, 11 bytes:

```
for slot 7, then slot 10:  u8 tag (0 main, 1 second, 2 switching to the second, 3 to the main),
                           u32 the press's time when switching, 0 settled
u8 version 1 (last)
```

- srp-validate's `AfterItems` reads it after the carry to the state's exact end (`AfterItems::sights`,
  `Reason::TailSights` for any other bytes): a closed tag set, no time on a settled sight. A state
  without it parses as before (every world of the corpus).
- The version is the last byte so a state can be read from its end too: the bots' `Motion::read`
  (`survarium::match_msgs::motion`) strips the record when the state ends with one (it decodes and the
  carry before it restores), else the carry alone; an older state's last byte is the carry's 0 or a
  generation's high byte.
- **A world without the record** (an older game's, a replay of one) leaves that player's life
  *unkept*: on the main sights, no press taken, nothing written, so this game's worlds are the
  loaded world's byte for byte (check_world_roundtrip_026g.py) until the player's next life. Every
  game of a match runs the same revision; a match's first world is a patched game's.
- `simulation_changes.tsv`: the `next` row says it (replays of older games are refused).

### Checked

- `cargo test -p second-sight` (the machine, the clock's wrap, the record's bytes), srp-validate's
  `the_second_sights_follow_the_carry`, survarium's `the_second_sights_after_the_carry_are_skipped`,
  the patches crate's `second_sights` tests (the table: a press, the end, the undo, a weapon of one
  sight, a new life, another gun, a load with and without a record, the room).
- `check_second_sights_026g.py` (tools/checks.tsv `second-sights`): every site above from its own
  site in the build beside the original (above in the cases' text).
- `check_world_roundtrip_026g.py`: the corpus worlds as they are (no record: nothing written) and
  with a record after each state (read whole, written as read).

### Not run in a game: what a local match of several clients must show

- Y (or the key bound to Options, Controls' "Switch sight") with a second sight fitted switches the
  view to it in about half a second and back with another press; nothing with a gun without one,
  and nothing while a switch runs.
- The other clients see the same: no resync that a match without the patch has not (the worlds'
  hashes agree), through a rollback and a late joiner mid-switch.
- A shot fired half way through a switch spreads as from the hip; settled on either sight, aimed
  shots are as before.
- Changing weapon mid-switch leaves the sight it was on; a respawn starts on the main sights.
- No world refused for its record (the server's log, the games' refusal log).

## The view (0.26g)

Client patch 161's side of the switch (`rust/patches/src/attachments/sights.rs`, its reads and writes of
the game `vostok/sight_view.rs`; the hooks in `attachments.toml`), over patch 206's state (`View` at a
time, `vostok::second_sight::sight_at`). Written 2026-11-26. PROVEN in the code of 0.26g and in the
Unicorn check below; not run in a game. Drawing only: nothing here is read by a match's computation (the
scope view byte is the world's, below).

### The frame's time

The switch's s is a line in simulation time, which steps by 50 ms ticks (the user's +324h). The view
reads it at the frame's time: the user's simulation time plus the game's frame milliseconds (game
+5854h, game::tick 0x1ECC47) since that simulation time was first seen, under one tick
(`FrameClock`, kept for the weapon drawn last). So the blend moves each frame and is never ahead of the
next tick. The scope view byte alone reads the tick's time (it is the world's).

### The sites and what each does

| site | VA | what |
|---|---|---|
| `hands_ik.aim_alignment` (order 0; patch 172's `on_aim_alignment` order 1 after it) | 0x295AA5 | the first-person gun's eye record (+7A8h, read by calculate_aim_alignment 0x244D40 next): the main eye placement blended toward the second by the second sight's share (s going to it, 1 - s coming back, 1 settled on it, 0 on the main), both `Eye::placed(EYE)` on the gun's aim bone (the main is the gun's own `aim` when no part moves the eye), `Placement::toward`; where it gives none (two bones, a skewed offset) the eye shown (past s 0.5). Written only when it changes, so patch 172's pose of it at rest stays (it poses the record each idle frame from what was written last); the weapon's load forgets what was written (`attachments.loaded`). The switch back writes the main eye again |
| `sights.zoom` (new) | 0x1F356E | update_camera's gun path after it set the camera's zoom (+94h) and near plane (+8Ch): settled on the main sight nothing (the game's own values); else f the two sights' factors blended by s (0.69d0's `current_camera_fov_factor`: (f_to - f_from) x s + f_from), the main sight's update_camera's own (the optic's fov_factor +114h and near_plane_factor +118h, or the weapon's +5B4h = 1 / aim_zoom_factor and +5B8h = 1), the second's the dictionary's `srp_sight_views` row of its optic (the weapon's own factors without one); then zoom 1 + (f - 1) x the effective aim, near (1 + (n - 1) x it) x 0.05 (the image's 0x9CC878) |
| `weapon_view.camera` | 0x1F3505 | the base branch's lens (view.rs, `Profile::focal_at`) at the effective aim (`sights::lowered`, one call in `on_view_camera`) |
| `sights.scope_view` (new, exit `decided` 0x244B5B) | 0x244B03 | weapon::tick (0x244AB0) where the scope view's rule applies (the local player's first view, an optic): while the second sight is shown at the tick's time, +181Eh = 0 and the tick ends there; else the game's rule. weapon::draw then draws the optic's idle model (0x244FBF), removes the aimed one and gives the body and the hands back (remove_aimed_rifle_scope_from_scene 0x244C50, 0x244F80..0x244FBA) and draws the body (0x244F00) |
| `attachments.draw` | 0x245017 | the second sight's part: its aimed model (`Module::aimed()`, the collimator with its dot) in place of its own at the part's record while the second sight is shown and the gun is drawn in first person (its model's foreground byte); its own otherwise. The aimed model is asked for at the cook after the parts' (`attachments.request_room`, `attachments.asked`), held with its part (`attachments.ready`), out of the scene with it (culled, hidden, released, lying) |
| `attachments.dictionary_configs` | 0x2C8AA0 | `srp_sight_views` read with the dictionary (`vostok::sight_view::load_views`, 16 rows at most; a bad row left out with a line) |

### The scope view byte is the world's, and already the local camera's

+181Eh is saved after weapon_core's record (weapon::serialize 0x244A14..0x244A34, a bool 0x00/0xFF)
and read back (deserialize 0x244A5E..0x244A6C). Its only writer while playing is weapon::tick, and only
for its user's first view (+F4E3h, `m_is_third_view`, 0 at 0x244AC0) of the local player (its place
the network client's local player's, 0x244AF1): the stock byte already depends on which game saves the
world and on its camera, and another game's copy of that weapon keeps what it last loaded. The patch
changes it only there, by the same kind of rule (the shown sight of patch 206's simulated state at the
tick's time), so a world's byte is the local player's own tick's as before: 0 where the stock would
say 1 while the second sight is shown. srp-validate's world parse takes it as the bool it was. No
`simulation_changes.tsv` row of its own (nothing a match computes reads it; patch 206's `next` row
covers the branch).

### Data

`gameplay/db_static_dictionaries`' `srp_sight_views` (edit list `sight-views-1.json`, its row at the end
of `release/content.tsv`'s edits): one row an optic, `{sight: dict id, fov: [fov_factor,
near_plane_factor]}`, the DeltaPoint Pro (2112) 0.8 / 0.5 and the RSA-S (2116) 0.5 / 0.5, each the
optic item's own config's (`shop_scope_leupold_deltapoint`, `shop_scope_hensoldt_rsas`).
`scripts/weapon-modules/sight_views.py` makes it from the optics the lobby's SecondSights offers (that
table is the pack record's: second_sights.py) and `sight_factors.tsv`, which `--live <the configs>`
takes from the scope items' configs; `--check` is tools/checks.tsv's `second-sights-zoom`.

### Not settled

- The second eye gets the gun's `parameters/aim_offset` like any eye (calculate_aim_alignment
  0x244D5B, along its own line): 0 with a fitted main optic, the iron sights' distance without one.
  The dot stays centred (the eye moves along its line); only its eye relief differs by that much.
- The commit: the tick's s reaches 0.9 at most (the simulation's), but the frame's time runs on to
  within a millisecond of the next tick (0.998 for a press at 104520: the frame at 105049, the commit
  at the tick 105050), so the settle shows no jump.

### Checked

- `cargo test -p patches` (`attachments::sights`: the share, the eye's blend and its jump between two
  bones, the zoom's blend and the effective aim, the frame clock), clippy.
- `check_sight_view_026g.py` (tools/checks.tsv `second-sights-view`), an HK416 with the sample's
  DeltaPoint Pro on its right rail through patch 161's cook and patch 206's sites: the aimed model asked
  for after the parts; the eye the main on the main sight, blended by the frame's time while switching
  (the crate's rule worked out again in the check), the second settled, the main after the switch back,
  the original's record left; +181Eh 0 while the second sight is shown, the original's rule on the main
  (also at s 0.5 coming back: the second is still shown); the camera's zoom and near plane from the
  second sight's row and the effective aim, the original's on the main sight with and without an optic;
  the weapon's own FOV at the effective aim (48.75 degrees half way); the reticle's model past half way
  in first person, its own before, out of first person and back on the main.
- `check_attachments_026g.py` with the second sight's aimed model among the cook's requests and held
  with the parts; the weapon-modules sample pins it (`aimed` after `second`).

### What a game must show

Aim with a second sight fitted, press Y: over about half a second the gun rolls (45 degrees on 0.69d0's
offset rail) onto the side sight, the view's zoom dipping half way (the effective aim) and coming back
at the side sight's own zoom; a magnified main scope's overlay goes off (its idle model, the gun and
the hands drawn) once past half way; the side sight's dot centred. Y again rolls back to the main sight
and its overlay. Firing works throughout; the other clients see the gun as before.

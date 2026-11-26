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

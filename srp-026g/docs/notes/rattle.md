# The Rattle (the owner, 2026-10-23)

A cut artefact brought back: it quiets its carrier's movement. 0.20e/0.20f had the item
(`artefact_rattle.options`, item type 8) and its model (`item/artefacts/artefact_pogremushka`);
the code (`artefact_rattle_core`, still in 0.26g's exe) was never written: every effect method
is an empty stub (0.20e PDB) and no build reads the config's `steps_sound_volume` keys. So the
numbers below are ours; 0.20e gives the model, the item's shape and the idea.

## The owner's rules

- Passive (from pickup to death, as every artefact): others hear the carrier's steps and
  movement noise at 70 % per Rattle. Copies multiply (patch 54: a stack adds up):
  one 70 %, two 49 %, three 34 %.
- Active (its key, patch 54's one press fires every ready Rattle): for 10 s others hear none
  of the carrier's steps and movement noise. Cooldown 40 s.
- Quieted: footsteps (walk, run, sprint, crouch), the movement rustles, jump and landing.
- Not quieted: reloads, weapon switches, equipment, hits.
- On top of crouching: crouch (40 %) with one Rattle is 28 %.

## Shots (the owner, 2026-10-23: "it should become silent if you dedicate your build to it,
at least 4 rattles? 50 -> 75 -> 87 -> 94?")

- Active only (the owner: "shots are active ability"): for its 10 s others hear the carrier's
  shots at 50 % per Rattle fired, copies multiply: one 50 %, two 25 %, three 12.5 %, four
  6.25 % (quieter by 50, 75, 87, 94 %). Four is a build given to it.
- The passive leaves shots as they are.
- A shot is the carrier's weapon firing (the sound others hear); the bullets' flight and
  impact sounds as before.

## Ours (to confirm)

- Others = every other player's game and the server's bots: a bot hears a carrier's movement
  and shots within its hearing range x the same factors (movement 0 and shots x 0.5 per Rattle
  while active).
- The carrier hears his own steps as before (0.20e lowered them to 65 % / 40 %).
- Owned: in an inventory with its holder alive, as the Sponge.

## Spawns

- As the Sponge: one Rattle in every map anomaly that spawns artefacts (a core with an
  artefact container and kinds), after the Sponge: exe patch 59 gives the anomaly cook the
  core's table with {758, 1} and {759, 1} after its own kinds (`sponge.md` "Spawns",
  client_patch.md 3.52); no map file ships. No timed anomalies (main has none since #345).

## Content (a pack, as sponge@2)

- A new items_dict id (the next free after the Sponge's 758), data type 8 (0.26g's exe still
  cooks `artefact<artefact_rattle_core>` for it), base_cost 0.
- `gameplay/items/artefacts/artefact_rattle.options` from 0.20e's: weight 0.2, duration 10,
  cooldown 40; the footstep keys dropped (unread) or replaced by the factors above.
- The model: 0.20e's `artefact_pogremushka.model` (770 vertices; 0.26g already has its
  materials `artefacts_02`, `artefacts_pogremushka` and texture `arttfacts02`). 0.20e exported
  no collision files for it: to check how a dropped or spawned Rattle is picked up.
- Texts in the six languages, one line each: 0.27d+'s pickup and activation hints exist in
  English ("Rattle artifact. Hushes the sound of steps by 50%." becomes 30 %).
- Icons: the lobby atlas cell and combat-log icon (to find, or draw as the Sponge's).

## How (verified in 0.26g, 2026-10-23; VAs of the original exe, image base 0x10000)

The Rattle's code
- PROVEN: 0.26g's `artefact<artefact_rattle_core>` vtable 0xA06124 (36 slots, RTTI locator
  0xA4C044) has 0.20e's stubs: +78h tick_impl 0x15A4B0 `ret 8`; +7Ch, +84h, +88h (passive on,
  passive off, active off) 0x1AF330 `ret`; +80h enable_active 0x75AF0 `ret 4`; +8Ch
  active_effect_ended 0x372420 `mov al,1 / ret`. The object is 360h bytes (artefact_base 340h,
  the drawable at +340h; constructor 0x25FA10, the cook's size `mov eax,360h` at 0x25E7C9).
- PROVEN, the active's timing: artefact_base::action 0x313300 (patch 54's hook at its entry)
  calls +80h with the time, writes +2A0h = the time, state 4. artefact_base::tick 0x313200 in
  state 4 calls +78h (dt, time), then +8Ch; true -> decrement_amount 0x313530: +88h, the
  cooldown +28Ch = floor(M38(+274h, `cooldown_sec` in ms)), state 3. With the stubs the active
  ends at its first tick and the cooldown starts at once.
- PROVEN, Spring's pattern (vtable 0xA05F44): enable_active 0x3118D0 sets +394h =
  floor(M39(+390h, the duration in ms)) (M39 artefact_duration, 0x2D5AE0 on the holder's
  modifiers), tick_impl 0x311890 takes dt off it, active_effect_ended 0x311990 is +394h == 0,
  serialize 0x3119A0 / deserialize 0x3119E0 add that u32 in state 4 after artefact_base's
  (0x313670 / 0x313760: state, cooldown, +2A0h), slot +48h 0x311A60 puts ceil(+394h / 1000)
  into the hud's timer field (+2) after artefact_base's 0x313880.
- PROVEN: only the constructor and the destructor (0x25FA34, 0x25FA6B) name 0xA06124, only the
  vtable names 0xA4C044: a Rattle with a vtable of its own (Rattle's RTTI locator, as the
  Sponge's) is taken by no code for anything else.

Steps and movement noise (others hear a 3D point sound, oneself in first person a hud sound)
- PROVEN: `player_equipment_sound_effect::on_player_sound_event` 0x2A6470 (thiscall, an
  animation sound event, `ret 4`; this+3Ch the player) dispatches five events: 0 and 1 the feet
  (on_foot_step 0x2A6520 -> step_manager::on_step 0x248AD0), 2, 3, 4 the rustles 0x2A6580,
  0x2A6630, 0x2A66C0. A jump's steps are the feet events with the jump material (movement 4,
  crouch_steps.md). The rustles 0x2A6580 and 0x2A6630 play only for the viewer's own player
  (`is_first_view` 0x1F3BD0 true): others never hear those two in 0.26g.
- PROVEN, landing: `player::on_landing` 0x1F3B30 (the controller's landing callback) calls
  0x23FBE0 at 0x1F3BB6 (cl not first view, edx the time, xmm0 the speed, four stack args,
  `ret 10h`): a step through on_step with the run or jump material when the speed is over its
  threshold and 50 ms went by.
- PROVEN: on_step and the rustles play through 0x657C0 (ecx the sound scene; the sound,
  &position, the hud flag; `ret 0Ch`): hud flag 0 (another player, or oneself not in first
  person) -> `sound_scene::new_point_sound_instance_proxy` 0x64F80 at 0x65874, else the hud
  proxy 0x65100. 0x64F80 has two callers: 0x65874 and emit_point_sound 0x65670 (at 0x656AB).

Shots
- PROVEN: a weapon's sounds are `weapon_sound_effect`s in the weapon core's sound handler
  states (the config's `states.<state>.sounds.first_view` / `third_view`); the shot is the fire
  state's (`states.fire.sounds`). The three fire handlers (initialize 0x251760, 0x2529D0,
  0x2533C0; vtables 0xA04EE4 weapon_core_fire_state, 0xA05150 double barrel, 0xA05248 pistol)
  hold theirs at +168h, +170h, +170h. on_sound_event 0x297C50 queues play_sound_callback
  0x297E20 (0x2981A0 schedules it; thiscall, the effect), which plays a hud proxy for the
  weapon's user in first person and else `emit_point_sound` 0x65670 at 0x297F79 (eax the scene,
  ecx the sound, two stack args, `ret 8`), the effect at [esp+2Ch] there. effect+30h is the
  weapon core, its +8 the user (set_user 0x2E2990; +594h the user's game object, user+110h).
- Reloads, switches (show, hide), chambering and the grenade's states are other handler states;
  bullets' flight and impacts are bullet_manager::play_sound 0x3222F0: not touched.

Where the volume is scaled per emitting player
- PROVEN: 0.26g's sound API has no volume per instance: a voice plays at clamp(amplitude x the
  options' volume) (notify_listener2 0x5C280, 0x5C710..0x5C743 -> 0x633F0), the amplitude the
  propagator's distance attenuation, written at 0x5CA37 (`movss [esi+4Ch],xmm0` in
  calculate_hdr_audio_frame 0x5C7A0; esi the propagator, +8 its proxy as 0.21d's
  new_sound_propagator). The same propagator's HDR loudness (dB) is [esi+40h], written at
  0x5C8BA. Both on the sound thread.
- PROVEN: a proxy's `m_ext_params` (atomic_half3, +38h) is zeroed by the proxy constructor
  0x5A6B0 (+38h, +3Ah, +3Ch); the sound code reads it once, emit_sound_propagators_impl
  0x5D0BB, and uses x alone (+38h, a start delay that sound_game_effect_presenter sets). LIKELY
  unread: y (+3Ah). Every atomic read of the sound code was found (cmpxchg8b at 0x5715A,
  0x571AD, 0x574DE, 0x57571, 0x57601, 0x57691, 0x5C1FE, 0x5C295, 0x5D0BB) and none takes y; a
  plain read cannot be excluded statically.
- So: the game thread marks the point proxy it makes for a carrier's step, rustle, landing or
  shot with a quiet q (u16 at +3Ah: gain 1 - q/65535, 0 the stock) before it plays; the sound
  thread multiplies the amplitude by the gain (0x5CA37) and adds 20 log10(gain) to the HDR
  loudness (0x5C8BA), so a quiet sound neither plays loud nor ducks the others. Gain 0 gives
  amplitude 0: no voice (0x5CA87). The carrier is known where the proxy is made:
  on_player_sound_event's this+3Ch and on_landing's edi around 0x657C0's point proxy (0x65874),
  the effect's weapon user at 0x297F79.

Who owns how many
- PROVEN: player+10Ch is the inventory (process_quick_slots 0x2D8D6E); its artefact slots are
  +10Ch + 4 x 13..18 (patch 54); a Rattle there is owned in state 3 (passive) or 4 (active);
  the holder's death makes it 5 (artefact_base::tick).
- PROVEN: at most 6 artefact slots: setup_from_profile sets the count (+16Ch, 0x2FD29F) to
  min(M14 artefact_slots, 13h - the first artefact slot). No 0.26g retail armour gives any
  (artefact_slots_modifier 0 in all 110 armour configs); the upgrades modifications[72] and [73]
  give +1 and +2 (defects[10] -1). LIKELY: four Rattles need such upgrades on two pieces.

The item
- PROVEN (data): 0.26g's Onyx and Spring have render meshes only (no collision files in
  `artefact_onyx.model`, `artefact_spring.model`); an artefact is taken through its anomaly's
  container (artefact_container_core::can_use 0x31C6B0), never by its model, and none is
  dropped at a death. 0.20e's artefact_pogremushka.model (no collision files) is enough.
- PROVEN: 0.20e's `artefact_rattle.options`: weight 0.2, data/type 8, data/duration 10 (0.26g's
  artefacts have it in data/active), cooldown_sec 0, four unread steps_sound_volume keys,
  effect `artefact_rattle` (0.20e's: Lifebone's particle human_zhivokost), icon 19 (0.20e's
  Onyx's too). 0.26g has no effect_artefact_rattle: ours names Lifebone's `artefact_lifebone`
  (the same particle, 0.26g's effect type 114).
- PROVEN: 0.26g has `st_rattle_pickup_hint` ("... by 50%") and `st_rattle_activation_hint` in
  all six languages, no name or description key. localization_add cannot change a text that
  exists (the porter refuses): those two by config_set on each localization file.
- The lobby icon: cell [21,1] of the consumables atlas (lobby_menu.swf 3276
  `LobbyMenu_AmmoIcons`), used by no 0.26g item; the hud's quick slots have it too (hud.swf 876
  `hudItems` cell 3: QuickSlots.hudIcons maps lobby id 20 there, its gray copy too). LIKELY the
  Rattle's own picture: orange cracked pods on grey twigs, the part of `arttfacts02` (the
  pogremushka's texture) Lifebone's blue flower does not use; the other free cell, [18,1], is a
  crystal coil. Seen (2026-10-23) in the atlas of the built patch's lobby_menu.swf (ffdec
  export of character 3276, the cell at (0, 1820) 171x83): orange pods on dark twigs, beside
  the Sponge's [22,1]; not yet seen in a running lobby.
- No combat-log icon: the Rattle deals no damage (0.26g's artefacts have no
  `combat_log_icon`).

The server's bots
- PROVEN: the match server keeps no inventory. A bot hears what a human's keys make
  (`bots/sense.rs`: a shot, sprint, jump, reload, switch, use, each within its range; walking
  is not heard) and the server never learns who took which artefact or when one is active.
  How it would learn of Rattles is an owner decision (not built).

Patch number
- Exe patch 59 with the Sponge (both artefacts ship together, `feat/sponge-rattle`): 56 is
  #318's, 57 the parked minimap work's, 58 #342's (on main), 60 #347's. The code is on the
  arena (`rattle.code`, `rattle.state`), not at a fixed `.srpx` offset.

## Release

- Changes what a match computes (a new artefact in the anomalies): the `next` row of
  `crates/session/src/v026g/simulation_changes.tsv`; the exe requires the pack.
- The site's /maps shows it as the Sponge (scripts/maps-tab/build.py, EXE_KINDS); its icon
  `artefact_rattle.webp` comes from the calculator's icon export of cell [21,1].
- The pack `rattle@2` is made by `scripts/rattle/make_pack.py` (its folder goes into
  SKHID_PACKS_DIR as `rattle-2`). @1 had 0.20e's `data/spawn_sec` 0.5, a float where every
  0.26g artefact has an int: release/build-content's value-type check refused it; @2 has
  Spring's 1.

## Built (2026-10-23)

- Exe patch 59 (README section 59, client_patch.md section 3.56): `rattle_026g.asm`,
  `rattle_026g.c` in the arena's blocks `rattle.code` and `rattle.state`; seven hooks and the
  cook's size; the game called through the engine library. Pack `rattle@2`
  (`scripts/rattle/make_pack.py`).
- The config (`data`): `active/duration` 10, `cooldown_sec` 40, `spawn_sec` 1,
  `passive/steps_sound_volume_3rd_person` 0.7, `active/steps_sound_volume_3rd_person` 0,
  `active/shots_sound_volume_3rd_person` 0.5 (each Rattle's own: they multiply); 0.20e's
  1st-person keys dropped.
- How much quieter: each Rattle in its passive x 0.7 on the steps; each active one x 0 on the steps
  and x 0.5 on the shots. Two presses that overlap (a Rattle picked up while another is active)
  count every Rattle active at the time of the sound.
- What a gain does: the sound's voice volume and its HDR loudness for every listener but the
  carrier in first person; gain 0 makes no voice. The carrier, and a spectator watching him in
  first person, hear him as before.
- Ours, beyond 0.20e's file: `amount` -1 (0.20e's 1 would use the Rattle up at its first active's
  end); spawn particle and effect Lifebone's (0.20e's `fx/explosions/grenate` and
  `effect_artefact_rattle` are not in 0.26g; 0.20e's effect was Lifebone's particle too); the
  model's LOD switching Spring's (0.20e's said default); `spawn_sec` Spring's 1 (0.20e's 0.5).
- Texts: name and description new; the pickup hint 50 % -> 30 %; the activation hint names the
  shots.
- Not built: the server's bots (they hear no Rattle: the server knows no inventory; an owner
  decision).
- Verified statically and in Unicorn only (`selftest_026g.py` check 59, `check_rattle_026g.py`);
  not run in a game.

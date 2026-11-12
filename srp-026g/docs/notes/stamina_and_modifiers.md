# Stamina, carried weight and the kit's modifiers in the 0.26g client

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`). PROVEN where an
address is given, unless said otherwise. Counted on saved worlds of recorded matches read with the repository's own
parser (538,779 worlds of living bodies, 470,716 pairs of consecutive worlds) and on a replayed match's traced
steps. S<n> below is the kit's modifier of index n (the table at the end); `apply_modifier` 0x2D5AE0. See also
`sprint_state.md`, `stagger_and_legs.md`, `damage_model.md`, `weapon_fire.md`, `player_bots_skills.md`.

## The stamina (player+6C0h; config loader 0x329120)

The bar is 0 to 100. Every world saves it whole (`player_stamina::serialize` 0x329310): the value, the low flag,
the last spending time, two boost times, and two modifier nodes.

- **The tick** (0x329790): in the sprint state it spends 5 x max(0, 1 + S19) x max(0, 1 + S67) a second (0x329A70);
  else it regenerates, when the bar is exactly 0 or 1,000 ms after the last spending, at 10 x max(0, 1 + S3) a
  second (0x3298C0); then the kill boost; then the low flag: set at 0, cleared over 20 (0x329868..0x3298AF).
- **A jump**: the gate asks the bar for the cost of 10 (0x2FC230); the cost is spent at the take-off, by the clip's
  event (0x3434A0 -> 0x329710), not at the press.
- **A hit drains it**: a bullet's first damaging hit takes the weapon's `stamina_damage` (5 to 80 by weapon) x the
  ammo's factor x max(0, 1 + S66) (0x2D9D0D..0x2D9DFE); a grenade its splash value.
- **A kill boosts the killer's** (a skill): about 100 x S63 over S28 ms (0x2DA1B1, 0x329960).
- **Holding the breath** (only while aimed; the reserve is `max_breath_holding_time` 5 s x max(0, 1 + S23), refilled
  at each aim and at each hold's end) to the end of its reserve empties the bar once, in that tick (0x31D7B0);
  never outside a hold. A hold needs the low flag clear.
- **A melee attack costs nothing**; its hit is a 1.7 m bullet of the weapon's `melee` block and drains the victim's
  bar like a round (0x33AB50, 0x2D9DC6).
- The record's two nodes are the carried weight's (0x329B20): the spending node 1.5 t on index 19 and the movement
  node -0.2 x max(0, 1 + S26) x t on index 4 (none with S32), t the weight's share over a threshold. So the stamina
  itself does not change the speed; the weight does, through that node.

## The carried weight

Summed over the 23 slots (0x2FC6E0): an item's unit weight (+218h) x its count (+260h where its kind stacks, else
1), and for a weapon its loaded rounds (+5E5h, one more with a round chambered, +5F1h) x its ammo item's unit
weight. An item's unit weight is its `weight`, or for ammo `clip_weight` / `clip_size` (0x2C8474..0x2C84F5). A life
starts with min(the kit's count, the item's storage + what the last body held of it) in each slot
(`setup_from_profile` 0x2FCB70, `unload_to_profile` 0x2FD2D0): 11,959 of 11,959 pocket slots of bots' later lives.
Overburdened is the weight at or over 30 + S5 unless S31 (0x329C60): no sprint. Index 32 refuses the sprint
outright (0x2DBFC4).

## The speed factor

F = max(0, 1 + S4) with nodes for the weight, each leg, the stagger, the weapon in hand, a carried battery and an
artefact; a walk moves at its gait's speed x F (`walk_blend.md`). Counted on the traced steps: every sprinting body
moves at 0.9917 x 5 x F: the sprint clip's root moves 3.2727 m in its 0.6 s (5.4545 m/s, not the config's `default`
5.5) and plays at desired / default = 5 / 5.5, so the body goes 4.9587 x F, under the cap 5 x F (10,909 of 11,214
steady sprint steps within 0.005 m/s). The other gaits' speeds from their clips are in `walk_blend.md`.

## Counted

- Both saved nodes are the weight's law in every world; the leg nodes too; the stagger's node too, a hit stamped at
  the world's own time left out.
- Between two worlds of a bot: at rest the bar is as the law gives in 99.7 % of pairs; a sprint's fall is the
  client's rate in 5,310 of 5,542 pairs; a rise in 21,997 of 22,335.
- A jump's spending starts 150 ms after the press in 4,617 of 4,799 jumps; 980 of 6,024 presses by bots never took
  off and spent nothing.
- The jump's gate (0x2FC230): the key, no leg wound (0x2FC254), the low flag clear (0x2FC260), the bar at the cost
  (0x2D6420), on the ground (0x7EF90); and no crouch state has a jump (`crouch_logic.md`). Of 6,069 lone presses by
  bots with 250 ms before the next world, 1,557 spent nothing: the bar under the cost in the world before 580; of
  the other 977: crouched 311, already in a jump 204, a leg pool empty 166, other 10, none of these seen in the two
  worlds around the press 339 (the worlds are a second apart). At a tick's precision (a replayed match's traced
  steps): 138 presses, 130 took off (129 of them one-tick presses), 8 did not: in a jump's air 5, falling 1,
  crouched 1, none of the controller's causes 1 (the bar or a leg, which a trace does not hold).
- A hit's drain clusters at the weapons' values (15, 20, 30, 35, 50, 60). The hits that drain nothing are hits
  with no id (400 of 772) and a bullet's later hits (339).

## Against the bots' reckoned bar (`bots/stamina.rs`)

The constants, the regeneration's rules, the low flag and the jump's gate are the client's. Different: a hit's drain
is not counted (the largest error: after a hit the reckoned bar is up to 70 over); the jump's cost is spent at the
press, also for presses that never take off; the weight is the kit's as bought, while a life starts lighter by the
ammo already spent (the spending rate a median 6.8 % high); the items' spending and regeneration modifications are
not in the rates; the kill boost is not ported (the breath's emptying needs a held breath, which the bots never
ask). Before any hit the reckoned bar is the saved one in 85.6 % of worlds; the low flag differs in 376 of 340,918.
The world saves the record, so a bot's own bar can be taken from it.

## Not read

Nothing of this section is left unread; the ten indices 9, 10, 25, 37, 40, 42, 43, 45, 47 and 64 have no reader in
the exe.

## The kit's modifier indices

"Reads" is the number of direct calls of the getter found for the index; the first site is given, with the kit's
nearest name at or under it (checked: 97 of 101 sites lie in that function; 0x30F61D, 0x3110D5, 0x31D1C8 and
0x33292B lie in a later one the kit does not name).

| index | name | what it scales | reads | a site |
|---|---|---|---|---|
| 0 | `dispersion_modifier` | the character dispersion's skill term (dispersion_calculator initialize / fire / tick, B = the weapon's cone values) | 3 | 0x3310c6 dispersion_calculator::initialize |
| 1 | `aiming_speed_modifier` | the aim's dispersion speed (dispersion_calculator::tick 0x331409) | 1 | 0x331409 dispersion_calculator::tick |
| 2 | `health_regeneration_speed_modifier` | a body part's regeneration speed (body_part::regenerate 0x32A4F4) | 1 | 0x32a4f4 body_part::regenerate |
| 3 | `stamina_regeneration_speed_modifier` | the stamina's regeneration speed (regenerate 0x3298FA): 10 x max(0, 1 + S3) | 1 | 0x3298fa player_stamina::regenerate |
| 4 | `movement_speed_modifier` | the movement factor F = max(0, 1 + S4) (max_speed 0x2DC097, movement_speed 0x2DBC3D / 0x2DBC7F; with 20 when aiming) | 4 | 0x2dbc1d base_player::movement_speed |
| 5 | `max_carried_weight_modifier` | the carrying capacity 30 + S5 (update_weight_modifiers 0x329B3B, is_overburdened 0x329CA2) | 3 | 0x2f768d player_profile::apply_kit_parameter |
| 6 | `pain_health_modifier` | the pain pool's maximum (base_player ctor 0x2D7007) | 1 | 0x2d7007 base_player::base_player |
| 7 | `artefact_search_time_modifier` | the artefact container's search time (0x31C7EF) | 1 | 0x31c7ef artefact_container_core::use_execute |
| 8 | `anomaly_damage_modifier` | anomaly damage (apply_anomaly_damage_modifier 0x3328E2) | 1 | 0x3328e2 body_part_parameters::apply_anomaly_damage_modifier |
| 9 | `engineer_use_time_modifier` | no direct getter call found | 0 | - |
| 10 | `engineer_success_chance_modifier` | no direct getter call found | 0 | - |
| 11 | `device_damage_modifier` | a trap's / grenade's damage (booby_trap hit_victim, grenade splash_parameters) | 5 | 0x304b1b booby_trap_core::hit_victim |
| 12 | `splash_radius_modifier` | a trap's / grenade's splash radius | 4 | 0x305657 booby_trap_core::splash_hit_params |
| 13 | `reload_speed_modifier` | the reload clips' speed (reload states, 4 functions): time / max(0, 1 + S13), LIKELY the form | 4 | 0x30e25a weapon_core_reload_state::get_weapon_lexeme |
| 14 | `artefact_slots_modifier` | artefact slots (setup_from_profile 0x2FD20F; additive) | 3 | 0x236151 game_world_ui::initialize_match |
| 15 | `pocket_slots_modifier` | pocket slots (UI 0x2360B0; apply_kit_parameter) | 2 | 0x2360b0 game_world_ui::initialize_match |
| 16 | `ammo_bags_modifier` | ammo bags (the lobby only: `apply_kit_parameter` with 16 at 0x222A5F, 0x288E3D) | 1 | 0x2f768d player_profile::apply_kit_parameter |
| 17 | `pocket_slot_size_modifier` | pocket slot size (the lobby only: with 17 at 0x222A93, 0x288E6F) | 1 | 0x2f768d player_profile::apply_kit_parameter |
| 18 | `recoil_modifier` | the recoil kick (weapon_recoil_calculator::on_shot 0x331DC7, 0x331EA5, 0x331F5A) | 3 | 0x331dc7 weapon_recoil_calculator::on_shot |
| 19 | `stamina_spending_speed_modifier` | the sprint's and the jump's stamina cost (sprint 0x329A92, the jump 0x32972F, its gate 0x2D643C) | 3 | 0x2d643c player_stamina::can_afford_jump |
| 20 | `movement_speed_when_aiming_modifier` | the movement factor while aiming: min(F(4), F(4 and 20)) (movement_speed 0x2DBC1D) | 1 | 0x2dbc1d base_player::movement_speed |
| 21 | `chamber_a_round_modifier` | the chamber clip's speed (weapon_core_chamber_a_round_state 0x30E58D), as 13 | 1 | 0x30e58d weapon_core_chamber_a_round_state::get_weapon_lexeme |
| 22 | `change_weapon_speed_modifier` | the weapon's HOLSTER clip's speed (weapon_core_hide_state_base 0x30DF7B): hide_time / max(0, 1 + S22); so also a battery's take | 1 | 0x30df7b weapon_core_hide_state_base::get_weapon_lexeme |
| 23 | `breath_holding_time_modifier` | the breath hold's reserve (the holding state's finalize 0x31D1C8, restart_on_aim 0x31D9C9) | 2 | 0x31d1c8 breath_state_holding::finalize |
| 24 | `dispersion_movement_influence_modifier` | the character dispersion of a moving state (apply_modifiers 0x331715, with 56) | 1 | 0x331715 dispersion_calculator::get_character_state_dispersion |
| 25 | `use_two_double_handed_weapons_modifier` | no direct getter call found | 0 | - |
| 26 | `overweight_penalty_modifier` | the weight's speed cost: -0.2 x max(0, 1 + S26) x t (update_weight_modifiers 0x329BF4) | 1 | 0x329bf4 player_stamina::update_weight_modifiers |
| 27 | `grenade_throw_power_modifier` | the grenade's throw power (throw_grenade 0x30A2DE) | 1 | 0x30a2de grenade_set_core::throw_grenade |
| 28 | `stamina_boost_time_modifier` | the kill boost's window, ms (tick 0x3297F6 / 0x329815, boost 0x3299A5; additive) | 3 | 0x3297f6 player_stamina::tick |
| 29 | `stamina_boost_interval_modifier` | the kill boost's tick interval, ms (tick 0x329840 through 0x2D5B80; additive) | 1 | 0x329840 player_stamina::tick |
| 30 | `fall_damage_modifier` | fall damage (on_physical_controller_landing 0x2D82C8, 0x2D83EE) | 2 | 0x2d82c8 base_player::on_physical_controller_landing |
| 31 | `can_sprint_modifier` | sprint while overburdened (is_overburdened 0x329C77: S31 > 0 never overburdened; additive) | 1 | 0x329c77 player_stamina::is_overburdened |
| 32 | `max_carried_weight_ultimate_modifier` | Heavyweight: no weight speed cost (0x329BD0) and no sprint (can_sprint 0x2DBFC4 through 0x2D5CD0, S32 > 0) | 2 | 0x2dbfc4 base_player::can_sprint |
| 33 | `drug_using_time_modifier` | a drug's use time (medkit::set_active 0x33D481) | 1 | 0x33d481 medkit::set_active |
| 34 | `drug_activity_time_modifier` | a drug's activity time (medkit set_active, tick, deserialize) | 3 | 0x33d416 medkit::set_active |
| 35 | `critical_regen_duration_modifier` | an armour's critical regeneration duration (armor::on_holder_assigned 0x33F28C) | 1 | 0x33f28c armor::on_holder_assigned |
| 36 | `pain_regeneration_speed_modifier` | the pain pool's regeneration speed (base_player ctor 0x2D7043) | 1 | 0x2d7043 base_player::base_player |
| 37 | `use_artefact_modifier` | no direct getter call found | 0 | - |
| 38 | `artefact_cooldown_modifier` | an artefact's cooldown (artefact_base::end_active 0x313613) | 1 | 0x313613 artefact_base::end_active |
| 39 | `artefact_duration_modifier` | an artefact's active duration (spring, lifebone, onyx enable_active_effects) | 3 | 0x31192b artefact_spring_core::enable_active_effects |
| 40 | `artefact_power_modifier` | no direct getter call found | 0 | - |
| 41 | `incoming_weapon_damage_in_anomaly_modifier` | a weapon's damage taken inside an anomaly (modify_incoming_damage 0x33286A) | 1 | 0x33286a damage_model::modify_incoming_damage |
| 42 | `artefact_synergy_modifier` | no direct getter call found | 0 | - |
| 43 | `armored_trap_modifier` | no direct getter call found | 0 | - |
| 44 | `friendly_trap_modifier` | friendly traps (booby_trap on_enter 0x304D4C; additive) | 1 | 0x304d4c booby_trap_core::on_enter |
| 45 | `can_defuse_traps_modifier` | no direct getter call found | 0 | - |
| 46 | `traps_explode_after_defuse_modifier` | a trap explodes after a defuse (use_execute 0x30500F; additive) | 1 | 0x30500f booby_trap_core::use_execute |
| 47 | `spot_enemy_traps_modifier` | no direct getter call found | 0 | - |
| 48 | `spot_artefacts_modifier` | artefacts spotted (player::st_draw 0x247054; additive, UI) | 1 | 0x247054 player::st_draw |
| 49 | `aim_time_modifier` | the aim transition's length (instant_aim_start / end 0x2E1B9A / 0x2E1C3B) | 2 | 0x2e1b9a weapon_core::instant_aim_start |
| 50 | `friendly_anomaly_modifier` | friendly anomalies (damage_model::friendly_anomaly 0x332AA3; additive) | 3 | 0x332aa3 damage_model::friendly_anomaly |
| 51 | `toxic_damage_modifier` | toxic damage (0x33292B) | 1 | 0x33292b body_part_parameters::apply_anomaly_damage_modifier |
| 52 | `pain_damage_after_hit_value_modifier` | the pain after a hit: value (pain_after_hit_listener 0x332D52) | 1 | 0x332d52 damage_model::pain_after_hit_listener |
| 53 | `pain_damage_after_hit_time_modifier` | the pain after a hit: time (0x332CFD; additive) | 1 | 0x332cfd damage_model::pain_after_hit_listener |
| 54 | `second_chance_damage_modifier` | the second chance's damage (damage_model::second_chance 0x3329AC) | 1 | 0x3329ac damage_model::second_chance |
| 55 | `second_chance_ramaining_health_modifier` | the second chance's remaining health (0x332A08) | 1 | 0x332a08 damage_model::second_chance |
| 56 | `dispersion_from_hip_modifier` | the character dispersion from the hip (get_character_state_dispersion 0x33177E, with 24 at 0x331715) | 2 | 0x331715 dispersion_calculator::get_character_state_dispersion |
| 57 | `drug_amount_modifier` | a drug's amount (medkit::tick 0x33D932) | 1 | 0x33d932 medkit::tick |
| 58 | `drug_hit_coeff_modifier` | a drug's hit coefficient (medkit::reduce_damage 0x33DBAF) | 1 | 0x33dbaf medkit::reduce_damage |
| 59 | `melee_damage_modifier` | melee damage (weapon::min_damage 0x2E1CC9, weapon::damage 0x2E1D2C) | 2 | 0x2e1cc9 weapon::min_damage |
| 60 | `respawn_time_modifier` | the respawn time (player_respawn_rule_base::on_event 0x316B4B) | 1 | 0x316b4b player_respawn_rule_base::on_event |
| 61 | `breath_vibration_modifier` | the breath vibration (breath_vibration_calculator::tick 0x31D722) | 1 | 0x31d722 breath_vibration_calculator::tick |
| 62 | `move_speed_weapon_penalty_modifier` | Bear Arms: a weapon's movement term <= 0 not added (weapon_core initialize / deactivate / deserialize; additive) | 3 | 0x2e2c5c weapon_core::initialize |
| 63 | `stamina_boost_amount_modifier` | the kill boost's amount: 100 x S63 over the window (boost 0x329980) | 1 | 0x329980 player_stamina::boost |
| 64 | `disable_second_weapon_modifier` | no direct getter call found | 0 | - |
| 65 | `injury_penalty_modifier` | the injury penalty on the cone (get_injury_penalty 0x331829) | 1 | 0x331829 dispersion_calculator::get_injury_penalty |
| 66 | `stamina_loss_on_hit_modifier` | a hit's stamina damage x max(0, 1 + S66) (hit_impl 0x2D9D3B bullet/grenade, 0x2D9DED) | 2 | 0x2d9d3b base_player::hit_impl |
| 67 | `stamina_spending_speed_skill_modifier` | the sprint's and the jump's stamina cost, after 19 (0x329AA5, 0x329742, 0x2D644F) | 3 | 0x2d644f player_stamina::can_afford_jump |

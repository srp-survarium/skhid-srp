# Armour configs: what each key does (0.26g)

For the wiki's Equipment section: which values of an armour item's config
(`gameplay/items/armour/<slot>/<item>.options`) change play, and how. Keys
are the same in the 127 stock configs (dump of 2026-10-25) and the 84 of
`equipment-029@3` (plus its `srp_premium`). Addresses are preferred-image
VAs of the 0.26g exe; re-kit has rows for each function named.

## Protection

A hit on a body part with damage D, damage type t and penetration P:

    D' = D * (1 - clamp(A - P, 0, 1))         0x32A07E..0x32A0AC (PROVEN)

A is the part's armour against t: the SUM of `armor` over every worn item
that covers the part (`armor::on_holder_assigned` adds each item's value to
the part's per-type total, 0x33F2DD; PROVEN). Same formula as the damage
calculator (`scripts/damage-calc/engine.js` `armourFactor`). Callbacks of
artefacts, drugs and the oxygen tank run around it (the calculator's
`incoming`/`final`).

| key | does | unit | combines | confidence | label |
|---|---|---|---|---|---|
| hit_params/<part>/hit_types/<type>/armor | armour of that part against that damage type | fraction 0..1 (shown x100) | summed over worn items, then 1 - clamp(A - P, 0, 1) | PROVEN 0x33ED49 (read), 0x33F2DD (sum), 0x32A07E (use) | Protection: <type> on <part> |
| hit_params/<part> (which parts exist) | the parts the item covers: brain, face, front, back, left/right arm, forearm, hip, foot | - | no key = no armour there | PROVEN (same reads) | Covers |
| hit_params/<part>/health | adds to the part's health pool | health points | added | PROVEN 0x33EC9B, 0x33F297 | (no config sets it) |
| hit_params/<part>/regeneration_speed | adds to the part's regeneration | per second | added | PROVEN 0x33ECCA, 0x33F2A4 | (no config sets it) |

Damage types (7; 0.26g dropped `blunt`): `ranged` bullets (PROVEN: the
gunshot path the calculator uses), `slash` melee and traps, `explosive`
grenades and explosions, `fire`, `radiation`, `shock`, `toxic` anomalies and
gas (LIKELY, by name and the configs that deal them). Penetration P is the
bullet's for `ranged`; for the other types, what P is was not traced
(LIKELY 0, so armour stops that fraction outright).

## Weight

| key | does | unit | combines | confidence | label |
|---|---|---|---|---|---|
| parameters/weight | carried weight: over 20 kg it slows movement (up to -20% at the limit) and raises sprint stamina use (up to +150%); at the limit no sprint | kg | summed with the kit | PROVEN `player_stamina::update_weight_modifiers` 0x329B20 (data.md "Carried weight") | Weight |

## Bonuses (modifications)

Every `parameters/*_modifier`, `*_bonus` and the slot counts
(`movement_speed_modifier`, `stamina_*`, `max_carried_weight_modifier`,
`pocket_slots_modifier`, `pocket_slot_size_modifier`,
`artefact_slots_modifier`, `ammo_bags_modifier`, `aim_time_modifier`,
`dispersion_modifier`, `recoil_modifier`, `reload_speed_modifier`,
`health_regeneration_speed_modifier`, `pain_health_modifier`,
`melee_damage_modifier`, `respawn_time_modifier`, `breath_vibration_modifier`,
`artefact_search_time_modifier`, `aiming_speed_modifier`, `money_bonus`,
`experience_bonus`, `reputation_bonus`) is 0 in all 211 configs. An item's
bonuses are its modifications (`default_modifications`, and what it rolls by
`available_modifications_mask`), which add to these paths
(`item_config_cook` 0x2F1750) and reach the player through
`armor::add_modifiers` 0x33F3A0 and
`player_params_modifiers_container::apply_modifier` 0x2D5AE0. Nothing hidden
here: the wiki shows them as "Comes with" / "Can roll" (modifiers page).

## Oxygen packs and masks

| key | does | unit | combines | confidence | label |
|---|---|---|---|---|---|
| data/amount_time_sec | how long the tank runs once switched on | seconds | used up while active; off when empty | PROVEN `oxygen_tank::oxygen_tank` 0x33E085, `tick` 0x33E5D0 | Oxygen: N s |
| data/influences[] {body_part, hit_type, threshold, hit_coeff} | while the tank is on, a matching hit (face, toxic) below `threshold` does nothing; above it, (D - threshold) x `hit_coeff` | damage points; factor | per hit | PROVEN 0x33E6F0 (formula), 0x33E246/0x33E270 (fields); "while on" LIKELY | Gas protection: hits up to N blocked, rest x0.01 |
| parameters/oxygen_amount | same number as amount_time_sec | - | - | not read by the client (no such string in the exe) | (skip) |

All 5 stock tanks and the 0.29 Firefighter tank use face/toxic, hit_coeff
0.01, threshold 30..100 (Firefighter 100), 30..140 s.

## Coverage and model

| key | does | confidence | label |
|---|---|---|---|
| data/additional_model_slots | other kit slots the model takes (a closed helmet takes the mask; an exo all): those slots stay empty | PROVEN srp `item_params::covered_slots` + client `profile_skin_visual_cook` 0x2048DF, lobby 0x28903E | Takes the <slot> slot |
| data/remove_head_model | the model hides the head | PROVEN read 0x2051CC; visual | (skip) |
| data/base_model, part_name, *_hud, additional_models, model_type, data_full/long/medium/short, data/type | which model and variant is drawn | visual (0x205337..) | (skip) |
| game_materials/* | footstep sound materials of boots | PROVEN `on_foot_step` 0x2A6520; identical in all 19 boots | (skip: no difference) |
| sounds[] | step, respirator and death sounds | audio | (skip) |

## Level, faction, economy

| key | does | confidence | label |
|---|---|---|---|
| parameters/item_level | the level the lobby shows | PROVEN (lobby, srp) | Level |
| parameters/match_making_level | the kit's level for matchmaking is the highest of its items' | PROVEN srp `matchmaking.rs` | Matchmaking level (show only if != level) |
| parameters/faction | the faction | PROVEN | Faction |
| parameters/faction_affinity | weight of the item in the kit's faction for reputation | LIKELY (0.21d `get_profile_faction_affinity`, srp v021d results.rs); 0.26g reader 0x2C8521 | (skip) |
| clothes_affinities[] {affinity_type, affinity_value} | loaded into the item's dictionary params | read 0x2C861A; effect UNKNOWN | (skip) |
| item_affinities[] | not read by the client (no such string) | - | (skip) |
| parameters/drop_weight, rarity, base_rarity_level | drops (srp drops.rs) | PROVEN srp | (skip) |
| parameters/degrade_per_use/match/death, reliability, repair_cost_ratio | wear and repair: items wear nothing on Skhid (#35) | client reads 0x2C86D3.., 0x286EC4 | (skip) |
| parameters/armor, ano_armor, arms_armor, back_armor | summary numbers; the client never reads them (no such strings); the card shows ui_desc/props_list | PROVEN (absent) | (skip; use hit_params) |
| ui_desc/props_list | the lobby card's lines (Armor, Isolation, ...) | PROVEN | as the lobby |

## For the wiki

Show: weight; per covered part the protection against each damage type (x100,
the parts grouped as the HUD doll does); the slots the model takes; for tanks
the run time and the gas protection; level, matchmaking level when it
differs, faction; "Comes with" / "Can roll" from the modifiers page.
Skip the rest (visual, audio, economy, unread).

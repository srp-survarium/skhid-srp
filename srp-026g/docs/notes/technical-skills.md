# Technical skill tree

The owner's tree (2026-11-08, laid out again 2026-11-15) is the spec; this note is how it is built. "My pick" marks a
reading chosen where the owner's list leaves it open (the owner may overrule it). Addresses are
0.26g preferred-image VAs.

## The tree

- The fourth skills tab, "Technical skills" (`st_engineering_skills_label`, the stock text in
  every language), ids 401..443 (branch 4, the tens digit the row). Seventeen skills about the
  equipment players have: batteries, oxygen tanks, smoke grenades, traps, grenades. No new items.
- Source: `scripts/content-edit/technical-skills-1.json` (`release/content.tsv`), names and
  descriptions in `client-patches/localization/*.tsv` (`st_srp_technical_<id>_name` /
  `_description`, the postfix `st_srp_technical_per_second`), icons in 0.69d0's
  skill-icon style (`scripts/technical-skills/icons-069.tsv`): an item's skill is its item's lobby
  render with a white badge (`scripts/technical-skills/make_icons.py`, the owner 2026-11-21), the
  others 0.69d0 engineering/utility artwork.
- Rows at 0/4/8/12/16 (the stock rule: a row needs that many points in the rows above it, in
  this branch). A full build is 30 points; Tank Pocket and Tank Depth exclude each other.
- A build can leave out one item's skills and still reach row V (the owner, 2026-11-21: "people
  cannot use this skill tree while ignoring some other skills like oxygen tanks"): without the
  tank skills rows I-IV give 17 points, without the traps' 18 (my thresholds).
- 2026-11-21 (the owner): Sapper 3 ranks of +15%; Rearm 2 ranks, 30 s then 20 s; Last Gasp in
  row IV; Tank Pocket and Tank Depth 2 ranks each (+2 pockets, +2 pocket size); Deep Breaths the
  sprint's energy use; Steady Breath the dispersion too.
- 2026-11-22 (the owner): Trophy Trap also gives back one's own disarmed trap, and a trap it pockets leaves the map at once; Rearm reopens a
  trap only when it was shot or hit with a weapon (a hand disarm stays the stock one: "only when
  they were shot at", "or hit with weapon").
- 2026-11-16 (the owner): no skill of one mode's objects, so Battery Hands is Dexterous Hands and
  speeds the ammo crate too.
- 2026-11-15 (the owner's list): Steady Breath, Blast Drill and Sapper take 2 ranks with bigger
  steps; Clean Air (421) is gone; Deep Breaths moves to row III (it keeps its id 431); Toxic Smoke
  halves the grenade pocket and Forest Protection works in its cloud; Bear Trap (444) is new.
- 2026-11-21 (the owner): a column per item: smokes, grenades, traps, oxygen tanks. Dexterous
  Hands (no item of the four) takes the traps' free place in row I; Tank Depth the smokes' in
  row II, Tank Pocket staying in the tanks' column (my picks).

| Row | Needs | Id | Skill | Ranks | Per rank (total) | Mechanism |
|---|---:|---:|---|---:|---|---|
| 1 | 0 | 401 | Dexterous Hands | 3 | battery pick-up and ammo crate resupply +10% (+30%) | exe |
| 1 | 0 | 402 | Dense Smoke | 3 | smoke cloud duration +10% (+30%) | exe |
| 1 | 0 | 403 | Steady Breath | 2 | weapon sway and dispersion while an oxygen tank is on -10% (-20%) | exe |
| 1 | 0 | 404 | Blast Drill | 2 | damage from its own and teammates' grenades -25% (-50%) | exe |
| 2 | 4 | 411 | Sapper | 3 | trap disarming +15% (+45%) | exe |
| 2 | 4 | 412 | Sharp Ears | 1 | hear an enemy's pin pull nearby | exe |
| 2 | 4 | 413 | Tank Pocket | 2 | with an oxygen tank: +1 pocket (+2); or Tank Depth | exe (lobby) + srp |
| 2 | 4 | 414 | Tank Depth | 2 | with an oxygen tank: +1 pocket size (+2); or Tank Pocket | exe (lobby) + srp |
| 3 | 8 | 422 | Return to Sender | 1 | pick up a live grenade nearby and throw it back | exe + srp |
| 3 | 8 | 423 | Trophy Trap | 1 | a disarmed trap, own or enemy, goes into the trap pocket; a kit without traps gets an empty slot for it | exe + srp |
| 3 | 8 | 431 | Deep Breaths | 3 | sprint energy use while an oxygen tank is on -20/-30/-40% | exe |
| 4 | 12 | 432 | Dead Man's Switch | 1 | killed: the throw key drops one of its smokes at the body | exe |
| 4 | 12 | 433 | Rearm | 2 | its traps that were shot open again after 30 s (20 s) | exe |
| 4 | 12 | 442 | Last Gasp | 1 | the tank runs out: +10% speed for 20 s, energy full | exe |
| 5 | 16 | 441 | Toxic Smoke | 3 | its smoke is tinted green and does toxic damage to enemies inside; a pocket holds half as many grenades | exe + pack + srp + lobby |
| 5 | 16 | 443 | Scavenger | 1 | a kill with its grenade gives one grenade back | exe |
| 5 | 16 | 444 | Bear Trap | 1 | its trap breaks the legs it catches; a pocket holds half as many traps | exe + srp + lobby |

"Weapon dispersion" in the owner's list is read as the aimed sway (my pick, as on 2026-11-08: a
skill never changes what a shot does).

## The ranks: modifier 47

Every skill travels packed in kit modifier 47, `spot_enemy_traps`: a mixed-radix number, a digit
of 0..=ranks per skill, each rank adding its digit's weight (the product of the radices before
it; `technical_skills::weight`), but Bear Trap's, the last, which takes `LOW` (15,925,248, the
others' bound) away: every sum lies in -LOW..LOW, each an exact f32 (the tree has 31.8 million
builds, more than 2^24 non-negative integers hold). The `technical-skills` crate
(`crates/technical-skills`) parses it once for exe patch 180 and srp; each skill's numbers are
there and nowhere else. A value that is not a whole number below the bound, or holds both of the
pair, is no skill at all.

| Order (weight) | Skill (ranks) |
|---|---|
| 1, 4, 16, 48 | 401 (3), 402 (3), 403 (2), 404 (2) |
| 144, 576, 1152, 3456 | 411 (3), 412, 413 (2), 414 (2) |
| 10368, 20736, 41472, 165888 | 422, 423, 431 (3), 432 |
| 331776, 995328 | 433 (2), 441 (3) |
| 3981312, 7962624, -15925248 | 442, 443, 444 |

Slot 47's evidence (2026-11-08, the exact 0.26g exe, read statically): no call of the modifier
getter 0x2D5AE0 or its wrappers passes 47 (the getter is read at 44, 46 and 48 around it:
0x304D4C, 0x30500F, 0x247054); the name `spot_enemy_traps_modifier` (0xA007E8) is referenced only
by the two name tables (0x928504, 0x9C9CA4); no access after a kit-stride `imul ..,704h` reads its
offset (+0BCh of the block); neither the stock resources.db nor the server game data names it.
Native modifiers 40, 43 and 45 carry the artefact and medical trees; 9 and 10 the regeneration
booster's modifications; 29 the hand-kill marker.

## Tank Pocket and Tank Depth (413, 414)

- Every kit pocket count of the game is the lobby's: the match takes a kit's pockets as the lobby
  filled them (srp sends them packed, `MatchKit::new`). The lobby asks
  `player_profile::apply_kit_parameter` (0x2F7660) for the pocket count (15) and size (17): the
  kit's modifier, then each worn item's own (0x2F7470).
- Exe patch 180 (`technical_skills`) at its epilogue (site `medical.pocket_count`, 0x2F76E8,
  order 1, before the medical tree's clamp to six pockets of five, order 2): a kit whose worn slots
  (0..6) hold an item whose config's `data.type` is 1 (an oxygen tank, the inventory cook's
  switch) gets a pocket a rank with Tank Pocket or a size a rank with Tank Depth (2 at most). The kit is
  apply_kit_parameter's EBX (profile +58h + kit x 704h: its slot words at +40h, its modifiers at
  +9Ch), the items the profile's (+1568h, 0B4h each), the dictionary its third argument.
- srp: `loadouts::KitLayout` counts the same pocket (an oxygen tank among the worn slots,
  `ItemClass::OxygenTank`), so a skill change (`narrowed`) takes off what the pair opened only when
  the pair goes, and never what the lobby's own limit allowed.

## The oxygen tank skills (403, 431, 442)

- "The tank is on": a player's inventory item whose vtable is `oxygen_tank`'s (0xA1C874, written
  only by its ctor 0x33E029) and whose m_active (+268h) is set. m_active and m_amount_ms (+26Ch) are
  saved with the inventory, so every game reads the same.
- Steady Breath (403): the aimed sway is `breath_vibration_calculator` (weapon +288h); its tick
  0x31D6E0 runs only while aimed and stores the horizontal and vertical offsets at +44h/+48h
  (0x31D95B, site `technical.breath_vibration`). With the tank on, both are scaled by
  `Skills::tank_sway` (0.9/0.8). The tick recomputes them from phase and amplitude and never
  reads them back, so nothing compounds. (Modifier 61 would scale only the dispersion-driven part:
  much less than the rank's visible share.)
- Steady Breath also scales a shot's dispersion by the same factor (the owner, 2026-11-21: "steady
  breath should also give dispersion", over the 11-08 rule that a skill never changes a shot):
  `weapon_core::get_dispersed_bullet_direction` 0x2E1D50 sums the static dispersion (weapon +3D8h x
  the ammo's) and the dynamic one (kept at 0 for a fully aimed shot) into XMM1, stored at
  0x2E1DBE (`technical.dispersion`; EBX the weapon, [EBX+8] the shooter); its only caller is
  instant_fire, every player's shot in every game. Stock modifier 0 scales only the dynamic part,
  so it does nothing to an aimed shot; this scales the whole. Shotgun pellets (0x2E2120) and the
  hud's hip crosshair (weapon::draw 0x244E10) keep their own sums.
- Deep Breaths (431; the owner, 2026-11-21: "energy consumption (same numbers)", in place of the
  regeneration): at the sprint's stamina cost a second (`medical.sprint` 0x329AAD, after both
  spending modifiers and Numb Legs, order 2) x `Skills::tank_sprint_cost` (0.8/0.7/0.6) while the
  tank is on. Jumps keep their cost.
- Last Gasp (442): the tank's tick spends the air (0x33E661, `technical.tank_empty`, a living
  holder's only); ECX 0 means it ran out this tick. Then the stamina is set to its maximum (as
  Larkspur II does) and the boost starts: `Gasp::On { until }` on the stamina clock (+70h,
  saved), 20 s. It lives in the tank's tail padding (+294h, no native access: the object is 298h
  bytes, its last native field +290h), saved after m_amount_ms (`technical.tank_save`, read back by
  `technical.tank_load`, checked against the stream's end), cleared by the ctor
  (`technical.tank_construct`) and by both refills (`technical.tank_refill`, `_2`: a respawn ends
  it). The speed: x1.1 at `medical.movement` (order 2, after Adrenalin); movement is cached, so the
  start and the end set the player's refresh_selection (+33Eh); the end is checked each tick at
  `battery.carrier_tick` (order 6). srp-validate's `OxygenTank` reads the word (`gasp()`): every
  tank of every world now saves 4 more bytes.

## Dexterous Hands (401)

The take state's time scale is built once per battery, with no player (0x338940); each time it
builds the take animation, `item_and_hands_expression` 0x338B20 stores the scale to the stack
(0x338B5E). There (0x338B61, `technical.battery_take`) the scale is multiplied by the taker's
`Skills::battery_take_speed` (1.1/1.2/1.3: a 0.5 s take is 0.45/0.42/0.38 s). The taker is the
battery's user (victory_item_core +8, `set_user` 0x300B00). The battery is in hand at once either
way; the carry state starts sooner (battery_pickup.md). LIKELY deterministic: every game builds the
expression from the same state, battery and taker (the mixer was not traced).

The ammo crate (the owner, 2026-11-16: "can do something else too, like refilling ammo in ammo
crates"): `resupply_zone_core::update_use` 0x2F8120 finishes the use when the ms passed since its
start (now - record +8) reach the zone's `resupply_time_in_ms` (+50h, 3000), and stores their
quotient as the progress (record +24h). After the subtraction (0x2F8147, `technical.resupply_time`)
the ms passed become ms x (10 + rank) / 10 for the user's player (`Skills::crate_ms`): the 3 s take
2728/2500/2308 ms and the hud's bar follows. Whole numbers, the same in every game.

## Traps: Sapper (411), Rearm (433), Trophy Trap (423), Bear Trap (444)

- 0.26g's trap: states 0 removed, 1 armed, 2 fired (stepped in), 3 disarmed (by hand, or shot: a
  bullet of the trap's `defuse_by_hit` type, "ranged", calls the same `defuse_completed` 0x3050C0).
  The look follows the state byte (four models; a new state value would read past them).
- Sapper: `use_execute` 0x304F30 floors the set's defuse_time (4 s since 2026-11-22, the owner: "make traps defussal 20% faster for everyone"; 5 s before) to ms each tick of a use; after
  it (0x304F92, `technical.defuse_time`) the ms become ms x 20 / (20 + 3 x rank) for the disarmer
  (`Skills::disarm_ms`: 3478/3076/2758 ms of 4000), whole numbers. The hud's bar divides by the same.
- Rearm: where a bullet springs a trap (0x305552, after the hit-type check; `technical.trap_shot`) a
  plain (not explosive) trap whose owner has the skill is disarmed as natively, then stays (state
  timer 0) and keeps its reopening time, the bullet's ms + 30 s (rank 2: 20 s), in its saved `m_action_time_in_ms`
  (+260h), which a plain trap never sets natively. Where the trap's tick reaches its action time
  (0x304DCF, `technical.trap_reopen`) a plain trap never runs the native delayed action (only an
  explosive one sets an action time natively); a disarmed one is armed again
  (`switch_to_state(1)` 0x305210 and `insert_collision` 0x304E20, as insert makes them). No new
  bytes: deserialize handles 3 -> 1 natively, srp-validate only parses.
- A gun-stock hit (patch 151, `trap.hit` at the function's entry) disarms a trap by going on at
  0x305552, this site: a stock hit is a shot to Rearm, and the trap opens again after its time (my
  pick: one rule for every way a hit disarms it).
- Trophy Trap (2026-11-22, the owner: "it only gives slot, not pocket depth", "only if you don't
  have traps equipped", "without having some at the beginning"): a kit with no traps gets an empty
  one-trap slot; a kit with traps gets nothing more. srp (`loadouts::trophy_pocket`, lobby-server
  `match_profile`) puts a trap stack of the match's own (instance 0, no account's; a stock of 0)
  in the kit's first free pocket of those its layout opens (`Account::open_pockets`). The game
  fills a pocket at the kit's load (`inventory::setup_from_profile` 0x2FCB70: min(count, the item's
  stock)) and at a refill (`stack_item::resupply_from_profile_slot` 0x2FFCE0) only from that stock,
  so the slot starts and stays empty. A finished defuse (after its defuse_completed, 0x305025,
  `technical.trap_defused`; a trap that goes off at the defuse, the owner's modifier 46, goes
  elsewhere) by a skilled player, of its own trap or an enemy's (never an ally's), puts one unit
  into the disarmer's trap stack if it has room (a placed unit, or the empty slot), and the trap
  leaves the map at once ("it also disappears immediately"): its state timer 0 and `remove`
  0x304F00, as defuse_completed does for a set with no disarmed life. The object is free again
  (`try_to_place_trap` 0x305EA0 takes an unplaced object first). After the match the slot counts
  as traps left (`autobuy::used`: owned less left), so it never charges a trap; an unplaced
  trophy is never an item made. (Before: a hidden spare unit on the trap pocket, which the kit's
  load filled: 3 traps for 2.)

- Bear Trap: the trap's hits are its config's two, `right_foot` and `left_foot` (slash, the amount
  0.6, piercing 1: `booby_trap_core::hit_victim` 0x304A40), each routed to its leg's pool (`right_leg`,
  `left_leg`, health 0.5; at 0 its threshold gives -20% movement and the leg's affect: broken). A leg
  with more health than the hit leaves (Strong Bones, armour) stays whole in stock. Where
  `body_part_parameters::hit_by_type` (0x329FF0) stores the new health (0x32A28B, `medical.wound`, order 2 after the medical
  tree's; ESI the hit since 0x32A284) a leg's pool hit by an item that is a trap set (vtable
  0xA044D4 or 0xA14414) whose holder's kit has the skill is stored as 0. The native thresholds and
  affects follow from the stored health. Only the legs: the body's and the pain's shares are the
  stock ones.

## Halved pockets: Toxic Smoke (441) and Bear Trap (444)

- "Decreases the carry capacity to 2 slots (like normal grenades)": the RGD-5 takes two of a pocket's
  units (`parameters/pocket_units` 2, `PocketShare::Pair`; one-pocket-a-type.md). With Toxic Smoke
  every grenade of the kit does, with Bear Trap every trap (`technical_skills::Skills::halves`).
- srp: `loadouts::KitLayout::share`, so every kit and preset is held to it, and taking the skill
  takes the units over the half off (a slot names units of the inventory's stack: nothing is lost).
- The lobby: `PlayerInfo.pocketUnits` by the item's category (grenades 24, traps 11), fed by the
  committed skills (`PlayerInfo.committedSkills`).
- The match plays the kit it is sent, so nothing of this is in the exe.

## Grenades: Blast Drill (404), Scavenger (443)

- A grenade's blast hits each body in reach through `base_player::hit` 0x2D9A60; its hit_info
  carries the kind (+1Ch, 3 explosive), the item (+40h: the thrower's grenade stack, or a trap's
  set) and the attacker's place (+44h, the place the game credits the kill to). A grenade stack is
  told by its vtable (the game's 0xA0B56C or the core's 0xA14824).
- Blast Drill: at `medical.hit` (order 2, after the hand kill and Brothers in Arms) a grenade
  blast whose attacker is the victim or a teammate does `Skills::friendly_blast` of its damage
  (0.75/0.5) to a victim with the skill. Enemies' grenades and traps are left as they are; the
  blast's stamina damage and stopping power too. On a teammate's grenade it multiplies with
  Brothers in Arms.
- Scavenger: at `player.killed_event` (order 2, after the ragdoll's) the kill's source (the frame's
  +50h) is the killer's own grenade stack for a grenade kill. For a killer that is the stack's
  holder, not the victim and not its teammate, with the skill, the stack gets one unit back
  (m_amount + 1, the weight dirty) when the next object to pull, grenades[count], is free: never
  armed and out of the world, or the very grenade exploding at the kill's time (+44h). Otherwise
  (another of its grenades still live) no unit: raising the count onto a live object would re-arm
  it. Spent objects are reused (arm clears spent), so no new world object is made.

## Sharp Ears (412)

- Every game already plays an enemy's pin pull as a 3-D point sound: each gun's
  `throw_grenade/pull_pin_substate` sound `weapons/grenade_throw/grenade_pin_out_01`, spl_preset
  `melee_wear` (0.755 at 0 m, 0.06 at 2.7 m, silent from 5 m). weapon_sound_effect::on_sound_event
  0x297C50 only dedupes the event (0x245430, per weapon); play_from_event 0x2981A0 queues
  play_sound_callback 0x297E20 in the weapon's delayed callbacks (+9FCh), which weapon::draw runs
  (0x244E28): mt_draw's tasks, after the step. No distance cut before the sound is made.
- Content edit `technical-sharp-ears-1.json`: a copy, `weapons/grenade_throw/grenade_pin_out_far`
  (the same .ogg/.rms byte for byte), spl_preset `material_hit` (1 at 0 m, 0.42 at 9 m, 0.22 at
  16 m, 0.06 at 23 m, silent from 35 m: heard to about 25 m, my pick); volume_k 0.9 and spl_value 60
  stay stock (grenade-bounce-1's lesson).
- Exe patch 180 (`technical_sharp_ears`) at `weapon_sound.made` 0x297F7E (order 2, after
  Concentration's tag): the effect is the pull-pin handler state's (+160h of it; the word 148h
  before the effect is that state's +18h vtable 0xA052F0, ctor 0x255386); the user's place
  (weapon +594h, +4) is not the local player's and not on its team (Concentration's rule,
  vostok::medical's ally); the local player's current kit has Sharp Ears (modifier 47, bit 10). Then
  game_world::play_sound 0x25B940 (the effect's world, +34h) plays the copy once at the thrower's eye
  (+44Ch, translation). Within 5 m the stock pull and the copy both play (my pick).
- The copy is asked for at the first match frame whose local player has the skill
  (`game_world.after_move`, order 8, the game's thread: the query calls back on the thread that
  asked) and kept for the process; a pull before it arrives is heard at stock range only. A game
  without the skill never loads it.
- Presentation only, the skilled player's own game: no simulation state, no `next` row; a
  spectator's or a replay's game (no local player) plays nothing extra.

## Return to Sender (422)

- The input is patch 85's (Use's press). While a grenade is offered, the press is sent as the
  command: marker bit 29 with all six quick bits (`technical_skills::throw_back`). Neither patch 85's
  pickups (patterns 3..25) nor patch 78's carry commands (one bit) use it. Use and the quick bits are
  consumed; patch 78's filter (carry.filter) takes the marker and the quick bits out before the game
  reads them. The match server relays it only from a player one of whose kits has the skill
  (`throw_back::sane`); the client sends it only from a current kit with the skill (its twin).
- Every game, at `technical.throw_back` (base_player::set_input 0x2D8747, past its early-outs,
  before the new input is copied), for a player whose current kit has the skill, the grenade is the
  nearest one, in the order places, inventory slots, stack objects (a tie keeps the first). It must
  be of a stack (vtable 0xA0B56C or 0xA14824) whose holder's team is not the player's; thrown
  (grenade_core +148h in the physics world, +14Ch armed, +154h not spent); with more than 100 ms of
  fuse left (two of the fuse's 50 ms steps, against the input's time [EBP+8]); its centre (+1C0h)
  within 2.5 m of the player's eye (+44Ch; my pick); with no static level between (patch 78's ray,
  group 24h mask 2). Everything read is saved or rebuilt by a load: the eye by deserialize
  0x2DB8CC, the grenade's transform by its deserialize 0x3040B5.
- The throw-back is two of the grenade's own virtuals, in the same call. First +1Ch
  remove_from_physics (0x248600 -> 0x303D50: model, contact callback and body out). Then +18h
  throw_grenade (0x248520 -> 0x303C10) from the player's eye matrix. The velocity is the eye's row 2
  x the stack's throw_power (+2B8h) x the player's modifier 27, as grenade_set_core::throw_grenade
  (0x30A2AD..0x30A321) throws from the hand. Neither call touches the explode time or the tick node,
  so the fuse keeps running. There is no animation and no sound (my pick: instant at the input). The
  saved state is the native one, so srp-validate needs nothing new.
- The kill stays the original thrower's (my pick). Explode credits grenade_core +8 (hit +44h), and
  grenade_core::deserialize sets it again from the stack's holder (0x303FEB..0x304013), so crediting
  the returner would need a new synced record (the saved flag byte has bits 3..7 free). A grenade
  thrown back into its thrower's team counts as the thrower's team kill or suicide. Blast Drill treats
  it as a teammate's grenade; Scavenger refunds only a kill of one of the thrower's enemies.
- Only enemies' grenades (by the stack's holder): the thrower's own team cannot take it back.
  Smoke grenades are the same class and are taken too (my pick). A thrower's respawn resets its
  stack (0x309E70), which also takes out a returned grenade still flying.
- The prompt (this game, composed in artefact_carry's on_hud before patch 78's and 85's targets):
  "Pick up <grenade> ({USE})" (`st_weapon_pick_up`, the stack's ui_desc name), shown when the local
  player is alive, has the skill, and the same rule finds a grenade.
- LIKELY, not proven: taking a Bullet body out of the physics world and putting it back in one tick
  is deterministic in every game, and changing the physics world in set_input (not mid-step) is safe.

## Dead Man's Switch (432)

- A dead player whose kit has the skill presses the throw key (G) during the respawn countdown. One
  of its smokes drops at its body with no speed and goes off as any smoke: once a life, while a smoke
  is left, and only if it would go off before the respawn.
- Channel: a dead player's game sends no player input (0x1F7883; set_input skips the dead,
  0x2D86EE), but it sends its profile input (select_profile 0x1F8070, 0xDE relayed as 0xF3). The
  switch is a profile input of value 80h (`technical_skills::dead_mans_switch::INPUT`). The place's
  tick (0x321790) keeps the next kit for any value of 3 or more, so it changes no kit.
- Every game (`technical.place_tick`, 0x321790's start): a switch node of the place's list
  ([core+89D8h] + place x 480h) whose wire tick is this tick plays. The player must be dead and in
  the world, with the skill, no smoke of its own armed or flying, and a smoke left. The smoke is the
  stack whose splash_radius and splash_max_damage are 0 (rdg-m). The fuse must end before the
  place's respawn (respawn rule +138h + place x 4), which takes the kit's grenades out unexploded.
  Then pull_pin 0x30A120 and the grenade's throw (vtable +18h) at the body's transform (+40Ch),
  0.15 m up (my pick), with zero velocity. From then on it is a stock thrown grenade: saved, late
  join and rollback as any. No new saved state.
- This game: the throw key at game_world::on_keyboard_action (`technical.game_key`, 0x25CD69, action
  20h). The frame's end (`game.frame_end`, 0x1ECC47, before the game's time moves on) queues it on
  the 50 ms grid: a profile input's time on the wire is its tick alone (0x3185A0 / 0x318540), while
  this game's own keeps its milliseconds (a latent stock skew: an owner's kit pick takes effect a
  tick later than in the other games). This game's profile inputs never share a wire tick
  (`technical.select_profile`): a kit pick in the switch's tick is held and queued a frame later.
- srp: a switch is relayed as any profile input and is not counted as a kit choice
  (`Game::profile_input`).
- "Once a life" is the sender's: the simulation refuses a switch while one of the player's smokes is
  armed or flying, but a second switch after the first went off would play.
- LIKELY: the game world's key handler gets G on the death screen (patch 58's key-3-after-death
  report shows keys reach it there); an on-grid local input is simulated at its tick, as relayed
  inputs are.

## Dense Smoke and Toxic Smoke (402, 441)

- The cloud is only the particle system `fx/explosions/smoke_grenade`: 55 still puffs, each living
  29 s, full from 1.9 s and gone around 26 s. The grenade has no cloud object. The shared numbers
  are in `technical_skills::smoke`.
- The client grenade (vtable 0xA046A0) is made 270h bytes instead of 258h at its only factory
  (`smoke.grenade_size`, 0x1F6642). The ctor (`smoke.grenade_construct`, 0x2483BE) clears the extra
  bytes: +258h the cloud word, +25Ch the blast ms, +260h the centre, +26Ch the green instance.
- Toxic Smoke, simulation:
  - `smoke.explode` (0x248720): a smoke grenade (its stack's `splash_max_damage` is 0, my pick) of
    a thrower with the skill keeps {explode time, centre +1C0h}. Any other blast of the object
    clears it. The cloud lasts until the same object goes off again, as the stock particle does.
  - The cloud is saved after the thrown fields: `current_state` bit 3 (native deserialize ignores
    it) and 16 bytes (`smoke.save_state` 0x303DF8, `save_cloud` 0x303E40, `load_cloud` 0x30406C);
    srp-validate's `GrenadeCore::cloud()` parses it. Every kit's grenade objects are always in the
    objects section, so a kit switch at a respawn cannot drop a cloud.
  - At each player's tick (`battery.carrier_tick`, order 6) a living enemy whose eyes (feet + 1.6 m)
    are within 8 m horizontally and -1.3..+5 m vertically of the centre takes `base_player::hit`
    0x2D9AE0: the face, kind 4, piercing 0, rate x the tick's ms inside [1.9 s, 26 s + Dense
    Smoke's extra], the thrower's place as attacker (the kill credits it) and the stack as source.
  - Rates 0.05/0.06/0.07 a second (the owner, 2026-11-21: "+5/+6/+7"; 0.05/0.08/0.11 before; the stock gas zones do 0.07-0.25 on the face). The
    oxygen tank protects by the stock rule (LIKELY: its threshold blocks hits this small; the
    Unicorn check stubs base_player::hit).
- Forest Protection (523) in the cloud ("also forest protection works inside of it"): its rule is
  the native one at 0x332810, a hit with an owner on a player whose damage-zone count (+2E4h) is
  above 0 is x modifier 41. Where it reads the count (0x332833, `smoke.anomaly_zones`) a player in
  no zone whose eyes are in a toxic cloud at the hit's time (hit +3Ch) leaves by the hook's exit
  (0x33283C), where the factor applies as in a zone. Any toxic cloud counts, the player's own
  team's too (my pick: a zone protects whoever stands in it). Patch 111's glow asks the same
  (`effect::Player::in_toxic_smoke`, at the player's stamina clock), so the protected player shows
  it. The toxic dose itself has no owner's initiator (as a gas zone's), so it is not reduced.
- Tint: the grenade cook (`smoke.cook_query`, 0x25E154) asks a smoke grenade for a fourth
  resource, `fx/explosions/smoke_grenade_toxic` (pack `toxic-smoke`, scripts/toxic-smoke/make_pack.py:
  the stock particle with green copies of its smoke materials). It is kept at `smoke.cook_ready`
  (0x25E308), played at `smoke.play` (0x248908) for a thrower with Toxic Smoke, and released in the
  destructor (`smoke.grenade_destroy`, 0x2483F0). One extra instance per smoke grenade object.
- Dense Smoke: the game cannot pause a particle system through the renderer (the facade 0x22B30
  binds paused = false). Patch 180 pauses the played instance itself: its pause byte +302h, at the
  instance tick's pause test (`smoke.particle_tick`, 0x35DAF5, the particle world's thread), from
  2 s after the play for rank x 2.9 s; at most 32 clouds held at once. The toxic window grows by
  the same time.
- Who: everyone whose eyes are in a cloud, the thrower and its team too (the owner, 2026-11-22:
  "everyone"). The hit is a gas zone's: no initiator and no source item (an item without an
  initiator ends the game: hit_impl reads the initiator's vtable once a hit with an item was
  applied, 0x2D9CD5), so it lowers the `infection` pool as a zone does and starts the poisoning
  (the heavy breathing, `damage_sound_effect::on_poisoning` 0x23EFD0) and the intoxication effect.
- Rounds (Freeze Tag, Artifact Hunt): while a round is about to start (the core's time state 4,
  `round_countdown::Play::RoundCountdown`) every toxic cloud is cleared, so none hits into the next
  round (the owner, 2026-11-22: "smokes should be reset in those"). The particles (grey, green) play
  out their life as the stock smoke does.
- Limits: a late joiner never sees an existing cloud (stock) but still takes its damage; a grenade
  thrown back with Return to Sender stays its original owner's cloud; Scavenger refunds a smoke on
  a toxic kill (the stack is the kill's source). The bots' smoke model (pr/1426, unmerged) would
  take its timeline from `technical_skills::smoke` plus the thrower's extra time.

## Status

- Built, all seventeen, on main of 2026-11-15 (the 2026-11-08 work carried over: main's Concentration
  no longer tags sounds, so `weapon_sound.made` and its fields are this patch's now; main's kit
  layout holds every kit to its pockets, so `KitLayout::new` takes the worn oxygen tank).
- Checked: clippy and the crates' tests (technical-skills, survarium, srp-validate, session,
  lobby-server, match-server, the patches crate), the generators and the arena link; Unicorn
  `check_technical_skills_026g.py` (the tank skills, Dexterous Hands, Sapper, Rearm, Blast Drill,
  Bear Trap) and `check_technical_smoke_026g.py` (Dense and Toxic Smoke, Forest Protection in the
  cloud); the lobby harness for the tab.
- Not Unicorn-checked: Tank Pocket/Depth (the lobby's item configs), Scavenger, Trophy Trap's two
  hooks, Sharp Ears, Return to Sender, Dead Man's Switch (crate and server tests only).
- Not run in a game.

# Artefact skill tree

The owner's table (2026-11-05) is the spec; this note is how it is built. "My pick" marks a
reading chosen where the table leaves it open (the owner may overrule it). Addresses are 0.26g
preferred-image VAs; `apply_modifier` is the kit-modifier getter 0x2D5AE0.

## The tree

- The fifth skills tab, "Knowledge of the Forest" (`st_anomaly_skills_label`, the stock text in
  every language), ids 501..542 (branch 5, the tens digit the row). Eleven skills: the owner's ten and Artifact Container
  (the owner, 2026-11-05, retail 0.31a2's 513). My pick: the fourth tab ("Technical skills") stays
  for the equipment skills.
- Source: `scripts/content-edit/artefact-skills-1.json` (`release/content.tsv`), names and
  descriptions in `client-patches/localization/*.tsv` (`st_srp_artefact_<id>_name` /
  `_description`), icons 0.69d0 `skills/anomaly/*.png` (`scripts/artefact-skills/icons-069.tsv`).
- A row needs its points spent in the rows above it, in this branch (the stock rule, as the
  medical tree).

| Tier | Needs | Id | Skill | Ranks | Per rank | Mechanism |
|---|---:|---:|---|---:|---|---|
| 1 | 0 | 501 | Quick Hands | 3 | collection time -8% | data: modifier 7 |
| 1 | 0 | 502 | Resistance | 5 | isolation +5 | exe hook |
| 2 | 5 | 512 | Knowledge of Artifacts | 5 | active duration +5% | data: modifier 39 |
| 2 | 5 | 513 | Artifact Sight | 1 | passive artefacts seen as leg glows, own too | exe (patch 111's gate) |
| 3 | 10 | 521 | Keeper of the Artifacts | 1 | starts with a random artefact | server + exe |
| 3 | 10 | 522 | Artifact Container | 1 | a pocket becomes an artefact container | data: modifiers 14, 15 |
| 3 | 10 | 523 | Forest Protection | 4 | bullet and fragment damage -5% in an anomaly | data: modifier 41 |
| 4 | 15 | 531 | Spring II | 1 | Spring: reload +5% passive, +15% active | exe (patch 54's Spring) |
| 4 | 15 | 532 | Onyx II | 1 | no fall damage while Onyx is active | exe (patch 54's Onyx) |
| 4 | 15 | 533 | Larkspur II | 1 | activation restores all energy | exe |
| 5 | 20 | 541 | Resonance | 1 | passives shared, 9 m; or Cascade | exe |
| 5 | 20 | 542 | Cascade | 1 | actives shared, 6 m; or Resonance | exe |

- Layout, four columns (1..4) (the owner, 2026-11-06: Artifact Container beside Keeper, Resonance
  and Cascade an either/or pair in row V, as every tree's last row; 2026-11-07: Artifact Sight in
  row II, Forest Protection moved to row III (511 became 523), beside Keeper as an either/or
  pair; 2026-11-09: Forest Protection and Keeper no longer exclude each other):

```text
| - Q R - |    Quick Hands, Resistance
| - - K S |    Knowledge of Artifacts, Artifact Sight
| F P A - |    Forest Protection, Keeper of the Artifacts, Artifact Container
| S O L - |    Spring II, Onyx II, Larkspur II
| - N X - |    Resonance or Cascade (antiskills of each other)
```

- Points (25 ranks in all; one of Resonance and Cascade): row I holds 8, rows I-II 14, rows
  I-III 20, rows I-IV 23, a full build 24. Without Forest Protection rows I-IV hold 19: row V
  needs at least one of its ranks.
- Thresholds 0/5/10/15/17 (the owner, 2026-11-14: row V at 17; the stock 0/5/10/15/20 from
  2026-11-07): every row opens (row I holds 8 for 5; rows I-IV hold 23 for 17, 19 without Forest
  Protection).
- Quick Hands: 3 ranks of -8% search time (max -24%; the owner, 2026-11-09, "should be 3", with
  more a rank; 4 of -6% from 2026-11-07, the saved ranks past 3 settled and refunded). Resistance: 5 ranks
  of +5 isolation (max +25; the owner, 2026-11-07, back from 4 of +6).
- Quick Hands (501) was "Concentration", 0.69's own name for this skill
  (`st_artefact_search_time_name`, in 0.27..0.69 alike); the survival tree's 333 has that name, and
  no other retail string names an artefact search skill, so the owner's "Quick Hands" (2026-11-06).
- The skills window's last tier is the branch's highest `branch_points` (was a constant 20, the
  stock and medical last tier), so the bar fills when tier 5 is taken.

## Transport

- Data skills: their native modifier, summed into the kit's 68 floats by srp's `SkillsTree`
  (`skills.rs`), as every stock skill.
- Exe skills: one packed integer in modifier 40 (`artefact_power`), parsed once (`Skills::parse`
  in the patch); invalid (fractional, negative, too large, a rank past its levels) is no
  artefact skill at all. Bits: 0..2 Resistance (0..5), 3 Keeper, 4 Resonance, 5 Spring II,
  6 Onyx II, 7 Larkspur II, 8 Cascade, 9..11 Keeper's draw, 12 Artifact Sight, 13 a late
  joiner's kit (srp's mark, not a skill: Keeper below).
- Modifier 40 evidence (the exact 0.26g exe, 2026-11-05): all 94 calls of `apply_modifier` and
  its wrapper 0x2D5B80 resolve to an index, none 40; `artefact_power_modifier` is referenced
  only by the two name tables (0x9284E8, 0x9C9C88); no other config, edit list or code
  names it. 42 (`artefact_synergy`) is patch 54's stopping list, 43/45 the medical ranks.
- Keeper's draw is not a skill rank: srp draws it once per match and writes it into modifier
  40 of every kit of every player, bots too, with the skill or not (bits 9..11: the kind's draw,
  1 Larkspur, 2 Onyx, 3 Spring; 4 the Sponge once it is on main; 0 none), so every game knows it
  from the static info. The parser takes 1..6; a draw past the kinds gives nothing.
- Bit 14 and above: nothing (a payload with one is refused).

## Quick Hands (501)

- Modifier 7 `artefact_search_time` -0.08 a rank (3 ranks, -24%): the container's search time x max(0, 1 + sum)
  (0x31C7EF). Adds with the gloves' 1130..1134 and the Artifact-master's -25%: rank 3 with
  both is x0.25.

## Resistance (502)

- Owner's rule: +5 isolation a rank, flat points (5 ranks, +25; 2026-11-07, back from 4 of +6).
- Isolation points are armour against the four anomalous hit types (toxic, radiation, fire,
  shock; table 0x92075C), the card's "Isolation" x100. A hit's damage is
  D x (1 - clamp(A - P, 0, 1)) (0x32A07E), A the part's summed armour for its type.
- Flat (the table's words): A + 0.05 a rank for those four types on every body part, at
  0x32A083 (`artefact.isolation`), before the piercing's subtraction and the clamp.
- Modifier 8 `anomaly_damage` is not this: it multiplies an anomalous hit's damage by
  max(0, 1 + M8) before armour (0x332890), a percentage (retail's 0.31+ Resistance).
- The owner (2026-11-05): flat points, the hook above, max +25. The skill card says it as an
  isolation bonus ("Isolation against anomalies: +5 points" a rank).

## Forest Protection (523)

- Row III, beside Keeper of the Artifacts. It was Keeper's either/or (the owner, 2026-11-07:
  each the other's `antiskills`); the owner took the pair away on 2026-11-09, so both can be
  taken. A saved set holds at most one of them, so no account is settled by it.
  It was 511 in row II; nothing in the exe names the id (patch 111 reads modifier 41).
- Modifier 41 `incoming_weapon_damage_in_anomaly` -0.05 a rank (4 ranks, -20%) (0x332810, before armour): a hit
  with an owner (hit+20h) on a player whose damage-zone count (player+2E4h, damage_zone_core's
  enter 0x319967 / leave 0x31980B) is above 0 is x max(0, 1 + sum).
- "While you receive isolation damage" = while inside an anomaly's damage zone, the area that
  deals it (my pick: the native count, not each damage tick).
- A hit with an owner is another player's (or one's own): bullets and grenade fragments, also
  a knife or a trap (my pick: the native rule; no hit-type filter).

- The glow (the owner, 2026-11-05): while it protects (ranks above 0 and inside a damage
  zone), a grey glow on the player's legs, like the active artefact's glow (artefact-glow),
  so other players see that the player is protected there: every client, enemies included, in
  the third person; the own first-person view optional. Every game knows the kit's modifier
  41 and the zone count, so it is derived where it is drawn: no new network data, no
  simulation change, no `next` row.
- Built (exe patch 111 `forest_glow`, client_patch.md 3.73; pack artefact-glow@5):
  - the effect `forest_protection` (gameplay/effects/effect_forest_protection): the Onyx glow's
    particle effect, `view` 2 (the others' third-person view and the player's own first-person
    legs), playing fx/artifacts/human_forest_protection: the stock Onyx glow (its emitters, sizes,
    rates, haze) with every body part but the six leg parts off, the emitters with no leg part
    left out (the hands' strokes, the chest's rays), the materials' grey copies
    (fx/artifacts/rays_grey, stroke_scroll_grey, up_grey: the stock material, constant_emissive
    0.5 grey) and the point light grey (scripts/artefact-glow/make_pack.py);
  - the trigger, in each player's simulation tick (so the effect is in the world's history, as
    the artefacts' glow): alive, its damage-zone count above 0 and its current kit's modifier 41
    factor below 1 (the native getter): the effect played when none plays, with the player's own
    calculator, which ends it when the condition ends. Nothing kept but the loaded resource; the
    player's effect list says whether it plays.
  - The point light of the glow (the stock `up_light`, grey) also lights the player's body and
    the ground near it (the renders); a lighter version would leave it out.
- On the hands in pack artefact-glow@9 (the owner, 2026-11-06; replaced in @10): the stock Onyx glow, grey,
  made as a leg quarter is (`quarter_particle`) with both forearms and hands (LeftForeArm,
  LeftHand, RightForeArm, RightHand) for its parts: the emitters with none of them left out (the
  hands' strokes and the point light now kept), each kept emitter's Rate and MaxParticles times
  its kept share of the stock emitter's parts. Everyone sees it, both views: the carrier too, on
  its own hands in the first person. It no longer lies on the leg quarters.
- Since @10 (the owner, 2026-11-06: the grey cloudy puffs @8 showed around the legs are what
  Forest Protection should look like; on the hands @9 lost them, those emitters have no arm
  part): the legs' system of @5..@8 value for value (its puffs and strokes, rays, haze and light,
  the same emitters, rates and sizes) with each animated domain's leg parts moved to the arm
  parts: upper leg to upper arm, lower leg to forearm, foot to hand. No rescaling (one part for
  one). `check_forest` proves the legs' system is the file @5..@8 shipped and that only those
  flags moved.
- Since @11 (the owner, 2026-11-07: the arms glowed from some sides only, the cloud must be
  outside the arm on every side, a fog as the leg quarters show): an animated domain puts each
  particle at a random point of its body part's shape, the point's coordinates times the
  domain's Scale per axis in the part's own frame, and keeps it there as the part moves (0.26g
  0x369030, `particle_action_animated_source::init`). The stock Scale was made for the legs:
  `up` pushes its puffs out on x only (1.76 -> 1.26) and keeps them inside on y (0.79 -> 0.83),
  the haze is flat (z 0.02), the strokes sit at each part's centre (0). On the arms that put
  the cloud inside the sleeve or along it, and the point light at the hands (lighting the
  face from the front) was most of what showed. Now the puffs, the strokes and the haze have on
  all three axes the x curve of `up` (the haze its own, 1.0), and the puffs are twice as large
  (`FOREST_OUTWARD`, `FOREST_PUFF_SIZE`); the light and everything else are @10's, and so is the
  first-person twin. `check_forest` allows exactly those values (and their data sources).
- Since @12 (the owner, 2026-11-07, from renders next to the stock Onyx activation: "dark grey
  smoke no light is good"): a dark-grey smoke around the arms. The stock activation's fog beyond
  the body is its `up` puffs and `body` strokes, which have no arm part; the arm emitters alone
  (strokes, rays, haze, light), at any rate or grey, showed only a tint. The system: the stock
  Onyx glow cut to the six arm parts, every kept emitter at its full stock rate (the strokes
  `left_hend`/`right_hend`, `rays`, the haze), no point light; plus the stock `up` and `body`,
  full rate, their domains on the six arm parts only and their spawn outward on every axis (each
  Scale axis `up`'s x curve, as @11). The puffs and the arm strokes are drawn with
  `fx/artifacts/smoke_dark_grey`, a copy of the stock alpha-blended `explosions/dark_smoke`
  (blend_mode 1; the glow's own materials are additive, 2, where a dark grey barely shows) with
  its constant_emissive 0.3 grey; the rays keep their grey copy. The carrier's own view plays its
  rays only (FOREST_FIRST_PERSON), nothing of the smoke. `check_forest` proves that construction
  against the stock glow, value for value. Seen (renders forest-15): a mid-grey smoke around the
  arms and shoulders, past the silhouette from the front and both sides, faint from behind.
- First person since @10 (the owner, 2026-11-06: the carrier's own view washed white/grey): its
  effect and each leg quarter's are made as effect_artefact_<kind> is, a composite (type 110) of
  the system with no `view` (everyone else's third person, unchanged: its point light and haze)
  and a first-person twin (`view` 0, `<system>_fp`) without `up_light` and `distortion`, the same
  time and end. The exe is unchanged: it plays the effect by name, its resource is the
  composite's.
- Since @13 (the owner, 2026-11-07: "a tiny bit" thicker): the smoke emitters (FOREST_SMOKE:
  `up`, `body`, `left_hend`, `right_hend`) have their Rate (curve and data source) and
  MaxParticles (rounded up) x1.5 (`SMOKE_THICKER`); colours, sizes, the rays and the twin as @12.
  `check_forest` allows exactly that factor.

## Artifact Container (522)

- The owner (2026-11-05): one rank, one pocket becomes an artefact container, as retail 0.31a2
  (513, tier 5, exclusive with Forest Sense there). Here tier 4, no exclusion (my pick).
- Data: modifiers 14 `artefact_slots` +1 and 15 `pocket_slots` -1, both additive (table
  0x927E08) and read by apply_kit_parameter (0x2F7660: 5, 14..17), whose value the lobby gets
  too (item_slots, artifact_slots; the skills are in the kit's modifier block). The six quick
  positions stay six: `ProfileSlot.artifactCount` caps artefacts at 6 - pockets.
- A pocket lost is what the game does when gear with a pocket comes off: the lobby shows the
  first `pocket_slots` pockets, the next places are artefact containers (locked, empty), and
  nothing of the server changes. A player with one pocket has none with it.
- Spending or refunding it goes through srp's skill change (`Account::change_skills`, the kit
  layout of `loadouts::KitLayout`): the item of the pocket the skill takes away comes off every
  kit and preset, as for Deep Pockets.

## Knowledge of Artifacts (512)

- Modifier 39 `artefact_duration` +0.05 a rank: each active's time x max(0, 1 + sum): Spring
  0x31192B (+394h), Lifebone 0x312300, Onyx 0x31291B (+F14h). Rank 5: Spring 10 -> 12.5 s,
  Onyx and Lifebone 3 -> 3.75 s. Patch 54's +1 s for each extra Onyx fired together is added
  after it, unscaled.
- The Sponge (#1280, not on main): its active time is checked when it lands.
- The active duration alone (the owner, 2026-11-07): the leg quarters it showed from 2026-11-06
  are Artifact Sight's now.

## Artifact Sight (513)

- The owner (2026-11-07): row II beside Knowledge of Artifacts, one rank. With it in the current
  kit, the game shows the leg quarters, every player's and its own ("Leg quarters" below).
- Transport: modifier 40 bit 12 (`artefact_power_modifier` 4096), parsed with the other exe
  skills (`Skills::sight`); srp sums it with the skills tree as any other bit and keeps it beside
  Keeper's draw (bits 9..11 only are the draw's). A visual alone: nothing a match computes.
- Icon: 0.69d0 `skills/anomaly/11.png` (rays bursting from an artefact).

## Keeper of the Artifacts (521)

- What it is for (the owner, 2026-11-05): now and then a player starts with an artefact "from
  a different map", one the map's anomalies may not hold. So the kind is drawn over every kind
  Keeper gives, never limited to the map's.
- The rule (the owner, 2026-11-05; when, 2026-11-07): tier 3, one rank. One kind is drawn once
  per match; every player of the match with the skill gets one artefact of that kind, once a
  match:
  - when the match itself begins: the end of the pre-match countdown, while everyone loads (the
    time rule in process, core +18FE4h 3). No extra delay;
  - into its first empty container. Every container held: it waits until one frees, in the same
    life or at a respawn with room. While dead it waits;
  - a late joiner gets nothing before its first death, then the same rule. Late: a player
    taken into the match after it has begun (the countdown's end), who could not choose its
    kit; one taken in the countdown chooses as everyone does, and gets it at the start.
- The table: `crates/artefact-carry/src/keeper.rs` (`KINDS`, `SHARES`, `POOL`), the one table srp
  draws from and the game takes by: Larkspur (dict 54, artefact_lifebone), Onyx (76), Spring (74).
  srp refuses game data whose dictionary names them otherwise.
- The shares (the owner, 2026-11-07: Onyx rare, "they should still happen, but not always";
  2026-11-09: Onyx 4 %, not 10 %):
  Spring 48 %, Larkspur 48 %, Onyx 4 % of the matches (`SHARES`).
- srp (the match server's registry, `survarium::v026g::keeper`): one kind a match, by the shares,
  from the registry's random number (SipHash of the clock under a fresh `RandomState`), drawn
  when the match is made, kept with it and written into every kit of every profile, a late
  joiner's too (`keeper::give`); the kits of a joiner taken once the match is in process (the
  phase the match loop last told the registry, told at once at each change: `LivePhase::Playing`)
  also carry modifier 40 bit 13 (`keeper::LATE`, `keeper::give_late`); a joiner taken in the
  countdown does not.
- Where the artefact comes from: a hidden anomaly on every map (the owner's idea, 2026-11-05),
  content edit `artefact-keeper-1.json` (made by `scripts/artefact-keeper/make_edit.py` from the
  live projects): one more generic_anomaly_core in every project's client_objects (not the
  lobby's), `srp/artefact_keeper/pool`, with
  - a pool of 20 dormant artefacts of each kind (one a place: place p's is the p-th of the kind
    by id), cooked by the map's own cook chain, so its
    resources (the activation effect's at artefact +290h, the models) are there on every map,
    level_08 too;
  - no artefact container and a chance of 0: it never spawns one (spawn_artefact 0x31B6C0
    returns without a container);
  - energy off and one state at threshold 0 with no groups: no zone, nothing drawn, heard or
    hit. The tick executes the selected state with no null check (0x31BCB0), so the state is
    there; the config lookup has no end check, so the object has every key a real anomaly has.
  - Its pool is cooked because `artefacts_enabled` is set (translate_query 0x2E9C70 cooks a pool
    only then); position under the map, layer default.
- Native from there: register_artefacts (0x31B8E0) gives the pool ids in the core world's
  artefact vector after the map's own (85 of its 128 at most: level_03's 25 + 60); the anomaly
  ticks its pool (vfn +74h each: cooldowns, state 5 on a dead holder) and its save (0x31AF20)
  writes every pool artefact in every world; the inventory's save names the one a container
  holds. Late join, resync and replay load it as any found artefact.
- The gift (exe patch 109, `artefact_keeper.rs`): a hook at register_artefacts' entry keeps the
  id range of the anomaly with no container (a load's first registration forgets the last
  map's). Each player's tick (base_player::tick before tick_animations, `battery.carrier_tick`
  0x2DAAD3, after the carrier's, Resonance's and the glow's: every tick in every game and every
  replay of it) decides, for a kit with the Keeper bit only (no other player's tick reads the
  match):
  - nothing before the match is in process (the time rule's state at core +18FE4h: 1 waiting,
    2 the countdown, 3 in process), nor, for a late joiner's kit (bit 13), while its place has
    no death or suicide counted (the statistics, any kit);
  - nothing when its place's artefact of the drawn kind was given (below);
  - else, alive with an empty container: its place's artefact, marked given, with the count and
    cooldown a spawn gives one (0x3134C0), through the game's own take_artefact into the first
    empty container; dead, or every container held: nothing this tick, and the next ticks decide
    again.
- Once a match, with nothing kept outside the world: the mark is the artefact's activation time
  (+2A0h): the constructor writes u32::MAX (0x313059), the gift 0, an activation (only ever after
  a gift) its time (0x313333). No other code writes it, only state 4 reads it, and every world
  saves and loads it in every state (artefact_base::serialize 0x313735, deserialize 0x3137F7), so
  every game, late joiner, resync and replay sees the same: a place's artefact whose time is not
  u32::MAX was given. Not the count: an artefact is not a stack, so the count's bytes are its
  condition, a float, and a death's degrade (0x2FFC40) zeroes a count of 1 (a denormal) when the
  game flushes denormals (0xFE5E0, MXCSR DAZ and FTZ).
- Once given it is an artefact as any found one (the owner, 2026-11-07). A death (0x2D77C0,
  inventory::on_holder_death 0x2FCA50) keeps it in that kit's container: passive off while the
  holder is dead (state 5, artefact_base::tick 0x3132E5), on again at the respawn in that kit
  (setup_from_profile 0x2FCB70's on_taken, state 3); the respawn's unload (0x2DA790,
  unload_to_profile 0x2FD2D0) returns nothing to the pool. Each kit is its own object, so a
  respawn in another kit leaves it in the first kit's container until a spawn in that kit again.
  Held in the hand, patch 78 drops it (state 6, anyone takes it). It goes dormant only when a
  take with every container held replaces the first container's (take_artefact 0x2FC897): still
  given, never given again. Places are never reused, so a late joiner's place has its own.
- A world's load and the kits it does not carry (the live crashes of rev51 and rev52, 2026-11-07,
  matches 0x69f885a, 0x52604c4f and 0x2e769931: every game of the match crashed at the same
  simulated moment). A world carries every registered artefact with its own record (inventory_item's
  load 0x2FFF8D: its holder's inventory by place and kit, 0xFF none, and its slot +254h;
  artefact_base's: its state and the gift's mark), but of a place's inventories only its current
  kit's (game_world_core_client::serialize 0x3218E0), and of that one only the saved container
  range (inventory::deserialize 0x2FE5E7: a saved count of 0 writes no slot). A kit's container
  range is set at the end of its spawn's setup_from_profile (0x2FD29F), so a world kept before a
  kit's first spawn (the countdown; the constructor's empty range 0x2FC515) has none. Every other
  slot keeps what the game's own run put there after the world was kept. So a rollback (or a
  join, a resync, a replay's seek) to a world from before a gift that followed a spawn, at the
  match's start or at a respawn in the same or another kit, left the gift in its container while
  the world has it dormant (state 0, no inventory, mark u32::MAX); the replayed spawn's
  setup_from_profile then called on_taken (0x313470), whose passive link reads [[item+250h]+174h]
  with no null check: Larkspur 0x3121AD, Spring patch 54's `lists` (0x311AD0's hook). rev52's
  fix (emptying the spawned kit's containers before that loop) saw no container at a kit's first
  spawn (its range not yet set) and missed the start's.
- The fix (patch 109, `keeper.world_loaded`, a lohook at game_world_core::deserialize's end
  0x2D2D5D, which every world load passes: move's rollbacks 0x2D2FF0 / 0x2D357A, a joiner's or
  resynced game's 0x2D0B70, a replay's seek): every kit of every place, in the world or not, is
  made to hold the registered artefacts the world says it holds. A slot holding one the world has
  in another inventory, another slot or none is emptied; an empty slot the world has one in is
  filled (the slot's pointer and its reference alone: not the weight +180h or its flag +184h, which
  the world saves and loads for the current kit and a spawn sets for the others (0x2FD2B8); no
  passive linked or unlinked: the kit's next spawn's on_taken does that, as in any game). The world alone decides,
  so every game that loads it holds the same, and the same as a game that did not load it (its
  slots and its artefacts' records agree: set_item, remove_item and the eviction write both); a
  replay, which loads no world, computes what it did: no `next` row. Each change is logged
  ("keeper: a world's load empties / restores place p kit k slot s: artefact id n").
- Costs: every world about 735 bytes more (the 60 dormant artefacts and the core, about +13 % of
  a 5-6 KB world); 60 more artefact objects a match, the three kinds' models loaded on every
  map; the edit list pins each project's client_objects count (a map change in between: run
  `make_edit.py` again). It names its maps one by one, so `release/build-content` refuses a
  build in which a map project (every client_project but the lobby scenes') lacks
  `srp/artefact_keeper/pool`, naming it: a map added later never silently gives Keepers nothing.
- Checks: `client-patches/check_artefact_keeper_026g.py` (Unicorn, the player's tick: nothing in
  the countdown, the gift at the start into the first empty container, waiting while every
  container is held or the player is dead, never a second: a death with denormals flushed then a
  replacement by take_artefact with every container held, a late joiner after its first death,
  the refusals, a new load, a late joiner's ids and container read; world loads, by the game's own
  inventory::deserialize and the load's end: the countdown's world after the start's gift, a world
  from before a respawn into another kit with two places' gifts after it, a world without the
  place, several loads in a row, a world with an artefact the kit's object lost; each fails on
  rev52's build), the patch's `gift` tests,
  `crates/mapdata/tests/artefact_keeper.rs` (the edit list against the table, the anomaly as a
  map object), `survarium::v026g::keeper` and the match-server test (the draw in every kit, a
  joiner's too; bit 13 for a joiner taken in process, none for one taken in the countdown).

## Spring II (531)

- The owner (2026-11-05): reload speed only, +5% while Spring's passive is on, +15% while its
  active is on, like Spring's other effects.
- Two more nodes of list 13 (`reload_speed`, reload time / max(0, 1 + sum)) in each Spring,
  as patch 54's aim-time nodes: the passive's +0.05, linked and unlinked with the passive
  (pickup to death); the active's +0.10 on top of it (+15% in all), linked and unlinked with
  the active. Only when the holder has Spring II.
- Stacking as Spring's other nodes: every carried Spring adds its passive's +5%; every active
  Spring adds its +10%. Two Springs: +10% passive, +30% with both active.
- Resonance and Cascade share these nodes as Spring's other passive and active ones.

## Onyx II (532)

- While one of the player's Onyx is active (state 4), fall damage is none.
- Confirmed by the owner (2026-11-05). Built as the landing itself (my pick, the same effect as
  a list-30 node at -1 with no node to keep): base_player::on_physical_controller_landing
  (0x2D8220) does only the fall damage (modifier 30), and it is skipped for a player with the
  skill and one of its containers' Onyx in state 4 (its enable_active 0x3128C0). No timer: the
  Onyx's end ends it.
- Cascade shares it (the owner, 2026-11-12: the II skills were not passed): a teammate that an
  Onyx's activation reached lands unhurt while that Onyx is active, when its holder has Onyx II
  (the aura's shared actives of the landing player, `shared`, each copy's holder's skills). The
  recipient's own Onyx II is for its own copies only (my pick: the skill goes with the copy, as
  Spring II's nodes do).

## Larkspur II (533)

- The activation (artefact_base's action, 0x313333, never a restore or a visual) sets the
  holder's stamina to its maximum. Repeat presses during a cooldown and key-up do nothing.
- Cascade shares it (the owner, 2026-11-12): the teammates that activation reached (the places
  Medic's share just healed, the copy's `ACTIVATIONS`) get their stamina's maximum too, when the
  holder has Larkspur II.

## Resonance (541) or Cascade (542): one mechanism, two triggers

- A carrier's artefacts share their effect with living teammates near the carrier:
  - Resonance: each carried artefact's passive, within 9 m;
  - Cascade: each active artefact's active, to the teammates within 6 m at its activation, for
    the whole active ("They glow, you shine"; the owner, 2026-11-11: "they should get full
    effect", below).
- The radii (the owner, 2026-11-07, after the rev51 games): 9 m and 6 m, were 8 m and 5 m;
  Larkspur's heal (Medic) is Cascade's 6 m too. One constant each, `RESONANCE_REACH` and
  `CASCADE_REACH` (`artefact_skills.rs`), which patch 94's Medic reads.
- One or the other (the owner, 2026-11-06: an either/or pair in row V, each the other's
  `antiskills`, as the other trees' last rows). A carrier shares by its own skill only; a
  recipient can still take passives from one teammate's Resonance and actives from another's
  Cascade.
- The II skills go with the copy, by its holder's skill (the owner, 2026-11-12): Spring II's
  reload nodes are the Spring's own, shared as its others (Resonance the passive's, Cascade the
  active's); Onyx II and Larkspur II act at the active, so Cascade shares them (their sections
  above) and Resonance has nothing of them to share. Knowledge of Artifacts' longer active is the
  copy's own length, shared whole.
- A recipient: another player of the carrier's team, alive, in the active roster (never the
  insertion flag +33Fh), within the radius of the carrier (positions as the game has them at
  that moment: Resonance's at each use, Cascade's at the copy's activation). Not the carrier,
  never an enemy.
- "Up to 4 effects for the same artifact" (the owner, 2026-11-05: own copies count): a
  recipient has at most 4 copies of one kind in all, its own carried ones and the shared ones;
  shared copies fill what its own leave, max(0, 4 - own), from all carriers together in place
  order. Two own Springs take at most two shared; four own or more, none shared.
- Read where the effect is used (my pick: an aura): Spring's modifiers at `apply_modifier`,
  Onyx's at the hit's protection, Larkspur's regeneration at its tick. Resonance's recipient
  walking out of the 9 m loses the passives there and then.
- The activation (Medic's path, medical-skills.md "Medic", every kind since the owner's rev51
  games): when a copy becomes active, each recipient within 6 m plays the kind's own activation
  glow (Spring blue, Lifebone green, Onyx orange) for the active's length, as its holder does; a
  Lifebone's also heals it at once as its active heals its holder (Larkspur's active is that
  heal). An Onyx fired with others is active 1 s longer for each (patch 54 adds it after every
  activation of the press): the recipients' glow ends at its own 3 s (my pick: a gap of the
  press, not of the share). The shared passives show in the leg quarters.
- The effect is the activation's, as the glow is (the owner, 2026-11-11, after a player's
  report that the effect stopped out of the radius while the glow went on: "they should get
  full effect"): the places that activation reached (the glow's recipients) keep the copy's
  active to its end, wherever the carrier or they go; a teammate out of reach at the activation
  gets neither the glow nor the active, however near it comes. Before, the glow was the
  activation's and the active was read within 6 m at each use (rev51 to rev55).
  - Kept as the copy's own: `vostok::artefact_skills::ACTIVATIONS`, by registry id (with its world
    and item, so another match's copy under that id has none), set at every activation of a
    Spring, Lifebone or Onyx (none reached without the share, so an earlier activation's places
    never carry over), read only while the copy is active (state 4). The aura reads it in place of
    the distance; the room and the copies that count are as before.
  - Saved with the world: artefact_base::serialize writes the places (u32, bits 0..19) after the
    cooldown of an active copy (0x313735, `artefact_skills::on_save`, patch 78's dropped copy's site,
    order 2; none in another state) and deserialize reads them back (0x3137EC, `on_load`), so a
    rollback and a late joiner have the activation's own. `srp-validate` reads them
    (`ArtefactState::Active::reached`, a place past the 20 refused).
  - Checked: `check_artefact_skills_026g.py` `check_cascade_activation` (a Spring's active kept at
    30 m, none for one activated at 7 m and walked to 1 m, an Onyx's active protection at 30 m;
    the record of place 0 alone with a teammate at 30 m and an enemy at 1 m, none without
    Cascade, none for a passive copy; a late joiner loads it, shares, saves the same 4 bytes; a
    load of none shares none); the rest of the check with every active copy activated through
    0x313333; srp-validate's object test. Not played.
  - Onyx II and Larkspur II with it: `check_cascade_seconds` (the landing of a teammate reached by
    an Onyx's activation, the carrier then 30 m away; the stamina of one reached by a Larkspur's;
    none without the carrier's skill, without Cascade, for a passive copy, another kind, a teammate
    out of reach then, an enemy). Not played.
- The cap per phase (my pick): at most 4 copies' passives and 4 copies' actives of a kind,
  each 4 less the recipient's own copies of the kind that count (in their passive or active, at
  most 4: "At most 4 copies of a kind" below).
  Larkspur's active heal is Medic's, which has no cap: a full heal twice is one.

### At most 4 copies of a kind (the owner, 2026-11-07)

- At most 4 copies of one kind count for a player, own copies included, with Resonance or
  without: Artifact Container (522) gives a fifth container, and of 5 Springs carried only 4 give
  Spring's passive.
- The stock game does not cap them: every carried copy's passive adds (artefacts.md, "Every
  artefact"). A Spring links its passive nodes into the holder's kit lists at pickup (0x311AD0)
  and the getters sum every node of a list (0x2D5B20, 0x2D5C50), with no count of who linked
  them; each Onyx puts one protector in each part it protects (0x312810), each called for a hit
  (protect 0x312BF0: n passive Onyx multiply a hit n times; patch 54 limits only the actives to
  the first); each Lifebone links one regeneration modifier into each part it heals (0x3121A0),
  each called at the part's tick (0x32A527). Nothing capped them because at most 4 containers
  could be had before 522 (the owner); the stock code itself has no limit. The
  Rattle's core is a bare artefact_base (the cook's 0x2EE9F1 calls 0x312FD0, 340h bytes): no
  passive to cap.
- Which count (one definition, `vostok::artefact_skills::own` with `Room::counts_own`): the
  player's own copies of the kind in their passive or active (state 3 or 4), in container
  order; the first 4 count. A copy past them gives nothing: no passive, and no active (the
  owner, 2026-11-07: "cap actives at 4 too"). An active Onyx is one of the 4 though it gives no
  passive then (as the sharing's room counts it). The sharing's room is 4 less the own copies
  that count, and the leg quarters count the own copies that count and give their passive, plus
  the shared.
- Actives: a press fires the ready copies of its kind among the 4 that count, so at most 4, in
  container order; a fifth is not fired and stays ready (state 3, no cooldown; my pick, the
  simplest: it is the same count, and a fifth never fills in for a cooling one of the four, so
  no fifth active ever adds: no Spring active nodes, no Lifebone heal, no Onyx +1 s). It counts
  again only when one of the four leaves the containers (or their passive and active). Five
  ready Onyx fire as four, 6 s each. A Spring that stops counting while active (a pickup into a freed earlier
  container) gives its active no more: the getters leave out all its nodes.
- Cascade and Resonance: a recipient takes shared actives (and passives) up to 4 copies of a
  kind, its own that count first: 4 less them, as before (three own and two active near: one
  shared). A carrier shares only its own copies that count for it (its fifth gives nothing to
  anyone).
- Where (patch 109, read where the passive is applied, nothing kept):
  - Spring: `artefact.own_modifier` (apply_modifier 0x2D5B0C, before it sums the index's list)
    and `artefact.own_modifiers` (apply_modifiers' loop 0x2D5C46): for a list of a Spring's
    lists with more than 4 nodes, the kit being a match player's current one with a Spring past
    the 4, the list's sum without that Spring's passive nodes (+340h, +350h, +3D8h), in the list's
    order as the game sums it (value + sum), given at the loop's end: 5 Springs give exactly 4's.
  - Onyx: `artefact.own_onyx` (protect's start 0x312BF0): an Onyx in its passive past the 4
    of its holder (its inventory's) returns at its `ret 0Ch`, the hit as it was.
  - Larkspur (Lifebone): `artefact.own_lifebone` (regeneration_modifier's start 0x312590, ECX
    the Lifebone its subscriber's invoker 0x312640 calls it on): one past the 4 returns at its
    `ret 8`, the part's tick as it was.
  - The press: patch 54's `ready` (artefacts_026g.asm, for the pressed copy and each of its kind
    in the slots) asks patch 109's entry `artefact.fire` (`vostok::artefact_skills::fire`), which
    leaves out a copy past the 4 that count; Onyx's extra time counts only the copies fired.
  - The carriers: `shared` offers each carrier's copies that count (`own`).
- Changes what a match computes: a clause of the `next` row.

### Leg quarters (patch 111, pack artefact-glow@13)

- The owner (2026-11-06): each passive artefact copy that counts for a player lights a quarter
  of its legs in that artefact's colour (how stacked a player is).
- Who sees them (the owner, 2026-11-07: Artifact Sight, 513, row II): a game shows the quarters,
  every player's and its local player's own alike, only while its local player's current kit has
  Artifact Sight (modifier 40 bit 12, `Skills::sight`; before it, Knowledge of Artifacts' modifier
  39 gated them, which is now the active duration alone); a game with no local player (a
  spectator's, a replay) shows them all. Read at each tick and in the
  calculator, so a respawn in another kit switches them on or off. The local player is its place's
  current kit's object (each kit of a place is a player object of its own, and the network
  client's local player can stay an earlier kit's: the owner's test 2026-11-07, Artifact Sight
  gone after a kit switch). Forest Protection's glow and the stock activation glow are everyone's
  as before.
- Another player's effects after its respawn (the owner's test 2026-11-07: B's legs lit on B's
  game, not on A's, after B's respawns with a kit change): a place has one third-person
  presenter, and a game plays a player's effects on the player object that presenter was last
  given. player::initialize gives it only to an object that never had one, so a respawn in a kit
  the place used before another kit's first spawn left it on that other kit's object, out of the
  world: the other games showed none of that player's effects (the quarters, Forest Protection's
  glow, the stock activation glow). The stock game's bug; patch 111 gives the presenter back at
  the tick of the place's current kit's player (and presents its effects again), in every game.
- The count, per kind: the player's own copies that count (the first 4 in their passive or
  active, "At most 4 copies of a kind") giving their passive (`Kind::gives`: Spring and Lifebone
  in 3 and 4, Onyx in 3 only) plus the passives Resonance shares with it (`shared`, the
  sharing's own code and cap), at most 4. A dead
  player counts none. Counted at the player's simulation tick in its world as it is then.
- Quarters (my pick), in the order they light: 1 the left lower leg (LeftLeg, LeftFoot), 2 the
  right lower leg, 3 the left thigh (LeftUpLeg), 4 the right thigh: one copy one shin, two both
  shins, four both legs (filling upward, symmetric at even counts).
- Colours: each kind's stock glow's own (its materials `*_<colour>` and the haze): Spring blue,
  Lifebone (Larkspur) green, Onyx orange.
- Several kinds (my pick, the simplest): each kind lights its own quarters from the first; where
  two kinds meet on a quarter their glows layer (the sprites are additive, so the colours mix,
  as two kinds' stock glows already do). No slot sharing between kinds.
- Effects: `artefact_leg_<kind>_<n>` (n 1..4), view 2: the kind's stock glow with only the
  quarter's parts on, its emitters with none of them left out. Forest Protection's grey glow
  plays beside them, unchanged.
- Since @13 (the owner, 2026-11-07: "it should be like on yourself for everyone"): each quarter
  is one particle effect (type 114) of view 2, everyone's view and the carrier's alike, playing
  its system without the point light (`up_light`) and the haze (`distortion`); no composite and
  no `_fp` twin (@10..@12 gave everyone else the light and the haze).
- Density (the owner, 2026-11-06: @6's quarters looked unlike the activation glow): each kept
  emitter's Rate (curve and data source) and MaxParticles (rounded up, at least 1) are the
  stock's times the share of the stock emitter's body parts it keeps (distortion 1 of 11, rays
  1 of 8, ...). A quarter is then the stock glow's own density on its parts, and four quarters
  together are the stock emitters' leg share (pack artefact-glow@7).
- The point light (the owner, 2026-11-06: yes, put it back): @6 and @7 left the stock
  `up_light` emitter out, and next to the activation glow the quarters read dark (that light is
  most of its look). @8 keeps it in every quarter as the other emitters: its domain cut to the
  quarter's parts, its Rate and MaxParticles times its kept share (19 parts in the stock glow: a
  shin quarter 2/19 of its rate 20, a thigh 1/19), its intensity, colour and range the stock's.
- Played as Forest Protection's glow: at the player's tick each quarter it shows and plays none
  of is played with the player's own calculator, which ends it when the count drops below it
  (or at death); nothing new serialized, no `next` row. The thirteen effects come in one query.
- Timing: the sharing's own effects read the roster where they are used (`apply_modifier`, the
  hit, the regeneration); the quarters read it at the player's tick, the same function on the
  same world, so a quarter and the effect agree but for a carrier crossing the 9 m line within
  that tick.

### How it is built (patch 109)

- One aura, `vostok::artefact_skills::shared`: the recipient's world's roster (core_active_player,
  places 0..19), each other living player of its team (teams 0 and 1) whose skill reaches it
  (Resonance its passives within 9 m, between the feet +43Ch; Cascade its actives to the places
  each copy's activation reached), its containers' Springs, Lifebones and Onyx (the vtable's
  enable_active) in container order. A copy gives its passive in state 3 or 4 (Onyx 3 only: its
  protect's passive branch), its active in 4. Nothing else is kept between two reads (but the
  movement's re-selection, below).
- Its cost: the getters' sites run at every modifier read of every player, so their entries call
  the handler for a Spring list only (`only = { index = "SPRING_LISTS" }`, rust_patches.md): any
  other index costs a compare and a bit test, no machine save. For a Spring list the roster is read
  where it lies, as core_active_player reads it (the places' vector +18458h, the bits +18FD8h, the
  place's kit +0Ch and that kit's player), without its reference (the hook runs no game code), and
  the walk stops when nothing more can change the result (no room, both rooms full; a carrier's
  skills before its distance). Not cached across calls: a carrier moves, dies or changes an
  artefact's state between two getter calls of one tick, and each call must see the world as the
  game has it then.
- Spring: the getters' sites `artefact.shared_modifier` (0x2D5AE6, apply_modifier after it reads
  the kit's own float) and `artefact.shared_modifiers` (0x2D5C18, apply_modifiers' loop) add, for
  an index of Spring's lists (4, 49, 3, 19, 42, 13), the shared copies' node values of that
  index (the passive's +340h, +350h, +3D8h; the active's +360h, +370h, +380h, +3B8h, +3C8h,
  +3E8h) to the float, so the native's node sum and rule follow. The kit is a player's when it
  is the current kit of one of the match's players: the match is the engine world's
  (0x1F1D310) game's embedded game world (+18h)'s match (+4D4Ch), live or replay alike, none
  in the lobby.
- Spring's movement (list 4) is read by movement_speed (0x2DBB50) only, which only
  select_animations (0x2D9690) calls, which tick_animations (0x2D8EE0) runs only while the
  player's +33Eh (m_need_to_select_animations, serialized) is set: a player holding one input
  would keep its old speed. So at each player's tick before tick_animations
  (`battery.carrier_tick` 0x2DAAD3, order 2 after the battery's) its shared movement is summed
  as the getters do, and when it differs from the tick before's, +33Eh is set, as patch 94 does
  for Adrenalin. The one thing kept: each place's sums of its last 64 ticks (a 50 ms tick's own
  slot), so a re-simulated tick compares with the tick before it as its first simulation did;
  a tick before that is not kept (a place's first, or since this game loaded its world) is no
  change. The sums are patch memory, not the world's: a game that loads a world (a late join, a
  resync) forgets them at `world_load.loaded`, so it decides the next ticks as the games that went
  on (the same answer unless the shared movement changed at exactly the load's tick; before, a
  loading game compared with 0 and selected the animations of a player getting shared movement
  where the others did not, a split of the world).
- Onyx: `artefact.shared_protection` (hit_by_type 0x32A020, after apply_hit_modifiers, before
  the part's protectors): each shared passive Onyx, then one shared active when none of the
  recipient's own Onyx is active, as its protect (0x312BF0) with that Onyx's numbers, for a
  hit of its type on a part of its config (+0C90h).
- Larkspur: `medical.regenerate` (0x32A53F, order 1, patch 94's booster order 2): each shared
  Lifebone's record for the part (+354h), as its regeneration callback (0x312590) does to the
  rate, delay and limit.
- Every kind's activation: skill 542's edit-list entry also sets modifier 43 (`armored_trap`)
  bit 20, patch 94's Medic (`share_active`, `medical.artefact` 0x313333): the kind and its
  active's milliseconds from the copy (`artefact_skills::activated`: Spring +394h, Lifebone
  +494h, Onyx +F14h, each its enable_active's), the kind's own active_resource (+290h) played on
  each recipient with Medic's timed clock, a Lifebone's heal; 20 instances of each kind's
  activation effect (the cook, `medical.effect_capacity`).
- Own copies past 4: four more sites, "At most 4 copies of a kind" above.

## Every game the same

- Every rule reads simulated state only (kits' modifiers from the static info, artefact
  states, positions, life, teams); the press is an input. No timers or caches of our own.
- Changes what a match computes: the single `next` row of `simulation_changes.tsv`.

## Texts

- One line a skill, the numbers in the rank lines, as the medical tree. Ten languages.

## Status

- Spec written 2026-11-05. Built and checked in Unicorn, not played, not released: the tree and
  its data skills, Resistance, Onyx II, Larkspur II, Spring II (patch 109 and patch 54's Spring
  nodes), Resonance and Cascade (patch 109's aura, Medic's bit for Larkspur's active), Keeper of
  the Artifacts (patch 109 and the hidden pool).
- At most 4 copies of a kind, own included (2026-11-07): implemented and checked with Unicorn
  (`check_artefact_skills_026g.py`, `check_own_cap`); actives too (2026-11-07, `check_active_cap`);
  not played, not released.
- Forest Protection's glow (2026-11-05): implemented and checked with Unicorn
  (`check_forest_glow_026g.py`); captured in a local game (private/renders/artefact-tree/);
  not played by the owner, not released.
- The owner's rev51 games (2026-11-07): "only green glows for larkspur on teammates". The
  cause: Cascade's activation was Medic's (patch 94, rev45: Larkspur's heal and glow on the
  teammates), still gated to a Lifebone (its enable_active) when patch 109 took it over, so a
  Spring's and an Onyx's activation played nothing on the teammates; their values (the getters,
  the protect) were shared, without a sign. Now one path for every kind (above);
  `check_share_matrix` (every kind with each skill, the carrier and the recipient on kit 1 with
  kit objects of their own, Spring's every list and the stagger's slow, every kind's activation)
  fails on main at a Spring's activation at 3.5 m (nothing played). The radii raised to 9 m and
  6 m. Implemented and checked with Unicorn, not played, not released.

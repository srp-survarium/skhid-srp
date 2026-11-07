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
| 1 | 0 | 501 | Quick Hands | 5 | collection time -5% | data: modifier 7 |
| 1 | 0 | 502 | Resistance | 5 | isolation +5 | exe hook |
| 2 | 5 | 511 | Forest Protection | 4 | bullet and fragment damage -5% in an anomaly | data: modifier 41 |
| 2 | 5 | 512 | Knowledge of Artifacts | 5 | active duration +5% | data: modifier 39 |
| 3 | 10 | 521 | Keeper of the Artifacts | 1 | starts with a random artefact | server + exe |
| 3 | 10 | 522 | Artifact Container | 1 | a pocket becomes an artefact container | data: modifiers 14, 15 |
| 4 | 20 | 531 | Spring II | 1 | Spring: reload +5% passive, +15% active | exe (patch 54's Spring) |
| 4 | 20 | 532 | Onyx II | 1 | no fall damage while Onyx is active | exe (patch 54's Onyx) |
| 4 | 20 | 533 | Larkspur II | 1 | activation restores all energy | exe |
| 5 | 24 | 541 | Resonance | 1 | passives shared, 8 m; or Cascade | exe |
| 5 | 24 | 542 | Cascade | 1 | actives shared, 5 m; or Resonance | exe |

- Layout, four columns (1..4) (the owner, 2026-11-06: Artifact Container beside Keeper, Resonance
  and Cascade an either/or pair in row V, as every tree's last row):

```text
| - Q R - |    Quick Hands, Resistance
| - F K - |    Forest Protection, Knowledge of Artifacts
| - P A - |    Keeper of the Artifacts, Artifact Container
| S O L - |    Spring II, Onyx II, Larkspur II
| - N X - |    Resonance or Cascade (antiskills of each other)
```

- Points: row I holds 10, rows I-II 19, rows I-III 21, rows I-IV 24; a full build 25 (one of the
  row-V pair; 26 ranks in all).
- Thresholds 0/5/10/20/24: row IV leaves one point of rows I-III free, row V needs every point of
  rows I-IV.
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
  6 Onyx II, 7 Larkspur II, 8 Cascade, 9..11 Keeper's draw.
- Modifier 40 evidence (the exact 0.26g exe, 2026-11-05): all 94 calls of `apply_modifier` and
  its wrapper 0x2D5B80 resolve to an index, none 40; `artefact_power_modifier` is referenced
  only by the two name tables (0x9284E8, 0x9C9C88); no other config, edit list or code
  names it. 42 (`artefact_synergy`) is patch 54's stopping list, 43/45 the medical ranks.
- Keeper's draw is not a skill rank: srp draws it once per match and writes it into modifier
  40 of every kit of every player, bots too, with the skill or not (bits 9..11: the kind's draw,
  1 Larkspur, 2 Onyx, 3 Spring; 4 the Sponge once it is on main; 0 none), so every game knows it
  from the static info. The parser takes 1..6; a draw past the kinds gives nothing.
- Bit 12 and above: nothing (a payload with one is refused).

## Quick Hands (501)

- Modifier 7 `artefact_search_time` -0.05 a rank: the container's search time x max(0, 1 + sum)
  (0x31C7EF). Adds with the gloves' 1130..1134 and the Artifact-master's -25%: rank 5 with
  both is x0.25.

## Resistance (502)

- Owner's rule: +5 isolation a rank, flat points.
- Isolation points are armour against the four anomalous hit types (toxic, radiation, fire,
  shock; table 0x92075C), the card's "Isolation" x100. A hit's damage is
  D x (1 - clamp(A - P, 0, 1)) (0x32A07E), A the part's summed armour for its type.
- Flat (the table's words): A + 0.05 a rank for those four types on every body part, at
  0x32A083 (`artefact.isolation`), before the piercing's subtraction and the clamp.
- Modifier 8 `anomaly_damage` is not this: it multiplies an anomalous hit's damage by
  max(0, 1 + M8) before armour (0x332890), a percentage (retail's 0.31+ Resistance).
- The owner (2026-11-05): flat points, the hook above, max +25. The skill card says it as an
  isolation bonus ("Isolation against anomalies: +5 points" a rank).

## Forest Protection (511)

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
- Built (exe patch 111 `forest_glow`, client_patch.md 3.71; pack artefact-glow@5):
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
- Second effect (the owner, 2026-11-06): with it (any rank) in the current kit, the game shows the
  leg quarters ("Leg quarters" below); its description's first line says so.

## Keeper of the Artifacts (521)

- What it is for (the owner, 2026-11-05): now and then a player starts with an artefact "from
  a different map", one the map's anomalies may not hold. So the kind is drawn over every kind
  Keeper gives, never limited to the map's.
- The rule (the owner, 2026-11-05): tier 3, one rank. One kind is drawn once per match; every
  player of the match with the skill and a container gets one artefact of that kind, on its
  first spawn of the match only (not after a death), in its first container.
- The table: `crates/artefact-carry/src/keeper.rs` (`KINDS`, `POOL`), the one table srp draws
  from and the game takes by: Larkspur (dict 54, artefact_lifebone), Onyx (76), Spring (74).
  srp refuses game data whose dictionary names them otherwise.
- srp (the match server's registry, `survarium::v026g::keeper`): one kind a match, uniform, drawn
  when the match is made, kept with it and written into every kit of every profile, a late
  joiner's too.
- Where the artefact comes from: a hidden anomaly on every map (the owner's idea, 2026-11-05),
  content edit `artefact-keeper-1.json` (made by `scripts/artefact-keeper/make_edit.py` from the
  live projects): one more generic_anomaly_core in every project's client_objects (not the
  lobby's), `srp/artefact_keeper/pool`, with
  - a pool of 20 dormant artefacts of each kind (one a place: a gift only in a place's first
    life, and one that comes back is dormant again), cooked by the map's own cook chain, so its
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
  map's); the respawn rule's spawn, after the player is in the world (0x316EAB, every spawn in
  every game and every replay of the tick), gives the player, when its kit has the Keeper bit,
  its place has no death or suicide counted (the statistics, any kit; none before the start)
  and its first container is empty, the lowest-id dormant artefact of the drawn kind of that
  range: the count and cooldown a spawn gives one (0x3134C0), then the game's own take_artefact.
- A kit switch before the start: the old kit's unload returns its gift to the pool (dormant);
  the new kit gets one (my pick: "the kit it spawns with").
- Death: as any artefact taken from an anomaly: state 5 on the body, back to the pool at the
  respawn's unload; held in the hand, patch 78 drops it (state 6, anyone takes it). Never given
  again after a death. Places are never reused, so a late joiner's first spawn gets one.
- Costs: every world about 735 bytes more (the 60 dormant artefacts and the core, about +13 % of
  a 5-6 KB world); 60 more artefact objects a match, the three kinds' models loaded on every
  map; the edit list pins each project's client_objects count (a map change in between: run
  `make_edit.py` again). It names its maps one by one, so `release/build-content` refuses a
  build in which a map project (every client_project but the lobby scenes') lacks
  `srp/artefact_keeper/pool`, naming it: a map added later never silently gives Keepers nothing.
- Checks: `client-patches/check_artefact_keeper_026g.py` (Unicorn: registration, the gift, the
  refusals, a new load, a late joiner's ids and container read), `crates/mapdata/tests/
  artefact_keeper.rs` (the edit list against the table, the anomaly as a map object),
  `survarium::v026g::keeper` and the registry test (the draw in every kit, a late joiner's too).

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

## Larkspur II (533)

- The activation (artefact_base's action, 0x313333, never a restore or a visual) sets the
  holder's stamina to its maximum. Repeat presses during a cooldown and key-up do nothing.

## Resonance (541) or Cascade (542): one mechanism, two triggers

- A carrier's artefacts share their effect with living teammates near the carrier:
  - Resonance: each carried artefact's passive, within 8 m;
  - Cascade: each active artefact's active, within 5 m ("They glow, you shine").
- One or the other (the owner, 2026-11-06: an either/or pair in row V, each the other's
  `antiskills`, as the other trees' last rows). A carrier shares by its own skill only; a
  recipient can still take passives from one teammate's Resonance and actives from another's
  Cascade.
- A recipient: another player of the carrier's team, alive, in the active roster (never the
  insertion flag +33Fh), within the radius of the carrier (positions as the game has them at
  that moment). Not the carrier, never an enemy.
- "Up to 4 effects for the same artifact" (the owner, 2026-11-05: own copies count): a
  recipient has at most 4 copies of one kind in all, its own carried ones and the shared ones;
  shared copies fill what its own leave, max(0, 4 - own), from all carriers together in place
  order. Two own Springs take at most two shared; own copies past 4 stay (native), no shared.
- Read where the effect is used, never stored (my pick: an aura): Spring's modifiers at
  `apply_modifier`, Onyx's at the hit's protection, Larkspur's regeneration at its tick. A
  recipient walking out of the radius loses it; nothing is serialized.
- Larkspur's active is a heal at the activation: Cascade heals the recipients within 5 m then,
  with the glow, by Medic's retained code (medical-skills.md "Medic").
- Glow: Larkspur's recipients glow (Medic's); Spring's and Onyx's shared actives show no glow
  on the recipient (my pick: a cosmetic gap). The shared passives show in the leg quarters.
- The cap per phase (my pick): at most 4 copies' passives and 4 copies' actives of a kind,
  each 4 less the recipient's own carried copies of the kind (in their passive or active).
  Larkspur's active heal is Medic's, which has no cap: a full heal twice is one.

### Leg quarters (patch 111, pack artefact-glow@12)

- The owner (2026-11-06): each passive artefact copy that counts for a player lights a quarter
  of its legs in that artefact's colour (how stacked a player is).
- Who sees them (the owner, 2026-11-06, Knowledge of Artifacts' second effect): a game shows the
  quarters, every player's and its local player's own alike, only while its local player's
  current kit has Knowledge of Artifacts (modifier 39, the skill's own transport, above 0: the
  native getter's factor above 1; it is a data skill, not in modifier 40's packed bits); a game
  with no local player (a spectator's, a replay) shows them all. Read at each tick and in the
  calculator, so a respawn in another kit switches them on or off. Forest Protection's glow and
  the stock activation glow are everyone's as before.
- The count, per kind: the player's own carried copies giving their passive (`Kind::gives`:
  Spring and Lifebone in 3 and 4, Onyx in 3 only) plus the passives Resonance shares with it
  (`shared`, the sharing's own code and cap), at most 4 (own copies past 4 count as 4). A dead
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
  same world, so a quarter and the effect agree but for a carrier crossing the 8 m line within
  that tick.

### How it is built (patch 109)

- One aura, `vostok::artefact_skills::shared`: the recipient's world's roster (core_active_player,
  places 0..19), each other living player of its team (teams 0 and 1) whose skill reaches it
  (Resonance its passives within 8 m, Cascade its actives within 5 m, between the feet +43Ch), its
  containers' Springs, Lifebones and Onyx (the vtable's enable_active) in container order. A
  copy gives its passive in state 3 or 4 (Onyx 3 only: its protect's passive branch), its
  active in 4. Nothing is kept between two reads (but the movement's re-selection, below).
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
  a tick before that is not kept (a place's first) compares with 0, nothing shared.
- Onyx: `artefact.shared_protection` (hit_by_type 0x32A020, after apply_hit_modifiers, before
  the part's protectors): each shared passive Onyx, then one shared active when none of the
  recipient's own Onyx is active, as its protect (0x312BF0) with that Onyx's numbers, for a
  hit of its type on a part of its config (+0C90h).
- Larkspur: `medical.regenerate` (0x32A53F, order 1, patch 94's booster order 2): each shared
  Lifebone's record for the part (+354h), as its regeneration callback (0x312590) does to the
  rate, delay and limit.
- Larkspur's active: skill 542's edit-list entry also sets modifier 43 (`armored_trap`) bit 20,
  patch 94's retained Medic.

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
- Forest Protection's glow (2026-11-05): implemented and checked with Unicorn
  (`check_forest_glow_026g.py`); captured in a local game (private/renders/artefact-tree/);
  not played by the owner, not released.

# Artefact skill tree

The owner's table (2026-11-05) is the spec; this note is how it is built. "My pick" marks a
reading chosen where the table leaves it open (the owner may overrule it). Addresses are 0.26g
preferred-image VAs; `apply_modifier` is the kit-modifier getter 0x2D5AE0.

## The tree

- The fifth skills tab, "Knowledge of the Forest" (`st_anomaly_skills_label`, the stock text in
  every language), ids 501..541 (branch 5). Eleven skills: the owner's ten and Artifact Container
  (the owner, 2026-11-05, retail 0.31a2's 513). My pick: the fourth tab ("Technical skills") stays
  for the equipment skills.
- Source: `scripts/content-edit/artefact-skills-1.json` (`release/content.tsv`), names and
  descriptions in `client-patches/localization/*.tsv` (`st_srp_artefact_<id>_name` /
  `_description`), icons 0.69d0 `skills/anomaly/*.png` (`scripts/artefact-skills/icons-069.tsv`).
- A row needs its points spent in the rows above it, in this branch (the stock rule, as the
  medical tree).

| Tier | Needs | Id | Skill | Ranks | Per rank | Mechanism |
|---|---:|---:|---|---:|---|---|
| 1 | 0 | 501 | Concentration | 5 | collection time -5% | data: modifier 7 |
| 1 | 0 | 502 | Resistance | 5 | isolation +5 | exe hook |
| 2 | 5 | 511 | Forest Protection | 4 | bullet and fragment damage -5% in an anomaly | data: modifier 41 |
| 2 | 5 | 512 | Knowledge of Artifacts | 5 | active duration +5% | data: modifier 39 |
| 3 | 10 | 521 | Keeper of the Artifacts | 1 | starts with a random artefact | server + exe |
| 3 | 10 | 522 | Resonance | 1 | passives shared, 8 m | exe |
| 4 | 20 | 531 | Spring II | 1 | Spring: reload +5% passive, +15% active | exe (patch 54's Spring) |
| 4 | 20 | 532 | Onyx II | 1 | no fall damage while Onyx is active | exe (patch 54's Onyx) |
| 4 | 20 | 533 | Larkspur II | 1 | activation restores all energy | exe |
| 4 | 20 | 534 | Artifact Container | 1 | a pocket becomes an artefact container | data: modifiers 14, 15 |
| 5 | 25 | 541 | Resonance Cascade | 1 | actives shared, 5 m | exe |

- Layout, four columns (1..4):

```text
| - C R - |    Concentration, Resistance
| - F K - |    Forest Protection, Knowledge of Artifacts
| - P N - |    Keeper of the Artifacts, Resonance
| S O L A |    Spring II, Onyx II, Larkspur II, Artifact Container
| - - X - |    Resonance Cascade
```

- Points: tier 1 holds 10, tiers 1-2 19, tiers 1-3 21, tiers 1-4 25; the whole tree 26.
- Thresholds 0/5/10/20/25 as the table (the owner, 2026-11-05, with Forest Protection's fourth
  rank and Artifact Container): tier 4 leaves one point of tiers 1-3 free, tier 5 needs every
  point of tiers 1-4.
- The skills window's last tier is the branch's highest `branch_points` (was a constant 20, the
  stock and medical last tier), so the bar fills when tier 5 is taken.

## Transport

- Data skills: their native modifier, summed into the kit's 68 floats by srp's `SkillsTree`
  (`skills.rs`), as every stock skill.
- Exe skills: one packed integer in modifier 40 (`artefact_power`), parsed once (`Skills::parse`
  in the patch); invalid (fractional, negative, too large, a rank past its levels) is no
  artefact skill at all. Bits: 0..2 Resistance (0..5), 3 Keeper, 4 Resonance, 5 Spring II,
  6 Onyx II, 7 Larkspur II, 8 Resonance Cascade, 9..11 Keeper's draw.
- Modifier 40 evidence (the exact 0.26g exe, 2026-11-05): all 94 calls of `apply_modifier` and
  its wrapper 0x2D5B80 resolve to an index, none 40; `artefact_power_modifier` is referenced
  only by the two name tables (0x9284E8, 0x9C9C88); no config or edit list names it; no open
  PR uses it. 42 (`artefact_synergy`) is patch 54's stopping list, 43/45 the medical ranks.
- Keeper's draw is not a skill rank: srp rolls it once per match and writes it into modifier
  40 of every kit of every player, bots too, with the skill or not (bits 9..11: the draw 1..6,
  0 none), so every game knows it at the map's load, before any Keeper player exists. The client makes it a kind
  of those the map has: present[(draw - 1) % n], uniform for 1, 2 or 3 kinds (Keeper's PR).
- Bit 12 and above: nothing (a payload with one is refused).

## Concentration (501)

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
  simulation change, no `next` row. Not built yet (a pack of the grey leg effect and its
  trigger, a small PR after Resistance).

## Artifact Container (534)

- The owner (2026-11-05): one rank, one pocket becomes an artefact container, as retail 0.31a2
  (513, tier 5, exclusive with Forest Sense there). Here tier 4, no exclusion (my pick).
- Data: modifiers 14 `artefact_slots` +1 and 15 `pocket_slots` -1, both additive (table
  0x927E08) and read by apply_kit_parameter (0x2F7660: 5, 14..17), whose value the lobby gets
  too (item_slots, artifact_slots; the skills are in the kit's modifier block). The six quick
  positions stay six: `ProfileSlot.artifactCount` caps artefacts at 6 - pockets.
- A pocket lost is what the game does when gear with a pocket comes off: the lobby shows the
  first `pocket_slots` pockets, the next places are artefact containers (locked, empty), and
  nothing of the server changes. A player with one pocket has none with it.
- To check in the test round: what a kit's item in the lost pocket does in a match (a booster
  carried there still counting would be the same as taking a jacket off).

## Knowledge of Artifacts (512)

- Modifier 39 `artefact_duration` +0.05 a rank: each active's time x max(0, 1 + sum): Spring
  0x31192B (+394h), Lifebone 0x312300, Onyx 0x31291B (+F14h). Rank 5: Spring 10 -> 12.5 s,
  Onyx and Lifebone 3 -> 3.75 s. Patch 54's +1 s for each extra Onyx fired together is added
  after it, unscaled.
- The Sponge (#1280, not on main): its active time is checked when it lands.

## Keeper of the Artifacts (521)

- At a player's first spawn of the match, the kit it spawns with gets one artefact in its
  first container slot, if it has a container (modifier 14 > 0). Lost at death like any other.
- The kind: random, rolled once per match by srp at the match's start: every player of the
  match with the skill gets the same kind, in every kit (the owner, 2026-11-05: "all kits with
  containers receive the same type" is per match); written into the static info's kits
  (Transport), so every game and every replay gives the same.
- "First spawn" = the place has no death yet (the match statistics' deaths, already
  serialized), so no new state (my pick: start of the match only, not every life).
- Late joiners: their first spawn, as anyone's.

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
- Confirmed by the owner (2026-11-05). A node of list 30 (`fall_damage`, damage
  x max(0, 1 + sum), 0x2D82C8 / 0x2D83EE) at -1 in each Onyx, linked and unlinked with its
  active, as patch 54 links Spring's active nodes; only when the holder has Onyx II. Several
  active Onyx: the sum is below -1, still none (the getter's max(0, ...)).

## Larkspur II (533)

- The activation (artefact_base's action, 0x313333, never a restore or a visual) sets the
  holder's stamina to its maximum. Repeat presses during a cooldown and key-up do nothing.

## Resonance (522) and Resonance Cascade (541): one mechanism, two triggers

- A carrier's artefacts share their effect with living teammates near the carrier:
  - Resonance: each carried artefact's passive, within 8 m;
  - Cascade: each active artefact's active, within 5 m ("They glow, you shine").
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
  on the recipient (my pick: a cosmetic gap).

## Every game the same

- Every rule reads simulated state only (kits' modifiers from the static info, artefact
  states, positions, life, teams); the press is an input. No timers or caches of our own.
- Changes what a match computes: the single `next` row of `simulation_changes.tsv`.

## Texts

- One line a skill, the numbers in the rank lines, as the medical tree. Ten languages.

## Status

- Spec written 2026-11-05. Nothing played or released.

# Player bots: skills and carried weight (0.26g)

The server's own bots ("Player vs Bots", `player_bots.md`) are given skills
by the tier of their kits, so that the heavier equipment of the higher tiers
does not leave them overloaded. Code: `crates/survarium/src/v026g/bot_skills.rs`
(the table `BOT_SKILL_TIERS`), used by the lobby's `play::server_bot`. Tags
as elsewhere: PROVEN, LIKELY, GUESS. Players by label only.

## 1. How carrying works in 0.26g

`gameplay/players/default.player`, `player.stamina_params` (PROVEN, data):
`min_carried_weight` 20, `max_carried_weight` 30,
`max_carried_weight_movement_speed_modifier` -0.2,
`max_carried_weight_spending_speed_modifier` 1.5
(`stamina_base_parameters::load` 0x329120 reads them to +18h, +1Ch, +20h,
+24h).

The client (PROVEN in the exe, image base 0x10000; how it plays is not seen
live):
- **Capacity** = 30 kg + modifier 5 `max_carried_weight`. The modifier getter
  (`apply_modifier` 0x2D5AE0) adds it, not multiplies: index 5 is in the
  additive list at 0x927E08. Its sources are the kit's modifier block (the
  skills) and the items' nodes (modifications).
- **The cost of the weight** (0x329B20, called from 0x2DAAB5 when the weight
  changes): `t = clamp((w - 20) / (capacity - 20), 0, 1)`.
  - **Stamina:** a node of `1.5 × t` on modifier 19 `stamina_spending_speed`,
    which multiplies. At the capacity, stamina is spent 2.5 times as fast.
  - **Speed:** a node of `m × t` on modifier 4 `movement_speed`, which also
    multiplies. `m` is `-0.2 × max(0, 1 + overweight_penalty)`, or 0 when
    modifier 32 `max_carried_weight_ultimate` is set. At the capacity that
    is -20 % speed.
  - So raising the capacity both moves the limit and flattens the slope from
    20 kg.
- **Overburdened** (`player_stamina::is_overburdened` 0x329C60): when the
  weight is at or over the capacity, unless modifier 31 `can_sprint` is set.
  The sprint decision (0x2DBF89) does not sprint an overburdened player.
- The lobby does not let a player press Play with a kit over the capacity
  (its own check; `loadouts.rs`, `rules.rs`). The server's bots are made by
  the server, so nothing but this table keeps them under it.

The skills that touch it (`skills_tree.options`, English texts from
`localization`):

| skill | name | per level | levels | row needs | effect at the most |
|---|---|---|---|---|---|
| 209 | Athlete | `max_carried_weight` +2 kg | 5 | 5 (row 2) | capacity 40 kg |
| 202 | Packrat | `overweight_penalty` -0.05 | 5 | 10 (row 3) | the speed cost × 0.75 |
| 213 | Sprint | `can_sprint` 1 | 1 | 20 (row 5) | "You can sprint while overloaded." |
| 214 | Heavyweight | `max_carried_weight_ultimate` 1 | 1 | 20 (row 5) | "Portable weight no longer affects movement speed, but you can't sprint." |
| 201 | (stamina) | `stamina_spending_speed_skill` -0.02 | 5 | 0 (row 1) | opens row 2 |

213 and 214 exclude each other. 209 needs five points in row 1 of the
branch (201 and 204), and 202 needs ten in rows 1 and 2. No item's own
parameters add to `max_carried_weight`. The modifications 1120 to 1123 add
+1 to +4 kg, and the defect 4080 takes 3 kg (`modifications.options`). No
bot item has them.

## 2. The bots' skills before this

A bot is no account. `server_bot` builds its match profile
(`MatchProfile::from_loadouts`) with a modifier block of zeros: no skills, a
capacity of 30 kg. The block is what a match knows of anyone's skills. The
match profile has no skills list (`skills.rs`, `static_info.md` §3.1), and
every client simulates every player from it. So a bot's skills go the same
way as a player's: summed into each kit's 68 modifiers
(`SkillsTree::modifiers`).

The match server's bots need nothing of it: their body model takes the
speeds from the worlds (`bots::body`, `Kinematics::level_of`).

## 3. What the best players take

From a copy of the 0.26g server's database (2026-10-23 04:35 UTC), 407
accounts, 220 with skills. Ranked by the server's hidden balance rating
(`g26_balance`) among the 106 with at least 30 matches. The top three of
`player_bots_research.md` (I, B, E) are among the top 15 by it.

Top 15 (A, B, E, I, J, N, Y and eight unlabelled), 24.4 points spent on
average (10 to 44):

| skill | take it | at the most |
|---|---|---|
| 209 Athlete (+2 kg) | **13 / 15** | 12 at 5 |
| 101 aim time | 12 / 15 | 11 at 5 |
| 208 movement speed | 11 / 15 | 11 at 5 |
| 201 stamina spent | 10 / 15 | 8 at 5 |
| 202 Packrat | 9 / 15 | 7 at 5 |
| 102 moving while aiming | 7 / 15 | 4 at 5 |
| 204 stamina regeneration | 6 / 15 | 4 at 5 |
| 103 chamber a round | 5 / 15 | 3 at 3 |
| 108 reload | 4 / 15 | |
| 213 Sprint | 2 / 15 (A, I) | |
| 214 Heavyweight | 0 | |

All 38 accounts with 20 points or more: 209 is taken by 25 (18 at 5), 202 by
20, 213 by 3, and 214 by none. The three best (I, B, E) all have 209 at 5.

What they carry (the same copy, every kit, weights as the lobby counts them,
ESTIMATED: stacks other than ammo are counted by unit). By the highest armour
or weapon level of the kit:

| levels | kits | median | 90th percentile | heaviest |
|---|---|---|---|---|
| 0–5 | 435 | 5.8 kg | 17.9 kg | 27.4 kg |
| 9–10 | 61 | 19.6 kg | 25.7 kg | 28.1 kg |
| 13–15 | 105 | 25.8 kg | 37.6 kg | 46.3 kg |

## 4. What the bots get

One table by the highest item level a bot's kits wear (`gear_level`). A kit
above the last tier gets the last. Every tier has the carry path at its most
(201 at 5, then 209 at 5: 40 kg), as the owner asked ("treat all bots as
high level and level up max equipment mass for them"), and the rest follows
the top players' order:

| tier: kits up to item level | level (points) | skills |
|---|---|---|
| 5 (today's roles) | 15 | 101:5, 201:5, 209:5 |
| 10 (the 9–10 kits) | 25 | + 202:5, 208:5 |
| 15 (the 13–15 kits) | 36 | + 102:5, 204:5, 213:1 |

Every set passes the skills window's rules (`SkillsTree::validate`) at its
level. 214 is left out: no player takes it, and it takes the sprint away.
213 at the top is the safety net for a kit that is over 40 kg: the bot still
sprints, and the cost is at most -15 % speed (with 202, and +5 % from 208)
and stamina × 2.5.

Mass against capacity (the server's game data, `Loadout::weight`; the tests
in `bot_skills.rs`):

| kit | mass | capacity | `t` (cost) before → now |
|---|---|---|---|
| flanker (Uzi), second kit | 21.0, 19.2 | 40 | 0.10 → 0.05 |
| support (AK-74U), second kit | 22.8, 24.6 | 40 | 0.46 → 0.23 (-9 % → -5 % speed) |
| rusher (TOZ-34), second kit | 21.8, 20.9 | 40 | 0.18 → 0.09 |
| camper (Mosin), second kit | 20.8, 22.6 | 40 | 0.26 → 0.13 |
| 9–10: Renaissance level 9 set, AK-74M, 600 rounds | 34.7 | 40 | overburdened → 0.74 (-6 % speed with 202 and 208) |
| 13–15: Renaissance level 14 set, AK-12, 600 rounds | 37.0 | 40 | overburdened → 0.85 (-8 % speed) |
| the whole Renaissance level 15 set, AK-12, 600 rounds | 50.1 | 40 | over any capacity; sprints by 213 |

Today's kits were already under 30 kg, but over 20, so they paid a little
speed and stamina. The tier kits being added must stay under 40 kg, the most
any player can carry. The heaviest pieces of level 15 (the 20 kg Renaissance
torso, the 35 kg Duty exoskeleton) cannot be worn whole with a rifle and its
ammo. The test `no_bot_kit_is_over_its_capacity` walks every role's two
kits. A tier kit added to `bot_kits` is checked against its tier the same
way.

## 5. Not verified

- In a live client: that a bot's kit with these modifiers moves and sprints
  as computed. The formulas are read from the exe, not measured.
- That the weight the match counts (`inventory::update_weight` 0x2FC6E0) is
  the lobby's sum (`Loadout::weight`). LIKELY.
- What 214 does to the sprint: the text says it stops it. Where the exe
  reads modifier 32 besides 0x329B20 was not traced.

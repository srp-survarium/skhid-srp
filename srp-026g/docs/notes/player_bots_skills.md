# Player bots: skills and carried weight (0.26g)

The server's own bots ("Player vs Bots", `player_bots.md`) are given skills
by the tier of their kits, so that the heavier equipment of the higher tiers
does not leave them overloaded, and with the builds the best players have at that gear level. Code: `crates/survarium/src/v026g/bot_skills.rs`
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

How the match server's bots follow each skill: §5.

## 3. What the best players take, by gear level

From a copy of the 0.26g server's database (2026-10-23 04:35 UTC): 407
accounts, 220 of them with skills. Each account's character level comes
from its experience (`Account::level`). Its gear level is the highest
`kit_level` of its kits, the matchmaking's measure
(`session::v026g::matchmaking`). Read with the server's own code. In each
band, the ten best by the hidden balance rating (`g26_balance`) with 20
matches or more. The top three of `player_bots_research.md` (I, B, E) are
in the 11–15 band.

Per skill: how many of the ten take it, and the mean of their points.

| skill | gear 1–5 (12 ranked) | gear 6–10 (23 ranked) | gear 11–15 (94 ranked) |
|---|---|---|---|
| levels of the ten (median, range) | 11.5 (7–22) | 11.5 (9–21) | **29** (16–44) |
| 101 aim time | **7**, 2.9 | **8**, 3.7 | 8, 3.7 |
| 201 stamina spent | 4, 1.1 | **9**, 3.3 | 7, 3.5 |
| 204 stamina regeneration | 4, 1.7 | 4, 1.3 | 4, 1.7 |
| 209 Athlete (+2 kg) | 3, 1.5 | 3, 0.9 | **10**, 5.0 |
| 208 movement speed | 4, 1.3 | 3, 0.6 | **9**, 4.5 |
| 202 Packrat | 2, 0.3 | 0 | 8, 3.6 |
| 102 moving while aiming | 4, 0.9 | 3, 0.4 | 6, 2.6 |
| 103 chamber a round | 2, 0.5 | 5, 1.1 | 2, 0.5 |
| 108 reload | 1, 0.1 | 0 | 4, 0.7 |
| 213 Sprint (overloaded) | 0 | 0 | 2, 0.2 |
| 214 Heavyweight | 0 | 0 | 0 |

Of all 220 accounts with skills, 3 have 213, 2 have 113 and 2 have 111.
None has 214.

So below gear 11 the good players mostly put points into aim time (101)
and stamina (201, 204). From gear 11 on, all ten have the carry skill 209
at five, and nine have 208 at five. Their builds there converge on
101, 201, 202, 208 and 209 at five: player B's whole build, and the core
of A's, Y's and Z's.

What they carry (the same copy, every kit, weights as the lobby counts them,
ESTIMATED: stacks other than ammo are counted by unit). By the highest armour
or weapon level of the kit:

| levels | kits | median | 90th percentile | heaviest |
|---|---|---|---|---|
| 0–5 | 435 | 5.8 kg | 17.9 kg | 27.4 kg |
| 9–10 | 61 | 19.6 kg | 25.7 kg | 28.1 kg |
| 13–15 | 105 | 25.8 kg | 37.6 kg | 46.3 kg |

## 4. What the bots get

`BOT_SKILL_TIERS` has one row per band. A bot's band is the gear level of
its kits (`bot_skills::gear_level`, the same measure). Each row is the
band's skills in the order of their mean points, each taken to five.
Every row keeps the carry path at five: a row-1 skill at five, then 209
at five, which gives 40 kg. The owner asked for it ("level up max
equipment mass for them"), and the 11–15 players all have it.

The carry path costs ten points that most players below gear 11 do not
spend. So the two lower rows sit a few levels above their band's median
(still within its range), so that aim time is not crowded out.

| band | level | skills | from the data |
|---|---|---|---|
| gear 1–5 (today's roles) | 15 | 101:5, 204:5, 209:5 | 101 first; 204 is that band's row-1 pick (1.7 against 201's 1.1) |
| gear 6–10 | 20 | + 201:5 | 201 is taken by 9 of 10 |
| gear 11–15 | 30 | + 202:5, 208:5 | the band's median level 29; B's build with 204 for 102 |

Every row passes the skills window's rules (`SkillsTree::validate`) at its
level. Each row keeps all of the row below it. Left out:
- **102:** it gives no speed (§5). Given only by the owner's kit (below).
- **103:** the bots would not use it (§5).
- **108:** the bots count a reload by its clip length (§5), and few of the
  ten take it. Given only by the owner's kit (below).
- **213:** two of ten take it.
- **214:** nobody takes it, and it takes the sprint away.

**The owner's kit (2026-10-26, on unless `SRP_BOT_KIT` is `0`/`off`/`false`, since 2026-10-30).** Every row also has
`bot_skills::AMMO_POUCH`: 102:5, 104:2, 108:3 and 109:1, 11 points (levels 26, 31 and 41),
for 109, the extra ammo bag (one more magazine of rounds in the kit). 109 needs 15 points in
the aim branch's first three rows; 101 has 5. The ten more are the least that is neither aim,
damage nor protection: the rest of those rows are 103 (the bolt's cocking speed, so the camper's
rate of fire), 105 (breath hold), 106 and 107 (dispersion). 102 gives +5 % movement speed while
aiming (no effect found, §5), 104 +15 % weapon swap speed (the bots carry one weapon), 108 +12 %
reload speed: the client reloads sooner than the bots' clip-length timing (`Gun::reload_ms`), so
they wait up to 0.2 to 0.6 s too long until the gun takes the factor from the profile's
modifiers. The test `every_tier_is_a_set_a_player_could_have` checks that the pouch changes no
other modifier.

Mass against capacity (the server's game data, `Loadout::weight`; the tests
in `bot_skills.rs`). The speed level is `movement_level`: 1 + 208's 0.05 −
0.2 × (1 + `overweight_penalty`) × t.

| kit | mass | capacity | speed level now (at 30 kg capacity, no skills, before) |
|---|---|---|---|
| flanker (Uzi), second kit | 21.0, 19.2 | 40 | 0.99, 1.00 (0.98, 1.00) |
| support (AK-74U), second kit | 22.8, 24.6 | 40 | 0.97, 0.95 (0.94, 0.91) |
| rusher (TOZ-34), second kit | 21.8, 20.9 | 40 | 0.98, 0.99 (0.96, 0.98) |
| camper (Mosin), second kit | 20.8, 22.6 | 40 | 0.99, 0.97 (0.98, 0.95) |
| gear 6–10: Renaissance level 9 set, AK-74M, 600 rounds | 34.7 | 40 | 0.85 (overburdened: no sprint, 0.80) |
| gear 11–15: Renaissance level 14 set, AK-12, 600 rounds | 37.0 | 40 | 0.92 (overburdened: no sprint, 0.80) |
| the whole Renaissance level 15 set, AK-12, 600 rounds | 50.1 | 40 | over any capacity: no sprint, 0.90 |

The tier kits being added must stay under 40 kg, the most any player can
carry. A 6–10 kit should stay near what those players carry (20–26 kg):
that row has no 202 or 208, so 35 kg costs it 15 % of its speed. The
heaviest pieces of level 15 (the 20 kg Renaissance torso, the 35 kg Duty
exoskeleton) cannot be worn whole with a rifle and its ammo. The test
`no_bot_kit_is_over_its_capacity` walks every role's two kits. A tier kit
added to `bot_kits` is checked against its row the same way.

## 5. Each skill against the bots' code

What every skill given or weighed does in the client, and whether the
match server's bots (`servers/match-server/src/v026g/bots/`) follow it.

- **101 aim time −10 % (given). Helps.**
  - The client: aiming in and out (0x2E1B50 / 0x2E1BF0) start a
    transition whose length is `apply_modifier(49 aim_time, base +5F8h)`.
    So it shortens the transition into and out of the sights. PROVEN
    (read).
  - The bots: no timer stands for that transition. The reflex holds AIM
    toward a target (`reflex.rs`, the `ads_near_m` rule) and fires by its
    own aim error and line check (`clear_shot`). PROVEN (read). A shorter
    transition only brings the sights' accuracy sooner. LIKELY a gain.
- **201 stamina spent −10 %, 204 stamina regeneration +10 % (given).
  Helps.**
  - The bots model no stamina. They press SPRINT where the way and the
    pace allow (`mod.rs`: sprint is dropped only for crouch, fire, aim,
    reload or a pace below Run). The client decides whether the sprint
    happens. PROVEN (read).
  - `kin.rs` predicts a held sprint at sprint speed, so a sprint the
    client stops for lack of stamina is mispredicted until the next world.
    More stamina means fewer such stops. LIKELY a gain.
- **202 overweight penalty −25 %, 208 movement speed +5 %, 209 +10 kg
  (given). Followed, and fixed.**
  - `body.rs` moves a body at its life's speed level, read from a world
    (`Kinematics::level_of`; `motor.rs` plans from the pose's speed, which
    comes from that). PROVEN (read).
  - Before this change, a new life moved at the nominal 1 until a world
    showed it moving. That is up to ~5 s, and a 6–10 kit at 0.85 lands
    ~2.6 m off. Also, `level_of` drops readings outside 0.85–1.15, so a
    slower body was never corrected.
  - **Fix:** the match server now gives every body the level its profile
    implies (`bot_skills::profile_movement_level`: the first kit's skills,
    its weight against its capacity, its items' modifications). That level
    is used until a world shows the life's own (`BodyTrack::set_prior_level`,
    `Tracker::set_prior_levels`, `Bots::set_speed_levels`; set in
    `game/driven.rs`).
  - It also applies to humans, who take 208 and 209 too. A weapon's own
    ±0.05 is not counted: it is not in `item_params`.
- **102 moving while aiming (weighed, left out). No effect found.**
  - `base_player::movement_speed` (0x2DBB50): when aiming, it applies
    modifiers 4 and 20 together (0x2D5BC0) and keeps the lower of that and
    modifier 4 alone (0x2DBC47), then the higher of that and the minimal
    speed.
  - A positive `movement_speed_when_aiming` is therefore never the lower:
    102 gives no speed. LIKELY (one function read, not measured). It would
    not break the bots either.
- **103 chamber a round (weighed, left out). Not used.**
  - The reflex lets a bolt action fire again only after `Gun::cycle_ms`,
    whose chambering is the clip's fixed length (`guns.rs`: the Mosin's
    65 frames). PROVEN (read).
  - A faster chambering would leave the bot waiting the old time.
    Following it needs the modifier's effect on the chamber state, which
    is not traced (`0x30E58D` reads modifier 21).
- **108 reload (not given).** `guns.rs` times a reload by its clip
  (`Gun::reload_ms`), and the reflex counts rounds from it. A faster
  reload would be mispredicted.
- **213 / 214 (not given).**
  - 213 lets an overburdened player sprint, which `kin.rs` would follow:
    it predicts the sprint from the keys.
  - 214 stops the sprint (the game's text). The bots would press SPRINT
    and be mispredicted, since `kin.rs` knows no such rule.
- **Pain and health skills (110, 211; not given).** Nothing in the bots
  reads them.

## 6. Not verified

- In a live client: that a bot's kit with these modifiers moves and sprints
  as computed. The formulas are read from the exe, not measured.
- That the weight the match counts (`inventory::update_weight` 0x2FC6E0) is
  the lobby's sum (`Loadout::weight`). LIKELY.
- What 214 does to the sprint: the text says it stops it. Where the exe
  reads modifier 32 besides 0x329B20 was not traced.
- 102's reading (§5): one function, LIKELY.

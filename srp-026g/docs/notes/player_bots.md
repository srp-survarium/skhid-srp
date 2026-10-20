# Player bots driven by the server (0.26g)

Status 2026-10-14: **planned**. Stage 0 (a test bot that sends inputs,
`srp-bot --drive`) is built and not yet run against a real client. Tags:
PROVEN (code, bytes or data read), LIKELY (strong inference or a few real
samples), GUESS.

## 1. Why no client change is needed

In lockstep every client simulates every player from the inputs the match
server relays (0xEB). A relayed input is applied unchecked, as a real
player's is (0x1FAB50 → 0x2D86E0; `match.md` §20). PROVEN. So a place whose
inputs the server makes up walks, turns and fires in every client's world.

The clients' respawn rule spawns an entered player with no input, in the
lowest loaded kit, and again 8 s after a death (`match.md` §19). PROVEN. A
bot never sends a kit choice (0xDE).

## 2. An input

0xDC from a client, 0xEB to the others: tick (`time / 50 mod 256`), place,
two IEEE half floats (rotation), u32 `actions_mask` (`match_msgs.rs`).

| bit | mask | action |
|---|---|---|
| 0-3 | 0x1 / 0x2 / 0x4 / 0x8 | forward / back / strafe left / strafe right |
| 4 | 0x10 | jump |
| 5 | 0x20 | fire |
| 6 | 0x40 | reload |
| 7 | 0x80 | aim (or aim toggle) |
| 8 | 0x100 | crouch (or toggle) |
| 9 | 0x200 | sprint (or toggle) |
| 10 / 11 | 0x400 / 0x800 | next fire mode / next ammo (key down) |
| 12 / 13 | 0x1000 / 0x2000 | weapon 1 (slot 7) / weapon 2 (slot 10) |
| 14-19 | 0x4000 .. 0x80000 | quick use 1..6 |
| 20 | 0x100000 | hold breath |
| 21 | 0x200000 | use: acts on the key going down only, if no use is running |
| 22 | 0x400000 | drop: acts only while carrying a battery |
| 23 | 0x800000 | throw grenade: held until the throw starts |
| 25 / 26 | 0x2000000 / 0x4000000 | next / previous weapon: **never** (below) |
| 27 | 0x8000000 | melee |
| 28 | 0x10000000 | spot (key down, 3 s cooldown) |
| 24 | 0x1000000 | never set by the game, yet tested (0x2D8BC1, on a path naming "brain"): **never** send it |
| 29, 30 | | unused |
| 31 | | a per-input flag the reader splits off |

PROVEN from the builder (0x241250) and the action table (0xA74AF0); the low
bits were checked against masks in real worlds. The consumer is
`base_player` 0x2D86E0:
- **Use** (0x2D88C3..0x2D88DF) starts on the edge, 0 then 1; holding 1 starts
  nothing new. PROVEN.
- **Grenade** (0x2D8E05..0x2D8E6E): pressing marks the grenade slot as
  wanted (0x309DC0), releasing clears it; the throw starts once the weapon is
  in state 6 (0x2E4090). So a bot holds bit 23 until the throw begins. PROVEN;
  the shortest hold that throws is not known.
- **Quick use 1..6** (0x2D8D7D..0x2D8DFF): key down calls 0x2FCAA0(slot, 1),
  key up (slot, 0); the slot is the n-th filled one in the order 13..18, 3,
  0, 1, 2, 4, 5, 6, then pocket items. PROVEN.
- **Drop** (0x2D8E73..0x2D8EC8): bit 22, or a new fire press, lets go of a
  carried battery. PROVEN that it runs; "a carrier who fires drops it" is
  LIKELY, not seen in a game.

Whether the other switch bits act on their level or their edge is a GUESS:
the bot sends key-down actions as one-tick pulses.

Batteries (code): taking one from the ground on use is instant (0x3007E0),
and so are putting one in and stealing from a container (0x31CC40); the
battery config's `take_time` (0.6, 0.5 from content-edit battery-pickup-1) is
the length the hands-up animation is time-scaled to (PROVEN,
`battery_pickup.md`); the bots plan with no pick-up time. Only resupply zones (`resupply_time_in_ms`), artefact
containers (`artefacts_search_time_sec`) and defusing a trap need use held.
A carrier's speed is ×0.95 (`movement_speed_modifier: -0.05`, read at
0x30016D; speed = max(0, 1 + sum of modifiers), 0x2D5AE0): 3.325 m/s
walking. The worlds agree: carriers' horizontal speed piles up at 3.32–3.33
m/s, and of 20 carriers seen with the sprint key and a move key down most
stay at 3.32–3.4 (LIKELY a carrier cannot sprint; the few at 4.5–4.75 m/s
are not explained).

Rotation: the first half is a change of yaw, the second of pitch, in
radians, made once at the input's tick (LIKELY: applied at 0x2D86E0, yaw
`+= x` at 0x2D8AD0, the builder zeroes it every frame 0x2410D1). Forward
moves along (−sin yaw, −cos yaw) in x/z with y up (LIKELY, from worlds).
Which way a positive yaw turns is to be seen in stage 0.

## 3. What a bot must never do (every client would crash or diverge)

- A kit with no weapon in slots 7 / 10: every client crashes at the spawn
  (0x1F25BF). PROVEN.
- Bits 25 / 26 while holding something other than the two weapons: every
  client reads past the inventory (0x2D8D0A). PROVEN. Use bits 12 / 13.
- A rotation that is no number or huge stays in every world; the server
  refuses those already (`PlayerInput::rotation_is_sane`). Keep each turn
  under π, one input a tick.
- An input that reaches a client after its horizon (500 ms) is moved and
  that client's world diverges: send for a tick 50-100 ms ahead of the
  clock, never behind it; more than 2.5 s ahead is pulled back.
- Two inputs for one tick are merged with their rotations added: one a
  tick, on the 50 ms grid (never the precise 0xDD).

## 4. What the server knows

All inputs at once (kept 60 s), and the worlds the patched clients upload
(about 0.5 s old, every 3-5 s today; `snapshot_rules.md` §7). Each player's
block head has alive, pitch, yaw and the last mask (PROVEN, `players.rs`).

**Position (PROVEN, 2026-10-14):** `base_player::serialize` (0x2DAF60)
writes `bullet_character_controller::serialize` (0x7F7F0, 67 bytes) for a
living player only (the skip at 0x2DB303). It starts `84 + 8n` bytes before
the end of the player's state, `n` being the recent hits counted just
before the end (`u32 n`, then `n × {u32 time, f32 speed}`, then one byte).
Its first 12 bytes are `m_last_pos` in physics space: the map frame is
`(x, y, -z)`, the capsule's centre, 0.9 m above the feet standing (checked
against the walker mesh of level_05: 0.90 ± 0.03 m at 9 of 12 standing
samples). Then the jumping and crouch flags, normals, and at +66 sprinting.
Earlier in the tail: the horizontal speed (about 3.1-3.6 walking, 5.2-5.3
sprinting; unit LIKELY m/s), the spawn time, and per body part a health
(part 0: 1.0 unhurt; LIKELY the player's health). Decoded on the four
worlds of a live match, every player read with no ambiguity. Not yet seen:
a crouched sample, the sign of A/D. `players.rs`'s "No 0.26g snapshot was
seen yet" is stale.

**Movement (measured 2026-10-14 on 163,369 player rows of 165 saved
matches; private analysis, numbers below):**

| What | Value | How sure |
|---|---|---|
| Forward at yaw `a`, map frame (x, z) | `(-sin a, cos a)`; a positive yaw turns left | PROVEN (median error 1.6° over 4,456 pairs of worlds 50 ms apart; aim at enemies agrees) |
| LEFT / RIGHT | yaw + 90° / yaw − 90°; diagonals ±45° at the forward speed | PROVEN |
| Yaw, pitch | the yaw is never wrapped (seen from −66 to +55 rad); pitch up-positive, ±1 | PROVEN; the clamp LIKELY |
| `m_linear_horizontal_speed` | m/s, equal to the moves between worlds | PROVEN (ratio 0.997-0.999) |
| Walk | 3.5 m/s; each life at ×0.95, ×1 or ×1.05 of it | PROVEN (the cause, kit modifiers, LIKELY) |
| Sprint | about 5.2 m/s (also 4.95-5.0; never the config's 5.5) | PROVEN as seen |
| Strafe, crouch / aimed / aimed strafe / back | 0.75 / 0.9 / 0.6 / 0.85 of walk | PROVEN; back LIKELY |
| Getting to speed | about +0.6 m/s a tick, −0.5 slowing (0 to 3.5 in 0.3 s) | LIKELY |
| A world's keys | not yet in its own position: a tick's keys move the body from the next | LIKELY |
| Respawn | about 7.9 s after death, a median 72 m from it | LIKELY / PROVEN |

The bots' movement model (`v026g::bots::kin`) is this table. Between worlds
a bot's position is carried forward from its own inputs (`bots::body`).

## 5. Map knowledge

There is no navigation data in the archive (the engine's loader is never
called; PROVEN). Each map project's `client_project` has the respawn
points, the battery spawners, containers and stations, resupply and damage
zones, and the placed collision objects (`collision_objects[]`: library
name, position, rotation, scale, group and mask); the meshes are in the
models' `collision` / `walker_collision` entries (vertices, triangles, a
u16 per face whose top 2 bits are the walker behaviour: 0 ground up to
60°, 1 always ground, 2 never ground). Many walker meshes are hard links in
the archive's index: a loader must follow them.

- The walking body (group 4, mask 2) meets objects with `cgroup & 2`;
  bullets (16, 8) meet `cgroup & 8`. Walls come from the walker mesh; the
  bullet mesh is for line of sight only. PROVEN.
- Placement: world point = (local point × the `#[sx][sy][sz]` scale in the
  library name) × `Rx·Ry·Rz`(rotation) + position, row vectors; the
  object's own `scale` field is not applied again. All 43 spawns of
  level_01/day_4 land within -0.04..+0.12 m of a walking surface this way.
  (The 0.100b srp's `combat/collision.rs` applies the scale twice.)
- Body: capsule 0.76 m wide, 1.8 m standing, 1.1 m crouched; walk 3.5,
  sprint 5.5 m/s (`default.player`); step 0.6 m (PROVEN). Jump height:
  GUESS.
- A walk grid from the walker mesh (Recast-style, 0.25-0.5 m cells) builds
  in 0.2-1.4 s per map; a ray tree on the bullet mesh in 0.2 s, 1-2 µs a
  ray. On level_01/day_4 all 43 spawns connect with jumps; on
  level_05/evening_2 the spawn areas are left by drops and never re-entered
  (players' word: spawns can be one-way, with nothing to go back for).
- Built: the `mapdata` crate (dump per project, walk grid, paths, line of
  sight), with its file format, API and timings on all 72 projects:
  `player_bots_mapdata.md`.

## 6. Stages

0. **A test bot that moves** (`srp::drive`, `srp-bot --drive`): no server
   change. After its entry it waits 2 s, then walks 3 s, turns +π/2 once,
   walks 2 s, sprints 3 s, walks crouched 3 s, fires 1 s, reloads, takes
   weapon 2, turns back, takes weapon 1; again after 5 s. Each input is
   printed with the sum of the turns sent, which the bot player's yaw in
   the worlds must equal. To check with one real game: it walks, turns a
   quarter, fires; no "rotation" warning, no resync, no input of the bot
   dropped (0xF1) or moved (0xEF) in its lines; positions and yaw in the
   dumped worlds (`SRP_SNAPSHOT_DIR`) as predicted.
1. Bot places made by the server (no account, no session, never timed out,
   never a voter), entered once a real client is live; the inputs go
   straight to the relay. **Built 2026-10-14, not run with a real game:**
   - `SRP_SERVER_BOTS=<n>` (lobby, testing only) put `n` bots into each
     match made, after its players, each on the side with fewer places:
     name `Bot-<n>`, clan tag `BOT`, the kit of its role, session and
     account 0, no seat. **Removed 2026-10-17** with the queue (the
     owner's): the server's bots play only "Player vs Bots" (§8).
   - **The bots' kit** (`survarium::v026g::loadouts`; `BOT_KIT`:
     item, slot, count, input bit, what every role wears; `BotRole`,
     `BotRole::gear()`, `bot_kit(role)`, `bot_kit_items(role)`,
     `pockets()`): the starter's TT-33 as weapon 2 (slot 10, bit 13, a
     clip in slot 11), `bm_back_1` (slot 3) and `bm_legs_5` (slot 4), and
     in the pockets two `medkit` (slot 13, quick use 1, bit 14), the radio
     `respawn_booster` (slot 14, no key: it works carried; default
     modification 1220, `respawn_time_modifier` -0.2) and two `rgd-5`
     (slot 15, bit 23).
   - **Roles** (`BotRole`: the match server's personalities and shares,
     flanker 0.30, support 0.25, rusher 0.30, camper 0.15): weapon 1 in
     slot 7 (bit 12) with a clip in slot 8, and armour. All non-premium,
     item level 5 at most:
     | Role | Weapon 1 (level) and ammo | Armour |
     |---|---|---|
     | flanker | `uzi` SMG (2), `ammo_9x19p_para` | `bm_torso_1`, `bm_boots_1` (level 1, light) |
     | support | `ak_74u` (5, the lowest of the assault rifle category), `ammo_5.45x39_7n6m` | `bm_torso_4` (3, 6 kg, a fourth pocket: a third medkit in slot 16), `bm_helmet_1` (3) |
     | rusher | `toz_34` double-barrel shotgun (1), `ammo_12mm_buck` | `scav_torso_1`, `scav_boots_1` (level 1, light) |
     | camper | `mosin_nagant` (0, free, the sniper category), `ammo_7.62x54r_old` | `scav_torso_2` (3, 6 kg), `bm_helmet_1` (3) |

     The lobby gives each team's `k`-th bot `BotRole::nth(k)` (smooth
     weighted round robin: flanker, rusher, support, camper, ...) and
     logs "bots' roles". The match server reads a bot's role back from
     its profile: `loadouts::bot_role_of(&MatchProfile)` (weapon 1
     decides; `Formed::profiles[place]`).
     - Pockets: drugs, boosters, traps and grenades go only in slots
       13..18 (the dictionary's `slots_restrictions`), and the client
       throws from the first of them holding a grenade set (0x2FE8B0,
       `grenade_set_core` alone answers `vtable+0x64`). PROVEN. Quick use
       `n` is the `n`-th filled slot of 13..18, 3, 0, 1, 2, 4, 5, 6
       (0x925FD8). PROVEN; that every item but an artefact counts is
       LIKELY (the item's `+0x24C` was not read), so the grenade is the
       last pocket.
     - A player has 1 pocket of 1 unit (`default.player`); gear adds them
       (modification 1030: a pocket, 1170: a unit more in each). The
       lobby opens that many of 13..18 and empties the rest
       (`PaperDollBase.handleQuickSlots`); a `fills_whole_pocket` item
       takes one. LIKELY the client's rule (the exe's sum was not read).
       The backpack (+1 pocket, +1 unit, level 3, 6,650 silver) and the
       trousers (+1 pocket, level 4, 6,830) are the cheapest pair of
       non-premium pieces that open three: 3 pockets of 2. Tested
       (`loadouts` test `the_bots_kit_has_a_pocket_for_each_thing_it_uses`).
     - The radio works **by being carried**; a bot need not press
       anything. PROVEN (client, 2026-10-14): a `survarium::booster`
       (vtable 0xA1CA34, constructor 0x33F690) makes a modifier entry for
       each non-zero one of its 68 parameters. At every spawn
       `base_player::respawn` clears the player's 68 modifier lists
       (0x2DACE4) and the inventory fill 0x2FCB70 (called at 0x2DADBB)
       calls slot 14 of every item in the 23 slots, pockets included
       (0x2FD1A5); a booster's (0x33F960 → 0x33F990) adds its entries to
       the kit's lists. At a death the respawn rule (0x316AA0) sets the
       delay through the getter 0x2D5AE0 (index 0x3C, which sums the
       lists): `respawn_time × max(0, 1 + modifier)`, rounded up to the
       50 ms grid; 8 s × 0.8 = 6.4 s. No use, no count check. LIKELY only:
       that the -0.2 of default modification 1220 is in the booster's
       parameters (the client applies modifications to an item's config,
       `item_config_cook`; the defaults' path there was not traced). Other
       passive adders of the same kind: `armor` (0x33F3A0) and the
       artefacts (0x311AD0..); the medkit's adds (0x33D892, 0x33DDC1) run
       only while it is active.
   - `MatchPlayer::driven` (match server): `game/driven.rs` enters the
     place once a real client simulates (0xE9 300 ms ahead) and relays the
     stage 0 script's inputs (`v026g::drive`, moved there from `srp`) as a
     player's, kept in the history for clients that start later. No
     witness of the world (`is_bot`); a match left to bots ends as one
     left by everyone.
   - The queue's roster (`Matchmaking::add_bots`) has the bots' seats, so
     a late joiner's place counts from past them; the rewards move no
     rating in a match with one and keep nothing for account 0.
   - Tests: `match-server` `tests/driven.rs`, lobby
     `the_servers_bots_take_the_places_after_the_players`,
     `a_server_bot_wears_its_roles_kit_and_a_client_reads_it` and
     `each_teams_bots_take_the_roles_by_the_shares`, rewards
     `a_match_with_a_server_bot_moves_no_rating_and_leaves_nothing_for_account_0`.
   - Not yet: any choice of what to do (the script is fixed); pressing the
     kit's keys (nothing sends bits 14 or 23 yet); a real game with the kit.
     (A bot leaving to make room for a late joiner: built later, see the
     end of this section.)
2. **Bots that walk the map.** Built 2026-10-14, not run with a real game:
   - `SRP_MAPDATA_DIR` (match server): the folder of `mapdata-dump`'s files
     (`player_bots_mapdata.md`). With it, the server's bots
     (`v026g::bots`) walk; without it, or for a map with no file, they run
     the stage 0 script. The map's walk grid is built on a thread of its
     own (0.3-0.8 s, 60-150 MB) when the match is made; the bots stand
     until it is there.
   - **Where its body is** (`bots::body`): the trusted worlds show it
     (`Game::observe` → `observe_driven`: the controller's position, yaw,
     keys, speed, alive, spawn time). Between worlds it is carried forward
     tick by tick with the bot's own inputs and the movement model above,
     kept on the walk grid (walls stop it, it slides along them); each
     world puts it back and the inputs since are played again. While bots
     walk, the clients are asked for a world every 2 s (1 s while a bot
     waits to see its body: just entered, or dead), not 5 s.
   - **Routes** (`bots::path`): A* on the grid, pulled straight to the few
     real corners; a jump or a drop is a corner of its own.
   - **Smooth movement** (`bots::motor`): the view turns with an eased
     speed (most of a turn in about 0.15 s, at most 5 rad/s, its speed
     changing by at most 45 rad/s²), never past the heading; it walks while
     turning and lets go of forward only for a turn over 80°; it steers at
     a point 1.6 m + 0.35 s ahead on the route (shortened until the line
     there is walkable, so it never cuts a corner into a wall); it sprints
     only with 10 m of straight route ahead, after facing the way for
     0.3 s, and stops 3.5 m before a sharp corner; its heading sways by
     about a degree; standing, it looks about every 1-2 s.
   - **Stuck** (`bots::stuck`): two worlds 1.5 s apart that show the body
     moved under 0.4 m while it walked. First a jump and a step back and
     to a side, and the same route again; caught again at the same spot
     within 20 s, a jump and a sidestep and another goal; a third time, it
     stands a few seconds and goes elsewhere. A spot that caught it twice
     is kept clear of for 45 s (routes past it are not taken, unless every
     route passes it).
   - **What it does** (the one play of this stage): walks to one of the
     map's objectives (a container, a battery's place, a station) or a
     floor 20-60 m off, never within 25 m of the other side's spawns (its
     side: the spawns it first came out of), looks about there 1.5-5 s,
     and picks the next.
   - Each bot's random numbers are its own, seeded by the match and its
     place (`bots::rng`), so a replay with the same worlds repeats it.
   - Tests: `bots` (movement model, encoder, the body's reckoning, routes,
     motor smoothness, stuck), scenario tests against a stand-in game
     that moves the body by the bot's inputs (`bots/tests.rs`: goal to goal
     round a wall, a post the grid lacks), and the match end to end
     (`tests/driven.rs`: a client's worlds show the body, the bot walks
     with eased turns, steps back when the worlds show it not moving, and
     the worlds are asked for every 2 s).
   - Not yet: a real game; what the bot does about other players (it does
     not see, aim or shoot); jumps up ledges are pressed but their reach is
     not checked; the cost of the extra uploads for the clients (two
     worlds every 2 s) is not measured.
3. **Bots that play** (built 2026-10-14, not run with a real game; the
   design of `player_bots_research.md` part B, the behaviour of
   `player_bots_behaviour.md`). `servers/match-server/src/v026g/bots/`:
   - layers: `view` (every body reckoned from its inputs and the trusted
     worlds, sounds from inputs, deaths from the statistics, batteries
     and their carriers), `sense` (sight on the bullet mesh in a 90° view
     within a ray budget; hearing), `know` (facts by source; the motor
     aims only at a `SeenTrack`, made after the noticing delay),
     `cheats` (every battery, the thief ±10 m, a radar ±15 m while nothing
     is sensed; never in the aim), `signals` (team modifiers from the
     score gap, time left and who is alive; self-balancing skill in damped
     steps, taken at respawn), `team` (the commander every 0.5 s: rank
     bands, a commitment margin, one bot per slot), `plays/` (one file a
     play), `tactics` (routes off damage zones and the enemy's spawn area,
     spot queries with sight lines, the bases' exits), `reflex` (target,
     reaction delay, settling aim, bursts, reloads, a turn to a shot, a
     push on a reload, breaking sight and re-peeking from another angle,
     the weapon's range, spotting, a grenade at a held angle, a partner's
     trade), `motor` (smooth turns, strafing while aiming), `encode`;
     `tuning` (every number; `SRP_BOT_TUNING` JSON read when a match is
     made, unknown fields refused, hashed) and `trace`
     (`SRP_BOT_TRACE_DIR`: a JSON line per decision with every candidate's
     considerations).
   - Plays: roam toward the enemy base by the routes humans took (17;
     drawn by what a life on each gave, less where a teammate died or
     teammates already went, more for a fighter where enemies are known;
     `SRP_BOT_ROUTES`, a private table from the players' worlds; without
     it to the exits by a way of its own), collect a battery and bring it home, ambush (1), intercept a
     carrier and answer a theft (2), escort (3), sneak and steal (4),
     flank a held angle (8), crossfire (9), retreat to resupply (10),
     cover a human (12), pair up against the top player (13), hold an exit
     (15), late denial (16), guard the lead (18), trade a partner's death
     (21), heal behind cover (22), the Team Deathmatch hunt, the timed
     batteries' station; raid the enemy's container (24), keep with the
     partner (25) and cut the enemy's best way (26), from a study of what
     the winners of 6+ player matches did (`player_bots_behaviour.md` §3).
   - By mode (the players' worlds, `player_bots_behaviour.md` §3):
     Team Deathmatch holds the enemy's exits long, favours crossfire and
     flanks, and ends with the hunt (the enemy out of tickets cannot
     respawn; a team out of tickets plays it safe); batteries are a race
     for the batteries lying about, then intercepting, escorting,
     stealing, late denial and guarding a lead; timed batteries wait by
     the station whose battery is due (6-20 m off) and keep moving (a hold
     weighs 0.3 of a Team Deathmatch one).
   - Kit and role (`survarium::v026g::loadouts::BotRole`): flanker (Uzi),
     support (AK-74U), rusher (TOZ-34), camper (Mosin), each with two
     medkits, two RGD-5 and the radio (passive: −20 % respawn time, PROVEN
     in the client); the role is read back from the gear and sets the
     bot's personality.
   - Weapons (`guns.rs`): each weapon's rounds a load, shot time, trigger
     and reload time from the client's item configs and clips; the reflex
     counts the rounds it fires (an automatic weapon a shot per pass of its
     fire clip while held; the TOZ-34, the Mosin and the TT-33 one press a
     round, the trigger up before the shot's state is over), reloads at
     empty with the weapon's time (the Mosin round by round), and takes the
     TT-33 when weapon 1's kit ammo is gone (the AK-74U's 30 are one
     magazine).
   - The kick (`recoil.rs`, 2026-10-14, from the client's code): a shot
     lifts the **view** itself, camera and bullet together; the view's
     pitch is the base pitch (`m_look_pitch`, what a world saves) plus the
     weapon's kick (0x2E0F60, PROVEN); a downward turn is taken off the kick
     before the base pitch (0x2E0FB0, PROVEN), so while firing a world's
     pitch is above the sum of the turns; each shot's kick is reached
     linearly over the shot interval and a PID controller brings it back,
     also inside a burst (0x331C50, 0x331A50, 0x3321E0; PROVEN), per weapon
     from its config (the five bot weapons and the AK-74M as the default).
     Each bot keeps its own weapon's kick with the ranges' middles, aims
     from the view (base pitch and kick: `body::Estimate::view_pitch`) and
     turns so the view stays on its aim: each tick's turn takes back the
     rise the kick will give the view that tick (`recoil_pull`, 1), any
     weapon, one-round ones too; the wall test is on the view. The tracked
     body keeps the base pitch the world has (the part of each turn the
     kick took added) and the kick of each tick. LIKELY: the shot cadence
     (60/rpm from the tick the trigger goes down). Not traced: where the
     input's turn falls against the tick's weapon update and shots (the
     model: update, turn, shots). UNKNOWN: the pitch's unit (LIKELY 60-64°
     a unit; the bots take it for a radian, so their elevations are ~5-11 %
     high, not changed), how far the side kick turns the view (not used),
     when in a reload the kick is reset (the bot resets as it starts one).
   - Sight and aim by body part (`aimpoint.rs`, owner 2026-10-14): an enemy
     is seen when some part of him shows on the bullet mesh (chest, head,
     belly, thighs, the chest's edges, shins; heights of the game's hit
     boxes as the 0.100b server poses them, standing and crouched; the
     same in 0.26g is a GUESS); the reflex aims at the first part that
     shows (centre mass first, the head last, the motor's usual settle when
     it changes) and fires only while that part shows where the enemy is
     and where his speed takes him, and the view's line now and after this
     tick's turn meets no wall short of him; with nothing showing it holds
     fire and keeps the view where he was last seen (1.5 s). Grenades are
     not held. Bullets pierce some thin materials in the game
     (`bullet::try_pierce`, material `armor`), but the mesh has no material
     per face: everything in it counts as solid. Soaks: shots into a wall
     short of the target 3.5-9 % → 0-0.5 %, at a target with nothing
     showing 1.4-2.4 % → 0-0.7 %; about 1.1 µs a ray on the real maps
     (`real_sight_cost`), 42-51 sight and aim rays a bot a second.
   - Anomalies (`anomalies.rs`, `hazards.rs`): every damage zone with a hit
     is kept off with a margin; level_03's electric pairs run on the match
     clock from the static seed (checked on 36 matches' worlds) and are
     put right by every world; level_02's cloud is kept off.
   - Movement model from the client's code (sprint 5.0 m/s, speeds per
     stance, a 6-tick blend; §4).
   - Tests: each layer and play; soak runs of whole matches of bots
     against a stand-in game (`bots/tests.rs`: 4 against 4 in each mode to
     a winner; the long ones with `cargo test --release -- --ignored`).
     The stand-in fires each bot's weapon at its rate (it fired every
     150 ms before) and kicks the view by the client's rule, each shot's
     kick drawn in the config's range. 4 against 4 for 900 s: hits 1,299 of
     7,746 shots (16.8 %), 41 into a wall short of the target, 19 at a
     hidden one; with the kick switched off in both the bots and the
     stand-in 17.5 %, 35 and 14 (before, with the old lift and the 150 ms
     rate: 19.9 %, 25 and 13 of 6,419).
   - A local test with one game and 7 bots: "Player vs Bots" 4 v 4
     (`SRP_PVB=on`), `SRP_MAPDATA_DIR` set (and `SRP_BOT_ROUTES` for play
     17's routes).
   - Human routes on the real maps (a test run with the private files):
     174 routes over 8 maps and 3 modes, all 992 points on a floor, 891 of
     them open to the side (the rest in the enemy's spawn area), every
     route leaving from its own side's spawns. Play 26 on them: 45 of the
     47 sides with enemy routes get cut points (163), and 136 of those a
     spot to hold.
   - Not yet: a real game.
4. Bots fill short queues and give their place up to late joiners. The
   giving way is built (2026-10-14, not run with a real game); filling
   short queues is not.

Bots and late joiners: a roster place is never reused, so a joiner cannot
take the bot's own place; the bot leaves (0xEA) and the joiner comes on a
new place. Every place ever used counts toward the 20 of a match, so bots
should fill a match to a modest size: with 20 places used, a bot leaving
lets nobody in.

**Giving way** (built 2026-10-14; `SRP_BOTS_GIVE_WAY`, match server;
**removed 2026-10-17** with the server's bots in PvP matches: a match
against bots takes no late joiner, so no bot ever gave way in one; kept
for the record):
- Who makes room: when late joiners are let in (their 0xE9 scheduled),
  each in the order of its place takes the place of a server's bot of its
  side if the side has one in the match and, with the joiner, would have
  more players than the other side (`session::matchmaking_policy::
  bot_gives_way`); else the side grows. The lobby places late joiners by
  the same rule (`place_late`, `LiveView::giving_way`), so a side full of
  bots takes humans and a side short of players is filled first.
- Which bot: dead or waiting to see its body first, else the living one
  farthest from any fight (an enemy, a shot, a death of the last 10 s);
  never one carrying a battery (as the bots' view guesses the carrier),
  never a living one of a side that can no longer respawn (a Team
  Deathmatch side out of tickets ends the match when its last life goes).
  Without `SRP_MAPDATA_DIR` the bots have no view: the side's last bot by
  place leaves, unless the side can no longer respawn. A side none of
  whose bots may leave now owes one: it leaves at the first poll one may,
  unless the joiner has gone by then.
- When and how: an ordinary roster change, 0xEA a tick after the joiner's
  0xE9, 300 ms ahead of the clock (`ROSTER_LEAD_MS`). The bot stops making
  inputs at once and is forgotten by the others (memory, callouts,
  partner). Its result row says it left.
- What it never does: end the match. The timelimit rule never goes back to
  waiting, the gather modes end by points only, a match ends for want of
  players only when no real client is left.
- PROVEN (tests): `match-server` `tests/driven.rs` (`a_servers_bot_gives_
  its_place_up_...`, `with_giving_way_off_...`, `a_bot_of_a_side_out_of_
  respawns_...`), `bots/tests.rs` `the_bot_that_gives_way_is_...`,
  `session` `bot_gives_way` and `place_late` tests, lobby
  `a_full_side_takes_a_late_player_in_the_place_of_a_server_bot_...`.
  LIKELY only: that a real client takes the 0xEA of a server's bot as any
  player's leave (it cannot tell the place from a player's, §1, and
  players leave matches every day); not seen with a real game. What the
  game does with a battery its carrier takes out of the match is the
  game's, as when a human carrier quits; a carrier is not chosen, but the
  carrier is the bots' guess.

## 7. Behaviour

What bots should do in a match, built on this data (senses, plays such as
battery ambushes, carrier interception and sneaking into an empty base,
reactions to events such as a stolen battery): `player_bots_behaviour.md`.

How to build them (what shipped games did, and a module layout for this
server: layers, utility scoring with rank bands, runtime modifiers from the
match state, self-balancing, tuning files, decision traces and tests), from
web research with its sources: `player_bots_research.md`.

## 8. Player vs Bots (2026-10-16)

The first use of the server's bots outside tests: one player against them,
chosen in srp's changed lobby movie ("Player vs Bots"; the wire is
`lobby.md` §2.16.2, the lobby's rule `matchmaking.md` §9.8, the settings
`servers/lobby-server/src/v026g/pvb.rs`). Off unless `SRP_PVB=on`.

- **The match.** The player alone on team 0 and 2N − 1 of the server's
  bots (stage 1's places, `play::server_bot`), N − 1 on its side: 6, 8 or
  10 a side, on one of 16 maps (the day of each level, batteries and Team
  Deathmatch). Its teams are fixed: no late joiner.
  `MatchSetup::vs_bots` marks it for the match server and the rewards.
- **One witness.** The player's game is the only one that simulates: its
  uploads are trusted on its word alone (one voucher, as with a test bot),
  and the bots see the world only through them. A game that stops
  uploading (a window out of focus, a stalled link; match 0x225dc391
  showed what that does to bots) ends the match: asked for its world for
  `SRP_PVB_STALL_S` (60 s) with none uploaded while it plays, the match is
  over as deserted (`game.rs` `check_pvb_stall`). A game that is gone
  ends it by `BOTS_ALONE` (45 s), one that waits for a world by the lost
  world rule, as before.
- **Its pay.** Half the silver and experience (`SRP_PVB_PAY`), no quest,
  no daily bonus; and as every match with a server's bot it moves no
  rating and no balance rating. The one witness could fake its numbers
  within what the server's checks allow: that is why it pays less.
- **Maps in memory.** Each match against bots may be on another map, and a
  bot match without its map plays on the script (stage 0) until it gets
  one. With `SRP_PVB=on` the store keeps up to `SRP_PVB_MAX_MATCHES` (5)
  maps in memory at once, or `SRP_MAPDATA_AT_ONCE` if that is more
  (`session::v026g::pvb::maps_at_once`, `bots::maps`): five matches on
  five maps each have theirs, a sixth map waits for one to be let go.
  The search arrays are the match thread's, one set however many matches
  run (sized to the largest grid searched, about 11 MB on level_06), and
  a match adds about 3 MB of its own (§ "On host-like limits" of
  `player_bots_mapdata.md`).
- **What a map costs** (2026-10-16, the Linux release build in WSL, the
  built files read as the server reads them: `bots::maps` test
  `pvb_maps_held_at_once`): resident memory 33-71 MB a map, 53 MB a map
  on average over the eight levels (day_2), all eight held at once 423 MB
  over the process's 11 MB. The five largest (levels 6, 7, 4, 3, 5) held
  at once: about 300 MB. With the server's own 190-200 MB that is about
  500 MB: the container's 512 MB (`SRP26_MEMORY`) is too little for five,
  1 GB is enough. The sizes the code counts (world and tactics,
  `world_bytes` + `TacticsBase::memory_bytes`) are within 2 MB of these.
- **CPU.** A 10 v 10 of bots costs 30-65 ms a second on the host's core
  (`player_bots_mapdata.md`), so five at once may take a third of it.

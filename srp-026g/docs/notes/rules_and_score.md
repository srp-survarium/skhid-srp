# The rules of a match and the score in the 0.26g client: spawns, protection, modes, statistics, spotting

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`) and the game data.
PROVEN where an address is given, unless said otherwise. Counted on the saved worlds and final statistics of
recorded matches read with the repository's own parser: 949 recordings of one live set and 446 of a later one; a bot
is a place whose account id is 0; their bots ran the code released then (revisions 40 to 50). Every client computes
these rules in the lockstep; the server only relays entries and inputs and ends the match. See also `static_info.md`,
`modes_maps.md`, `snapshot_rules.md`, `results.md`, `spotting.md`, `damage_model.md`, `thrown_and_placed.md`.

## The respawn (`player_respawn_rule_core`, tick 0x316820)

- **When**: at a death (`on_event` 0x316AA0), if the victim's team may still respawn, the body's respawn time is
  now + floor(`respawn_time` x 1000 x max(0, 1 + the dying kit's modifier 60) + 0.5), rounded up to the 50 ms tick.
  `respawn_time` is the mode's: 10 s in the battery modes and Research, 8 s in Team Deathmatch; a kit with the radio
  has modifier 60 at -0.2. Counted: the delay fits 102,103 of 102,159 deaths of the first set and 48,962 of 49,164 of
  the second; the others were shorter by about a quarter (its source in those kits is not read).
- **Who is placed**: at the tick whose time equals a body's respawn time; a body left without a free point is tried
  again 50 ms later.
- **Where** (`select_respawn_points` 0x3171D0): the team's points whose zone checkers are empty and that have no
  body nearer than `ally_check_radius` (a mate) or `enemy_check_radius` (an enemy); shuffled by the rule's own
  generator (x 0x08088405 + 1; its seed is 0 at a match's start, moves only at these shuffles and is saved in every
  world), sorted by `priority`; the queued places take the first ones in place order. **There is no rule of distance
  to the enemy beyond the radius** (0.8 and 1.2 m on the stock maps; 20 to 60 m on one ported Team Deathmatch map).
  Counted: the draw replayed from a world's seed gives the point used in 49,250 of 49,256 clean cases; bodies spawn
  as near an enemy as the average point of their team (a mean 83.2 m against 83.2).
- A spawn takes the kit chosen for the next life, resets the body and the damage model, and sets the protection.
- **The stations of the timed battery mode**: only `spawn_points_count` of a map's stations are active (3; 2 on one
  map), drawn at the match's start by each station's `spawners_weight` on the gather rule's generator (0x315250),
  whose seed is the match's seed advanced a fixed number of steps a map (535 of 535 recordings). The server chooses
  the match's seed, so the active stations can be computed from the server's own data.

## Spawn protection (the damage model's invulnerability end and attenuation)

- Two sources and no other: **the spawn point** (now + `invulnerability_time`, 6 s on every point, no fade;
  0x316E72) and **a team's base zone** in the battery modes (`effect_zone_core` 0x31ACA0): every tick a body of the
  zone's team is inside it, the end is set to now + the zone's duration with its fade, OVERWRITING the point's end.
  The stock maps' zones: 2 s + a 2 s fade (one map 3 s fade; two maps 3 s and no fade). Team Deathmatch has no zone.
- So in the battery modes a body is protected while in its base and for 2 to 3 s (and a fade) after leaving it, which
  can be SHORTER than the point's 6 s. Counted: the zone set the end in 23,594 lives of a battery mode against 547
  with the point's plain 6 s; in Team Deathmatch 81,054 lives show exactly spawn + 6 s.
- **Nothing ends it early**: not firing, not moving. Fully protected, a hit does nothing at all (`hit_impl`
  0x2D9BBE); during the fade the damage is scaled. **A protected body deals full damage** (39 of 21,522 kills by
  played bodies and 85 of 25,258 by bots in Team Deathmatch were made within 6 s of the killer's spawn). **It is not
  shown to players**: nothing draws it. No death falls within the 6 s in Team Deathmatch (0 of 72,165).

## The modes

Team Deathmatch: a death costs the victim's team a ticket unless a mate made the kill (a suicide costs one,
0x31614E); with no tickets the team's respawn flag is cleared and a death sets no respawn time (0x316B04). The
battery modes' and Research's points are in `modes_maps.md` and `snapshot_rules.md`. The clients compute a rule's
completion and winner; nothing in the client reads it: the server ends the match.

## The statistics (the handler's events; every increment site read)

| event | what counts it |
|---|---|
| 0 kill | a kill of an enemy (0x2F5084) |
| 1 assist | at a kill: every OTHER body of the killer's team whose share of the victim's damage from that team is 10 % or more (0x2F56A0). No time window: the victim's received damage is cleared only at his death and when his pain pool is full again |
| 2 death, 3 suicide, 4 team kill | every death; killer = victim; killer a mate |
| 5 | an artefact taken from an anomaly |
| 6, 17, 18, 24, 26 | a battery's delivery: the shares by carried distance; a solo delivery; stolen and brought home |
| 7 cure, 14 silent kill | **never counted** (the medkit's call goes to an empty handler) |
| 8 rescue | only after a TRAP's hit: the killed enemy's last trap hit was on a mate now alive under half body health, within 2 s (6 in both sets) |
| 9 revenge, 10 | the victim killed someone within 2 s before; the victim carried a battery |
| 13 melee, 15 headshot, 25 grenade | by the killing hit |
| 19 found | **the first spot of a battery in its life** (0x334B22): the only way to it |
| 20, 21 | a battery put into a container; a trap's hit on an enemy |
| 22, 23 | bullets fired; a bullet's hit on an enemy |

The score is the events x the match's weights. The scoreboard in a match adds each event as it happens and so runs
over the recomputed sum where a weight is not whole (6,001 of 18,720 final records); a world's load recomputes it,
and the results use the recomputed form.

## Spotting

One press (input bit 28) casts a 100 m ray from the eye. **An enemy** hit is marked for 10,000 ms (0x2DC0B0) for
the spotter's team (his name and map mark through walls and smoke); it pays no event. **A battery** can be spotted
by anyone; its first spotter in its life gets event 19. Traps are not spottable. No cooldown (patch 38).

## Counted: bots against the others (first set; the second in brackets)

| | played bodies, Team Deathmatch | bots, Team Deathmatch | played bodies, battery mode | bots, battery mode |
|---|---|---|---|---|
| kills | 32,161 (9,091) | 37,226 (12,289) | 9,357 (6,385) | 14,114 (9,243) |
| **team kills** | 547 (216) | **11,892 (3,999)** | 229 (87) | **2,603 (1,744)** |
| suicides | 105 (28) | 689 (174) | 47 (28) | 269 (120) |
| assists | 2,911 (973) | 29,485 (9,334) | 1,061 (806) | 8,454 (5,448) |
| batteries found, a 100 records | | | 28 (19) | 16 (15) |

## Against the bots

- The bots press the spot key only at an enemy, and again only after 15 s: a mark lasts 10 s, so it is off for 5 s
  of every 15; they never spot a battery (event 19).
- `plays/station.rs` waits at every station of a map; only the drawn ones are active.
- `tactics.rs`'s spawn area of 20 m is a guess: protection is the point's 6 s and, in the battery modes, the base
  zone's own shape (in the players' resources.db only).
- The same as the client: the respawn flags, tickets and points read from the world, the kit choice for the next
  life, the results' score.

## Not read

What shortens some respawn delays by a quarter (an item's modification in those kits); who sets a body's
respawn-disabled flag; the original game's weight of a team kill.

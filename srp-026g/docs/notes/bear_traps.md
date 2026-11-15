# Bear traps (`base_trap`, dict 68) in 0.26g

Written 2026-10-26 for the PvB bots' traps (WP-T). PROVEN unless marked: read in the exe (`booby_trap_core`
0x3043C0.., `booby_trap_set_core` 0x305950..0x3068E0), in `gameplay/items/devices/blackmarket_trap.options`, and counted in 987
traps of 402 live matches of 12+ humans. The full record with scripts is outside the repo (`traps-record`).

## The item
- The only trap, category 11, a stack, level 1, Black Market. A bear trap: no explosion (`is_explosive: false`).
- Placed on the **release** of its quick-use bit (`inventory::action(slot, 0)` -> `booby_trap_set_core::action` 0x306720 ->
  `try_to_place_trap` 0x305EA0), no hold, no animation, weapon stays in hand. The bit is `0x4000 << (n-1)`, n the trap's rank
  among the kit's filled pockets, boosters not counted (519 placements, no exception).
- Ray from the head along the view, **2.0 m**, the static walk geometry (group 0x404, mask 0x202), flat ground only (normal.y >=
  cos 30 deg), the surface must allow it (`material_can_place_test`), not on another trap's sensor. Armed at once; `armed_life_time` 0:
  it stands for ever. Fired 8 s then gone; disarmed (use 5 s or a ranged hit) 8 s then gone.
  - Where the numbers come from (2026-11-07, A328/A324): both are the item's config, not constants in the code
    (`scripts/content-edit/throwables.tsv`, row `blackmarket_trap`: `deploy_m` 2 = `data/max_deploy_distance`, `slope_deg` 30 =
    `data/max_slope_angle`). The item loader stores `max_deploy_distance` at `[set+0x284]` (0x305A7F..0x305AA7) and
    cos(`max_slope_angle` x 1/180 x pi) at `[set+0x280]` (0x305AA0..0x305ADF: constants 0xA2D7E8 = 1/180, 0x9CC970 = pi, cos 0x39000).
    The placing path loads `[set+0x284]` as the ray length (0x306401) for `get_visible_place_transform` 0x306490, which casts
    with group 0x404 / mask 0x202 (0x3064B6), refuses a hit whose object is in group 0x200 (a trap's sensor: 0x306598..0x3065A0)
    and a hit whose normal y is under `[set+0x280]` (0x3065C3..0x3065D0), then the material tests (0x306618..0x306638).
- Amount per life = the pocket's count; the trap objects = the profile's whole stack; with all out the next placement
  **recycles the oldest**. A death or a respawn removes nothing.

## What it does
- Sensor: a cylinder radius 0.25 m, 0.2 m high; in play a body whose foot point is within **0.6 m** has set it off (565 of 566
  samples). Any body: enemy, ally, the placer (only the skill `friendly_trap`, modifier 44, stops a same-team catch).
- Damage: `right_foot` and `left_foot`, `slash`, the config's `amount` each (0.6 to revision 49, 0.55 from 56) times the placer's `device_damage` modifier (0x304B1B), armour piercing 1 (armour does not hold). Both leg pools to 0: no
  sprint, no jump, about 8 s, speed -0.2 per leg. Pain + half of the amount per foot (the foot's slash route 0.5: +0.3 then, +0.275 now) and **both count** (0.40 left of 1.0 in 113 of 122 catches).
  No root: the victim keeps aim and fire. The kill is the placer's; an ally's death is his team kill.
- Humans (12+ matches): 85 % of catches are enemies, 11 % allies, 4 % the placer (median 3 s after placing); 9.4 enemy catches
  per armed trap-hour; 46 % of caught enemies die within 20 s.

## What a world holds
`booby_trap_core::serialize` 0x3052E0 in the world's game-objects section: a placed trap is 77 bytes (state 1/2/3, a u32, the
64-byte transform, a u32, the placing time in ms); `survarium::v026g::match_msgs::traps::find` reads them. The server simulates none
of this; the clients do.

## Where humans trap a holder (live, 2026-11-08..12)
The 329 recorded human matches of 6+ players, 1,554 placed traps (`booby_trap_core` in the objects section: owner, deploy time,
place) against every body's feet and life in the 5 s worlds. A HOLD: one body's feet within 5 m for 30 s or more; KILLING: 2 or
more of the other team died within 60 m during it (the bots' 2-kill cue; the worlds carry deaths, not killers). His WAY IN and WAY
OUT: his feet over the 30 s before and after it. A trap is ON his way within 2 m of it; a STEP is a body's straight way between two
worlds passing within 1 m of the trap while it stands (an estimate: worlds are 5 s apart).

| Traps within 40 m of an enemy killing hold (span within 120 s), 633 | n | holder stepped | other enemy | mate | median m from the hold |
|---|---|---|---|---|---|
| on his way in | 83 | 9 | 2 | 0 | 15.6 (P25 5.8, P75 22.1, P90 26.9) |
| on his way out | 15 | 0 | 0 | 0 | 8.5 |
| within 10 m, not on his ways | 50 | 2 | 4 | 6 | 5.9 |
| elsewhere within 40 m | 485 | 1 | 23 | 33 | 27.0 |

- 9 of the 12 steps by a holder were on traps on his way in. With 3+ deaths within 40 m instead: 51 on his way in, holder 3;
  elsewhere 1 of 359.
- The way-in traps' timing: 49 down before his hold (a lane he then came in by), 17 after he left, 9 after his death, 8 during it.
- Of 2,334 killing holds 680 ended with his death; 5 traps lay within 10 m of such a hold after his death (a death point).

Script `private/pvb-343-check/fight-world/traps/human_traps_killing_holds.py` (outputs beside it, `human-traps-killing-holds-k2-r60.txt`
and the `k3-r40`, `k4-r30` cuts) over the routes measure tables (`ht/traps.csv`, `rt/rec_worlds.csv`, `rt/rec_levels.csv`).

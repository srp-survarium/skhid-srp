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
- Amount per life = the pocket's count; the trap objects = the profile's whole stack; with all out the next placement
  **recycles the oldest**. A death or a respawn removes nothing.

## What it does
- Sensor: a cylinder radius 0.25 m, 0.2 m high; in play a body whose foot point is within **0.6 m** has set it off (565 of 566
  samples). Any body: enemy, ally, the placer (only the skill `friendly_trap`, modifier 44, stops a same-team catch).
- Damage: `right_foot` and `left_foot`, `slash`, 0.6 each, armour piercing 1 (armour does not hold). Both leg pools to 0: no
  sprint, no jump, about 8 s, speed -0.2 per leg. Pain +0.3 per foot and **both count** (0.40 left of 1.0 in 113 of 122 catches).
  No root: the victim keeps aim and fire. The kill is the placer's; an ally's death is his team kill.
- Humans (12+ matches): 85 % of catches are enemies, 11 % allies, 4 % the placer (median 3 s after placing); 9.4 enemy catches
  per armed trap-hour; 46 % of caught enemies die within 20 s.

## What a world holds
`booby_trap_core::serialize` 0x3052E0 in the world's game-objects section: a placed trap is 77 bytes (state 1/2/3, a u32, the
64-byte transform, a u32, the placing time in ms); `survarium::v026g::match_msgs::traps::find` reads them. The server simulates none
of this; the clients do.

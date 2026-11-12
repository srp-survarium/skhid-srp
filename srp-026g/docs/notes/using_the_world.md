# Using the world in the 0.26g client: the use key, batteries, crates, artefacts, traps

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`) and the game data.
PROVEN where an address is given, unless said otherwise. Counted on the saved worlds and relayed inputs of recorded
matches read with the repository's own parser: 322 battery matches of one live set and 214 of a later one; a bot is a
place whose account id is 0 (the server's own mark of a body it drives; every take counted as a bot's is one). The
matches ran revisions 40 to 50: their bots ran the code released then. See also
`battery_pickup.md`, `battery_taker.md`, `battery_respawn.md`, `battery_credit.md`, `weapon_pickup.md`,
`held_artefact_transfer.md`, `bear_traps.md`, `client_patch.md` (patch 23: the take of a battery under the feet).

## The use key

- The key is action bit 21. **A use starts only on the key's down edge**, with no use running (0x2D88B1, the edge
  0x2D88C3..0x2D88DF in `apply_new_input` 0x2D86E0). Holding the key while walking into reach starts nothing.
- The scan (`detect_usable_object` 0x2D84C0): one ray from the eye (the head bone's place, player+47Ch) along its
  forward, `usable_objects_detection_distance` long; the first usable hit whose `can_use` accepts; a wall or a
  floor hit first ends it. **No angle, no cone: the look test is the ray meeting the object's collision shape.**
  Patch 23 lets the ray pass floors for a battery whose centre is below the feet and at most 1 m across.
- **The distance is 1.80 m**: the cvar's compiled value is 2.0 (0xA710D8) and the stock `startup.cfg` sets 1.80.
  Counted: of 293 battery uses caught at their start none had the eye over 1.75 m from the point.
- The eye, counted at battery uses: 1.30 m over the feet standing, 0.98 crouched for a played body; looking down at
  the pitch's stop it is 0.135 to 0.17 m AHEAD of the feet. The pitch stops at 1 unit, 67.5 degrees down
  (0x2E1012..0x2E1037).
- The battery's shape is a box of half sizes 0.30 x 0.12 x 0.16 m, and **its saved position is the box's centre**,
  0.12 m over its floor (hits from above land a median 0.114 m over it; a standing body's capsule centre is 0.784 m
  over it). A crouched capsule's centre is 0.559 m over the floor.
- A world saves the body's use (its start time, the eye's offset from the feet at the start, the point, the
  progress) and the object's users.

## What a use does (each tick `update_use` 0x2D86A0 asks the object; on a no it ends, 0x2F85B0)

| object | goes on while | ends at | time |
|---|---|---|---|
| a battery on the ground | no hand broken and the start's eye, carried with the feet, within 1.80 m of the point (0x300830). **No key, no look, no stance test** | the weapon's holster ends and the battery becomes the held item (`tick_active_object` 0x2D7650) | the weapon's `hide_time` / (1 + S22), on a 50 ms tick; the carry state `take_time` (0.5 s) later |
| a battery in the enemy's container | `can_use` each tick (below) and the same 1.80 m (0x31CCF0) | the same | the same |
| the own container, carrying | the same | the put state's end: **put in only if the carrier still uses that container, else dropped** (0x30102D..0x301096) | `put_time` 0.6 s |
| an ammo crate | the key HELD and the ray still on the crate (0x2F8120) | the time | `resupply_time_in_ms` (3,000 on every crate of all 116 map projects) |
| an anomaly's artefact | the key held and the ray on it (0x31C780) | the time | `artefacts_search_time_sec` x (1 + S7) (6 s; 5 s on 44 containers of two older maps) |
| a trap, to disarm | the key held and the ray on it (0x304F30) | the time | the trap's `defuse_time` |

- Refused at the press for a battery (`can_use` 0x300710): carrying one already, a broken hand, **sprinting**, dead.
  Not refused: moving, crouching, a jump, the air, the weapon's state, any team for a battery on the ground.
- Not ended: letting the key go, turning away, firing, reloading, a jump. A firing weapon only delays the holster.
- No progress is kept: an ended use starts whole again at the next press.
- S22 (`change_weapon_speed_modifier`) is read in the HOLSTER's clip (0x30DF7B, `weapon_core_hide_state_base`).
- Counted: the hold equals the rule in 1,188 of 1,198 and 770 of 775 takes by bots, 435 of 498 and 329 of 384 by
  the others (with S22 0.075 a hold of 750 ms, with 0.15 of 700 ms); the carry state 500 ms after it in 995 of 995.

## Under and over the player

- Over: only along the plain ray within 1.80 m; a floor or a shelf's edge in the way ends it.
- Under (patch 23): the 3.6 m ray from the eye, looking down at most to the stop, must meet the box. With the eye
  as counted, a battery 1.04 m under the floor is taken crouched from where its centre is 0.56 to 1.0 m across from
  the feet (its long side toward the body; 0.70 to 1.0 m with the narrow side), standing from 0.74 to 1.0 m. Nearer
  than that the steepest ray passes beyond the box; over 1.0 m the rule refuses. A centre 2.0 m under can only be
  taken crouched, from 0.95 to 1.0 m.

## A dropped battery

A drop (the carrier's death, firing, a weapon key, a hand broken) puts the battery at the hands' place with zero
speed and starts its physics (0x300F40, 0x300E50; for a living carrier after that tick's physics step, 0x2D954B, at
once for a dead one): it falls as a rigid box. **Only under y = -128 m is it put back**
to its spawn place (0x3011E1); a battery that comes to rest under a floor stays there. Counted: of 471 batteries
dropped by a carrier who died, 82 lay over 1 m under his feet. Of the dropped batteries' rests of 30 s or more,
127 (62,481 s together) had bodies within 3 m across and none ever within the client's reach.

## Counted: the time of a take from the ground

| | bots, set 1 | the others, set 1 | bots, set 2 | the others, set 2 |
|---|---|---|---|---|
| takes with the press found | 1,853 | 997 | 1,276 | 1,129 |
| movement keys let go to carrying, mean ms | 3,135 | 1,791 | 3,073 | 1,976 |
| movement keys let go to the first press, mean ms | 589 | 155 | 618 | 193 |
| the first press to the press that started the use, mean ms | 1,266 | 396 | 1,244 | 540 |
| takes that needed more than one press | 807 | 405 | 545 | 486 |
| movement keys still down at the starting press | 27 | 217 | 12 | 224 |

The hold itself is the same for all and is the client's. The bots' excess (1.34 and 1.10 s a take) is two waits of
their own code, neither forced by the client: the wait for the view to settle before the first press, and first
presses that started no use, each followed by 1.2 s and a world before the next.

**Why a first press started no use.** The client's ray rebuilt at every press (the view of the world before plus
the inputs up to the press; it passes the saved use point within 0.013 m, median): of 1,339 failed presses of the
first set, the ray landed on the map before the box in 552, passed short of it in 253, beside it in 265, over it
in 120 (129 met it by the rebuild: its own error; 20 other). **The bots pressed while the client's view was still
turning onto the battery**: in 844 of 1,210 misses the view met the box later, a median 550 ms after the press. The
bots' aim point itself was good (its ray from the counted eye meets the box in 952 of 1,191); the view the client
had was within the bots' settle angle of it at 5 of those presses. Two causes on the bots' side are read: the bots
released then took the pitch's unit for a radian (`MAX_PITCH 1.0`, "radians", at the tag `026g-rev46`; a unit is
67.48 degrees down), which put the client's view about 6 degrees under the aim; and the press was sent on the bot's
own belief of its view, which ran 0.3 to 1.2 s ahead of the client's. This branch has the client's pitch map
(`kin.pitch_scale`); its presses are not counted this way yet.

## Against the bots (this branch's files)

- `collect.rs`'s take of a sunk battery aims 0.22 m over the position and 0.16 m beyond the centre, taking the
  position for the box's foot: with the eye where it is, the client's ray passes about 0.35 m beyond the centre,
  off the box either way round.
- `view.rs` puts the eye straight over the feet; its height is right (0.878 m counted, 0.87 modelled).
- `collect.rs` presses once the view has settled, and again 1.2 s and a world later; the client takes any press
  edge whose ray meets the box, and a use's outcome is known a holster later.
- `tactics.rs` stands a bot for a battery on a shelf where the crouched eye is within 1.8 m in a line; the ray must
  meet the box before the shelf's edge (bots pressed 10 to 37 times from 0.5 to 2.0 m under such batteries).
- `collect.rs`'s `PUT_MISS_M` is no number of the client's: a put needs the ray to meet the container's own box
  within 1.80 m (the box is in the map's project: `container_collision`, its scale the half sizes, about 1 x 0.85 x
  0.65 m) and the use still running at the put state's end.
- The same as the client: the reach of 1.8 m, the put time, the crate's and the trap's hold with the view on it.

## Not read

Why the bots' belief of their view ran ahead of the client's at a press (a trace of the bot's own view and input
times at each press settles it); the eye's place between uses other than by count (the look clips).

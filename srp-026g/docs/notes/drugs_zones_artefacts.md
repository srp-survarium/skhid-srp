# What acts on a body over time in the 0.26g client: drugs, damage zones, artefacts, boosters

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`) and the game data.
PROVEN where an address is given, unless said otherwise. Counted on the saved worlds of recorded matches read with
the repository's own parser: 949 recordings of one live set (476,315 worlds) and 446 of a later one (222,556); a
bot is a place whose account id is 0, and their bots ran the code released then (revisions 40 to 50): the counts
say how those bots played. 748,184 bodies of the first set (an older pickup record) were not parsed and are in no
count. See also `medicines.md`, `drug-interrupt.md`, `artefacts.md`, `damage_model.md`, `using_the_world.md`.

## The effect records are presentation only

Every caller of the effect player's `play` 0x2DE090 was read (the spawn's effect, an artefact's glow, a zone's own
effect, a hit pool's effect, an affect's end, the cameras and the HUD): its emitters make presenters and nothing
else. No effect record of a world writes a pool, an affect, a modifier or an input.

What does act over time: the damage model's tick (regeneration, the affects' expiry) in the player's update; **a
drug in use, in tick list 6** (after the step's bullets); the damage zones and anomalies as game objects; an
artefact as an inventory item.

## A drug (`medkit`: every healing item, the painkiller, the antitoxin)

- **The press** (`process_quick_slots` 0x2D8BB0 -> `medkit::action` 0x33D700): on the key's down edge, when the item
  is not active and has a unit, `set_active` and **the unit is taken at once**. A press while the same item runs is
  ignored and spends nothing (0x33D70A). Two different items run at once.
- **Nothing the body does gates or interrupts it** in the stock exe: the one gate of the pockets is a byte of the
  hands object that both kinds of hands set to 1 and nothing clears (0x2D8D9A). So a drug can be pressed while
  sprinting, aiming, firing, jumping, reloading, with broken hands, and while carrying a battery; neither the press
  nor the tick reads the selector, the stamina, the weapon or the controller. The live game's artefact-carry patch
  only watches the quick keys of slots that hold an artefact.
- **The course** (`medkit::tick` 0x33D790): the delay `activation_delay_sec` x max(0, 1 + modifier 33), then the
  activity `activity_time_sec` x max(0, 1 + k x modifier 34). At the first active tick the listed wounds are removed
  (0x33DA50). Each tick, for each listed pool: (total / activity time) x max(0, 1 + modifier 57) x the tick's time,
  clamped at the pool's maximum (0x33D932). The totals are course totals. The heal already done stays when it ends.
- **What ends it early**: the holder's death (0x33D7AE), and in the live game exe patch 84 (a ranged hit that took
  health ends every healing drug, not the painkiller; `drug-interrupt.md`). Counted: of the bots' medkit courses
  that ended early, **828 by a new hit record and 853 by the holder's death, none otherwise**.
- **The protections** (`reduce_damage` 0x33DB30): under the threshold 0, else `hit_coeff` x max(0, 1 + modifier 58)
  x (damage - threshold): the painkiller on pain from ranged hits x 0.9, the antitoxin on infection from toxic hits
  x 0.5 (PROVEN; `medicines.md` had it LIKELY).
- **In a world**: the item's active flag, the remaining activity and delay in ms, and what each pool has received.
- Counted: every active drug row past its delay equals the rule (bots' medkit: 462 + 199 with today's pain total,
  8,291 + 8 with the total of revisions to 44, 9,064 + 7,295 with a pool at its maximum, 19 + 11 under it; none
  over). Played bodies also show the medical skills' factors (x 1.25, a longer painkiller).

## Damage zones and anomalies (`damage_zone_core::hit_objects` 0x319AF0)

- Each tick, for each body in the zone's sensor: an interval zone acts only when its last hit + interval has come;
  damage = the curve at the body's distance x `max_hit`, x (the tick / the interval) for a continuous zone; one hit
  of the zone's type on each box of its filter, **with no hit id: no hit record is kept** (0x319C63); then the
  zone's `last_hit_time` := now (0x319D15), saved in the world.
- Toxic and radiation hits go to the `infection` pool (lethal at 0; poisoning below full); fire, shock and blunt to
  the box's routes and the pain. **No pool's threshold sets radiation sickness**: 0 of 698,871 worlds hold it
  (poisoning: 53,231). Modifier 8 scales a hit of type toxic, radiation, fire or shock; modifier 51 a toxic one on
  the infection pool; the antitoxin halves it.
- What a player gets as a warning: the pool's own hit effect and the zone's sounds and visuals. No zone outside a
  map's bounds exists in the exe: every damage is a project's `damage_zone` object.
- The maps' kill zones (game data): Vector Laboratory's `death_damage_zone` (radiation, lethal in about a second)
  and its artefact spawner's zone (radiation 0.08 a second, always on); Mamayev Kurgan's `fire_dead_damage`;
  Cologne's blunt zone (0.1 a second on all 12 boxes).

## Artefacts and boosters

- An artefact's states: free, spawning, ready, taken, active, holder down; its key fires it in the taken state with
  no cooldown left; the cooldown is `cooldown_sec` x max(0, 1 + modifier 38) from the active's end.
- **Spring**: passive `movement_speed_mod` **+0.05** (every revision), active (10 s) +0.10, stamina regeneration
  x 6 and no sprint cost; **each copy adds its own terms**. Onyx: ranged damage x 0.95 passive, to x 0.25 active
  until 5 is absorbed. Lifebone: regeneration +0.1 a second to full on body, head, pain, hands and legs.
- A booster in a pocket makes kit modifiers at every spawn from its item's default modifications (sway -0.20,
  artefact search -0.25, regeneration +0.15, respawn -0.20, speed +0.05 and stamina +0.10, weight +5 kg): all
  change the simulation. Premium changes the pay only.

## Counted: bots against the others (first set + second set)

| | bots | played bodies |
|---|---|---|
| medkit uses | 7,710 + 3,217 | 4,587 + 2,965 (and 11,726 + 8,881 of other drugs; bots 6 + 0) |
| the medkit pressed after the last hit, median | 5.8 s / 6.45 s, at body 0.50 | 3.7 s / 3.85 s, at body 1.0 (limbs and pain) |
| deaths with a healing drug unused in a slot | 72,037 of 84,742 + 29,931 of 34,876 | 15,119 of 20,236 + 12,451 of 15,420 |
| deaths in a zone | 192 + 51 (1 in 441, 1 in 684) | 188 + 164 (1 in 108, 1 in 94) |
| runs of 10 or more worlds in a row losing health to a zone | 50 + 10 (38 + 6 ended in death) | |
| artefacts taken / fired | 0 / 0 | 336 + 210 / 735 + 349 |

The bots' long runs are Vector Laboratory's artefact spawner pit (63 + 12 runs of 5 worlds or more; 57 + 15
deaths): one traced bot fell 6.4 m into it, walked in it for 20 s without a sprint and died of the infection pool.

## Against the bots

- `plays/heal.rs`'s item table equals the configs from revision 45 on, as literals; each value has a game-data key.
- `heal.rs:372-391` refuses a press while carrying a battery ("the client most likely ignores the key"): the stock
  exe takes it.
- The bots judge a zone's danger from geometry and the anomalies' schedule only (`drive.rs:654-664`); they never
  read that their own pools fall with no hit record, nor a zone's saved last hit time.
- `mapdata/src/controller/speed.rs`: the Spring's passive +0.02 as one on/off term; the data has +0.05 a copy.
- `tests/standin.rs`'s zone gives a share every tick; an interval zone hits whole at each interval (the same mean).
- The same as the client: the course's rate and delay, exe patch 84's stop, a press while active, the zones'
  rate, where a zone's damage goes.

## Not read

30 course rows under the rule with the pool not full; whether a battery's carrier can heal in the LIVE game (the
stock exe allows it; not counted on recordings); the medical skills patch's delay rule against the stock one.

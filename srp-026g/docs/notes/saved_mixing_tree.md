# The animation's mixing tree a world saves, and the clips' clocks read from it (0.26g client)

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`). PROVEN where an
address is given, unless said otherwise. Counted on the first 60 recordings of one live set read with the
repository's own parser (27,382 worlds, 456,702 trees; a bot is a place whose account id is 0). The stream's bit
layout is `snapshot_rules.md` 7.2.1 (the crate `world-frame` walks it: `animation_validate`); this note says what
each field MEANS. See also `outside_a_world.md`, `weapon_fire.md`, `walk_blend.md`, `bullet_flight.md`.

## The tree's clock

- A save clones the player's tree with a time shift of 1,000,000,000 - the tree's clock (0x395AAA) and writes the
  clone; a load shifts it back (0x395B9C). **Every time in the stream is relative to the tree's clock, which is
  saved as 1,000,000,000 and is the world's time t**: game ms = saved - 1,000,000,000 + t. Counted: the first time
  is 1,000,000,000 in 456,702 of 456,702 trees; every tree decodes, ends at its bit count and uses both of its
  tables exactly.
- The reader takes the times and the floats in the writer's order (both tables are reversed, then popped).

## What each field is

| stream field | member | meaning |
|---|---|---|
| first time | tree +2Ch | the tree's clock = the world's time |
| 1 bit, 1 bit | state +5Ch, +60h | the clip's interval in play; the one before |
| float | state **+64h** | **the clip time inside the interval at t, seconds** |
| float | state +68h | the node's weight at t |
| 1 bit | state +70h | a threshold: 0 or the clip's length; the bit alone is saved and the load puts the length back (`resolve_links` 0x39594F..0x395A67) |
| 1 bit | state +75h | the clip is held at its end |
| 6 bits, time, bit, float, 8 + 4 bits | state +78h..+87h | **the node's next animation event**: its type (4 an interval's end, 8 the clip's end, 20h a channel's knot, 40h a change of direction) and **its ms (+80h)**, the interval and clip time it starts |
| 2 bits, time | state +8Ch..+97h | the next weight event (100h a weight transition ends; 102h the node ends at weight 0) and its ms (+90h) |
| not saved | state +A0h | the event the tick takes next: the earlier of +80h and +90h, rebuilt at the load (0x3B17E0) |
| 12 floats | | the body's transform |
| 10 bits + 1 bit an interval | interval +8 | **the clip** (its id in the match's registry) and which of its intervals |
| 5 bits | node +40h | **the time group**: 0 the gait, 1 the shake, 2 show and hide, 3 idle, 4 reload (and one round of a tube reload), 5 a tube reload's start and finish, 6 fire, 7 chamber, 8, 9, 10 a grenade's pull, hold and throw, 11 melee, none the on-site clip and the recoil |
| 2 bits | node +44h | the weight group (legs, upper body) |
| 1 bit | node +5Dh | the node animates the item's model, not the body |
| expression 0 (2 floats, a time) | time-scale node +8h, +0Ch, +10h | **the play rate S, the clip time t0 at the base, the base's ms T0**: the clip time at any T is t0 + (T - T0) S / 1000 |
| expressions 1, 2, 3 | | a weight; a weight transition and a rate transition (their start ms) |
| 3 floats a state | | the root motion's translation so far |

- A clip's id is its rank by name among the clips registered for that match (`animations_registry`: 0x2EF130,
  sorted at 0x2EF280): the same in every game of a match, another table in another match. The clips are told apart
  without it, by the time group and the node's own-object bit.
- Every hands clip is a pair: the hands' node (with the rate's expression) and the item's (none; timed by its group).

## A weapon's clip position, read from a world

1. The node: the time group (6 fire, 4 reload, 2 show or hide, 7 chamber), the own-object bit clear.
2. **The next shot**: the node's next event's ms (+80h) when its type is an interval's end. When it is a channel's
   knot (the hand knots at frame 1 of some rifles' clips): T0 + floor(f32(f32(f32(L - t0) / S) x 1000)), L the fire
   clip's length (frames x 0.033333335, from the game data's clip lengths). Counted: the saved event is that
   formula's value in 21,762 + 606 of 22,466 fire nodes (98 hold another weapon's clip). With FIRE held and rounds
   left, the next bullet seen is at the foretold ms in 581 of 671, the other of N - 1 and N in 6, at a later
   selection on the 50 ms grid in 82 (single-shot fire states): 669 of 671.
3. **The time left of a reload** (a draw, a chamber): the node's next event's ms less t; a weight event of 102h on
   the node means the reload is being cancelled. Counted for magazine reloads: the magazine is refilled by the next
   world exactly when the end was due in 2,352 of 2,384, and never before it.
4. The clip's position now is +64h; later, t0 + (T - T0) S / 1000.

## The burst's passes that looked 1 to 3 ms off

**The clip's clock is exact; the bullets' saved born time is not the shot's once a bullet changes course**: a
change re-bases the born time by floor(1000 t_hit) (`change_trajectory` 0x32EE20, `bullet_flight.md`), so a bullet
that pierced something a few ms after the shot carries a born time that much later. Counted: of 1,676 gaps off the
pass P, 1,524 hold a bullet that had changed course. On bullets still on their first segment the rule

- a pass is P = floor(f32(f32(L / S) x 1000)) (`weapon_fire.md`);
- or, when 60000 / rpm is a whole number N, the other of N - 1 and N after the tree was rebuilt during the pass
  (the event recomputed from the re-based clock; the rebuilt bases are in the saved trees: 313 of 1,979 such);
- or longer, when the next shot comes from a new selection on the 50 ms grid

gives **8,685 of 8,685** passes. (An independent recount by the most common gap of each body: 6,992 of 7,153
first-segment gaps at the pass; 1,342 of 1,945 gaps with a re-based bullet off it.)

## What bakes a start

A movement start is baked by every baking event of any node on the body inside its 0.3 s blend (the event's type
and 1DFh, 0x3A0280), and each node's next such event is in the world. On 2,756 clean starts seen at their first ms,
the first bake was the on-site clip's interval end (18 frames at rate 1) in 1,042, the weapon's idle hands clip's
in 603, a weight transition's end in 125, the fire clip's in 82, none in 834. The on-site clip's end falls in
1,356 of the 2,756 windows; its 0.6 s period foretells half.

## Not read

A clip id's name (the match's registered set); 15 magazine reloads due and not refilled with rounds in the
pockets (a magazine fills from the one stack the weapon is bound to, `load_magazine` 0x2E17D0; which one is not
in the dumps). Settled since: 97 of the 98 fire nodes with another weapon's clip are guns picked up from the
ground.

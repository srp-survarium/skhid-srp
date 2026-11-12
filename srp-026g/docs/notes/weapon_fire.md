# A shot in the 0.26g client: the weapon's states, the draws, the recoil, the cone

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`). PROVEN where an
address is given, unless said otherwise. Counted on saved worlds of recorded matches read with the repository's own
parser (`srp-validate`): 25,417 worlds of 50 matches against bots and 13,013 worlds of 37 matches between players;
the weapon numbers from the rev56 game data, which is not the table of every match counted: patch 45 changed
the `reload_time` of 27 weapons and the sample holds matches from both sides of it (rpm, the recoil, the dispersion
and the stamina damage do not differ from revision 43 on). See also `player_bots_weapons.md` (the cone's parameters),
`sprint_state.md`.

## The weapon's states

- The machine is `weapon_core`'s logic (+320h), ticked in `update_logic` 0x2E0BA0 after the animation selector, the
  fire-mode key and the ammo switch. States: show, hide, idle, reload, fire, and for the weapons that have them
  chamber a round, throw, melee. Its 38 transitions are built by `add_weapon_state_transitions` 0x2DF780; the first
  true one in that order is taken.
- **To fire** (`fire_pred` 0x2E3CB0): the weapon usable, the body not in its jump state, no sprint animation
  awaited, FIRE (bit 0x20 of the input's actions), a round (the chambered one for a weapon with a chamber state,
  else the magazine over 0), no ammo switch pending.
- **To reload** (0x2E3EF0): RELOAD (bit 0x40), or FIRE held with nothing in the weapon, or an ammo switch's own
  flag; and it can (0x2E3DA0): not in the jump state, not trying to jump, not trying to sprint, the magazine under
  its capacity, spare rounds. A weapon with a chamber state and no `chamber_a_round_on_reload` holds its capacity
  and one (0x2E39D0).
- **An emptied weapon reloads itself**: from the fire state, at the end of the last shot's clip, with no key
  (`reload_or_empty_pred_and_ended` 0x2E3F70).
- **The magazine fills at the reload clip's end**, not at the key (0x310E40 -> `load_magazine` 0x2E17D0), and the
  recoil is reset there (0x310E52). A jump ends a reload early and nothing is loaded (0x2E4010); so does death.
- **A reload's length**: the hands' first reload clip is scaled to `reload_time` and its speed x max(0, 1 + the
  reload speed modifier) (0x30E000, 0x30D750, 0x30E25A): reload_time x 1000 / (1 + the modifier) ms, floored. A
  pistol's reload from empty and a double barrel's of both shells play the hands' second clip at the first's speed
  (0x30FE90, 0x30F5A0): `reload_time` x clip 2 / clip 1 of the HANDS' clips (`user_animations`), not the item's. A
  round-by-round reload's start, round and finish clips are all sped by the modifier (0x3110D5). A bolt's chamber
  clip lasts `chamber_a_round_time` / (1 + the chamber modifier) (0x30E2B0, 0x30E58D).
- **The load at a spawn**: slot 7's weapon is in hand, else slot 10's (0x2DADC6); a weapon loads min(carried,
  capacity) when it is first taken in hand (`load_ammo_on_next_activate` +5F4h, set at 0x2FCBEC, read once at
  0x2E2B71 -> 0x2E27C0).
- Idle goes back to fire on FIRE held alone, no new press needed (`fire_pred` 0x2E3CB0).
- No aiming in a reload (`can_aim` 0x2E3AA0: idle or fire). The aim goes in or out over `aim_time` x max(0, 1 + its
  modifier) from whatever it was (0x2E1B50, 0x2E1BF0); only an aim of exactly 1 counts as aimed.
- The fire state (0x30E910, 0x30EA00, 0x30EA50, 0x30EBF0): the queue starts at min(the fire mode's count, the
  rounds in the weapon); each pass of the fire clip's shot event fires and takes one; the state ends at a clip end
  with the queue spent or, for automatic fire, FIRE up. The fire-mode key works only in idle with FIRE up (0x2E0CA0).

## An automatic burst

- The weapon changes state inside a tick, at an animation event's own time: `tick_animations` 0x2D8EE0 walks the
  animation player event by event and runs the selection (and with it `update_logic`) at each event that asks for
  one.
- The first shot is at the press's tick. **A pass of the fire clip is floor(1000 x L / S) whole ms in single
  precision**, L the clip's seconds (frames x 0.033333335) and S = L x (rpm x 0.016666668) its speed (0x30B336,
  0x30E5C0): an event's time is truncated to a ms (0x3B6C5F), an event at the tick's own time is taken in that tick
  (0x3A1A2C), and the loop restarts from the event with no remainder (0x3A0734). That is floor(60,000 / rpm) when
  60,000 / rpm is not whole (92 at 650, 85 at 700); when it is whole, one less (99 at 600, 74 at 800, 59 at 1000)
  **or the whole value, by the clip's length (80 at 750, 250 at 240, 500 at 120, one rifle's 96 at 625)**. So the
  rate is 60,000 / pass a minute (606 at 600 rpm), not rpm. Counted on the bullets in flight: where this and
  ceil(60,000 / rpm) - 1 differ, 440 gaps are on this value and 156 on the other; a fifth of all passes (4,020 of
  20,280) are 1 to 3 ms off it, on both sides, the rhythm staying shifted (not explained: below).
- At each clip end (`on_animation_end` 0x30EA50) the FIRE in force decides: held, a shot and another pass; up, the
  state ends. **A pass under way when FIRE is let go still fires** if its end comes at or before the release's
  tick. FIRE pressed again before the next clip end goes on in the old rhythm; after it, a new run starts at the
  press.
- So a lone run of n held ticks fires floor(50 n / pass ms) + 1 shots. Counted against the shots the recoil's
  draws give: this rule is exact on 23,436 of 23,904 windows of automatic fire by bots (98.0 %; the others are
  presses while the weapon was busy); a shot at the press and one every 60,000 / rpm ms is exact on 15,394
  (64.4 %).

## One shot (`instant_fire` 0x2E23A0)

1. The cone (0x2E1D50): the weapon's base dispersion x the ammo's, plus the character part and the bloom unless the
   aim is exactly 1; the bullet leaves at half an angle drawn from a normal distribution inside it, about a random
   axis.
2. Each pellet is turned again by the weapon's base x the ammo's buck dispersion (0x2E2120).
3. A weapon with a chamber state empties its chamber, any other takes one from the magazine (0x2E2533).
4. The recoil's `fire` 0x331C50, the dispersion's 0x331150, the statistics, the wear.

**Two generators.** The weapon's own (+5BCh, minstd) gives the direction: 3 steps a bullet, 3 more a pellet. The
recoil's is another: x = x x 0x8088405 + 1, a draw (x >> 12) / 2^20, **4 draws a shot in the order right, left,
top, shake**, seeded with the time the weapon was taken in hand (0x2E2C1E). Both states are saved in a world, so the
number of shots between two worlds of a body is known exactly. Counted: 364,910 of 365,049 pairs of worlds have a
draw count that is a multiple of 4 (130 were seeded again); 286,917 of 286,945 pairs of bullet weapons have 3 steps
a shot; the magazine fell by exactly the shots in 56,617 pairs and by more in none.

**What the kick turns.** The side kick x plays the hands' clip `recoil_horiz` at the fraction 0.5 - 0.5 x of its
length (0x2E12B6, 0x2E165A); the clip turns the eye bone `Item`, and with it the head matrix (0x2DA4B0), **2.22
degrees to the right at x = +1 and 2.44 to the left at x = -1**. The shake axis is the weight of the hands' clip
`camerashake_v2` (0x2E39B0, for any living body), which turns `Item` 0.17 to 0.90 degrees off the aim at weight 1
(mean 0.59; mean yaw 0.52 to the left) and moves only while the weapon is in its fire state (weapon+330h is the
weapon machine's current state, its +124h the state's id: 1 show, 2 hide, 3 idle, 4 reload, 5 fire, 6 chamber, 7
throw, 8 melee; 0x2E13DB).

## The recoil

A shot adds to each axis a draw between its least and most, less as the axis nears 1 (`recoil_power`), times
max(0, 1 + the recoil modifier); the kick is reached at a constant speed over **ceil(60,000 / rpm) ms**, then a PID
(0x3321E0) pulls it back. Counted: the PID's speed is the saved one to the bit on 125,932 of 126,069 worlds whose
last tick was all PID; a body's kicks over the draws from its saved seed are one factor to 1e-4 (1.0, or 0.92 and
0.90 with recoil items).

## The cone's two moving parts

- The character part (0x331330): a target by the body's state (idle, walk, sprint, jump, crouch, crouch walk; hip
  or aim), at least `low_stamina_dispersion` x (20 - the bar) / 20 while the stamina's low flag is set, plus
  `injury_penalty_for_double_handed` or `_one_handed` x max(0, 1 + modifier 65) for broken hands (the keys by the
  loader 0x2F0FC4..0x2F1036), times max(0, 1 + the state's skill influence x modifier 0); it moves linearly, up at
  `growth_speed`, down at `speed_of_aiming`. The bot's own target and current value are saved in every world.
- **A frame**: the player's tick to T (the weapon's tick first: the cone, the recoil, the breath when aimed; then
  the selector and the animations' events, the stamina, the damage model, the use), the input stamped T, the walk
  vector, the controller's step ending at T, then the bullets (`update_to` 0x2DA9A0; the world's tick lists 2 to 5,
  0x2D1E20). So the weapon's tick sees the stance, the keys and the wounds of the frame before.
- The bloom (0x331150): each shot adds `one_shoot` up to `max_dispersion`; it falls to 0 linearly at
  `speed_of_aiming`. A weapon just taken in hand starts with `reload_dispersion_amount` of bloom.
- Counted: every saved transition of the three (character, bloom, aim; 372,764 rows each) is on its line.

## Against the bots' model of their own weapon

- **The shots of an automatic burst**: the bots count a shot at the press and one every 60,000 / rpm ms while FIRE
  is held. The client's pass is a millisecond shorter, a pass under way at the release still fires, and a quick
  press again keeps the rhythm (the section above): the bots' count of rounds is exact on 64.4 % of the windows of
  automatic fire and one off on most of the rest (one too few fired in 4,337, one too many in 3,228); the mirror
  of the kicks misses the shot of the pass that ends after the last held tick (one kick short in 10,212 windows,
  two in 3,319). One-round weapons and bolt actions agree with the client's rule and the bots' alike.
- **The reload**: the bots count the rounds in at the key and reset the recoil there; the client at the clip's end.
  Of 2,104 reloads pressed with rounds left (weapons whose time no patch changed) none refilled sooner than
  `reload_time` / 1.12 and 1,988 held `reload_time` itself (a bot's reload speed is 0, or 0.12 with gloves whose
  default modification gives it: 51 of 51 such bodies). The bots take a pistol's empty-reload share from the item's
  clips: for two pistols they differ from the hands' (1,621 and 1,447 ms against the client's 2,692 and 2,403). (An
  earlier count here, 7,503 of 10,932 within 50 ms, laid one table's times on matches of two tables.)
- **The recoil's draws**: the bots take the middle of each range; the client draws from the saved seed.
- **The cone**: the bots take no bloom, the character part settled, no skill, low-stamina or broken-hands term; the
  pellet cone lacks the weapon's base as a factor (bases 0.8 to 2.0).
- The same in both: the kick's law and the PID, the turn that takes from the vertical kick, the aim times, an
  emptied weapon's own reload, a reload key inside the last shot's clip being lost.

## Not read

Why a fifth of the passes are 1 to 3 ms off (the clips' clocks ARE in every world, as the animation's mixing tree:
`outside_a_world.md`; its decode settles this without a traced run); who seeds the weapon's own generator (only
its constructor and `deserialize` write it).

Settled: the hip and aim multipliers of the cone are the weapon config's `dispersion/from_the_hip_multiplier` and
`aim_multiplier` (1 when absent, 0x330FB2, 0x330FEB); no modification and no scope changes them. The played
bodies' cones at 2 or 4 times their kit gun's value were **a gun picked up from another body** (client patch 85):
with the gun really held (the head's pickup record), 0 of 33,912 and 1 of 54,077 player cones are off, and all
3,845 player aim times fit. A weapon
tick that is not 50 ms comes with an input at a time off the 50 ms
grid inside the step (1,466 of 1,472 such worlds).

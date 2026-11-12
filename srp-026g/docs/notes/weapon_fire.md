# A shot in the 0.26g client: the weapon's states, the draws, the recoil, the cone

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`). PROVEN where an
address is given, unless said otherwise. Counted on saved worlds of recorded matches read with the repository's own
parser (`srp-validate`): 25,417 worlds of 50 matches against bots and 13,013 worlds of 37 matches between players;
the weapon numbers from the rev56 game data. See also `player_bots_weapons.md` (the cone's parameters),
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
- No aiming in a reload (`can_aim` 0x2E3AA0: idle or fire). The aim goes in or out over `aim_time` x max(0, 1 + its
  modifier) from whatever it was (0x2E1B50, 0x2E1BF0); only an aim of exactly 1 counts as aimed.
- The fire state (0x30E910, 0x30EA00, 0x30EA50, 0x30EBF0): the queue starts at min(the fire mode's count, the
  rounds in the weapon); each pass of the fire clip's shot event fires and takes one; the state ends at a clip end
  with the queue spent or, for automatic fire, FIRE up. The fire-mode key works only in idle with FIRE up (0x2E0CA0).

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

## The recoil

A shot adds to each axis a draw between its least and most, less as the axis nears 1 (`recoil_power`), times
max(0, 1 + the recoil modifier); the kick is reached at a constant speed over **ceil(60,000 / rpm) ms**, then a PID
(0x3321E0) pulls it back. Counted: the PID's speed is the saved one to the bit on 125,932 of 126,069 worlds whose
last tick was all PID; a body's kicks over the draws from its saved seed are one factor to 1e-4 (1.0, or 0.92 and
0.90 with recoil items).

## The cone's two moving parts

- The character part (0x331330): a target by the body's state (idle, walk, sprint, jump, crouch, crouch walk; hip
  or aim), at least a floor while the stamina is low, plus a penalty for broken hands (LIKELY, by the uses), times
  the skill's factor; it moves linearly, up at `growth_speed`, down at `speed_of_aiming`.
- The bloom (0x331150): each shot adds `one_shoot` up to `max_dispersion`; it falls to 0 linearly at
  `speed_of_aiming`. A weapon just taken in hand starts with `reload_dispersion_amount` of bloom.
- Counted: every saved transition of the three (character, bloom, aim; 372,764 rows each) is on its line.

## Against the bots' model of their own weapon

- **The shots of an automatic burst**: the bots count a shot at the press and one every 60,000 / rpm ms while FIRE
  is held; in 23,405 of 43,460 windows with fire the client fired more (by 1 in 16,896, by 2 in 5,362); single
  bursts agree in 6,226 of 6,977. One-round weapons agree in 94.6 %, bolt actions in 90.6 %. Why is not read (the
  fire clip's loop).
- **The reload**: the bots count the rounds in at the key and reset the recoil there; the client at the clip's end.
  Of 10,932 timed reloads the client's refill held the bots' time within 50 ms in 7,503.
- **The recoil's draws**: the bots take the middle of each range; the client draws from the saved seed.
- **The cone**: the bots take no bloom, the character part settled, no skill, low-stamina or broken-hands term; the
  pellet cone lacks the weapon's base as a factor (bases 0.8 to 2.0).
- The same in both: the kick's law and the PID, the turn that takes from the vertical kick, the aim times, an
  emptied weapon's own reload, a reload key inside the last shot's clip being lost.

## Not read

Why the client fires more shots than one every 60,000 / rpm ms when bursts follow each other; who seeds the weapon's
own generator; a weapon tick that is not 50 ms (1.3 % of worlds); the names of the low-stamina and broken-hands
terms; where the reload-speed modifier enters; the pistol's and the double barrel's reload clips; the shake axis's
reader.

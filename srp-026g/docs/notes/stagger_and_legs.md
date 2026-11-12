# How a hit and a leg wound slow a body in the 0.26g client

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`). PROVEN where an address
is given, unless said otherwise; config values as the game data has them. Counted on one replayed match.

## The stagger

- **What starts it** (`hit_impl` 0x2D9B90): every hit that does damage after armour (`hit_body_part` 0x2E5080), from
  a grenade or from a bullet on that bullet's first damaging hit. The body part and the damage's size do not matter; a
  hit that armour takes whole staggers nobody. Its size is the weapon's `stopping_power` times the ammo's factor.
- **Where it lives**: the recent hits (player+2E8h, {time, stopping speed} each) and the player's own node in the
  movement-speed modifier list (value at player+300h).
- **Its value** (`update_stagger` 0x2DBCD0, every frame of a living body): minus the smaller of the sum of the hits
  younger than `stagger_duration` (0.5 s) and `max_stagger` (0.5). **There is no decay**: a hit counts in full for
  500 ms of its age and then drops out. Hits add up to the cap; a new hit restarts nothing.
- **How it reaches the speed**: (1) `base_player::max_speed` 0x2DC010 reads the modifiers every step, so a gait at
  its cap (walking forward, sprinting) drops at once; (2) a change selects the animations again in that frame, and
  the same clip's play rate goes to its new value along a 0.3 s linear ramp of the rate itself (a time-scale
  transition, `new_time_scale` 0x3A28D0, read at 10 ms pieces: (k - 0.4) / 6 of the change on step k, 7 steps; a
  second change during it nests), which is how gaits under the cap (sideways, backwards, aiming, crouched) slow;
  (3) `can_sprint` 0x2DBF40 is false while the value is not zero:
  the sprint ends. The jump is not refused (`is_trying_to_jump` 0x2FC230 has no stagger test).

## Leg wounds

- Each body part has its own node in the same list (`update_movement_speed_modifier` 0x32A9C0): the term of the last
  threshold, in the config's order, that the part's health matches. A leg (pool 0.5; thresholds 20 % and 0): **-0.1
  while its health is over 0 and under 0.1, -0.2 at exactly 0**, nothing above 20 %. Two legs at 0: -0.4. Hands have
  no speed term.
- The **`leg_damage` affect** is a separate thing (`apply_affects` 0x32AB90, counted at the damage model's +388h by
  0x2E5370): it starts on the hit that leaves the leg at 0, lasts 8 s from that hit and is not extended by later hits.
  While one leg holds it: no sprint, no jump. While both do: the crouch is forced. Both hands (+389h): no aiming.
- **The speed follows the pool, the gates follow the affect**: 8 s after the breaking hit the body may sprint and jump
  again while the leg can still be at 0 (-0.2) if it was hit again meanwhile.
- **Healing**: the pool regenerates from 8 s after the part's last damaging hit, at 0.03 a second, up to 20 % (a
  broken leg: -0.2, then -0.1 for 3.3 s, then nothing). A bandage or a medkit (`medkit::tick` 0x33D790) removes the
  leg and hand affects at its first tick and heals over its activity time (bandage 0.3 a second for 1 s, medkit 1 a
  second for 3 s).

## Who else writes the movement-speed list

The kit's base (a skill, +0.01 a level), the weapon in hands (+0.05 submachine guns, -0.05 heavy guns), the carried
weight (-0.2 times its share between 20 and 30 kg), a carried battery (-0.05), the Spring artefact (+0.02, +0.1
active), armour (0 in the live data). Drugs write another list (stamina regeneration), not this one.

## Counted (one replayed match, 352 hits; 71 on a steadily moving body, none sprinting)

- unhit bodies run at 0.95 to 1.05 of the expected speed in 97.5 % of 88,899 steady steps;
- the first changed step ends 50 to 99 ms after the hit;
- walking forward (the cleanest case): 0.664 of the speed at once, about 0.68 until +450 ms, back to 1.00 by +850 ms
  (the 0.3 s blend after the 500 ms window);
- gaits under the cap, single hits (median): sliding to 0.815 by +350 ms, flat to +600 ms, back to 1.00 by +850 ms;
- no walking body fell under 0.48 of its speed.

The samples are small. Not in a recording: each hit's stopping speed, the leg pools and the affects' end times, so
the sums, the stacking and the leg terms could not be counted.

## Not read

Melee's stagger factor; the grenade's scaling by its thrower; the blend's exact law; when a drug's first tick comes
after the key; how hit zones feed the limb pools.

# How the 0.26g client decides that a body sprints

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`). PROVEN where an address
is given, unless said otherwise. Counted against a replayed match (`docs/notes/exactness_rig.md`: every body's
controller before and after every step) laid on the recording's inputs.

## The rule

A body sprints exactly while the **stand sprint state** of the held item's animation selector is the current state.
Its `initialize` 0x336A10 writes 1 to the character controller's sprint flag (+204h) and `finalize` 0x336A50 writes 0;
nothing else in the game's logic writes that byte. The selector's states and transitions are built in 0x2FAB70 and
ticked by the state machine 0x38CA20: each tick the current state's transitions are tested in the order they were
added, the first true one is taken, one a tick (the weapon's `update_logic` 0x2E0BA0 can tick the selector a second
time in a frame, so a sprint can start in the frame a reload ends).

## Starting

From the run, on-site, jump and both crouch states, after the death and crouch transitions, `sprint_predicate`
0x2FBC90, in this order:

1. the input's actions (player+330h) have SPRINT (0x200) and FORWARD (0x1) and none of BACK (0x2), FIRE (0x20),
   CROUCH (0x100); the side keys are allowed;
2. `base_player::can_sprint` 0x2DBF40: no broken leg (the damage model's +388h), the low-stamina flag (player+738h)
   clear, not overburdened (0x329C60), the held item's `can_sprint` (vtable +60h; a battery's always refuses), a
   skill test (0x2D5CD0) false, no stagger (player+300h within 1e-5 of zero, 0x927DF8), room to stand (the
   controller's +202h);
3. the held item's `can_sprint` again. A weapon's (0x2E3A10) needs its config's flag, the aim transition at exactly
   0, no wish to aim (0x2FC2A0) and the weapon idle (or in melee with an animation pending).

## Ending

The sprint state's exits, in order (none while the weapon is in melee, `is_ready_for_transition` 0x336A90): death;
crouch (the key, no room to stand, or both legs broken); **the jump, at the press** (the flag is already 0 while the
body is still on the ground preparing); then run or on-site when `sprint_predicate` fails or AIM is held.

So aiming both refuses and ends a sprint. There is no timer and no cooldown in the selector's tests (after a stop, a
landing or a turn): each reads the keys and the present state. The only times are inside the gates: the stamina
flag's hysteresis, the stagger, the aim transition, the weapon states' lengths, the jump state's length.

## Stamina

`player_stamina::tick` 0x329790 drains the bar only while the held item's `is_sprinting` (0x2E30A0) is true, that is
while the body is in the sprint state: a refused sprint costs nothing. The drain is `sprint_spending_speed` (5 a
second) times (1 + 1.5 x the weight's share between `min_carried_weight` 20 and `max_carried_weight` 30) and a
skill's factor. The low flag sets when the bar reaches 0 and clears above 20; regeneration (10 a second) starts
1,000 ms after the last spending (0x3297C6), at once from an empty bar. A full bar of 100 lasts 8 to 20 s.

## Speed

`base_player::max_speed` 0x2DC010 caps the walk vector's length per step at the larger of `desired` x max(0, 1 + the
movement-speed modifiers) and `minimal`: (5.0, 1.25) while the item's `is_sprinting` is true, (3.5, 0.875) else (the
four values are `player_params`' `desired` / `minimal` `walk_fwd` / `sprint_fwd`, loaded by 0x2F0B00).

## Counted

One replayed match, 16 bodies, 220,994 steps:

- the sprint flag of the step ending at time t follows the input stamped **t** (99.5 % of the steps), where the walk
  vector follows the input stamped t - 50 ms (`exactness_rig.md`): the flag is one input ahead of the speed;
- the recorded inputs never carry SPRINT without FORWARD or with BACK, FIRE or CROUCH: the client clears the bit before
  it sends (where is not read), so those key rules cannot be counted from a recording;
- SPRINT was held on 38,410 steps and the flag was set on 37,395 of them; never set with SPRINT up. The 1,015 steps
  held and not sprinting, by the first cause in the code's order: the jump state 209 (preparing 119, in the air 79,
  landing 11), a reload's window 380, low stamina 213 (one body, the heaviest: 11.3 s of sprint, about 3 s of walk),
  aiming or lowering the aim 129, crouch or no room 32, stagger 6, the press itself 3, a grenade 1, **not explained
  42** (9 episodes; 18 of those steps walk slower than the body's own cap: LIKELY leg wounds, which a recording does
  not hold). The reload, aim and grenade windows are assumed lengths (the longest stock times; a recording does not
  hold the weapon's state): upper bounds.

## Not read

The jump state's `is_ready_for_transition` 0x337330 (when a landing lets the sprint resume); where the client
clears SPRINT in its outgoing input; the frame order of the weapon's `update_logic`, `apply_new_input` and the
controller's step (measured above, not read).

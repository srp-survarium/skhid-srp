# The crouch in the 0.26g client: the logic's side

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`). PROVEN where an
address is given, unless said otherwise. Counted on a replayed match (16 bodies, 220,994 steps, 707 clean presses
and 553 clean releases of the key; `docs/notes/exactness_rig.md`). The controller's side (the four flags, the
capsule's swap, the stand test) is `exactness_rig.md` section 3a; see also `sprint_state.md`, `turning_and_jumps.md`.

The controller's flags, by the exe's names: i = +200h `m_input_is_in_crouch`, l = +201h `m_logic_is_in_crouch` (the
wish), s = +202h `m_can_stand`, c = +203h `m_capsule_is_in_crouch`.

## The key

- **A hold, to the logic.** `base_player::apply_new_input` 0x2D86E0 writes bit 0x100 of the input's actions into i on
  every input of a living body (0x2D89EE): no toggle, no filter, no timer. A toggle is the client's key layer: the
  actions `crouch` (id 5) and `crouch_toggle` (id 6) both set that bit (`process_first_person_mode` 0x241250,
  0x241F5D..0x241F7F). How the key layer keeps a toggle's state is not read.
- Counted: i before a step equals the key of the input in force on 199,524 of 199,525 steps.

## The wish

- **The wish belongs to the held item's animation selector.** Its two crouch states (on site, vtable 0xA1A1BC; run,
  0xA1A1E8) set l in `initialize` 0x337810 and clear it in `finalize` 0x337830 only when s is set. Besides them only
  `base_player::jump` 0x2D7920 writes it (l = 0 when s, 0x2D793E), and the controller's constructor, `initialize`
  and `deserialize`.
- **Entering**, tested first of all from every stand state (before sprint, jump and run):
  `crouch_on_site_predicate` 0x2FBDF0 = both legs broken (the damage model's +388h == 2), or (the key or s == 0)
  and no move key; `crouch_run_predicate` 0x2FBE50 = (the key or s == 0) and a move key (forward xor back, or left
  xor right).
- **Leaving** (0x2FBEA0 to on site, 0x2FBF00 to run, or the sprint): the key up, s set, and not both legs broken.
- So where the selector tests the crouch, **l = the key, or both legs broken, or no room to stand**.
- **What refuses a crouch with the key held**: the jump state until its landing is done (`is_ready_for_transition`
  0x337330), the sprint state with a melee under way (0x336A90), no selector (between one item's `deactivate`
  0x2FF4A0 and the next one's `activate`, which starts crouched when s == 0, 0x2FBA50), and death (i is frozen,
  0x2D86F5). Nothing else: no test of the ground, a reload, the aim, a stagger, the weight or the stamina.
- **What forces a crouch**: both legs broken (whatever the keys, and standing up is refused), and no room to stand.
- **The selector is not run every frame.** `tick_animations` 0x2D8EE0 runs it (`select_animations` 0x2D9690) when
  the player's +33Eh is set: by a change of the actions (0x2D899D), by animation, stamina, stagger, weight and leg
  events, and by a test made for the release (0x2D8EF1..0x2D8F2A): the wish set, the key up, room to stand, legs
  not both broken. A released key changes nothing until a step's stand test passes; the next frame then leaves the
  crouch state.

## Timing (counted)

| event at input t | i | l | the capsule | counted |
|---|---|---|---|---|
| press: still, walking, sprinting, falling | t | t | the step ending t | 686 of 686 |
| press in a jump | t | the first ground step + 50 ms | with l | 8 of 8 measured |
| release, a free head | t | t + 50 ms | the step ending t + 50 ms | 477 of 553 |
| release under a ceiling | t | 50 ms after the stand test passes | with l | 50, at 100 to 1,400 ms |

Every run of steps whose key and wish differ is one of the rules: the key held without the wish, 17 runs, all in a
jump; the wish without the key, 742 runs: 678 the one step after a release, 49 a ceiling, 12 after a hit (LIKELY
the legs: the leg pools are not in a recording; 7 of them start without the key 50 ms after a hit and last up to
3 s), 3 at a death. None is left over.

## The capsule, the speed, the jump, the sprint

- **The capsule swaps whole** at the head of the first step after l changes (`player_step` 0x7BDB6 ->
  `setup_crouch_state` 0x7F1A0), feet kept: (radius, half height) (0.38, 0.52) standing, (0.38, 0.17) crouched.
  No transition clip exists; what is seen is the clips' 0.3 s blend.
- **The speed follows the state, not the key**: the crouch states ask `movement_speed` with their own stance
  (0x337D5E): crouch forward 2.5, side 2.0, back 2.0 (aimed 2.0 / 1.5 / 1.5) times the body's factor, through the
  0.3 s blend (0x337D93); the cap stays the walk's. Counted: the first change on the step ending t + 50 ms (42 of 42
  presses, 18 of 19 releases); forward ratio 0.7143 and 1.4000. Under a ceiling the body keeps the crouch speed
  until it stands.
- **No jump from a crouch state** (no such transition), and `on_ground` 0x7EF90 is false while c is set: after a
  release a jump starts 100 ms later at the earliest, and only if the jump key is still held then.
- **No sprint while the key is held** (the key is in the sprint predicate's mask and the crouch is tested first). A
  press while sprinting ends the sprint at that input (94 of 94); after a release with the sprint keys held the
  sprint flag comes with l = 0 at t + 50 ms (16 of 16).

## In the port

`mapdata::controller::tick`: `Controller::crouch` takes the key and whether the selector tests the crouch
(`CrouchInput::Tested { key, legs_broken }`, or `Held` in a jump or a sprint's melee); `Controller::jump` is
`base_player::jump`'s writes. Not ported (not seen in the recording): the item change's start state, the frozen
key of a dead body.

## Not read

How the key layer keeps a toggle; the order of the input, the step and the logic's tick in a frame (measured:
the input, the step, then the logic); the conditions of some setters of +33Eh (0x2E0A59, 0x2E0B6E, 0x330990), so
whether a held jump key gets a selector tick 100 ms after a release without another event; with both legs broken,
the key and a move key held, LIKELY a change of state every tick (no data).

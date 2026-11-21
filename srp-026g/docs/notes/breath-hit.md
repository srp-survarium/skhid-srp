# A hit breaks the breath hold

The owner (2026-11-21): "can we make it so when player gets hit they lose their breath holding".
Exe patch 176 (`client-patches/rust/patches/src/breath_hit.rs`).

- **A hit:** any hit that took health. This is the same rule as patch 84 (`drug-interrupt.md`), but
  for every hit type: bullets, melee, grenades, the trap, falls and anomalies. A hit that armour
  stopped completely does nothing. Spawn protection means no hit at all. The shooter can be anyone,
  a teammate too.
- **While holding:** the hit victim's held gun is in the holding state. In any other state (normal,
  or short breathing) a hit changes nothing.
- **The hold ends as running out of breath does:** the gun goes into short breathing at its next
  tick. The hold's sound stops, the reserve is refilled, and the out-of-breath penalty rises to 1
  and then falls back to 0. No new hold can start until it is back at 0.
- **The stamina stays.** When the reserve runs out by itself, the game also empties the holder's
  stamina in that tick (0x31D7B0). A hit does not: the hold ends before that check runs.
- **The same in every game:** it is decided in the match simulation, so every game, a late joiner
  (the breath is in the saved world while aimed) and a replay agree. The match simulation changes
  (`simulation_changes.tsv` `next`). The world format does not change: the patch writes a field
  the world already holds, with a value it already takes.

## How (exe patch 176)

- The site is `player.hit_applied` (0x2D9CB1, `base_player::hit_impl` once the hit took health,
  with EBX the victim). Patch 84 shares it, as order 0, and this patch is order 1.
- The victim's held item (+128h), if it is a gun (vtable 0xA04174 or weapon_core's 0xA11094), has
  its `breath_vibration_calculator` at +288h. Its machine's current state is at +10h. When that
  state is holding (vtable 0xA16FAC), the patch sets the reserve (+50h) to 0.
- At the gun's next tick (`breath_vibration_calculator::tick` 0x31D6E0, only while aimed),
  fsm::tick tries holding's two transitions in order. not_holding_breath (0x31D400) ends the hold
  in normal if the key has been up for a second or the gun is lowered. Otherwise
  insufficient_breath (0x31D430) sees the reserve at 0 and goes to short breathing. Holding's
  finalize (0x31D190) then refills the reserve to holder +268h x modifier 23, stops the sound and
  zeroes the delay. Short breathing ticks the penalty up to 1, then normal ticks it back down to
  0. can_hold_breath (0x31D3B0) allows a new hold only once the penalty is 0.
- If the gun is lowered before its next tick, it stays in holding with no reserve until it is
  aimed again, and then goes on as above.
- Two bullet tasks that hit one holder in the same tick both write the same 0, so no lock is
  needed.
- Check: `client-patches/check_breath_hit_026g.py` (Unicorn, `tools/check breath-hit`).
  - It runs the hook in the patched exe for every hit type, both gun vtables, the other states,
    another item, no item, and patch 84 at the same hit.
  - It runs the game's own machine, from initialize_logic and the calculator's tick, through
    short breathing and back to a hold.
  - For comparison, it runs a reserve that runs out by itself in the original exe.
  - Not run in a game.

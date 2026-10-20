# Battery pick-up

The owner's rule and what it changes. Data: `gameplay/victory_items/default.options`
`states[0].take_time`; the change ships as a resources.db patch
(`scripts/content-edit/battery-pickup-1.json`, one `set`).

- **Decision (the owner, 2026-10-20):** the battery pick-up animation 20 % faster.
- **Values:** `take_time` 0.6 -> 0.5 s: 1.2 times the speed (0.6 / 1.2; 0.48 would
  be 1.25 times). The field stays f32. Unchanged: `put_time` 0.6 (`states[2]`,
  putting a battery into a container), the carry state, the animations.
- **What plays faster** (the client's code, 0.26g): the battery's cook reads
  `states[0]/take_time` and `states[2]/put_time` (0x2FA2B1, 0x2FA2D5) and gives
  them to `victory_item_core::initialize_logic` (0x301920), which makes the take
  state (constructor 0x338940) with `take_time`. The constructor stores the time
  scale `(end - start) / 30 / take_time` (0x30D750): the length of the state's
  first player animation (`key_objects/player/.../hands_only/show`) over
  `take_time`, in [state+0xE8]. The take state's expression (0x338B20) plays the
  player animation once at that scale, in time-synchronization group 2, and the
  item's animation (`key_objects/item/.../show_electrobox`) is built against that
  lexeme (0x345070). PROVEN: the player's take animation is time-scaled to last
  `take_time`, so a shorter `take_time` plays it faster, not cut. LIKELY: the item
  animation follows it (the same lexeme; not traced to the mixer). The take state
  ends on the animation's end (`victory_item_core_animation_end_aware_state`,
  0.21d `victory_item_core_take_state::on_animation_end_impl`): the carry state
  starts 0.1 s sooner. The battery itself is in hand at once, as before
  (0x3007E0); the put state is made the same way with `put_time` (0x338F00).
- **Who computes it:** the client, in the lockstep simulation (the item's state
  is serialized with the world, 0x344950): a match plays out otherwise. srp
  reads nothing of `states`.
- **Bots:** no pick-up time in their code (they press use every 1.2 s until they
  carry; `use_every_s`); nothing to change.
- **Release:** the resources.db patch, made by `scripts/content-edit/content_edit.py`
  from `scripts/content-edit/battery-pickup-1.json` (with
  `gameplay/victory_items/` from the live server game data) and
  `release/build-patch`; the server game data built again with that patch (the
  server data must be the one the players' patches make); and the `next` row of
  `crates/session/src/v026g/simulation_changes.tsv`, which this change shares
  with patch 38 and fall damage (one `next` row a release).

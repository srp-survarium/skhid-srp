# Anomalous Force Protein: grenade throw distance

- **The owner (2026-11-05):** the Anomalous Force Protein (`weight_booster`, item 339) also gives
  +15 % grenade throwing distance while in a pocket, besides its +5 kg weight limit.
- **How a throw is computed** (PROVEN, code): `0x30A250` sets the grenade's launch velocity: the
  aim direction x `apply_modifier` 0x2D5AE0 (index 27, `grenade_throw_power`, multiplicative):
  `throw_power x max(0, 1 + S27)`, then the grenade's vtable +18h (`grenade_core::throw_grenade`
  0x303C10 puts the body into the physics world with it). `throw_power` is the grenade's
  (`throwables.tsv`: 17 m/s, the RGD-5 and the RDG-M). S27 is the thrower's kit sum: the Grenadier
  skill (203, +0.1 a rank, 2 ranks; its card says "Throwing distance: +10 %") and every carried
  item's entries.
- **The effect:** built-in modification 1320 on the booster, `parameters/grenade_throw_power_modifier`
  +0.15 (type 1), as its +5 kg is 1124: the booster adds each non-zero parameter to the kit's lists
  at every spawn while it is in a pocket (`survarium::booster` 0x33F690 / 0x33F990, data.md). The
  booster's config gets the key (0.0), as it has every key its modifications change. Type 34 (a
  booster type that never rolls, drop weight 0). Data only, no exe change.
- **"15 % farther" is the game's own unit:** +0.15 of S27, throw speed x1.15, as the Grenadier's
  "Throwing distance +10 %" is +0.1 of the same sum. Throw speed 17 -> 19.55 m/s (with Grenadier 2:
  20.4 -> 22.95 m/s; the sum adds, 1 + 0.2 + 0.15). A drag-free flight's range on flat ground grows
  with the speed squared, so the real range is about +32 % (x1.3225); a range of +15 % would be
  +0.0724 (sqrt 1.15), one cell of `modifications.tsv` (1320's value) and the card's 0.15.
- **The card:** a built-in bonus, "Throwing distance: +15%" (property 174, a percentage made like
  movement speed's 105, its name `st_srp_throw_distance` in the ten languages; icon 12, the
  booster's own), shown as the +5 kg is. With Alt, a grenade's throw speed counts the pockets' boosters
  (`MatchEffects`: the armour's and the pockets' modifications, property 174 -> modifier 27).
- **Simulation:** every client computes every thrower's throw from his kit: a `next` row.
- **Checked:** `release/build-content` into a scratch folder; `content_edit` tests. **Not run in a
  game.**
- **In game:** one kit with the Force Protein in a pocket, one without; throw an RGD-5 at 45 degrees
  on flat ground from the same spot: about a third farther with it. The booster's card shows
  "Throwing distance: +15%" under its +5 kg; Alt on a grenade's card shows 19.55 m/s (no skills) once the
  grenade cards' throw speed row (PR #1311) is in.

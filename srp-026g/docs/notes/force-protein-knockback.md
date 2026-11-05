# Anomalous Force Protein: melee knockback (visual)

- **The owner (2026-11-05):** the Anomalous Force Protein (`weight_booster`, item 339) in a
  pocket makes a melee kill's ragdoll fly, "funnily". Visual only (the ragdoll is, ragdoll.md).
- **The carrier:** built-in modification 1330 on the booster, `parameters/stamina_boost_interval_modifier`
  +3 (player modifier 29: no 0.26g code reads it, no getter call names it, nothing sets it but
  skill 205, to 0; additive), as its +5 kg is 1124: a booster adds its parameters to the carrier's
  kit while it is in a pocket (`survarium::booster` 0x33F690 / 0x33F990). The booster gets the key
  (0.0). Type 34 (a booster type that never rolls). Card: "Melee knockback: +300%" (property 175,
  a percentage made like movement speed's 105; `st_srp_melee_knockback` in the ten languages).
- **The gun-stock hit:** a bullet whose ammo data's `damage_type` (+4Ch) is blunt: a weapon's
  `melee/data` (rounds are ranged).
- **The knockback** (patch 76's ragdoll, Rust `ragdoll` and `crates/ragdoll`): a living player's
  gun-stock hit by a player whose current kit sums modifier 29 above 0 gives the death it makes a
  `Knockback` of 1 + that (4). The death's hits then push 4 times as hard (the killing hit's
  3.5 x its momentum push, x4), the part's speed and spin caps 4 times theirs, the whole body's
  share 4 times more (its cap 4 times too), and the whole body is thrown up at half its level
  knock (`KNOCKBACK_LIFT` 0.5). Every other death: `Knockback::ONE`, as before.
- **Numbers** (the solver offline, a standing body, a gun-stock push of 8 N s: mass 0.01, 1000 m/s):
  the body flies about 2 m and lands about 2.5 m on (before: falls in place, about 1 m on).
  Renders: `private/renders/force-protein/knockback-front.gif`, `-side.gif`, `.png` (before left,
  after right; `cargo test -p ragdoll render_knockback -- --ignored` with `RAGDOLL_DUMP`).
- **Every client the same:** each reads the killer's kit, which every game has (the static info);
  the ragdoll is drawn by each game alone, never in the simulation: no `next` row.
- **Checked:** `cargo test -p ragdoll` (a knockback's knock and speed, the bounds), the patches'
  `cargo test`/Clippy, `release/build-content`. **Not run in a game.**
- **In game:** with the booster in a pocket, kill a player with a gun-stock hit: the body flies a
  couple of metres; without it, or with a gun, the death is as before.

# Anomalous Force Protein: melee knockback (visual)

- **The owner (2026-11-05):** the Anomalous Force Protein (`weight_booster`, item 339) in a
  pocket makes a melee kill's ragdoll fly, "funnily". Visual only (the ragdoll is, ragdoll.md).
- **The carrier:** built-in modification 1330 on the booster, `parameters/stamina_boost_interval_modifier`
  +15 (player modifier 29: no 0.26g code reads it, no getter call names it, nothing sets it but
  skill 205, to 0; additive), as its +5 kg is 1124: a booster adds its parameters to the carrier's
  kit while it is in a pocket (`survarium::booster` 0x33F690 / 0x33F990). The booster gets the key
  (0.0). Type 34 (a booster type that never rolls). Card: "Melee knockback: +1500%" (property 175,
  a percentage made like movement speed's 105; `st_srp_melee_knockback` in the ten languages).
- **The gun-stock hit:** a bullet whose ammo data's `damage_type` (+4Ch) is blunt: a weapon's
  `melee/data` (rounds are ranged).
- **The push** (patch 76's ragdoll, Rust `ragdoll` and `crates/ragdoll`): the hit's own impulse,
  where a living player's hit is recorded (the step): a gun-stock hit by a player whose current
  kit sums modifier 29 above 0 pushes 1 + that times its own (`shot::knocked_back`; +15: 16 x
  8 N s = 128). The death takes it as any hit: 3.5 times it on the part, 0.6 of that on the whole
  body. 128 N s is a heavy hit (`HEAVY_HIT` 100, more than any bullet's push: `shot::impulse`
  makes at most 0.04 kg x 3000 m/s x 0.8 = 96), which throws the whole body: its share is not
  capped (1.6 m/s a bullet's) and goes up at half its level speed (`HEAVY_LIFT`). Every other
  hit pushes as before. Why 16 (the owner, 2026-11-05: "let it be 16"): the played build
  (#1326's first version, +3) gave the death a `Knockback` 4 that scaled the push and again the
  whole body's share (and the caps), 16 on the body; one push 16 times as big draws the same.
- **Numbers** (the solver offline, a standing body, a gun-stock push of 8 N s: mass 0.01, 1000 m/s):
  the centre of mass goes 0.19 m up and ends 3.70 m on (front) and 4.11 m (side), stopped by
  the ground's friction; the played build 0.19, 2.16 and 2.59, stopped at the old 2.5 m travel cap
  (gone, ragdoll.md section 7); the plain gun stock does not go up and ends 0.69 and 0.98 m on. Renders of the played build: `private/renders/force-protein/knockback-front.gif`,
  `-side.gif`, `.png` (before left, after right; `cargo test -p ragdoll render_knockback --
  --ignored` with `RAGDOLL_DUMP`).
- **Every client the same:** each reads the killer's kit, which every game has (the static info);
  the ragdoll is drawn by each game alone, never in the simulation: no `next` row.
- **Checked:** `cargo test -p ragdoll` (the protein's throw as far as the played build, a plain
  gun stock not thrown, no bullet's push heavy), the patches' `cargo test`/Clippy,
  `check_ragdoll_026g.py`, `release/build-content` (the first version's; +3 to +15 is a one-cell
  change). The first version was played by the owner (2026-11-05, "fine"); this one is not.
- **In game:** with the booster in a pocket, kill a player with a gun-stock hit: the body flies a
  couple of metres; without it, or with a gun, the death is as before.

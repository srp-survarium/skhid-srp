# A gun-stock hit disarms a trap (patch 151)

- **The owner (2026-11-15):** "hitting with stock the trap should disarm the trap".
- **Who and what:** any player's gun-stock hit that lands on an armed trap, whoever set it (the
  owner's own, a teammate's, an enemy's), as any player's bullet does. A battery swing strikes with
  the gun-stock bullet, so it does too.
- **Effect:** exactly a bullet's: the game's own defuse. The trap leaves the world's collision (no
  more catch, no Use prompt), shows its disarmed model for `disarmed_life_time` (8 s) and is gone.
  The stock hit's own decal and sound are as on any surface.
- **Reach:** the melee bullet's (`melee/data/distance`, 1.7 m from the eyes), on the trap's hit
  shape: standing over it and aiming down, or crouched.
- **The hit shape is the trap's own size** (the owner's first test, 2026-11-15: "you need to hit
  the trap dead center, which is a bit frustrating"). `data/hittable_object` was a cylinder of
  radius 12.5 cm and half-height 5 cm, half the cylinders that catch a foot (`collision_sensor`)
  and take Use (`usable_object`): 25 cm and 10 cm. Edit list `trap-hit-shape-1.json` gives it
  their size, so a hit anywhere on the trap counts. A shot meets the same shape: shooting a trap
  is as much easier.
- **Before** (PROVEN, code and data): a trap is a hit receiver with a body in the bullets' ray
  filter (group 808h against the ray's mask 48h). `bullet::try_pierce` (0x32D960) calls the hit
  receiver's slot +24h for what the ray met, with the ammo's `damage_type` (0x32DA00);
  `booby_trap_core::hit` (0x305540) goes on only when that is the trap set's `defuse_by_hit`
  (+298h; `ranged` in `blackmarket_trap`, the one trap), to `defuse_completed` (0x3050C0), or to
  the trap's slot +40h (remove and blast) when its byte +220h is set (its writer not traced). A
  weapon's melee is a bullet of `melee/data`, `damage_type blunt`: it reached the hit and was
  dropped at the type test.
- **The change** (exe patch 151, Rust `trap_stock_hit`): a lohook at the hit's entry
  (`trap.hit`, 0x305540). A blunt hit leaves by the exit where the trap's own type goes on
  (0x305552); every other hit runs the game's test as before. Nothing but a melee bullet is blunt
  there (a fall is dealt to its player, not through a bullet).
- **Simulation:** every client runs every bullet: a `next` row.
- **Checked:** `check_trap_stock_hit_026g.py` (Unicorn: the hit from its entry on a made-up trap, in
  the original and the patched exe); `cargo test`, Clippy.
- **Played** (the owner, 2026-11-15, revision 61 test build, the stock hit shape): a stock hit
  disarms the trap, only dead centre. The wider shape: built (`release/build-content`), not played.

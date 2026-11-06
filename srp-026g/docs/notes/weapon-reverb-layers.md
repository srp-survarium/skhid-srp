# Ported guns: other players' shots at a distance

Pack `weapons-reverb-layers` (release/content.tsv), made by resource-porter's `relayer-weapons`
(`ports/weapons-reverb-layers.tsv`, `src/weapons_026/reverb.rs`, docs/weapons-026.md).

## What it does

- The later builds' third-view reverb item is a composite of its own on Rudnya outdoors
  (`rudnya_outdoor_3rd`, `_3rd_smg`, `_one_shot_3rd`): the near tail and the distant shot.
- The weapon ports (patch 19, weapons-031, -046, -069) kept only the near tail, at 0-2000 m: the
  ported guns' shots fell silent for other players at 19-50 m.
- The pack lays the donor's layers into each gun's third-view composite (0.26g's composites nest
  none): windows intersected, start offsets added, `enabled` kept (0.69d0 has its LMG core off).
- Each nested layer ends where its volume curve does: the near tail 20 m, `main_distant` 200 m,
  `core_distant` and `core_distant_one_shot` 240 m. A preset `default` keeps its window.
- First view: unchanged but for two tails that start late as their donors' do (KS-23M 80 ms,
  MP7 SD / KRISS Vector 55 ms).
- Out of scope: the per-location echo (0.26g has no sound zones); 0.69d0's own recordings of the
  distant layers (the pack carries 0.29a3's, as patch 19 did for the near tail).

## How 0.26g plays a sound by distance (static, exe 0x10000 base)

- Distance: listener to source, metres, `max(d, 1)` (0x5c280).
- Gain: the `.spl` preset's `volume` curve at d (0x5c7a0, curve 0x25f30), clamped to [0, 1],
  times `volume_k`; preset `default`: 2/d past 2 m. Past the last key the curve keeps its y.
- A voice is made only while the gain is above 0.01; with `can_be_displaced` it is released below.
- The `spl` curve feeds only the HDR level of the debug statistics: nothing audible.
- key_type 3 is linear; 0, 1 and 2 are cubic Hermite.
- A composite layer is emitted once, at the shot, when `enabled` and min <= d <= max (0x63d60).
  Under `use_master_layer_index` every layer follows the master's gain.

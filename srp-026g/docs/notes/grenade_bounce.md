# Grenade bounce sound

The owner's suggestion and what it changes. Data: the nine samples' options
`sounds/single/weapons/f1/grenade_hit_{ground,wood,metal}_0{1,2,3}.raw_options`; the
change ships as a resources.db patch (`scripts/content-edit/grenade-bounce-1.json`,
one `replace_string`).

- **Suggestion (the owner, 2026-10-21):** "make grenade bounce sounds louder | can we?"
  Yes, in data: no exe change, no new audio.
- **Retuned (the owner's test of revision 27, 2026-10-21):** the first version
  (`material_hit`, `volume_k` doubled, `spl_value` 20 -> 75) was "too loud, and almost
  like they use different sound". Now only the distance preset changes; `volume_k` and
  `spl_value` stay stock.
- **Where the bounce comes from** (stock 0.26g; no Skhid patch carries these files, PROVEN
  on patches 12 to 27):
  - the RGD-5, the RDG-M smoke grenade and the Black Market trap are game material 45
    (`grenade`, `data/mtl_idx`); their item configs have no bounce sound of their own;
  - `game_materials/material.pairs` gives each pair of materials its sounds: grenade with
    ground, forest ground, `default`, gravel, stone, asphalt, concrete, brick, grass ->
    `weapons/f1/f1_hit_ground`; with wood (silent, squeaky, thin), schiefer -> `f1_hit_wood`;
    with thin and thick metal, rabitz -> `f1_hit_metal`; with deciduous foliage -> the
    bullets' `material/foliage_2/bullet/foliage_hit` (shared with bullets, left alone); with
    24 others (mud, water, cloth, glass, cars, tree trunks, soft grass, the wooden bridge...)
    -> none. The `*_walkable` variants take their `base_material`'s pairs (LIKELY);
  - the three `f1_hit_*` collections are used by those grenade pairs only (46 references,
    all pairs 160 to 199), each one picks one of three samples at random.
- **Why it is quiet:** the samples have a reload click's options, not an impact's:

  | option | grenade bounce, stock | a bullet hitting dirt |
  |---|---|---|
  | `spl_preset` (distance curve) | `weapons_reloading`: 0.755 at 0 m, 0.135 at 3.5 m, 0.02 at 6 m, silent from 9 m | `material_hit`: 1 at 0 m, 0.42 at 9 m, 0.21 at 16 m, 0.06 at 23 m, silent from 35 m |
  | `volume_k` | ground 0.5, wood 0.4, metal 0.3 | dirt, mud 1.0, metal 0.6 |
  | `spl_value` | 20 | 75 (dirt, mud, ricochet) |

- **Values:** `spl_preset` `weapons_reloading` -> `mid_range` (stock preset, used by 3
  stock sounds; no patch changes it). Unchanged: `volume_k` (ground 0.5, wood 0.4, metal
  0.3), `spl_value` 20, the samples, `use_hdr` 1, `lp_filter_k` 1, `can_be_displaced` 1,
  the explosion sounds, every other pair.
- **The preset is distance only:** every `.spl` file (all 52 stock ones) holds two curves
  by distance, `volume` and `spl`, and nothing else: no filter, no reverb. The filter is
  the sample's `lp_filter_k`, kept. So the sample sounds the same; what changes is how it
  falls off. The second curve: `weapons_reloading` 100 at 1 m, 35 at 3 m, 0 from 7 m;
  `mid_range` 100 at 0 m, 65 at 3.5 m, 30 at 8 m, 0 from 20 m (the same peak).
- **What it means** (ground, `volume_k` 0.5 x the `volume` curve, read linearly between
  its keys; wood and metal scale alike):

  | distance | stock (`weapons_reloading`) | new (`mid_range`) | revision 27's test (`material_hit`, x2) |
  |---|---|---|---|
  | 0.5 m | 0.35 | 0.46 (+2 dB) | 0.96 |
  | 1 m | 0.31 | 0.41 (+3 dB) | 0.93 |
  | 2 m | 0.17 | 0.33 (+6 dB) | 0.85 |
  | 3 m | 0.10 | 0.25 (+8 dB) | 0.77 |
  | 5 m | 0.03 | 0.15 (+15 dB) | 0.64 |
  | 7 m | 0.007 | 0.09 | 0.54 |
  | 10 m | silent | 0.05 | 0.40 |
  | 15 m | silent | 0.016 | 0.24 |
  | 20 m | silent | silent | 0.13 |

  Close by it stays about as loud as stock (+2 to +3 dB within 1 m); it carries: stock, a
  bounce farther than about 6 m is not heard at all, now it fades out by 20 m.
- **How the client reads them:** `single_sound_cook` (0x61220) reads `options` of
  `resources/sounds/single/<name>.raw_options` when the sound loads: `spl_value` (default
  60), `volume_k` (default 1), `lp_filter_k` (default 1), `can_be_displaced`, `use_hdr`,
  and `spl_preset` (loads `resources/sounds/spl/<preset>.spl`). PROVEN (0x612F4..0x61482,
  the defaults at 0xA2D740, 0xA70884). `pan_3d` is not read. `volume_k` has no cap in that
  reader; data already uses 2.0 (the F1 blast, melee hits). The `.spl` file's `volume`
  curve is the gain by distance (LIKELY: its keys run to the preset's range, 9 m to 500 m;
  0.10b's sources call the curve's end the distance a sound is unheard from). `spl_value`
  with `use_hdr` is LIKELY the sound's loudness for the HDR mixer (0.10b has the HDR
  hooks, `m_La`/`m_Lp`, not the code); revision 27's test with 75 sounded different
  (the owner), so it stays 20.
- **Who hears it:** everyone near the grenade, not only the thrower. The match is lockstep:
  every game has the grenade in its own physics world and runs its flight and bounces
  (`grenade_core::deserialize` 0x303F80, `client_patch.md` 3.14), and the pair's sounds
  are loaded by the game client (the pair reader asks for them as resources,
  0x2CD7EE..0x2CD894). The bounce is a 3D sound where the grenade hits, attenuated by
  each listener's distance on the curve above (LIKELY: the play call itself was not
  traced). The thrower is usually the farthest from where it lands, so the gain is
  mostly for the players it lands near. The trap (same material) gets the same sound if
  it ever lands with a hit.
- **Who computes it:** nothing of the simulation changes (sounds only): no
  `simulation_changes.tsv` row; a game with and one without the patch stay in step. srp
  reads none of these files, and the server game data has none of them.
- **Release:** the resources.db patch, made by `scripts/content-edit/content_edit.py`
  from `scripts/content-edit/grenade-bounce-1.json` with the nine `.raw_options` from the
  stock resources.db (`dbgrab resources.db get sounds/single/weapons/f1/grenade_hit_ live/`,
  then the `.ogg` and `.rms` removed), and `release/build-patch`. Patches 19, 20 and 22
  already carried `.raw_options` files the same way.
- **Other presets that fit** (stock, `volume_k` 0.5): `steps` (the footsteps': 0.34 at 1 m,
  0.12 at 5 m, 0.04 at 10 m, silent from 17 m; its `spl` curve peaks at 120, not 100),
  `low_range` (0.33 at 1 m, 0.08 at 5 m, silent from 12 m) for less reach.

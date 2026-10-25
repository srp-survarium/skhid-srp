# Missing textures

The owner (2026-10-25): "we should fix those", the game logs' `cook of '<texture>' [class 5]
failed` and `Texture <texture> not found`. Visual only: no simulation change.

## How the list was made

A static scan of the live data (the stock resources.db and patches 12-34, as players mount them),
not only what logs happened to show:

- every material a model's default material set (`material_settings`) or a config (projects'
  decals, particles) names, and each texture parameter it samples: the parameter's `use_*` switch
  on (`texture_bump` by `use_parallax`, `texture_roughness` by `use_troughness`, ...);
- every texture a config names directly (projects' probes, colour grading, sky): all resolve; the
  probes' `textures.<weather>` tables are the editor's, the game reads `texture`;
- the camouflage baker's own names (on_base_texture_loaded, 0x12E0DB..0x12E1E7): for each baked
  surface with diffuse D, `D_aoac`; on a decal without a camouflage also the layer
  `D_camo`, `D_camo_nmap`, `D_camo_smap` (type f asks for neither). Patch 44 bakes every surface of
  a camouflaged weapon (the first 8), the stock game each armour part with `camo_available`.

## What was missing, and the fix

| texture(s) | who asks | cause | fix |
|---|---|---|---|
| `ak_12_mag_camo_aoac`, `saiga_12_mag_camo_aoac`, `saiga_12_grip_camo_aoac` | patch 44 on the premium AK-12 and Saiga-12 | premium-only textures (base patch 19) not in guns.tsv | weapon-camo@2: premium copies of the plain weapon's `_aoac` (ak74m, vepr12, ak74n: same mesh UVs, same layout) |
| `vepr_12_mag_camo_aoac`, `vepr12_attach_aoac` | patch 44 on the Vepr-12 (plain and premium) | the magazine had no row | weapon-camo@2: a row, the magazine painted (the premium repaints it) |
| `12_cal_aoac` (Benelli M4, Lancaster, TOZ-34), `hk_mp7_attachment_aoac`, `hk416_barrel_aoac`, `hk416_rear_sight_aoac`, `kriss_vector_suppressor_aoac`, `mask_setka_aoac` (SV-98) | patch 44 | bare by design, and the tool wrote no `_aoac` for a texture with nothing painted | weapon-camo@2: a neutral `_aoac` (G 1, B 0.5, R 1): the bake leaves the surface as it is |
| `black_market_05_legs_camo_nmap`, `black_market_05_torso_camo{,_nmap,_smap}` | the stock baker, the Black Market 5 pants and jacket wearing a decal without a camouflage | stock 0.26g data gap (every other armour part has its layer) | pack missing-textures@1: the two normal maps from 0.31a2; missing-textures-1.json: the jacket's `_camo` and `_camo_smap` copies of the pants' (the same leather; 0.31a2's are green placeholders) |
| `weapons/vepr12/vepr_attach_nmap` | stock material weapons/vepr12_attach (the Vepr-12 magazine) | a typo: base patch 19 shipped `vepr12_attach_nmap` | missing-textures-1.json: pointed at `vepr12_attach_nmap` |
| `item/main_for_boxes_fresnel` | stock item/main_for_boxes_alpha (supply boxes) | 0.26 renamed it `main_for_boxes_fmap` (the boxes' other material has it) | missing-textures-1.json: pointed at `_fmap` |
| `terrain/debrisstone_bump` | stock tiled/trash/debrisstone_s and its v0.21d port (levels 5, 6, 7) | no build from 0.11 to 0.46 ever had it | missing-textures-1.json: `use_parallax` off |
| `fx/level_05_anomaly/uran_crystals/uran_smap` | stock fx/level_5_anomaly/uran_crystals (level 5) | 0.26's art pass named it, never shipped it (0.20e-0.23h's `diffuse_small_smap` belongs to the old diffuse) | missing-textures-1.json: `use_troughness` off (the material's constant roughness) |
| `flora_2/saksaul_tile_bump` | level_09's saxaul trees (base patch 14), parallax on | not in the port | pack missing-textures@1: 0.31a2's (its `saksaul_tile` the same layout) |
| `tiled/metal/metal_column_05_fmap` | the v0.11e and v0.21d maps' ported metal_column_05 (pipes, factories) | not in the ports | pack missing-textures@1: 0.46c2's (its `metal_column_05` the same layout) |
| `tiled/stucco/stucco_ikona` | stock church icon materials (level_01), vertex-blended | missing from 0.26g | pack missing-textures@1: 0.21d's (its `stucco_01` is 0.26g's byte for byte) |

## Listed, not changed

- **Decals without a camouflage on weapons**: the baker asks for `D_camo`, `D_camo_nmap` and
  `D_camo_smap` of every surface (about 115 textures; the armour's stock layer, which weapons do
  not have). Patch 47's fill leaves the camouflage layer empty then; the fix is in the exe (fill it
  with the surface's own textures), not content.
- **Not proven sampled** (no log ever showed them; forward, particle, water and terrain-blend
  parameters, no switch read): fx/artifacts caustic, flow_circular, force_wave2, halo and
  level_02 plasma1 (transparency), anomaly_stump_opacity (emissive), windows_02_trans,
  the stalactite particle's stalaktits and _nmap, eight water reflection probes and
  test/sky_00_cube, cubemap/refl_cloud, decal_stained_004 (variation mask), grass_03_bump
  (terrain height), vehicles_spec (tyre covers).
- **Legacy parameters**: `texture_specular_power` / `_intensity` names (editor/default_spec,
  ak74_pow, ...) on hundreds of stock materials; 0.26g's shaders do not read them (the battery's
  ak74_pow is in every battery match and in no log).

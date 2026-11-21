# London's forward effects: a proposal (not written, not decided)

The two Londons (0.32a4's and 0.44a5's) draw their fires, blowing sand and cloth with forward-stage
materials that use two switches 0.26g's executable has no field for:

| switch | 0.32a4's London | 0.44a5's London | what its fields say (the donor's code is not read yet) |
| --- | ---: | ---: | --- |
| `use_uv_tiling` | 12 materials | 2 (day) | the emissive and transparency maps tile by factors of their own (`emissive_map_tiling_u/v_factor`, `transparency_map_tiling_u/v_factor`) |
| `use_ttransparency_dxt1` | 18 (2 more in the lighting stage) | 2 (day) | by its name, the transparency map is a DXT1 texture; what the effect binds or reads differently is to be read |

Neither is a shader macro: the donors' `forward_base.ps` mask has no macro for them, and their keys
take the same values as 0.26g's but for three macros (`CONFIG_ANGLE_DEPENDANT_ALPHA_TEST`,
`CONFIG_USE_FUZZINESS`, `CONFIG_USE_OLTA`). They change what the effect binds and which constants
it sets. Today these materials are players' own where patch 14 shipped one of the name (its
stand-ins), and the donor's with the switches dropped where it did not: the conversion lists them
(`report.json`: `switches_not_in_026g`), it does not imitate them.

## What 0.26g lacks

Read off the three executables' forward effect (the strings each refers to) and off the programs
(resource-porter `examples/near-build-reads.rs`):

- 0.26g's forward programs read `uv_distortion_*`, `sequence_parameters`, `solid_transparency`,
  `attenuation_power`, `constant_tile_uv` (8 bytes). The donors' also read
  `uv_scrolling_parameters0` and `uv_scrolling_parameters1` (16 bytes each: the two maps' scroll
  and tiling), `attenuation_falloff_start_angle_cosine`, `attenuation_falloff_end_angle_cosine`,
  `attenuation_falloff_end_min_opacity`, `scene_slomo`, and `constant_tile_uv` as 16 bytes.
- 0.26g reads `use_uv_scrolling` in two effects (0x1DE4F2, 0x1E0C1B) and sets
  `uv_scrolling_parameters0` in the g-buffer's (0x1E0816, 0x1E10E2); its forward effect reads
  `use_angle_depended_attenuation` (0x1D90B0, 0x1DD7C9, 0x1DDB5A) and has no string for
  `use_uv_tiling` or `use_ttransparency_dxt1`.
- 0.32a4's forward effect reads all of them in one function (0x8041EC to 0x804DB6:
  `use_ttransparency_dxt1` 0x8041EC, `use_uv_scrolling` 0x804391, `use_uv_tiling` 0x8043DA,
  `use_angle_depended_attenuation` 0x804417, `uv_scrolling_parameters0` 0x804B2A,
  `transparency_map_tiling_u_factor` 0x804D9B); 0.44a5's the same at 0x4BCA2F to 0x4BD630.
- Patch 14 already made 0.26g key its forward programs by `CONFIG_USE_UV_SCROLLING`: `shaders/masks`
  is data, it added the macro to `forward_base.ps`'s mask and shipped 510 programs under the new
  keys. So the key side needs nothing more.

## The patch

One client patch on the forward effect's compile (`forward_default`), in the `.srpx` arena:

1. Read the donor's fields from the material where they are present: `use_uv_tiling`,
   `use_ttransparency_dxt1`, the four tiling factors, the scroll directions and speeds, the three
   attenuation falloff values. A material without them compiles as today.
2. Set the constants the donor's programs read, as the donor's effect packs them:
   `uv_scrolling_parameters0/1`, the three `attenuation_falloff_*`, `scene_slomo` (1 outside a
   slow-motion scene: 0.26g has none), `constant_tile_uv` as four floats.
3. With `use_ttransparency_dxt1`, bind the transparency map as the donor's effect does.

Then the donor's own forward programs can be supplied by the converter's gate as its g-buffer
programs are (`--programs`): every constant they read is one 0.26g sets. No shader is rewritten
and no material is tuned.

## Before writing it

- Read the donor's function (0.32a4 0x8041EC) for the exact packing of the two
  `uv_scrolling_parameters` and for what `use_ttransparency_dxt1` binds; the list above says which
  fields it reads, not how it combines them.
- Add the 0.26g forward effect's compile and its constant setters to the re-kit
  (`tools/re-kit/v026g`): the addresses above are string references, not reviewed names.
- Count the programs: the donors' `forward_base.ps` keys the two Londons' materials select, against
  the 510 players have.
- Prove the path on one material (a roof's blowing sand) in both engines at one view before the
  rest.

# 0.69d's normalized local probe tier in 0.26g (design, not built)

A client-patch design for ported 0.69d scenes, after the tester's Rudnya reports of 2026-11-23. Nothing
here is built. PROVEN = read at the instruction (or in `renderer-026g.md`, which says so); TO READ = an
assumption the patch rests on that has not been read yet.

## What differs

0.69d composes its probes in tiers (PROVEN, bytecode of `environment_probe.ps`,
`compose_{detail,local,global}_probe_lighting.ps`, `create_probe_mask.ps`; the probe stage
`environment_probe_stage::execute` 0x20050B200 runs three passes, each a `clear_to_zero`, the tier's
probes, its compose):

- each probe of a tier adds `L w` (its SH light times its falloff weight) and `w / 4` into two
  accumulators (additive);
- the tier's light is `sum(w L) / max(W, 1)`, times `sat(1 - W_above)` for the tiers below the first
  (`W_above` the summed weights of the tiers drawn before: detail, then local, then global);
- `layer` (+0x214 of the render properties, 0x20029A3C0) picks the tier; Rudnya: 7 global (layer 0: the
  map-wide boxes, the swamp, the outdoor box), 62 local (layer 1), no detail.

So inside a tier, overlapping probes are averaged by weight, and the tier below shows only where the
weights sum to less than 1.

0.26g draws every probe as one list, largest `radius` first, each lerped over the result so far by its
alpha (`SRC_ALPHA / INV_SRC_ALPHA`, 0x18A3D5; `renderer-026g.md` 5.3). Where two local probes meet with
their side-width bands, each is half transparent and the light under both (the map-wide sphere, the
global probes: outdoor light) shows through.

The size, from both builds' composition evaluated on the donor's 69 probes (band-0 light, 372 000
points in the local probes' volumes; `work-rudnya-b2/py/probes.py`):

- 2.0 % of the local volume is more than 1.25 times 0.69d's light where 0.69d's local weights sum to 1
  (median 1.42, 90th percentile 1.84): seams between abutting local probes (4 164 points) and a full
  probe's edge over another's band (2 799). 79 probe groups; the most: the tractor station's
  atp(4)/atp(5) and atp(2)/atp(3), houses 20/21, 25/26, 32/5, 1/6, the school's indoor 1/2 and 5/6, the
  church's 1/2 and 2/4.
- 0.9 % is below 0.8 times: the same order the other way, a smaller (drawn last) darker probe taking a
  larger one's place in full where 0.69d averages the two (one full probe over another's band 2 204,
  two full 369, seams 650).
- The church nave (view 05-church-inside-d of the reports page, the "pale line across the floor"):
  across the church(2)/church(4) seam 0.69d gives 83 -> 87-94 -> 100, 0.26g 83 -> 160-293 -> 100 (the
  global probe, 2 213, through a 0.2-0.3 m strip).

No converter rule can give an average from an ordered lerp; the patch draws 0.69d's tier.

## 0.26g's side (PROVEN unless marked)

- Order in `stage_ambient_lighting::execute` (0x1826F0): skylight (off), SSR mask, ambient volumes
  (multiply `accum_diffuse`), environment probes `render_environment_probes` (0x180980), ambient lights
  (0x181EB0), SSAO / height ambient. Everything after the probes multiplies or adds to what they leave.
- The probes write `$user$accum_diffuse` (rt 31) and `$user$accum_specular` (rt 34), both HDR:
  `R11G11B10_FLOAT` on feature level 10 and up (`create_targets` 0x162DA0). **No alpha channel**: the
  weight sum cannot ride in those targets.
- `$user$accum_ambient_lights` (rt 32, `R8G8B8A8_UNORM`) is cleared to 1 by the ambient-light pass that
  runs after the probes, so it is free while the probes draw: it can hold `W / 4` (0.69d's own encoding:
  `max(x, 0.25) * 4` gives the divisor to `W = 4` at 8 bits).
- Per probe: the light mask (stencil) on its volume, then techniques 0 and 1 of effect
  `stage+14h + 4 * (8 geometry + 4 smart + 2 blend + affect_diffuse)` with `light_range`,
  `inner_radius`, `outer/inner_cube_side_sizes`, `world_to_outer_probe` etc. (0x18137B..0x181609).
- `radius` (+320h) is, for a box probe, only the sort key (`sort_by_size_predicate` 0x1BA0C7); boxes cull
  by their transform. TO READ: that the loader (0x28F3B0) and `insert` (0x28FFD0) take a negative radius
  unchanged and that nothing else (the visibility query 0x1B8190) reads it for a box.

## The patch, for a 0.69d scene only

1. **Which probes are local.** The converter writes a local probe's `radius` negated (its magnitude the
   sort key it has now). A native map never writes a negative radius, and the sort (largest first) puts
   every local probe after every global one, which is the order the tiers need. A probe drawn with a
   negative radius in an unmarked scene is impossible: only the 0.69d converter writes them.
2. **When.** At the start of `render_environment_probes`, the scene's marker: the view's
   `mie_fog_base_height` (an exposure profile's, 33000 + level; the patch-170 gate log read it at view
   +0x238 block, `rudnya-069d.md`). Per frame, one compare; any other scene runs the native loop, byte
   for byte.
3. **The global tier**: the probes with a positive radius (the map-wide sphere first), native.
4. **The local weights**: each local probe once more into rt 32's red (blend `ONE / ONE`), its program
   writing `w / 4` (a marked copy of `environment_probe_lighting.ps`, the falloff it already computes,
   and 0.69d's box falloff instead of the smoothstep, `(1 - t)^1`).
5. **Make room**: one fill over `accum_diffuse` and `accum_specular` with `ZERO / SRC_COLOR` (the blend of
   `apply_ambient_lights` 0x182081) by `1 - min(W, 1)`, read from rt 32.
6. **The local light**: each local probe with `ONE / ONE` (the additive blend of list B, 0x187EBC), its
   program writing `L w / max(W, 1)` (diffuse and specular), `W` read from rt 32.
7. rt 32 is then cleared to 1 by the ambient-light pass as now.

Result per pixel: `G (1 - min(W, 1)) + sum(w L) / max(W, 1)`, 0.69d's local over global. The
multiplicative fill lights then multiply it as now (0.69d's ambient-light stage also runs after the
probe stage's composes: the pipeline builds `environment_probe_stage` before `ambient_light_stage`,
0x2004CDB69 / 0x2004CE56B, and the probe stage composes all three tiers inside its execute).

Specular: 0.69d's local tier composes the diffuse only (`environment_probe.ps` writes the SH light and
the weight); its reflections come by another path (not read). Drawing the specular by the same tier
rule is the closer choice; TO READ before the patch: 0.69d's specular composition.

## What it needs

- Exe: a lohook at the head of the per-probe loop of `render_environment_probes` (TO READ: the loop's
  shape between 0x180980 and the effect selection 0x180F38) that, for a marked scene, splits the sorted
  list at the first negative radius and runs steps 4 to 6 for the rest; three effects with the blend
  states above (the patch creates them like the native effects' compile, or reuses the state objects of
  `apply_ambient_lights` and list B: TO READ which is smaller); binding rt 32 as a texture for steps 5
  and 6 (patch 170's `engine_set_texture` row binds a render target by name at compile time: reused).
- Lighting pack: the marked programs for steps 4 (weight), 5 (fill) and 6 (normalized light), for each
  of the 32 `environment_probe_lighting.ps` variants the effects select (geometry 0/1 (+ cylinder, below),
  smart attenuation, blend, affect_diffuse, lod); `renderer-026g.md` 5.3 reads the lighting-model digit 0
  in all 16 effects at every shading quality, so there is one path. TO CHECK as the composition twins
  were (`______0` / `______1`): that no quality level (`r_lighting_quality`, `r_shading_quality`,
  `r_ambient_occlusion_quality`) compiles another probe permutation set.
- Converter: `layer` read (today ignored), a local probe's radius negated, and the probes' own 0.69d
  falloff power (`attenuation_power`, default 1: linear) in place of the smoothstep.
- MSAA: 0.26g's deferred path has none (the G-buffer targets are single-sampled, 1.1).

## Cylinder probes

0.69d's geometry 2 (Rudnya: the belfry church(8) and the tower church(9); Vostok 1, level_06 3, Duga 35)
is a box of the footprint's area in the port, off over 17-19 % of their volume. The same marked
programs can draw a cylinder in the box around it if the converter flags it: for a local probe, in the
fraction of its (negated) radius, or in `probe_side_width`'s sign (TO READ: which the loader keeps).

## Cost

Each local probe is drawn twice more than now (weight, light) and two full-screen fills; Rudnya's views
see 1 to 6 local probes. rt 32 already exists. Unmarked scenes: one compare a frame.

## What can go wrong

- A probe drawn by a path other than this loop (the lobby, the weather blend `texture_blend_to`, the
  `probes_blend_factor` between two cubes): the marked programs must keep the blend variant.
- The stencil light mask per probe is set up for the native draw; the extra draws need it again (or the
  depth-tested technique 0 alone; TO READ).
- rt 32's red holds at most `W = 4` at 1/64 steps (0.69d's own limit).
- Patch 170 (closed, #2345) bound tables at effect compile time for the fog and sky programs; its
  binding helper is the part reused, not its hooks.

## The proof it is judged by

The church nave seam (0.69d 83 -> 87-94 -> 100) and the 79 groups' seams by the model above, shot at the
tester's cameras against 0.69d's frames; the 0.9 % darker class by the same model (the smaller probe's
full place replaced by the average).

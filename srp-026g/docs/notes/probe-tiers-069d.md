# 0.69d's local probe tier in 0.26g (exe patch 203)

The tester's Rudnya (v0.69) reports of 2026-11-23: in the church nave "there is still strange light
line in the middle" (05-church-inside-d), "here also seen on seams" (-e, -f). The owner's rule for
ports: what the donor draws is drawn by its own technique, by rule, never by place. PROVEN = read at
the instruction or in the data named; INFERRED = not read, the nearest choice.

## What differs

0.69d composes its probes in tiers (PROVEN, bytecode of `environment_probe.ps`,
`compose_{detail,local,global}_probe_lighting.ps`, `create_probe_mask.ps`; the probe stage
`environment_probe_stage::execute` 0x20050B200 runs three passes, each a `clear_to_zero`, the tier's
probes, its compose): each probe of a tier adds `L w` and `w / 4` into two accumulators; the tier's
light is `sum(w L) / max(W, 1)`, times `sat(1 - W_above)` for the tiers below the first; `layer`
(+0x214 of the render properties, 0x20029A3C0) picks the tier. Rudnya: 7 global (layer 0), 62 local
(layer 1: 60 boxes, the two cylinders church(8) and church(9)), no detail; every local probe's
`attenuation_power` 1 (linear falloff).

0.26g draws every probe as one list, largest `radius` first, each lerped over the result so far by
its alpha (`SRC_ALPHA / INV_SRC_ALPHA`, compiled in the four box effects at +0DFh and +310h;
`renderer-026g.md` 5.3). Where two local probes meet with their falloff bands, each is half
transparent and the light under both (the global probes: outdoor light) shows through. Across the
church(2)/church(4) seam 0.69d gives 83 -> 87-94 -> 100, 0.26g 83 -> 160-293 -> 100. Over Rudnya's
local probe volume, 2.0 % is more than 1.25 times 0.69d's light and 0.9 % less than 0.8 times
(79 probe groups: the church, the tractor station, houses, the school). No converter rule can make
an average out of an ordered lerp.

## The marks

- **A 0.69d scene**: its environment's `mie_fog_base_height` is an exposure profile's marker,
  33000 + the 0.69d level (Rudnya 33001; School 33002, Vostok 33003; the lobby room 33100), read at
  scene_view+280h (environment +168h, load_environment_properties_impl 0x34ED38; the frame's
  `atmosphere_parameters3.w`, 0x14A00E, where the lighting pack's programs read the same marker).
  PROVEN; every project of the live content and of this content checked: only the six
  `v069d_level_01` projects carry a marker from 33000 to 33999.
- **A local probe**: the 0.69d converter writes its radius below 0, `-(shape + 0.25 + 0.5 / (1 + R))`
  (shape 1 a box, 2 a cylinder about its local y in its circumscribed box; R the sort radius
  0.26g sorted it by before). A probe's radius is, for a box, only the sort key (PROVEN: the loader
  0x28F42C stores it as read, insert 0x28FFD0 makes the inner box from the transform, set_properties
  0x1C36F0 adds the tie-breaker +380h, 0 for the first 4096 probes, and makes a box's collision and
  AABB from the transform; the culling query 0x1B8190 reads the AABB; the sorted list
  scene_view+16CF4h feeds only render_environment_probes). Elsewhere: render_forward_lighting
  (0x192D10) gives a forward model the containing probe with the smallest +324h (the fraction keeps
  that choice among local probes) and sends max(radius, 0) as `probe_parameters0.w`, which no
  `forward_probe_lighting.ps` (225 stock, 210 of v021d-forward-fog@5) or
  `offscreen_particle_lighting.ps` (2448 stock, 2448 of forward-effects@1) program reads (their RDEF
  marks it unused); the particles' probe is the first of the insertion-ordered list whose
  calc_attenuation (0x1C3150) is below 1, which for a box does not read the radius.
- **Not every negative radius is the converter's**: stock `level_07` has a box probe
  `indoor(28)` of radius -0.4 in 18 projects (day, rain, evening, every mode). Its scene has no
  0.69d marker, so it stays 0.26g's draw. That is why the gate is both marks.

## The patch

Rust `probe_tiers` (`rust/patches/src/probe_tiers.rs`, `vostok/probe_tiers.rs`; 31 lohooks of
`sites_026g.tsv`).

**The compiles.** effect_environment_probe_lighting<1,0,b,a>::compile (0x18A720 <1,0,0,0>,
0x18B770 <1,0,1,0>, 0x18C7C0 <1,0,0,1>, 0x18D810 <1,0,1,1>: the box effects without smart
attenuation; one code at the same offsets but for their configuration bytes) each compile their two
techniques three more times, as patch 178's twins: at the epilogue after `pop edi` (+406h,
`.compiled`) the compile goes back to technique 0's start (+0Eh) until three rounds are done, so the
effect holds techniques 0/1 (its own), 2/3 (weight), 4/5 (damp), 6/7 (light). In a round:
`.begin_pass0` / `.begin_pass1` (+83h, +262h) name the round's pixel shader
(`environment_probe_tier_weight`, `_damp`, `_light`; the VS `light` and the configuration stay,
so the key is the stock family's); `.blend0` / `.blend1` (+0E4h, +315h, after set_alpha_blend, only
when not caching) set the colour blend of all eight records (weight ONE/ONE reverse subtract, damp
ZERO/SRC_ALPHA add, light ONE/ONE add; alpha as set, the write mask RGB as set);
`.texture0` / `.texture1` (+142h, +33Dh, where the caching and building ways meet) bind
`t_probe_weights` to `$user$accum_ambient_lights` (rt 32). Every compile makes them, on every map:
techniques 0 and 1 are compiled by the same instructions as before, and nothing selects 2 to 7
without the two marks.

**The draw.** stage_ambient_lighting::render_environment_probes (0x180980) binds accum_diffuse
(rt 31) and accum_specular (rt 34) and loops over the sorted probes: per probe the light mask, the
constants, then the two passes of its effect (stage+14h + 4 x index), each `[effect+5620h]` = the pass
(0x181109). The patch's state is one enum, `Native | Tiers { tier, first, targets }`, with one
transition function over two events:

- `env_probes.next` (0x180B94, the loop's head): a probe whose radius is below 0 in a scene with a
  0.69d marker, met in Native, starts the tiers: the loop's four render-target slots kept, the
  stage's own clear_surfaces (0x180820: rt 32 to 1, rt 33 to 0), rt 32 bound alone. The sort puts
  every local probe after every global one, so the globals have been drawn natively. A probe not
  local met in Tiers stops the game (the sort's order broken).
- `env_probes.technique` (0x181109): in Tiers, the pass's technique is 2 + 2 x tier + pass. A local
  probe on an effect without the rounds (smart attenuation, a sphere) stops the game with the
  probe, its number, its radius bits and the effect's index (the converter refuses those).
- `env_probes.done` (0x181639, after a probe's passes): at the list's last probe, Weight -> Damp
  (the loop's targets bound again, the loop back to the first local probe through `again`
  0x180B90), Damp -> Light (back again), Light -> Native (clear_surfaces again: rt 32 back to 1 for
  the ambient lights' list A, rt 33 0; the loop's targets bound again).

Per pixel the result is `G (1 - min(W, 1)) + sum_i w_i L_i / max(W, 1)`, G the global probes' light:
the damp pass multiplies by `(1 - min(W,1))^(w_i/W)` per probe, whose product over the probes at a
pixel is `1 - min(W, 1)`, in any order; all damps come before all lights. 0.69d's local tier over its
global one. The ambient volumes (drawn before the probes) and the ambient lights (after) act on it
as they did on the lerp.

**The programs** (three-maps-021-lighting@16, `environment_probe_tier_*`, the stock family's mask
and its 8 box keys: GLOBAL_LIGHTING_QUALITY 0 and 3, affect_diffuse, blend). The stock box program
is reconstructed as HLSL (`../../shaders/sm_4_0/environment_probe_lighting.ps`, `probes.sh check`:
6 of the 8 byte-identical with the shipped DXBC, the other two the same instructions but one mul's
operands, the 0.25 of the specular taken after the SSAO factor). The tiers compile from it:

- weight: `w` is 0.69d's falloff (`resource-porter` `Volume::weight`, `environment_probe.ps`): per
  axis linear from the inner face `max(half - side width, 0.01)` to the outer one, a cylinder's
  distance from its axis for x and z; the side width and the shape decoded from `light_range`
  (0.98 radius) and `inner_radius` (radius - side width - 0.05), which the exe sends as for any
  probe; out: `w / 4` subtracted from rt 32 (R8G8B8A8_UNORM, W up to 4 in 1/255 x 4 steps, 0.69d's
  own encoding).
- damp: `W = 4 (1 - rt32)` (never below the probe's own w), out alpha `(1 - min(W, 1))^(w/W)`.
- light: the stock program's diffuse and specular outputs times `w / max(W, 1)`; nothing else
  changes: the normal's decode, the diffuse by the normal, the specular by the reflection vector,
  the BRDF, SSAO and the blend_to cubes are the stock program's instructions.

Specular follows the same tier rule as the diffuse (INFERRED: 0.69d's local tier composes the SH
light; its reflections' composition was not read).

## What every frame of every map runs

- Three lohooks of render_environment_probes, per probe of the sorted list: `next` reads the
  probe's radius and the scene's marker and, without both marks, returns with the state Native
  unchanged; `technique` and `done` read the state (Native) and return. No render target, no
  technique, no constant is written in Native. The kept slots exist only inside `Tiers`.
- The four box effects' compiles: six more techniques each, at load; techniques 0 and 1 are compiled
  by the game's own instructions with the game's own arguments (the hooks write only in a round).
- The exe bytes the patch changes outside the arena: the 31 site spans, each a `jmp` to its entry
  (5 bytes) and nops to the span's end; the entry saves the machine, runs the handler, restores it
  and replays the span's own instructions: 0x180B94 (10), 0x181109 (6), 0x181639 (8), and per
  compile +83h (7), +0E4h (11), +142h (7), +262h (9), +315h (7), +33Dh (5), +406h (5) at 0x18A720,
  0x18B770, 0x18C7C0, 0x18D810. In Native and outside a round every handler returns `Continue`.

## Other maps: dead code

- **The content.** Two quick content builds of revision 72's manifest, one with main's
  rudnya-069d@7 and three-maps-021-lighting@14, one with this change's map and lighting packs,
  each mounted over the live patches 12 to 71 (`srp-game-data files`, every file as the game
  mounts it, 227 446): 31 files differ, the six `projects/v069d_level_01/*/client_project`, the 24
  new `environment_probe_tier_{weight,damp,light}.ps` programs and `shaders/masks`. The masks'
  diff is 18 added leaves, the three new families' rows (the stock probe family's six macros
  each); every existing family's row is the same. A family's key is made from its own row (looked
  up by its name), so a row added for a new family selects nothing for an existing one, as
  composition_v044a.ps's row did (patch 178).
- **The data.** Every project of both contents (278 with probes) read for a probe radius below 0:
  stock level_07's `indoor(28)` (-0.4, 18 projects) and Rudnya's six projects (62 each); and for
  the scene marker: only Rudnya's six projects carry 33000 to 33999 (33001).
- **What an unmarked scene executes.** See "What every frame of every map runs" above: three
  pass-through handlers per probe, six more techniques compiled per box effect at load, none of
  them selected.

## Checked

Rudnya only (patch 176's capture, the tester's cameras, vertical fov 70 at 1280x1024; before: this tree
without patch 203 with main's map plus the sun colour fix and lighting@14; after: patch 203, the
map's local probes marked, the tier programs). The seam's ridge (median display luma on the line
the seam draws, and 20-30 px either side), 0.69d / before / after:

    05-church-inside-d   49 between 43 and 45 / 97 between 41 and 43 / 46 between 41 and 43
    05-church-inside-e   50 between 40 and 46 / 79 between 37 and 45 / 48 between 39 and 45
    05-church-inside-f   49 between 44 and 45 / 80 between 53 and 53 / 48 between 44 and 47

Pixels more than 25 levels brighter before than after: church c2 to j and the station's a2, h and k
(d 33 092 -> 0, e 10 157 -> 21, f 15 292 -> 607, g 25 136 -> 196, i 17 092 -> 248, station a2 1 737 ->
522, h 604 -> 32). The first batch's seven views: mean |change| 0.5-1.6 levels, largest 160x128
tile 0.3-2.3. The 112 kept poses: one tile moved more than 12 levels (charger_216_0, nearer 0.69d,
10.3 -> 8.6); mean tile distance to 0.69d 12.88 -> 12.82; 7 poses nearer, 2 farther (at most 0.5).
At r_lighting_quality 0 and r_shading_quality 0 (the 0 program set) the same: d 49 / 98 / 47, e 49 / 79 / 48,
f 48 / 79 / 49 on the seam, the sides as above.
Left: the belfry's opening (05-church-inside-f) keeps a soft rim, the global tier's light in the gap
between the church's environment_probe and church(8), where 0.69d's own weights are 0 too (W = 0 at
y 30.0-30.2 by the model): 0.69d's global tier there is darker than 0.26g's lerped one (not this
patch).

## Limits

- rt 32 holds W/4 in 8 bits: W to 4 at 1/64 steps (0.69d's own).
- A 0.69d scene's sphere or smart-attenuation local probe, or a local power other than 1, is refused by
  the converter; the exe stops the game on one (no silent fallback).
- The global tier stays 0.26g's lerp (0.69d averages its global probes too; not in this patch).
- The lobby room (marker 33100): its builder places its probes as 0.26g draws them (no negative
  radius), so it draws as before.

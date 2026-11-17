# Rudnya v0.21

Rudnya's 0.21d day projects are maps 260 (Protective device) and 261 (Team
Deathmatch), map number 29. They require the wide-map-ID change, exe patch 154
(PR #2126). The donor bog, central terrain anomaly and team 0 base are retained;
the base is 27 m from native Rudnya's, and its ammo crate follows that move.

The owner chose the 0.21 look, original colours and intensities, and shared
shader/engine compatibility rather than local lighting calibration. `maps-021d-rudnya@4` retains donor geometry, local lights, probe
volumes and baked cubemaps with native material schemas. The porter binds each
weather cubemap to every mode alias, including `day_4`, so native template
textures cannot survive on TDM. Native School supplies a missing plain-light
schema only: its objects and placements are not copied into Rudnya.

The reproducible pack recipe is resource-porter `tools/content/rudnya_021d.sh`,
branch `port/rudnya-v021`. Its third argument is the built School/Fort pack.
Version 4 requires `maps-021d-school-fort@4`: 744 common files are byte-identical
and supplied once. Two van materials also use that pack's native Fresnel map;
`rudnya_shared_finishes.json` pins both old/new hashes and the reason. Their
only decoded difference is the reflection texture binding, previously rebound
to the donor texture by Rudnya's dependency scan. The helper refuses every
unreviewed difference before modifying the pack.

`release/content.tsv` pins the pack and applies:

- `v021d-rudnya-environment-1.json`: selects the original shader path,
  converts cloud coordinates/defaults, and restores omitted height-ambient
  defaults to black/white, as initialized by the original engine. The port
  previously inherited native blue/purple values. Authored sun intensity 3,
  fog, probe multipliers and blueshift remain unchanged.
- `three-maps-021-lighting@4`: shared guarded shader adapters, described in
  the scope inventory.

The room-specific `v021d-rudnya-lighting-1.json` overrides were removed in
favor of the shared engine compatibility rule. Every ambient light in these
projects uses the original multiplicative semantics even above intensity 1.
The tractor-station fill retains authored intensity 2.7 and RGB 216/229/255;
the northern corridor retains intensity 1 and RGB 76/76/76. No individual
light is retinted or retuned. See [the scope inventory](three-maps-lighting-scope.md)
for the engine evidence, remaining asset-specific exceptions and validation
status. Ambient-light packed colours are ABGR, with red in the low byte;
`object_ambient_light::insert` at 0x290930 confirms the extraction and gamma
conversion, recorded in the re-kit ABI inputs and `renderer-026g.md`.

## Appearance review

The rig reuses all 100 saved donor viewpoints and captures only the port,
one pose at a time. Each substantive flag is inspected before advancing.
The report includes automatic flags and notes for retained differences.
The owner approved the current @3 hall, northern corridor and church
comparisons as the desired standard on 2026-11-17: a close match with stock
0.21 values and minimal rendering changes, not pixel identity. All three
interior views and the exterior horizon have no serious comparison flags.
The rest of the sweep remains pending; prior calibration shots are historical.
Captures use a
headless display, zero game volumes and null Wine/OpenAL/ALSA audio; the
running client is checked for PipeWire output streams.

Local report path:

`/home/sheep/Projects/survarium/work-rudnya-v021/rig/reports/step/rudnya_day/index.html`

The owner's final appearance review is pending. These local quick builds use
the wide-ID test executable and do not set or reserve a player release revision.

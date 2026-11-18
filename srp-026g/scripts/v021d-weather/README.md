# The 0.21 maps' weathers

Rain and evening for Rudnya, School, Vostok Radar, Chemical Plant,
Tarakanovsky Fort and Mamayev Kurgan. Each has Protective device (mode 2) and Team Deathmatch
(mode 4). Resource-porter commit `a9d8e64` on `port/v021d-five-weathers` made
`maps-021d-weathers@2` (five maps); Chemical Plant's four projects are
`maps-021d-chemical-weathers@1`, by the same builder and script with
`WEATHER_MANIFEST=ports/survarium-021d-chemical-weathers-to-026g.json` on
`port/v021d-chemical`, which also makes `three-maps-021-lighting@6` (eighteen
profiles). The five maps' pack is not rebuilt: the importer has since kept
alpha mip chains and visibility in private copies, which the edit lists below
retrofit to it; Chemical Plant's files are made that way from the start.

| Map | Rain 2 / 4 | Evening 2 / 4 |
| --- | --- | --- |
| Rudnya | 262 / 263 | 264 / 265 |
| School | 1 / 2 | 3 / 4 |
| Vostok Radar | 170 / 171 | 172 / 173 |
| Chemical Plant | 268 / 269 | 270 / 271 |
| Tarakanovsky Fort | 5 / 6 | 7 / 8 |
| Mamayev Kurgan | 179 / 180 | 253 / 254 |

The weather builder replaces only the environment, local lighting, probes
and weather-layer client objects of the **authored day source**. The day
source includes the ordered content edits: gameplay, anomalies, existing lamp fixtures and Keeper pools are retained.
The source geometry edit restores the original weather-only deltas: School
rain and Chemical Plant rain each add two hanging lamp assemblies (six donor visuals, four collisions,
two broken-state visuals and two breakable fixtures); Fort rain/evening each
omit three barrel groups and their six collisions. Existing lamp indices
follow those removals; day variants are unchanged. These geometry differences
come from the original donor variants, not new layout design. Its options merge
the native weather's identity and loading screen with current day gameplay
and music. Conflicting edits are refused. Original donor lighting is kept;
the existing original ambient/particle-intensity engine guard now includes
these exact rain/evening project aliases and still rejects unrelated scenes. Rudnya's
IDs require the already shipped wide map-ID support.

`source_projects.py` composes `release/content.tsv` through build-content's
own implementation, excluding the weather pack and weather-only edit lists.
Its temporary view of `modes.tsv` excludes only those twenty weather rows,
and subtracts the excluded pack’s twenty map rows from the existing
append-count guards, without rewriting other edit sources. Thus adding
the weather pack does not create a regeneration cycle. The export
records each resource's checksum and the content manifest checksum. The
porter verifies the exported bytes before reading them. Never use a server
game-data database or a previous release's extracted projects as this source.

Run the exporter with its `--help` arguments, then resource-porter's
`tools/content/five_weathers_021d.sh BUILD SKHID_CHECKOUT SOURCE_EXPORT`.
The importer and lighting example must be built from the same porter tree.
The script uses the manifest's published base patches, the pinned retail
0.21 donor and stock 0.26 archives. Weather probes have separate resource
paths to preserve the day maps' existing probe metadata. The combined
lighting pack contains all five day and ten weather profiles; the weather
asset pack depends on it and writes no shared shaders.

Copy the two generated pack directories to `SKHID_PACKS_DIR`, pin their
`pack.json` checksums in `release/content.tsv`, and regenerate the twenty
weather rows of `modes.tsv` with `content_edit.py --dump-stats modes` over
the weather pack's `files` directory. `tools/check v021d-weather` rebuilds
the authored day source and checks preservation, registration, fixture
bindings and all fifteen shader markers. It also checks every weather entry
through the server’s actual `MatchMap::of(MapId)` parser over composed server
data, including Rudnya IDs 262–265. This needs current main’s shipped wide
map-ID support: `MapId` is `u16`; the legacy option byte carries its low byte
with the whole ID in srp’s extension. Never truncate an ID to a byte when
combining this work with an older branch. `tools/check` also composes the
candidate through the ordinary quick content build.

The weather environment edit makes omitted original height-ambient defaults
explicit (black low, white high), using the same constructor evidence as day
ports. Regenerate it with `make_environment.py WEATHER_PACK OUTPUT_JSON`.

Static validation does not establish visual parity. Rendering retains the
same documented native portions as the day ports: probe shading; box-edge
ambient saturation; unsupported tone, AO, sun, particle and water variants;
foliage lighting; and native scheduling, bindings and shadow-map generation.
See `docs/notes/three-maps-lighting-scope.md` for the complete pass inventory.

**Required visual acceptance is in progress, not complete.** The map-port
skill requires generated regional sweeps against original donor captures,
followed by investigation and correction of serious findings. Static/source
checks do not replace this requirement or waive inherited rendering defects.
The current capture plan and evidence live under
`private/v021d-five-weathers/visual`; `validation.json` in its parent records
paired coverage separately from port-only inspection. Vostok evening has 110
kept original views under `donor-refs/021d`. Frozen Vostok rain and Mamayev
rain/evening reference archives are missing; replacement captures require an
owner exception to their existing freeze. First-time Rudnya, School and Fort
weather references use approved static input-free headless capture. Preserve
all kept donor images and reshoot only affected port views after a correction.
No visible game run, release or deployment is authorized by this check.

The native guard harness passed 600 ambient and 2,496 particle cases, including
unrelated maps/scenes, exact alias boundaries, and load/teardown transitions.
The independent DXBC checks preserve every executable instruction for all five
existing day profiles and the native fallback in all 88 resources. Five rain
profiles additionally bypass the native rain albedo attenuation that 0.21 did
not have. Sun and rain guards reuse a scratch register whose live ranges do
not overlap. The real-retail direct-sun regression compares the original sun
block and every native fallback instruction, including both rain-enabled and
day-only compositions.

`make_geometry.py` regenerates the layout edit from the original donor
projects and the weather porter's import bundle; `geometry-inputs.json` pins
every input used. The four required converted inputs can be kept as a minimal
`geometry-import` export; the full temporary import bundle is unnecessary. Its fixture metadata comes from the same `author` function
as day lamps. Both School rain modes have ten breakable fixtures (the
existing eight plus two); Fort keeps its fixtures with corrected indices.

The final check also scans all twenty projects’ recursive asset references
over stock, published patches and the canonical candidate patch. It uses the
weather porter’s layered `scan` command (`SKHID_WEATHER_PORTER` can name its
binary; the local default is the build under `private/v021d-five-weathers`).

The content type allowlist preserves original numeric representations only at
78 reviewed weather paths. Native environment reads test type 2 and load a
float, or convert an integer with `cvtsi2ss`; the re-kit records their exact
sites. Two legacy keys have no 0.26 reader. Rudnya retains the same fractional
swamp sensor as its day source. No value is rounded to satisfy a type check.

## Original material visibility

The required visual sweeps found two concrete template-conversion errors:
School's basement showed backfaces absent in the original, and Rudnya's broken
tractor windshield became opaque. An audit of all fifteen original weather
projects (912 models, 618 materials) found 52 differing face-culling,
alpha-test/blend and shadow flags in 46 materials. The porter now preserves
these original selectors before native deduplication; unsupported native
schema fields reject conversion.

`material-visibility-inputs.json` records the original and converted flags,
every affected model binding, and hashes of all unchanged material leaves.
The original `vehicle/interior_fwd` soft-edge switch is also restored: the
native template had disabled it on UAZ/Niva glass. Five targeted captures
confirmed that restoring this original switch alone does not fix the pale
glass. The green vehicle in the `in_13_out` diagnostic is also a UAZ; it is
not evidence about an AZLK material.
`make_material_visibility.py` authors `v021d-material-visibility-1.json` to
retrofit the existing packs. Eight shared materials receive private copies;
the other materials were already private. Native maps retain their archived
materials, and the reviewed day-port native finishes and textures stay intact.
This source correction avoids rebuilding unrelated legacy content while the
importer fix prevents recurrence in future imports.

The registered weather check reads the original archive through the local
`private/v021d-five-weathers/original-resources.db` symlink and verifies the
composed materials, exact original flags, private model bindings, unchanged
other material leaves and absence of shared native material edits. Corrected
visual comparisons are **in progress**, with baseline frames retained; source
validation alone does not establish visual acceptance.

Restoring Rudnya's `terrain_tiled/dirt_vc` alpha flag requests shader paths
absent from stock. `v021d-material-visibility@3` covers the complete shipped
feature family: 28 paths across seven native vertex classes, two quality
variants and two native aliases. The builder enumerates and checks the
family; it does not wait for one missing resource per native launch.
No existing shader is replaced.

For quality `0`, native parallax-enabled/disabled programs are byte-identical,
so the corresponding existing native alpha program is reused. For quality
`3`, classes 1 and 2 retain every instruction except their mask cutoff:
`mask - .4` becomes the shipped native alpha expression
`mask * diffuse_alpha - alpha_ref`. Other classes have a compile-time zero
vertex mask; their unconditional discard becomes the existing alpha neighbor's
`-alpha_ref < 0` comparison, using one fresh temporary. This preserves each
native input layout's behavior, including unsupported combinations; it does
not invent vertex data. Reflection marks only the existing alpha constant used.

`visibility-shader-inputs.json` pins all native source programs and neighbors.
`check_visibility_shaders.py` independently enumerates the family, verifies
composed files, native isolation, unchanged instructions and bindings, and
exactly these cutoff/reflection deltas. The normal importer emits the additions
when it imports the affected original material and accepts already-installed
byte-identical additions. Runtime acceptance of the complete closure remains
pending the next composed candidate.

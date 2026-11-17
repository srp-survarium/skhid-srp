# Five 0.21 map weathers

Rain and evening for Rudnya, School, Vostok Radar, Tarakanovsky Fort and
Mamayev Kurgan. Each has Protective device (mode 2) and Team Deathmatch
(mode 4). Resource-porter commit `a9d8e64` on `port/v021d-five-weathers` makes
`maps-021d-weathers@2` and `three-maps-021-lighting@5` together.

| Map | Rain 2 / 4 | Evening 2 / 4 |
| --- | --- | --- |
| Rudnya | 262 / 263 | 264 / 265 |
| School | 1 / 2 | 3 / 4 |
| Vostok Radar | 170 / 171 | 172 / 173 |
| Tarakanovsky Fort | 5 / 6 | 7 / 8 |
| Mamayev Kurgan | 179 / 180 | 253 / 254 |

The weather builder replaces only the environment, local lighting, probes
and weather-layer client objects of the **authored day source**. The day
source includes the ordered content edits: gameplay, anomalies, existing lamp fixtures and Keeper pools are retained.
The source geometry edit restores the original weather-only deltas: School
rain adds two hanging lamp assemblies (six donor visuals, four collisions,
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
See `docs/notes/three-maps-lighting-scope.md` for the complete pass inventory. Cached donor references
exist under `donor-refs/021d`; no donor recapture, visible game run, release
or deployment is part of this source change.

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

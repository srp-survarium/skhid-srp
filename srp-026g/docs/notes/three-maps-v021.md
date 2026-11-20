# School, Fort and Rudnya v0.21 together

The review candidate combines the three map branches with main and the wide
map-ID implementation (#2126). School/Fort are pack `maps-021d-school-fort@4`;
Rudnya is `maps-021d-rudnya@4`, which requires the former for shared resources.
The manifest order, Keeper lists, lobby map selectors, dictionary count guards,
localization (including Croatian), music and map assets include all three.

The shared lighting compatibility also covers the existing Vostok and Mamayev
day ports, including their three mode aliases. Their native substitute lights
and probes are replaced with donor objects through the shared porter conversion;
their geometry and gameplay objects remain. The historical shader-pack name
`three-maps-021-lighting` now covers five maps at version 4.

The appearance target is the original 0.21 geometry, materials, local lights,
probes, and authored environment colours and intensities. The rendering
compatibility candidate adapts the original shader instructions to native
bindings and restores the original ambient-light rule. It removes the room
intensity overrides and the later whole-map artistic calibrations. Remaining
asset exceptions and native rendering passes are explicit in
[the scope inventory](three-maps-lighting-scope.md). Source recipes are on
resource-porter branches `port/v021d-school-fort`, `port/rudnya-v021` and
`port/v021d-three-lighting`.

The complete local review starts at:

`/home/sheep/Projects/survarium/skhid-srp/private/v021d-review/index.html`

Captures reuse saved donor views, run headlessly and silently, and record local
worlds under `private/world-corpus/local/v021d-<map>_day/`. On 2026-11-17 the owner
accepted the shown corrected comparisons and requested a release. The final
particle candidate has partial fresh sweep coverage, recorded below; approval
does not turn earlier captures into validation of that correction.
No player release or revision increment is part of this
change; local builds are quick builds that the release script refuses to ship.

## Rendering compatibility refinement

The engine candidate and the precise scope of every appearance change are
listed in [the lighting scope inventory](three-maps-lighting-scope.md). It
removes the two room-specific ambient edit lists. The results under
"Validation of the combined tree" describe the preceding candidate.

The owner-approved final lighting candidate has 99/103 Mamayev views reviewed
(two buried/intersecting cameras excluded from lighting-fidelity conclusions),
49/91 Fort views and 19/117 Vostok views. School and Rudnya retain their earlier
captures, explicitly labelled as preceding the particle-intensity correction.
The two native School controls pass without flags. The gallery and per-pose
notes retain differences from cloud/fire animation, mode effects, and the
native rendering passes listed in the scope inventory. This is partial fresh
coverage, not a claim of a completed five-map sweep.

The final full-corpus check covered 48,787 worlds: 47,906 passed and 881
were refused, with none unchecked, unread or skipped. All 44,117 live worlds
and all 3,098 worlds from this work passed (Fort 506, Mamayev 450, Rudnya 940,
School 846, Vostok 144, native controls 56, owner preview 156). The refusals
belong to other local test sets: 802 earlier spawn-count mismatches, 15 earlier
Training Camp player-ID failures, and 64 Training Ground anomaly-test worlds
with object event-mask/slot failures. The whole-corpus check remains red;
these are not hidden or bypassed by weakening the parser.

## Validation of the combined tree

School and Fort each have 91 checked donor comparisons; Rudnya has 100. The
reports retain explicit rendering differences and automated colour/exposure
flags. Two supplementary port-only close-ups inspect the shared van
materials. The final Rudnya capture used the combined executable and content.

The local patch and server data built successfully, with archive read-back
identical. Content-edit, localization, the three executable generators and
20 live/replay map-ID cases plus all six native readers passed. The combined
Rust checks passed lobby-server, mapdata, match-server, srp-validate and
web-server. Three previously seen fixtures still fail: session's `shop_owned`
migration, SRP's PvB map-row count, and survarium's badge price. The capture-bot
startup fix passed Clippy with warnings denied and joined map 260 using the
configured game data.

The full world corpus checked 45,908 worlds: 45,091 passed, 817 refused, zero
unchecked/unread/skipped. All 283 new worlds passed (School 16, Fort 16,
Rudnya 251, including 53 from the final combined build). Historical refusals
are 802 spawn-count mismatches and 15 invalid player-ID worlds. Validation
rules remain unchanged. The earlier broader School/Fort run also reported
the Calibri infinity-glyph harness failure and an existing Vostok layout
mismatch; those unrelated checks were not rerun.

The native/server agreement check passed all 2,719 sampled worlds (all 1,791 local uploads plus 928 live samples), including their deliberately damaged copies: 48,942 checks, zero disagreements.

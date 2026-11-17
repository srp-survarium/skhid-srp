# School, Fort and Rudnya v0.21 together

The review candidate combines the three map branches with main and the wide
map-ID implementation (#2126). School/Fort are pack `maps-021d-school-fort@4`;
Rudnya is `maps-021d-rudnya@4`, which requires the former for shared resources.
The manifest order, Keeper lists, lobby map selectors, dictionary count guards,
localization (including Croatian), music and map assets include all three.

The appearance target is the original 0.21 geometry, materials, local lights
and probes, with 0.26 sky, atmosphere and sun rendering allowed. Local intensity
corrections and retained differences are documented in `school-fort-v021.md`
and `rudnya-v021.md`. Source recipes are on resource-porter branches
`port/v021d-school-fort` and `port/rudnya-v021`.

The complete local review starts at:

`/home/sheep/Projects/survarium/skhid-srp/private/v021d-review/index.html`

Captures reuse saved donor views, run headlessly and silently, and record local
worlds under `private/world-corpus/local/v021d-<map>_day/`. Final owner appearance
review remains pending. No player release or revision increment is part of this
change; local builds are quick builds that the release script refuses to ship.

## Validation of the combined tree

School and Fort each have 91 checked donor comparisons; Rudnya has 100. The
report retains 2, 1 and 10 explicit differences respectively, plus automated
colour flags. Two supplementary port-only close-ups inspect the shared van
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

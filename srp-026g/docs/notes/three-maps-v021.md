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

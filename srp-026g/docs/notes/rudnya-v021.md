# Rudnya v0.21

Rudnya's 0.21d day projects are maps 260 (Protective device) and 261 (Team
Deathmatch), map number 29. They require the wide-map-ID change, exe patch 154
(PR #2126). The donor bog, central terrain anomaly and team 0 base are retained;
the base is 27 m from native Rudnya's, and its ammo crate follows that move.

The owner chose the 0.21 look on 2026-11-17, allowing 0.26 sky, atmosphere and
sun rendering. `maps-021d-rudnya@4` retains donor geometry, local lights, probe
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

- `v021d-rudnya-environment-1.json`: native sky, atmosphere, distance fog and sun
  shading; donor sun angles, height ambient and local lights remain.
- `v021d-rudnya-lighting-1.json`: the tractor-station ambient light at
  (-48.76, 25.447, -84.614) retains its position, radius and colour, with
  intensity 1.1 instead of 2.7. The corridor comparison changed from luma
  50.3 to 14.5, against the donor's 20.5, without comparison flags. Native
  intensity 1 was too dark (9.2). The remaining distant haze differs with
  the chosen native atmosphere. Both day mode copies receive the correction.
  The northern corridor ambient_light(58), at (-46.635, 26.084, -102.738),
  takes intensity 1.5 instead of 1: its floor recovers detail, with luma
  30.3 vs the donor 27.3 (previously 12.3). Its position, radius and colour
  remain authored; its reach covers this view and the later team 1 spawn view.

## Appearance review

The rig reuses all 100 saved donor viewpoints and captures only the port,
one pose at a time. Each substantive flag is inspected before advancing.
The report includes automatic flags and notes for retained differences,
including cooler native sky fill and darker sun shadows. Church panels and
ceiling, and the team 0 base interior, remain visibly darker/cooler than the
donor; they are explicit owner-review items, not exact appearance matches. Captures use a
headless display, zero game volumes and null Wine/OpenAL/ALSA audio; the
running client is checked for PipeWire output streams.

Local report path:

`/home/sheep/Projects/survarium/work-rudnya-v021/rig/reports/step/rudnya_day/index.html`

The owner's final appearance review is pending. These local quick builds use
the wide-ID test executable and do not set or reserve a player release revision.

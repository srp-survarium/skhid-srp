# dynamic-anomalies: the release's timed anomalies as 0.26g map data

`build.py` turns the anomaly places of the release builds into timed 0.26g
`generic_anomaly_core`s, the kind exe patch 42 (`client-patches/dynamic_anomalies_026g.asm`)
chooses from. It writes a patch folder: the client projects of level_01 to level_09
rewritten, plus the anomaly particles the game lacks. The rules are in
`docs/notes/dynamic_anomalies.md`.

## What it does

- **Sources.**
  - The players' game: stock `resources.db`, then the live patches in `release/patches.tsv`
    order (`--live`, a later archive wins). patch_14 has London; patch_17 changes level_08/prerain_3.
  - The release: London from 0.31a2 (`--release-london`, its `resources.NN.db`). level_01 to
    level_08 from 0.28d0 (`--release-natives`), the first release with the mechanism and the
    0.26g map geometry.
  - A 0.26g project of mode N (`day_N`, `evening_N`, `rain_N`, `prerain_N`) takes the release's
    `day_N`. The release's weathers of a mode have the same places (checked: level_01, 03, 05).
  - The release's mode configs must hold the rule the exe has (30/30/10/16/15). The after-pick-up
    time comes from them (`respawn_anomalies_timeout_s`, 60 everywhere).
- **A place's anomaly of kind lifebone (54), spring (74) or onyx (76) becomes:**
  - The release's `anomaly_collision` and its `artefact_collision_<kind>`, in 0.26g form (as
    patch_14 converted them: the 0.31-only keys out, layer "vip"). A place's four kinds share
    `anomaly_collision`.
  - Its `artefact_container_<kind>`: search 2 s, cooldown 0, both the release's.
  - Its `damage_zone_<kind>`.
    - The release's zone without its 0.28/0.31-only keys (`game_effect_on_inside` too, as patch_14).
    - The kind's damage type, body parts, max_hit 0.5 and sounds come from patch_14's London zone of
      that kind: lifebone toxic/face/gas, spring shock/body/electric, onyx radiation/body/crystals.
      The 0.28 natives already have the same types and 0.5.
    - `idle_particle` = the core's `damage_anomaly_particle` (`pre_X2` / `pre_X2_indoor`).
  - A new `charge_zone_<kind>`: a damage zone with max_hit 0, no sounds, `idle_particle` = the
    core's `charging_artefact_particle` (`pre_X1`), at the container, on its use box. It is the
    release's charging and damage-state particle at the artefact.
  - Its timed core, named as the release's `timeout_anomaly_<kind>`:
    - energy on, initial 0, maximum 0, decrease 0, after-pick-up energy (`energy_af_container_use`) 50, every `energy_on_*` 0;
    - 5 artefacts of its kind, one at a time, first spawn and respawn 100000 s (the exe's mark of
      a timed core: they never come by themselves), chance 1, its own container.
    - States, highest first (each group: keep all zones active, not sequential, the stock's
      recharge 20, min/max 0):

      | state | threshold | active (s) | energy on exit | zones |
      |---|---|---|---|---|
      | charging | 3000 | the release's `charging_state_timeout` (20) | 2000 | charge |
      | damage | 2000 | the release's `damage_state_timeout` (4) | 1000 | damage + charge |
      | artefact | 1000 | 0 | — | damage |
      | after pick-up | 50 | the mode's 60 | 0 | none |
      | idle | 0 | 0 | — | none |
    - energy_maximum stays 0. 0.26g clamps the energy to a maximum above 0 at every player action
      near the core (0x33BB50), which would end an anomaly a player walks into.
  - Every core has zones of its own: the server bots' prediction takes a zone's first core only.
- **Mochalka (758, 0.31a2's `artefact_sponge`) is left out.**
  - It is a thrown artefact (data type 13) 0.26g has no item, dictionary row or exe support for.
  - Mapping it to the nearest artefact, onyx, would give onyx two anomalies of its four at every
    place. Under the 15 m rule only one of a place's anomalies lives at a time anyway.
  - Each place keeps three kinds: the places are drawn as often as before, each kind a third of
    the time (the release: a quarter).
- **Stock anomalies out.** The release removed the always-on anomalies of the native maps: the
  swamp, the school's fruit cloud and stalactite, the antenna's electric pairs, and so on.
  - Every `generic_anomaly_core` is removed. So is every object only removed objects name, and
    London's 7 patch_14 places.
  - Anything another object still names stays. The script checks that nothing in the written
    project names a removed object and that every name a new object uses exists.
- **Particles.** The ones the game lacks come from 0.31a2, converted as patch_14 converted its four.
  - `MarginMultiplier` is removed (0.26g has no such property), and `fx/artifacts/big_sphere` becomes
    patch_14's `big_sphere_l09` (0.31a2's look; 0.26g's own big_sphere has another texture).
  - `test_bcw.py` shows the rule gives patch_14's four particles byte for byte.
  - The materials they name are all in the game.
- **Everything else** of each project is untouched: the written project reads back as its input
  minus the removed objects plus the new ones. The writer is `bcw.py`, patch_14's layout byte for byte.

## Size

The new projects hold up to 103,000 values (level_02, 86 places). London has 61,800 to 65,902.
The most a 0.26g project is known to have loaded is London's 43,799 in patch_14. The
format has no total limit (u64 offsets; u16 counts per table only), but a load of the big
ones in a game is still to be seen.

## Commands

```bash
B=private/local-clients/base-patches R=/mnt/hdd/Projects/weapon-survey-20261016/work/builds
python3 -I srp-026g/scripts/dynamic-anomalies/build.py \
  --live <stock>/resources.db $(for n in 12 14 17 19 20 21 22 23; do echo --live $B/resources.db.patch_$n; done) \
  --live private/rooms-test/resources.db.patch_24 \
  --release-natives $R/v028d0/out/game/resources.db \
  $(for f in $R/v031a2/out/game/resources.*.db; do echo --release-london $f; done) \
  --out <folder>/content
release/build-patch <folder>/content <revision> --out <folder>/resources.db.patch_<revision>
python3 -I srp-026g/scripts/dynamic-anomalies/test_bcw.py $B/resources.db.patch_14 $R/v031a2/out/game/resources.03.db
```

`DBGRAB` names dbgrab when it is not at `tools/re-kit/common/dbgrab/target/release/dbgrab`.
The test build (2026-10-20): 75 projects, 3,870 places, 11,610 timed cores, 5 particles, 80 files,
`resources.db.patch_29` (141,608,434 bytes).

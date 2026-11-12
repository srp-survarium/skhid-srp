# Vostok v0.11 electrical anomalies

The owner's request (2026-11-12): add the main-building Electras from
0.26g and the transformer anomalies from 0.21 Vostok, with artifacts at
those locations, while preserving the updated Vostok prepared for release.

This implementation is based on `origin/main` at `b4e08f97f`, including
Radar visual repairs #1672. The earlier revision-37 prototype used an old
checkout and is superseded. Its files remain under the main checkout's
`artifacts/v011e-vostok-anomalies/` for diagnosis only.

## Composition

`v011e-vostok-anomalies@1` appends 70 native objects to each published base
v0.11 project, `day_2` and `day_4`. The normal content manifest then applies
all current map edits. The two projects cover Protective Device and Team
Deathmatch; the Blackout variant also uses the battery project.

| Group | Source | Objects added per project | Artifact locations |
|---|---|---|---|
| Main building | 0.26g `level_03`, `logic/anomaly/random_anomalies/antena/` | 3 cores, 6 damage zones, 6 containers, 12 collision geometries | 6 candidates for Onyx (76) |
| Transformers | 0.21d port `v021d_level_03`, `anomaly/electro/` | 1 core, 10 damage/effect zones, 5 containers, 6 collision geometries, 21 particles | 5 candidates for Spring (74) |

Each group has one enabled artifact spawner, at most one artifact present,
and a first spawn after 60 seconds. Building respawn is 60 seconds with
30 seconds variation; transformer respawn is 70 with 30 seconds variation.
The current v0.11 row of `maps.tsv` sets all container searches to 6 seconds
and cooldowns to 80 seconds. Locations come from each container's linked
collision geometry, rather than the container's editor origin.

`v011e-vostok-electric-1.json` applies the released transformer damage
correction from `v021d-radar-electric-1.json`: 0.6 every second on both
feet. Its two entries are checked against the donor edit. The building
zones already use 0.26g's feet-only damage.

The manifest retains:

- `radar-v011e-seams@1`, `radar-v011e-textures@1`, and
  `radar-v011e-render-1.json`: repaired wall/barrel seams, native-compatible
  tire atlas mip chains, and detailed terrain materials.
- Runtime vegetation, breakable lamps and their light/cone bindings.
- `maps.tsv`, the v0.26g spawn layout (14 points per team with six-second
  protection), and the Keeper of the Artifacts pool.
- Existing rust-area anomalies and all original map objects and geometry.

The three transformer props already match the donor's positions, rotations
and scales exactly. All 21 referenced particle/sound assets already exist
in the published base archives; no replacement effects or props are made.

## Rebuild and checks

The pack builder consumes the six files in `inputs.sha256`, extracted from
stock 0.26g with published base patches 20, 22 and 23 mounted. These are
source inputs for the normal manifest composition; later release edits
are applied afterwards. The builder refuses different source hashes,
retains native binary-config nodes and checks exact typed-node round trips.
Its `replace_file` operations pin the destination base hashes too.

In `nix develop ./srp-026g`, from this checkout:

```sh
python srp-026g/scripts/v011e-vostok-anomalies/make_pack.py \
  --base <extracted-base> --out <packs>/v011e-vostok-anomalies-1
tools/check
release/build-content <next-revision> --full --out <candidate>
```

`tools/check vostok-anomalies` extracts the pinned base itself, rebuilds
the pack in temporary storage, checks the manifest hash, native types,
object references, artifact ownership, duplicate/dangling-link rejection,
asset availability, and current donor damage tuning.

`check_composed.py` checks the finished candidate against the released
archives. Extract the six source project paths from all released patches
into a reference folder. Extract those two v0.11 projects, every file of
the two Radar repair packs, and all
`models/ported/v011e/level/03/terrain_p_*.model/settings` from the same
archives plus the candidate into a composed folder. Then:

```sh
python srp-026g/scripts/v011e-vostok-anomalies/check_composed.py \
  --reference <released-reference> --composed <candidate-files> --packs <packs>
```

Removing the 70 additions must leave the exact native typed tree of the
released map. This checks all pre-existing values, object ordering,
spawns, lights, lamps and the Keeper pool. Imported groups must match
the current released donors with v0.11's artifact search rule applied.
The seam/texture files must equal the current repair packs, and terrain
settings must retain the detailed materials.

The simulation change is recorded as `next`. No release revision is
changed or reserved by this feature. Local playtests use the normal
release builder's candidate and matching current-source server/client.

## Validation and local test (2026-11-12)

The full candidate build passed: 19,316 manifest files compare exactly to
what the complete released archive chain plus the candidate supplies.
Equipment and weapon tables are unchanged. The finished-map comparison
passed for both projects: removing the 70 new objects leaves every released
value intact, including all 28 spawn points and the Keeper pool. Six
seam/texture files match the current repair packs exactly; all nine terrain
model settings match the released settings with the approved material edit.
The Keeper edit's expected object counts are increased by 70 for these two
projects (282 to 352 and 280 to 350); its pool is otherwise unchanged.

Local candidate `resources.db.patch_57` is 77,224,039 bytes, SHA-256
`ccdd7943b80fb55ad92b9ea6f96ab02b143b6e2e8ebcc4abc6c93afb5bdd9ade`.
Matching server data is 15,905,194 bytes, SHA-256
`b752405a78e2d7fcde0b4a28cb165612583b543fe7cf58bd00bfd8db50f15407`.
These are local validation artifacts; revision 57 has not been released
or reserved by this work.

Evidence in the main checkout: `private/v011e-vostok-current/` contains
`build-content.log`, `comparison.log`, `pack-check.log`, `checks.log`,
and `content/files.tsv`. The isolated source branch is
`feat/v011e-vostok-anomalies` in that folder's `tree/`.

The obsolete test was replaced with the current-source executable and
server, all released archives through 56, and this full candidate. One
client and its loopback server were confirmed running, with DXVK reporting
2560x1440. Its retained config is
`private/v011e-vostok-anomalies-test/config.json`, account `VostokElectra1`,
login `127.0.0.1:25490`. Launch and data are verified; the owner reported the gameplay test passed
(2026-11-12: "everything good, review and merge"). Nothing has been published or deployed.

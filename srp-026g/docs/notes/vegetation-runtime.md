# Runtime vegetation

The revision-36 client crash at 2026-10-28 01:47:48 Europe/Warsaw failed a
57,082,932-byte managed-arena request for
`projects/v021d_level_06/vegetation/suh_middle.veg`. The resource manager had
waited ten seconds without a deallocation. Its 512 MiB arena reported
221,332,096 bytes allocated, leaving more aggregate space than the request.
The dump does not contain the heap: fragmentation is the leading explanation,
not a recovered heap layout. Other players leaving the match does not prove
they all hit this exact failure.

This file matches the original 0.21d `projects/level_06/vegetation/suh_middle.veg`
byte for byte (SHA-256
`a5465bb468225e1e4483aa9b9671afd328ca7802b627660fb49fe932e91e68e7`).
The porter retained editor painting data from the donor; the port did not
create it.

## Format and build change

The `.veg` stream consists of little-endian `u32 id, u32 size, payload`
chunks. Chunks `0x4d` and `0x4e` hold editor painting data; `0x58` contains
float3 plant positions and `0x59` contains float3 normals. The 0.26g runtime
reads the latter two. Their bytes, headers and order must remain unchanged.

`vegetation-runtime-1.json`, applied last through `release/content.tsv`, strips
the two editor chunks from the four supported early ported maps:
`v021d_level_03`, `v021d_level_06`, `v011e_level_03`, and `v011e_level_04`.
Each has `day_2` and `day_4` client projects, giving the requested 4×2 scope.
Both variants reference their map's shared vegetation directory, so 45 layers
cover all eight projects. The edit lists these four directories explicitly;
stock maps, the lobby, separate 0.10b imports and S.T.A.L.K.E.R. packs are
outside this cleanup. Donor archives and source packs remain intact.
Unknown or duplicate chunks, truncated streams, missing or
mismatched arrays, and non-finite coordinates fail the build. Empty arrays
are valid; an already compacted file is unchanged.

## Audit

Effective stock data plus every offered patch through revision 36:

| Project | Layers | Before (bytes) | Runtime (bytes) |
| --- | ---: | ---: | ---: |
| v021d_level_06 | 9 | 119,019,791 | 1,499,880 |
| v021d_level_03 | 11 | 21,245,547 | 2,179,352 |
| v011e_level_03 | 13 | 31,737,918 | 188,824 |
| v011e_level_04 | 12 | 3,574,880 | 272,496 |
| Cleanup subtotal | 45 | 175,578,136 | 4,140,552 |
| Stock projects, including lobby | 84 | 7,610,734 | 7,269,312 |
| Total | 129 | 183,188,870 | 11,409,864 |

The crashing layer shrinks to 441,424 bytes while retaining all 18,392 plants.
All 129 layers retain their 475,325 instances. The savings are raw resource
bytes, not a measurement of total process memory or peak GPU memory.

The separate 0.10b donor used by `ports/survarium-010b-to-026g.json` also has
the problem. Its three Factory weather projects have 13 layers each:

| Donor project | Before (bytes) | Runtime (bytes) | Plants |
| --- | ---: | ---: | ---: |
| level_03 | 31,738,230 | 189,136 | 7,872 |
| level_03_evn | 32,218,082 | 189,688 | 7,895 |
| level_03_rain | 35,644,561 | 184,192 | 7,666 |

These are donor measurements, not files in the revision-36 patch manifest.
The importer preserves these streams; they are outside the requested cleanup.
Its empty lobby layer was also
validated (24 to 16 bytes, zero instances).

The local `stalker-maps-6` pack (not in the revision-36 patch manifest) has six
projects. Training Camp's two layers shrink from 1,337,024 to 1,206,824 bytes.
Bath's 13 layers (3,020,464 bytes) and Military Base's 14 layers (1,865,528
bytes) are already runtime-only. Rembasa, Underground Depository and
Underground Station have no `.veg` references in their client projects.
Pack versions 1, 2, 3 and 4 were also checked: the same two Training Camp
layers retain editor data. The latest pack's largest individual converted
model is 1,239,774 bytes; its largest resource is a 22,369,665-byte sky DDS.
These are runtime resources, with no demonstrated equivalent removable
vegetation payload. This audit does not prove absence of other map bugs.

## Validation

Run focused malformed-input, preservation, idempotence and CLI tests:

```sh
python3 -m unittest discover -s srp-026g/scripts/content-edit
```

The investigation also executed the installed revision-36 x86 instructions
with Unicorn: the grass constructor's parsing block at VA `0x15fb2d` through
`0x15fc62`, including its real sequential chunk reader at `0xf1540`.
For all 129 installed layers and all 29 layers in `stalker-maps-6`, the original
and compacted files produce identical counts and position/normal hashes,
both with normals enabled and disabled: 632 executions, 729,173 instances.
The 40 additional 0.10b donor layers also pass (160 executions, including
the empty lobby), bringing the total to 198 layers and 752,606 instances.
Executable SHA-256:
`70c0abf574b2f10b3fd00d60a86f2591474225f9b234c97fab6a21b43ce12905`.

The original first-fit allocator at VA `0x10a240` was also exercised with a
synthetic 512 MiB heap: 315,538,816 free bytes in holes no larger than 32 MiB
reject the original layer and accept the compacted layer. With the same
free space contiguous, the original succeeds. This illustrates the failure
mechanism; it is not the crash's recovered heap. Native parser tests are not
a full client map-load or rendering test.

Build the candidate through `release/build-content <next-revision>` and
inspect `files.tsv` before release. Do not create a patch folder manually.
The candidate targets exactly the 45 shared vegetation files of the four
supported maps. It supersedes the earlier broad candidate that changed all
129 installed layers. All 2,571 other generated files match installed content.
The final archive was reopened: its path set is exactly the requested scope,
and every payload matches the original runtime chunks byte for byte. All
172,493 plants are retained; 171,437,584 bytes (163.50 MiB) of editor data are
removed. The local candidate is 4,148,744 bytes, SHA-256
`e1d918e77bd97edfe163671d4e24613836172811004e922601046ebd8d79cdf9`.

The combined revision-37 candidate built from main `68017f27` adds the three
Research/room UI movies to these 45 files. Its patch SHA-256 is
`9a0d0b7b9e8ffbdab755baee10cb6f5d481cf3d818dfb2f82a1cb4133cff1a95`.
All 45 vegetation payloads were compared against the approved candidate and
the original runtime chunks: they match byte for byte. All 2,616 composed
files matched the manifest, and the revision-37 local server smoke test passed.
On 2026-10-28 the owner reported testing the other release features and asked
to release; the eight map-variant playtests remain outstanding.

# Packet validation split

This series extracts the shared checks and native guards from PR #478
(`fc1f3f6c2`) and the server integration associated with #282/#289. It is
updated from the original baseline `bd359ad4e` onto revision-45 main `6a547df87`,
then rebased onto main `2b8f930e6` (revision 50 and #1354, #1363). The existing `world-frame`
crate and patch 87 already protect snapshot framing on both sides; this series
must retain that ownership and the weapon-pickup preparation at world load.

Merge the PRs in their listed dependency order. Each intermediate commit must
build and pass its affected tests. None of this work has been played or released.

## Shared message checks

`srp-validate` is an allocation-free, safe Rust library. The first part checks
message lengths, roster indices, input rotations, compressed-output bounds and
static options. It introduces no runtime call sites or hooks; the new receipt codec is inert until negotiated by later integrations.
Integration is a separate review step.

Compatibility changes from the old PR:

- Input bit 29 is now the shipped artefact-carry/weapon-pickup marker. Only bit 30
  remains unused; the native per-input flag in bit 31 remains allowed.
- Static options can carry the shipped `SRPX` Blackout extension. Its layout is
  checked against the current server encoder, including malformed extensions and
  truncated native options.

Validation: `cargo test -p srp-validate` and
`cargo clippy -p srp-validate --all-targets` under the pinned Nix environment.
These are host tests, not proof of a native hook or a complete match.

## Remaining boundaries

World rules/statistics, player inventory, projectile/object/effect checks, native
index guards, server integration and client integration have separate review
boundaries. The current serialized extensions are covered before runtime enforcement;
the retained strict effects template and unknown-context limits are described below.
The old source PR remains available as the provenance of the extracted work.

## World context, rules and statistics

The second part provides semantic views over `world-frame`, without a second
framing parser, and extracts the player-head, rules and statistics checks. It
still has no runtime enforcement. The statistics reader includes patch 92's
battery progress and sparse held-step records, including cell/count/order bounds,
and #1155's last taker: bit 7 of the held count marks a taker byte (a place under
20, or 0xFF for none).

Validation: validator tests and affected-crate Clippy pass. Production-encoder
fixtures cover TDM rules, statistics in all four modes, one and twenty players,
truncation, malformed held steps and body-relative player offsets.

## Player-state and inventory checks

The third part extracts the player-tail parser and its fifteen regressions:
hit effects, raw-pointer rejection, stamina, artefact slots and inventory item
owners/ammo indices. The parser stops where the native format ceases to be
self-describing; optional profile/inventory context determines how far it can
read. This is still a library change with no runtime enforcement.

Validation: all validator tests and affected-crate Clippy pass.

## World sections and current serialized extensions

The next part adds object/effect checks and a complete validation entry point.
Current-format compatibility includes the 29-byte version-8 pickup record before
player item fields, fixed 69-byte bullet records, and the optional `LMP1` lamp
history suffix. A bullet's ammo ordinal is not bounded by the original profile:
pickup swaps can change the gun while a bullet remains in flight, and the shipped
native loader already clamps the ordinal.
Since patch 98 the six sections grow past the history item's inline buffers;
only the body's 0xF000 bytes (`world_frame::MAX_BODY`) and a player state's
0x1000 bound them. The packed 0xE8 decodes into 0xF000 bytes as well.

`drift_dump` selects context from statistics/rules independently of the full
verdict, then checks each world and sixteen mutations. All **1,786** worlds from
`private/local-clients-rev43-new-weapons/runtime/snapshots` pass the host check.
These contexts omit profiles, inventory and map objects, so this is current-format
compatibility evidence, not complete semantic validation. Older version-4 pickup
records from `private/live-worlds-1c0ebd31` are a different protocol baseline.

On revision-50 main all **3,920** worlds of the 2026-11-05 local tests (rev49/50
builds: the 8v8 bot rooms, Blackout, Research, Team Deathmatch, battery soak
runs; `private/local-clients-{49,room-bots,blackout,soak,repro,rgd,sights,map-names}`)
pass the host check.

Validation: validator tests and affected-crate Clippy pass, including native
server-encoded player state and lamp count/length boundaries. Native integration
and real-match acceptance remain separate gates.

## Server upload enforcement

The next part calls the shared validator before an upload can enter any snapshot
cache, vote, resync or relay. A refused upload retains its outstanding request.
The server supplies the current map, roster, usable kits and profile item counts;
map-object tables and inventory shapes remain unknown and are not invented.

Scripted clients now emit valid effects, inventory headers and large bullet
sections. Result fixtures use ten total batteries on a ten-battery map. Tests
cover framing-valid unsafe effects followed by a valid initial retry, late joins,
lossy resync and the existing refusal paths. No match calculation or wire format
changes; valid snapshots retain their existing policy.

## Native deserialization bounds

Patch 113 ports the FSM, anomaly and artefact guards to generated lohooks, typed
fields and the current `vostok` boundary. It is independent of runtime message
and semantic-world enforcement. The checked stock instructions confirm the FSM's
ESI/EDI and two stack inputs; the re-kit records that stronger ABI evidence.

## Client message enforcement

Patch 114 connects the shared envelope/input checks at the native dispatch. It
preserves the late-join profile validator and runs before id subtraction rather
than overwriting the dispatch table's relocation. The separate static-options
parser remains library-only, matching the source PR's integration scope.

## Client world enforcement

Patch 87's existing hooks now enforce the shared semantic checks, preserving
pickup preparation and framing ownership. Native rule parameters are read once
into values; Research uses its native type 5 rather than an arena address.
The context rejects duplicate/missing mode or respawn rules and invalid vector
bounds. Profiles, swapped inventories and authored object tables remain unknown.

Validation includes both native load paths, the i686 patch tests and Clippy, plus
the eight historical snapshots loaded/saved by three simulated client states.
The emulator stubs OS calls and resource boundaries; this does not substitute
for playing a release candidate. Nothing in this series has been deployed.

## Diagnostic tools

The final optional part ports the original `bench` and `explain` examples to the
current frame API. Context inference uses statistics/rules independently of the
full verdict, so a refusal cannot be hidden by choosing only passing worlds.
The synthetic stress cases grow the bullets (current fixed 69-byte records) or
the rules until the body is 0xF000 bytes, patch 98's world buffer. `explain` compares against the production server decoder and handles
files shorter than a world time.

The client/host drift gate passes all **32,148** comparisons: **1,786** current
snapshots through both native load paths and sixteen mutations per snapshot.
This corpus uses unknown profile/inventory/object context, as the client does;
server profile bounds have their separate tests. Native resource creation is
stubbed in the drift harness and exercised separately by pickup/round-trip tests.
On revision-50 main the gate passes for the 3,920 current worlds above
(70,560 client/host comparisons with the mutated copies, none differ), and the round trip loads and saves 706 of them
(`local-clients-{49,room-bots,blackout}`) in three client states with their
inferred match contexts (`--contexts`).

## Replacement stack and merge order

This is a new independent stack from main, replacing the proposed behavior of
#1164–#1172. Those PRs and original #478 stay open as references. Merge one stack,
not both. No old PR commit is an ancestor of these replacement commits.

1. Shared message checks and receipt codec (library only).
2. Server input isolation and bounded sender removal.
3. World context, rules and statistics (library only).
4. Player-state and inventory checks (library only).
5. World sections and current formats (library only).
6. Native FSM, anomaly and artefact bounds.
7. Server world-upload enforcement.
8. Negotiated server world-load receipts and recipient recovery.
9. Client message checks and recovery, including the existing framing guard.
10. Client semantic world checks.
11. Optional diagnostics and benchmarks.

Merge in order and retarget the next PR to main after its predecessor merges.
Merge commits preserve the shared ancestry; squash merging requires rebasing
later branches. The library-only parts have no runtime callers. The input-only
server change is the smallest behavior change and the first runtime review.
The native reception/recovery boundary stays together so no intermediate client
changes rejection policy without a functioning recovery path.

The replacement stack changes neither main's release record (revision 50) nor
its simulation compatibility table.

## Bad traffic policy and recovery

The server rejects unsafe input before relay using the same check as clients,
plus its existing pickup-target check. One bad input is discarded and its owner
is told to drop it. Eight distinct invalid inputs within ten seconds remove only
the sender. Retransmissions of the same input time count once; isolated strikes
expire. This policy uses bytes verified from that sender, never client accusations.
Semantic world refusal is not treated as proof of cheating and causes no ban.

The client (patches 114 and 97) consumes a rejected message and skips its native handler without
closing the match. Bad E3/E4 handshake envelopes request the recipient's pending
handshake again; bad E8/world bodies request a fresh world generation. Native
world deserialization must return before the new Loaded receipt is sent. The
server waits for this application receipt as well as delivery (or existing
matching-state hash proof) before admitting a recipient. Transport ACKs alone
cannot advance it past a rejected world. A lost receipt causes a two-second
retry; rejection retries are limited to one per second per connection.

D8 0x82/0x83 negotiates this behavior. Receipts are fixed 20-byte E0 carriers with
a separate impossible upload prefix, match seed, generation, outcome and version.
Only a pending offer on the sending connection accepts Loaded/Rejected. Handshake
retry is accepted only while that connection is Connecting/Loading; the seed is
not required before options decode. Legacy 0x80/0x81 clients keep their prior
behavior. **Deploy the server before a client with the new capability.**

The client owns a per-match budget for rejection sends, checks packet-pool
headroom before allocation, and clears it on match teardown.
Server tests cover rejected first/resync worlds, lost reports, successful retry,
repeated refusal, forged/stale reports, corrupt handshake envelopes and source
input removal. A native harness covers 2,000-message bursts, a valid input after
the burst, successful/failed loads, early handshake, full pool, teardown and reuse.
A receiver report cannot kick its donor or restart the other players.

## Every refusal is logged

Nothing either side refuses goes unlogged. Each kind of refusal gets at most one
line a second, and a line counts the refusals since the one before it, so a
flood cannot fill the disk and the count is never lost.

- Server (its log, per player and kind): a refused input with its reason; a
  refused upload with its section and offset and the match it was checked as
  (mode, batteries, respawn points, places), and an upload it ignored as unasked
  or out of time; a world or handshake the player's game refused (its Rejected
  receipt); a receipt that matches no offer. The match-end line of each player
  counts its refused inputs, uploads and loads.
- Client (`srp_patches.log`, one limit for the game): a discarded message with
  its id, length, part and offset; a refused world with its part, offset and
  length.

## Performance evidence and limits

An optimized host-library benchmark over 1,786 captured worlds measured 3.25 µs
median, 5.28 µs p99 and 6.83 µs maximum. All captures use battery mode; most have
sixteen players. On the 3,920 current worlds: 4.53 µs median and 8.56 µs maximum
for 13 to 20 players (`bench --quick`); a 20-player world grown to 0xF000 bytes of bullets 9.1 µs.
The client searches at most 70 candidate item counts per player
when profile context is unknown. A separate i686 Linux build using the arena's
size optimization and LTO measured 21–22 ns per valid/refused input and up to
1.33 µs per player state across 264 synthetic patterns (244 refused). These are
microbenchmarks, not native Windows frame timings or exhaustive worst cases.
The `hostile` example has no host-library imports; it can be built in a tiny
external Cargo package depending only on srp-validate to avoid host-only database
dev-dependencies that do not compile for i686.

The validators allocate no heap memory. Recovery limits packet allocation, but this work is not a complete audit of datagram reassembly,
decompression, all opaque native item readers or volumetric DDoS protection.
The shared static-options parser is still not connected to the decompressed E4
reader. Repeated incompatible snapshots can still prevent the affected recipient
from catching up; retrying them does not establish that their content is correct.
A real-match release-candidate test remains necessary before shipping client
changes. Nothing here changes a release revision, merges or deploys the stack.

## Build audit compatibility

The revision-45 medical hook exposed a static-auditor gap: a generated lohook
uses its final `ret` to dispatch a saved continuation. It is not the enclosing
native function's return. The call-shape walker now follows every target from
the built dispatch, including replay, skip and named exits. Negative fixtures
still reject an exit with the wrong stack cleanup and a malformed dispatch ret.
The existing native weight harness also runs the audit over installed functions.

The late-join binary is regenerated from the changed NASM prefix and the existing
reviewed C payload: exactly one byte changes, capability 0x80 to 0x82. The JSON
hash and assembler version match that artifact; no C compiler migration is part
of the recovery change.

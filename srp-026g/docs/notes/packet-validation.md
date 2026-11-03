# Packet validation split

This series extracts the shared checks and native guards from PR #478
(`fc1f3f6c2`) and the server integration associated with #282/#289. It is
updated from the original baseline `bd359ad4e` onto main `2480a68b3`. The existing `world-frame`
crate and patch 87 already protect snapshot framing on both sides; this series
must retain that ownership and the weapon-pickup preparation at world load.

Merge the PRs in their listed dependency order. Each intermediate commit must
build and pass its affected tests. None of this work has been played or released.

## Shared message checks

`srp-validate` is an allocation-free, safe Rust library. The first part checks
message lengths, roster indices, input rotations, compressed-output bounds and
static options. It introduces no runtime call sites, hooks or wire-format changes.
Integration is a separate review step.

Compatibility changes from the old PR:

- Input bit 29 is now the shipped artefact-carry/weapon-pickup marker. Only bit 30
  remains unused; the native per-input flag in bit 31 remains allowed.
- Static options can carry the shipped `SRPX` Blackout extension. Its layout is
  checked against the current server encoder, including malformed extensions and
  truncated native options.

Validation: `cargo test -p srp-validate` (10 tests) and
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
battery progress and sparse held-step records, including cell/count/order bounds.

Validation: 16 validator tests and affected-crate Clippy pass. Production-encoder
fixtures cover TDM rules, statistics in all four modes, one and twenty players,
truncation, malformed held steps and body-relative player offsets.

## Player-state and inventory checks

The third part extracts the player-tail parser and its fifteen regressions:
hit effects, raw-pointer rejection, stamina, artefact slots and inventory item
owners/ammo indices. The parser stops where the native format ceases to be
self-describing; optional profile/inventory context determines how far it can
read. This is still a library change with no runtime enforcement.

Validation: all 32 validator tests and affected-crate Clippy pass.

## World sections and current serialized extensions

The next part adds object/effect checks and a complete validation entry point.
Current-format compatibility includes the 29-byte version-8 pickup record before
player item fields, fixed 69-byte bullet records, and the optional `LMP1` lamp
history suffix. A bullet's ammo ordinal is not bounded by the original profile:
pickup swaps can change the gun while a bullet remains in flight, and the shipped
native loader already clamps the ordinal.

`drift_dump` selects context from statistics/rules independently of the full
verdict, then checks each world and sixteen mutations. All **1,786** worlds from
`private/local-clients-rev43-new-weapons/runtime/snapshots` pass the host check.
These contexts omit profiles, inventory and map objects, so this is current-format
compatibility evidence, not complete semantic validation. Older version-4 pickup
records from `private/live-worlds-1c0ebd31` are a different protocol baseline.

Validation: 40 validator tests and affected-crate Clippy pass, including native
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

Patch 94 ports the FSM, anomaly and artefact guards to generated lohooks, typed
fields and the current `vostok` boundary. It is independent of runtime message
and semantic-world enforcement. The checked stock instructions confirm the FSM's
ESI/EDI and two stack inputs; the re-kit records that stronger ABI evidence.

## Client message enforcement

Patch 95 connects the shared envelope/input checks at the native dispatch. It
preserves the late-join profile validator and runs before id subtraction rather
than overwriting the dispatch table's relocation. The separate static-options
parser remains library-only, matching the source PR's integration scope.

## Client world enforcement

Patch 87's existing hooks now enforce the shared semantic checks, preserving
pickup preparation and framing ownership. Native rule parameters are read once
into values; Research uses its native type 5 rather than an arena address.
The context rejects duplicate/missing mode or respawn rules and invalid vector
bounds. Profiles, swapped inventories and authored object tables remain unknown.

Validation includes both native load paths, 120 i686 patch tests and Clippy, plus
the eight historical snapshots loaded/saved by three simulated client states.
The emulator stubs OS calls and resource boundaries; this does not substitute
for playing a release candidate. Nothing in this series has been deployed.

## Diagnostic tools

The final optional part ports the original `bench` and `explain` examples to the
current frame API. Context inference uses statistics/rules independently of the
full verdict, so a refusal cannot be hidden by choosing only passing worlds.
The synthetic bullet stress case now uses 133 fixed 69-byte records within the
native cap. `explain` compares against the production server decoder and handles
files shorter than a world time.

The client/host drift gate passes all **32,148** comparisons: **1,786** current
snapshots through both native load paths and sixteen mutations per snapshot.
This corpus uses unknown profile/inventory/object context, as the client does;
server profile bounds have their separate tests. Native resource creation is
stubbed in the drift harness and exercised separately by pickup/round-trip tests.

## Merge order and baseline repair

1. Shared message validation (library only).
2. World context, rules and statistics (library only).
3. Player-state and inventory checks (library only).
4. World sections and current serialized extensions (library only).
5. Server upload enforcement.
6. Native FSM, anomaly and artefact bounds.
7. Client message enforcement.
8. Client world enforcement.
9. Optional diagnostics and benchmarks.

The PRs form a stack: after merging a predecessor, retarget its successor to
`main` before merging it. Merge commits preserve the shared ancestry; squash
merging requires rebasing the remaining stack. Nothing is merged automatically.

Main's medicine change (#1163) added a second `next` entry to the replay
compatibility table, whose parser requires one final pending row. The first
chunk combines the SCAR and medicine descriptions into that single row, without
changing either pending change or assigning a release number. This repairs the
replay tests inherited from the new baseline.

## Abuse resistance and performance review

Client enforcement (#1170 and #1171) is draft pending recovery and abuse tests.
A rejected message or world currently closes the recipient's match. Discarding
an initial world can leave loading frozen; discarding a resync world can leave
simulation out of step. Removing the close call alone is not a recovery design.
The server must reject sender-controlled bad input before relay, and a victim
must not be disconnected merely because another player supplied malformed data.

The server chunk now uses the same input validation as the client, in addition
to its existing pickup-target check. Previously it checked rotation and pickup
but allowed unused action bit 30, which the client rejects. A three-player
regression reproduced that mismatch before the fix; afterward bad input and its
repeat go to nobody else, the sender is told to discard it, valid bits 29/31
still pass, and all players continue. This closes that specific relay path; it
is not an audit of every transport, recovery or flooding path.

An optimized host-library benchmark (`cargo run --release -p srp-validate
--example bench -- --worlds <current-corpus> --quick`) across all 1,786 captured
worlds measured 3.25 microseconds median, 5.28 microseconds p99 and 6.83
microseconds maximum. All these captures use battery mode; most have sixteen
players. These are per-world batch timings on this host, not native 32-bit hook
cost, adversarial worst cases or proof of frame-time behavior in the game.
The client guesses up to 70 item counts per player without profile context;
that bounded search still deserves hostile-input and native profiling.

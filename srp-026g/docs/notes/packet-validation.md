# Packet validation split

This series extracts the shared checks and native guards from PR #478
(`fc1f3f6c2`) and the server integration associated with #282/#289. It is
rebased onto current main, starting at `bd359ad4e`. The existing `world-frame`
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
boundaries. The historical strict effects template and context assumptions must
be reconciled with current serialized extensions before runtime enforcement.
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

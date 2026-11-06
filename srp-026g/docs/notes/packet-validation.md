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
The effects template takes any finite float at offsets 36, 40 and 48: the
effect's five values (read at 0x2DDEAF..0x2DDEF0, stored at 0x2DDFE4..0x2DE017)
are two u32 at 32 and 44, still held to 0, and three floats, none an index;
live games save 0.05 in them (2 of the 25,456 uploads of the second live sample).
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
The server supplies the current map, roster, usable kits and profile item counts,
the world's game objects and every kit's items (next sections).

Scripted clients now emit valid effects, inventory headers and large bullet
sections. Result fixtures use ten total batteries on a ten-battery map. Tests
cover framing-valid unsafe effects followed by a valid initial retry, late joins,
lossy resync and the existing refusal paths. No match calculation or wire format
changes; valid snapshots retain their existing policy.

## The world's game objects on the server

The game-objects section is the objects of one list, each its own bytes with no
count, id or length (`srp_validate::world::objects`, RE in its module doc). The
list is every bound place's grenades and traps, the newest place first, then the
map's damage zones that no anomaly drives, its resupply zones and its anomalies,
each anomaly with its zone groups, its artefact containers and its artefacts. A
map's battery containers are the gather rule's (the rules section).

- **The map's part** comes from the project's `client_objects`: the server game
  data keeps those a world saves (`resources_db::server::objects_of`, data
  format 4; data 3 is refused at start), and `maps::world_objects` reads them
  once into the validator's types, the anomalies' states in the game's sorted
  order (`state_prio` 0x33C400, insertion sort), and each artefact with its
  item's core: an active one (state 4) saves its core's own bytes after the
  base's (Lifebone `data.type` 3 and Spring 6 a u32, Onyx 7 a u32 and an f32;
  the artefact cook's switch 0x2EE95F, the cores' `deserialize` 0x3124B0,
  0x3119E0, 0x312AF0).
- **The places' part** is each profile's usable kits, slot by slot, one object
  for each of a grenade set's or trap set's count (`MatchProfile::held`, by the
  item's `data.type`, `ItemClass`). The first roster is bound from the start
  and each late joiner when a game installs it, so an upload is checked as a
  game that installed any number of the late joiners, in join order.
- **Refused:** a section that does not read to its exact end; a grenade or trap
  float that is not a number; flags an object has not; more than 20 users, a
  user past the roster or using two objects; an anomaly state past its states;
  zone bits past a group's zones; a container's artefact past the map's; an
  artefact owner that is no kit with a player; an artefact state over 5.

Evidence: all **3,920** worlds of the 2026-11-05 local tests read to the exact
end of the section with their match's own profiles (each match's 0xE4 from the
packet logs, joined to its worlds by the players' ports) and the maps of server
data built from the live patches up to 50. So do all **50,205** uploads of 118
live matches of revision 50 (two spread samples of the 366 recordings from
2026-11-05 20:00 to 2026-11-06 21:00 UTC: 39 map configs of modes 2 to 5; every
complete 0xE0 upload checked as `world_validation::check` does, with the
recording's own 0xE4). Before the artefact cores 85 of the first sample's
24,749 were refused, all at an active artefact. The client's world check still
has no object list: its native guards stay its bound.

## Which worlds reach another game, and the animation streams

The semantic check reads every section of an upload; each player block's
animation stream is read by the frame's walk, which since `animation_validate`
(`world-frame`, `snapshot_rules.md` 7.2.1) does not only measure it but bounds
every count and index a native client decodes from it (the times/floats table
indices, the channel picks, the expression-tree recursion, the tree's length
and its table pops). A world reaches another game only as (`game.rs`,
`usable_snapshot`):

- the snapshot, once its CRC (`game_state_history_item::hash` 0x325150, over
  the whole body, animation streams too) is the majority of the clients' hash
  reports for its tick (`check_candidates`);
- the newest upload, once another game reported its CRC as well (two
  vouchers), or when one game alone simulates;
- the initial world, the first upload, which no other game vouches for.

So one client can put an animation stream of its own into another game's world
only through the initial world, or as the one game a newcomer joins. The
initial world has no player block: every player enters with 0xE9 after the
clock starts on it. Its active mask is 0 in all 46 kept initial worlds and in
the first uploads of the 118 live matches (539 uploads), and `not_initial` now
refuses one with a player in it (`SRP_CHECK_INITIAL_WORLD`, on by default).
Scripted worlds write a place's block only once it has entered, as games do.

That single-source path (the one game a newcomer joins, and the initial world)
is why the frame walk now validates the animation stream and not only measures
its length: those streams reach another game with no second source to vouch for
them, and `animation_validate` runs in the walk on every upload the server
checks and every world patch 87 loads. All 177,672 animated player blocks of
the 20,668 kept worlds in `private/live-worlds-*` pass it.

## Player states to their exact end on the server

Each player's state is read to its block's exact end
(`srp_validate::world::inventory`, RE in its module doc): the head, the
effects, the stamina, the inventory header (a carried battery's state too), the
kit's items, and everything after them (the damage model's body parts, the
usable-object data, the character controller of a living player, the spotting,
the recent hits, patch 78's carry snapshot).

- **The kits** come from the match's profiles (`MatchProfile::inventory`): a
  weapon slot's gun with its state machine (its config's `states` classes) and
  its ammo list (the kit's ammo slots holding a stack it takes), an ammo slot's
  stack of a count over 0, any other slot's item by its `data.type`. A profile
  holding an item the game makes no object of leaves that match's states read
  to their inventories, with a warning.
- **Live state the stream does not say** is tried both ways: a weapon with a user
  (one put in a hand) saves more than one without; a picked gun (patch 85) is
  its recipe's gun with the picker's ammo list, and a pick may fill the
  picker's ammo slots. The world is refused only when no shape reads it to the
  end.
- **Refused:** an item owned by no kit with a player, or saving another slot; an
  ammo id or next ammo id past its list; a state machine state past its states
  (a weapon's, its sub-states, the breath's, the portable object's selector and
  its jump, a battery's); a throw of a grenade slot past the inventory's; more
  than 8 affects or 64 hits on a body part, an affect type past 9; recent hits
  that cannot fit; a battery carried by another player or in a container past
  the two; bools that are not 0x00/0xFF; floats that are not numbers; a carry
  snapshot of another version; any byte left over.

Evidence: with their matches' profiles, **3,919** of the 3,920 worlds of the
2026-11-05 local tests pass the whole check, every player block read to its
exact end, the 1,244 states carrying a battery included (the one refused has a
late joiner whose profile its packet log lacks; the server has it). Of 125,440
single-byte mutations of those worlds 28,959 are refused, against 18,726
without the objects and kits: most bytes of a state are free values
(positions, times, health) that no layout bounds.

## Native deserialization bounds

Patch 115 ports the FSM, anomaly and artefact guards to generated lohooks, typed
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

## Read-site audit (2026-11-07)

A pass over the player-to-player crash surface from the native read sites, not
the docs: the 0.26g exe decompiled in Ghidra (`private/function-map/inputs/026g`,
sha ac33d99), the 0.10b byte-matching source (`~/Projects/survarium/vostok`, the
engine's fixed-array sizes) as the ground-truth caps. For each native reader of
data one client sends another, the unbounded count/index at the read site and
what bounds it:

- `game_state_history_item::deserialize` 0x324D80: iterates the active mask's
  set bits and writes block `bit * 0x9054` -- a bit >= 20 is an out-of-bounds
  player slot. Then six sections, each a u16 length `memcpy` into a history-item
  buffer (grown by 0x7AB70, a null allocator before patch 98). Bounds: the frame
  walk (`Frame::parse`) -- every mask bit < places (<= 20), the six sections walk
  to the body's exact end, body <= `MAX_BODY` 0xF000.
- the player state 0x33FD80: a u16 length `memcpy` into the state buffer; bound
  `StateOverCap` <= 0x1000.
- the animation stream 0x33FEE0 -> 0x3A99C0: bound `animation_validate` (7.2.1),
  proven on 177,672 animated blocks of the kept worlds, 0 refused.
- the player effects `base_player::deserialize` 0x2DB490 -> 0x2DDDF0: `E = r<u32>`
  then an `E`-iteration loop; bound `player_tail` MAX_RECORDS = 0x1000/9 = 455,
  and each record spends state bytes under the 0x1000 cap.
- the weapon FSM state id 0x38CCF0: walks the state list counting to the read
  byte; past the list the target stays null -> a null-vtable call (crash). The
  anomaly state index 0x31B0A0: `state_array[read_byte]`, out of bounds past the
  state count. Bounds: the server's semantic check (`world::inventory` /
  `world::objects` refuse a state past its states), and -- because the client
  world check has no profile/object context -- the native guards (patch 113) at
  the read sites.
- player-controlled strings (chat, name, clan tag, URL, daily message): the
  engine's `packet_reader::r_string` (0.10b `packet_reader_inline.h`) reads a u8
  length `L`, does a buffer-clamped copy, then `string[L] = 0` -- the NUL write
  is NOT clamped, so `L` must be < buffer or it writes out of bounds. Every path
  enforces that: the lobby server clamps with `lobby::Writer::str`/`::wide` to
  `cut(v, buffer-1)` (NAME 63, CLAN_TAG 15, TEXT 255, WIDE 245), and the match
  0xE4 profile/kit names go through `messages::options::text`, which refuses
  `len >= buffer` (NAME 64 -> <= 63, CLAN 15 -> <= 14).

0.10b hard caps match the validator: `body_part::affects` fixed_vector<_,8> ==
MAX_AFFECTS 8; `match_options::player_profiles[20]` == MAX_PLAYERS 20; `inventory
m_slots[max_slots_count]` == SLOTS 23; `artefact_lifebone_core` four protectors.

Evidence this pass: host `world-frame` (14) and `srp-validate` (46) tests, the
i686 patch crate `cargo test --target i686-unknown-linux-gnu` (173, the client
twin: patch 87 -> `world-frame`/`srp-validate`), and the animation corpus. No
unbounded player-to-player read was found without a matching server bound or
native guard; the animation stream was the one that lacked one and now has it.

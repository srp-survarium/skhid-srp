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
The effects section is read as `world::Effects`, its effect as
`world::Effect` (the record a player's state holds too): borrowed views
whose reader takes the one form a game writes (root emitter indexed, the
effect's own time calculator, every float a `Finite`), with an accessor for
each field (`effects.rs` has the field map). The section's rules are then
plain comparisons of fields: its three free floats take any number (live
games save 0.05 in them, 2 of the 25,456 uploads of the second live sample),
its two interval ids (`current_interval_id`, `previous_time.interval_id`)
are held to 0.
A bullet's owner need not have a block in the world: a shooter who left with
bullets in flight (2 uploads of rec026g_20261106-205639_4900abd9, place 12). The
loader (`bullet::bullet` 0x32BBC0) takes the owner's player object of its current
kit from the core's place table (`game_world::get_player` 0x1F7660 ->
`network_client::get_player` 0x27A980, no bound, no mask check) and then reads its
inventory slot (0x32BDA1) and that item as a gun (0x2E2750) with no null check.
A place's objects last the match (one per usable kit of every roster place;
leaving clears only the events' masks, `on_player_left_match` 0x2D2040), so such
a bullet is allowed when its owner is under the places and, with the server's
roster kits, a roster place every usable kit of which has a gun in the slot
(which kit is current for him differs between games). The game's own check,
without the kits, bounds the place alone.
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

## Typed views of the world (from 2026-11-07)

The checks move from reading bytes at offsets to parsing each record once
into a zero-copy view: a type that borrows its bytes from the world (no copy,
no allocation, so the game's arena builds it as it does the rest), made only
by its reader, which checks its shape once, with an accessor for every field
(named by what it is, or by the native object's offset when the meaning is
not known yet). A check is then a rule over named fields. Naming every field
makes each layout claim explicit: a reader that takes a record's one form
states what that form is, where a byte comparison only said what was seen.

Each step keeps every verdict: the server's verdicts on all 50,205 live
uploads and 16 flipped copies of each (853,485 checks) are diffed against
main's before it lands, and the game's are held to the host's by the drift
gate. Done: the effects section and the player effects (`world/effects.rs`;
a player effect is now held to the whole form, not only its first byte, and
none is in any world seen). Next: the game objects, the player states, the
bullets, statistics and rules, then a typed `World` that only the parse
makes, for the server to cache and relay.

The player states (`world/player_tail.rs`, `inventory.rs`, `weapon.rs`): a
state is read into `PlayerState` (the head, the player and hit effects, the
active objects, the stamina, the inventory header with a carried battery),
then, with the match's kits, `ItemsAndRest` (each item and its body: medkit,
oxygen tank, `WeaponState` with its state machine's state and its
`Portable`; the damage model and its body parts, the usable data, the
controller, the recent hits, patch 78's carry snapshot). Every field is
named as the 0.26g writer's assert labels it, and a field the writer only
ever saves certain values of is read into that type: the active and grenade
slots (a slot or 23 for none), the artefact slots (from a quick slot 13..18
to its end), at most 3 hit effects, the jump types (0..12, a selector's
jump 1..12 with its sub-state of the same kind and, on the spot or in a
sprint, its on-site weight 1.0), a sprint animation 0, 2, 4 or 6, a
battery state's animation 0, 2 or 0xFF, a weapon seed in 1..2^31-2, an
active medkit or oxygen tank that has an owner. Every live upload and local
world still passes; of the flipped copies, 2,570 more are refused (207,392
of 853,485), none fewer. The domains the writer can widen (counts the
config sets, float ranges, the recoil seed, the spotted mask) are left open
and say so where they are read.
The views hold their records as bytes and the few facts the parse decided
(each slot's item, the shape it read in, the counts) and hand the records
out by reading them again, as the statistics do: a check holds one record
at a time, and no large value lives on the item walk's recursion. Holding
them parsed had put `PlayerState::read` at 0x1D40 bytes of the game's
stack, under `world_check::check`'s 0x51AC, and a flipped world overflowed
the loader's 32 KiB; the player path is now some 0x5B0 bytes past it.
The check's time counts too: `check_world_roundtrip_026g.py` runs the 0xE8
handler's preparation in at most 500,000 instructions, and the typed parse
had taken a recorded world to 560,218 (main: 212,595). Three costs were
not the checks': the animation bit reader took one bit a step, which the
floats table's number checks made the bulk of the work (now a byte's run
a step); the statistics rule read every place's record again for its
`last_hit_enemy_id` (now kept by the parse); a guessed player walk decoded
the head's kits and pickup record once for each item count it tried (now
once). The same world takes 324,975, the largest 349,906.
The bullets, statistics and rules (`world/bullets.rs`, `statistics.rs`,
`rules.rs`) are read as `world::Bullets` (each `Bullet`), `Statistics`
(`PlayerStatistics`, `BatteryStatus`, `Tail`) and `Rules` (`Timelimit`,
`Respawn`, `ModeRule`: `Gather`, `Tdm`, `Research`; `KillsDeaths`), and the
effect's fields have the game's names (the writers pass each field's
expression to the buffer writer as text: `.\bullet.cpp`,
`.\game_effect_player.cpp`, `.\player_shared_statistics.cpp`, ...). From
this step the parse also holds each field to its domain, once, into its
type: a byte that selects a state to its enum (a rule's phase, a team, an
effect's activation, playback and serialization mode), a bool to 0x00 or
0xFF (the bool writer's), an index to its table, a count to its bound,
and a none sentinel to an `Option`; the accessors only return those. So the
gate is: every real world still passes (all 50,205 live uploads, all 3,920
local worlds as before), and of the flipped copies this step refuses every
one main refuses and 1,826 more (206,648 of 853,485). The domains and the
ones left open are in each module's doc.

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
- **Read as typed views** (`world::GameObjects`, from 2026-11-07): each object
  is parsed once by its reader into a view named for the game class that saves
  it (`GrenadeCore`, `BoobyTrapCore`, `DamageZoneCore`, `ResupplyZoneCore`,
  `GenericAnomalyCore` with its `AnomalyState`, `ZoneGroup`s,
  `ArtefactContainerCore`s and `ArtefactBase`s), each field named by the
  string its `serialize` hands the writer (`m_trap_state`, `m_hitting`,
  `current_state_index`, `events_mask`, ...; the field map is in
  `objects.rs`). A byte with a closed set of values becomes its enum, bool or
  bounded index there, once; the accessors return those types. An anomaly's
  groups, containers and artefacts are handed out lazily, read again by the
  reader that checked them, so nothing is collected.
- **Refused by the readers** (the field's values, named in the refusal): a
  section that does not read to its exact end; a float that is not a number;
  a grenade's `current_state` with a bit past its three; a trap state past
  the four (removed, armed, fired, disarmed); a bool that is not 0x00 or 0xFF
  (`m_hitting`, `m_need_to_spawn_artefacts`, the writer 0x80FA0); more than 20
  users; a place past the table of 20, a kit past the three; an anomaly state
  past its states; zone bits past a group's zones; `events_mask` bits past
  its two; an artefact state over 5. **Refused by the match's rules** over the
  views: a user past the roster or using two objects; a container's artefact
  past the map's; an artefact owner that is no kit with a player. Left open:
  the u32 times, ids and seeds, an artefact's amount (0 or 1 live), its slot
  (13 to 18 live) and its container id (0 to 26 live): the writer copies them
  from live objects, and no bound is shown.
- **The domains' gate** (2026-11-07): every live upload still passes (all
  50,205), and so do the 3,919 local worlds `corpus2` passes on main (its one
  refusal, a place with no profile, is the player check's). Of the 16 flipped
  copies of each live upload (802,280), the readers refuse 107 more than
  main and accept none main refuses: 58 at `m_hitting`, 32 at `m_trap_state`,
  17 at `m_need_to_spawn_artefacts`.

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

The client sends one receipt per event and never drops one for time (a
wait the server keeps for a refusal must get it): Loaded/Rejected once per
generation, a handshake refusal once per refused E3/E4, and an Unreadable for
a world whose generation cannot be read (the generation the client stored is an
older world's); these two are numbered on their connection (the generation
byte: 1, 2, ...) so that the server can tell each from a copy (E0 is plain and
resent until acked). It checks packet-pool headroom
before allocation; a receipt with no room waits, the newest only, and goes on
the next frame with room (`game.tick`). Match teardown forgets both.
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

## When the check, not the game, is wrong

The check is exact, so a change of what a game writes (a patch that appends
bytes, a new item class, weapon state, map object, mode rule or record version)
or of the data it is read with makes real worlds unreadable. On 2026-11-06 the
check of 50,205 live uploads found two such layout faults (active artefact
cores, the effects floats) that every local test had missed. Four guards:

- **Before a release: real worlds, the server's own check.**
  `srp-world-check --data <server-game-data.db> <recordings, dumps or folders>`
  (`servers/match-server/src/bin/srp-world-check.rs`,
  `v026g::world_check`) checks every complete upload of match recordings with
  `world_validation::check`, the match server's own function, against each
  match's roster (its 0xE4s and header), and the kept dumps of a match whose
  recording is given with them. Exit 1 on any refusal, any world it could not
  check, or none checked. `release/release.sh` runs it in step 2 on the new
  server data with the target's newest 3 kept recordings (or `--worlds
  <folder>`), and stops before the go on any refusal. A release that changes
  what games write is checked with recordings of its own games (a match on the
  test environment; one against bots is kept), as the live games' are refused
  then. With rev 51's content data and the typed check (#1458), the 372
  non-empty live recordings of 2026-11-05..07: 175,872 worlds, all pass;
  one world with its effects' last byte changed is refused at `effects at
  0x67e9: the closing part at 12 is not every world's`.
- **At start: data the check depends on names what it lacks.** An item
  `data.type`, a weapon state class, an anomaly's artefact core already stopped
  the data; now also a playable map whose `mode.rules` are not the
  `[93, 91, <mode's rule>, 94]` the rules section is read with
  (`RuleMode::rule_classes`), no `hit_params` body part (it was 0, every state
  refused), and, when the server data is built, a client object of a
  `game_object_type` srp knows neither as saved in a world (`WORLD_OBJECTS`)
  nor as saving nothing (`resources_db::server::UNSAVED_OBJECTS`; it was
  dropped silently). The kit layout (`MatchProfile::inventory`, `held`) names
  every `ItemClass`, so a new class fails to compile until it is placed.
- **In a match: loud when many games are refused alike.** A kind of refusal
  (its part and reason) from two different players of the match is more
  likely the check or the data than the games: every honest game writes the
  same world, so a wrong check refuses every uploader's, while a cheater is
  one place and cannot put a kind in doubt alone, however often it uploads.
  One error line `WORLD CHECK IN DOUBT` with the count and the first refusal
  (section, offset, the match it was checked as), once a kind; the match's
  end line counts the uploads refused, those the fall-back relayed (below),
  and lists the kinds in doubt (`world_validation::Refusals`). A kit the check
  cannot lay out (an item the game makes an untraced object of) leaves that
  match's states read to their inventories only: said once at error level when
  the place comes, not per upload. That stays: the kit is the lobby's, the
  rest of the world is still read exactly, and refusing every world of the
  match for it would leave all its players without joins or resyncs.
- **Formats our own code writes are shared, not copied.** The carry snapshot's
  size and version are `artefact_carry::Carry::SNAPSHOT_SIZE`/`SNAPSHOT_VERSION`
  for the writer (patch 78) and `srp_validate`; the pickup record is
  `weapon_pickup::Record` for both; every playable map's 0xE4 options, `SRPX`
  block included, must pass the clients' check
  (`current_protocol.rs::the_options_of_every_map_are_accepted`).

### The fall-back to the games' own check (TEMPORARY)

The owner's decision of 2026-11-07 (option B). A refused upload goes to
nobody, unless its kind is in doubt in its match:

- **What it does.** A kind stands `Refusing { place }` (one player's uploads),
  `InDoubt { places }` (two players' or more, the fall-back armed, none relayed
  yet) or `FallingBack { places, relayed }` (`world_validation::Standing`, one
  transition, `Standing::refused`). While a kind is in doubt, an upload the
  server's check refuses **for that kind** is checked again with the context
  the game's own check (patch 87, `world_check.rs::Match::context`) uses: the
  match's places, mode, batteries and respawn points, and no profiles,
  artefacts, objects or inventory, as `srp-validate/examples/drift_dump.rs`
  builds it (`world_validation::game_check`). If it passes, it is accepted and
  relayed like any other upload (votes, the recent world, joins, resyncs); if
  not, it is refused as before. Every other refusal (a kind of one player's,
  or one the games' check refuses too) is refused as before. The upload that
  puts a kind in doubt is itself checked again.
- **How it is logged.** Each relay writes, at error level, at most one line a
  second for each player (the refusal logs' limit, counting the lines held
  back):
  `Match <id>: WORLD CHECK FALLBACK: player <p>'s upload at <t> ms, refused as
  <part> at <offset>: <reason>, passes the game's own check and is relayed
  (temporary, see packet-validation.md)[; <n> more relayed by the fall-back
  since the last line]`. The upload is kept as a `fallback` dump (within the
  match's doubtful-dump limit), which `srp-world-check` then checks with the
  rest. The match's end line has `..., <n> of them relayed by the fall-back`,
  and each kind in doubt `(<players> players, <n> relayed)`.
- **Why it is safe.** Every game that receives a world runs this same check
  before it loads it (patch 87) and refuses one that fails it, so a relayed
  world is one no receiving game refuses: the fall-back lets nothing in that a
  game would not take from a world the server relayed anyway. It needs two
  different players' uploads refused alike, so one forged or broken game
  cannot turn it on.
- **What it gives up.** For that kind in that match only: the server's
  context-only checks, the rules the games cannot make without the match's
  roster: the profiles (usable kits, item counts), the game objects against
  the places' grenades and traps and the map's objects, and the kits' items
  to a state's exact end. A forged world that breaks only those, in a match
  where the kind is in doubt, reaches the other games.
- **When it goes.** It is temporary: once live releases show no `WORLD CHECK
  FALLBACK` line (the check is right for every world the games write), it is
  removed, the doubt detector kept. Removing it is deleting the `TEMPORARY`
  code: `Standing::FallingBack`, `Answer::Relay` and its arm in
  `game.rs::uploaded`, `game_check`, the peer's `fallback_log`, and making
  `InDoubt` refuse.

## The world corpus

Every change of what the game writes into a world, or of how the server or the
game reads one, is checked against real worlds by `tools/check`. The worlds
are `private/world-corpus/` of the main checkout (private: the recordings carry
players' names and accounts; never committed):

- `live/`: live match recordings (`rec026g_*.srprec`), copied by
  `release/world-corpus [--newest <n>]` (default 100): the host's newest
  recordings it does not have, finished ones only, in tar batches of 10 read
  under `nice -n 19 ionice -c 3` with 30 s between them (the host has one vCPU
  and runs the matches), through `release/lib.sh`'s way to the host. Reads only.
- `local/<config>/`: the local test matches. `srp-026g/scripts/local-clients/
  run.py`'s server records every match there from one player on
  (`SRP_RECORD_DIR`, `SRP_RECORD_MIN_PLAYERS=1`), so a feature's own test games
  are checked too; on by default (its README).
- `dumps/<config>/`: the kept world dumps of the local runs
  (`private/local-clients-*/runtime/snapshots`, copied once on 2026-11-07,
  14,748 worlds). They have no recording, so only `world-drift` reads them.

Two rows of `tools/checks.tsv` read it (`tools/checks/worlds.sh`); the corpus is
in their key (`{main}/private/world-corpus/**`), not in the diff: a refresh
alone does not select them, `tools/check world-check world-drift` runs them.

- `world-check`: this tree's server game data (`release/build-content`'s quick
  build of the next patch; the offered-patch list's when the manifest ships
  nothing new), then `srp-world-check` of every recording in `live/` and
  `local/*/`. One world refused or not checked fails it. Inputs: srp-validate,
  world-frame, the shared format crates, the survarium modules the context comes
  from, resources-db, the check and the recording reader, the content sources,
  the Rust patches and the corpus.
- `world-drift`: the game's own check against the host's
  (`check_world_check_026g.py --drift` on srp-validate's `drift_dump`) over a
  sample: one upload in 50 of each live recording (`WORLD_DRIFT_ONE_IN`),
  written out by `srp-world-check --worlds <dir> --one-in <n>`, every upload of
  the local recordings and every dump, in parallel chunks
  (`WORLD_DRIFT_JOBS`, default the cores less 4). Needs the original exe. Inputs:
  the arena's sources and lock, the exe build, the host check and the corpus.

On 2026-11-07 (main 1287a0f61 with this tooling; the machine shared with
another drift run) the corpus held 481 revision-50 live recordings (2.4 GB)
and 14,748 local dumps. `world-check` passed 241,785 worlds in 278 s, with
build-content cached (858 s with an uncached build). `world-drift` passed
19,832 worlds (5,084 sampled live uploads and the dumps, 18 loads each with
the mutated copies) in 20 chunks in 1,214 s.

A release that changes the world format refuses the older recordings: move them
out of `live/` and refresh, with recordings of the new revision's games, before
the next change is checked.

## Performance evidence and limits

An optimized host-library benchmark over 1,786 captured worlds measured 3.25 µs
median, 5.28 µs p99 and 6.83 µs maximum. All captures use battery mode; most have
sixteen players. On the 3,920 current worlds: 4.53 µs median and 8.56 µs maximum
for 13 to 20 players (`bench --quick`); a 20-player world grown to 0xF000 bytes of bullets 9.1 µs.

Those numbers predate the objects, the whole player states and the
animation streams. `world_check_bench` (match-server example) times the server's
whole `world_validation::check`, its late-joiner retries included, on every
upload of match recordings against the roster the server had when it came
(the header's initial places, then each `places` note), and the game's check
(patch 87's context) on the same worlds. On 2026-11-07, one desktop core, 20
calls a batch, over the 50,205 uploads of 98 revision-50 recordings and 3,183
kept local worlds whose match was recorded:

| | server median | p99 | max | a match in all (median, max) | game median | p99 | max |
|---|---|---|---|---|---|---|---|
| uploads, `packets-v2/11-diagnostics` | 2.1 µs | 3.6 µs | 20 µs | 1.1 ms, 2.3 ms | 3.7 µs | 7.3 µs | 21 µs |
| uploads, `packets-v2/14-animation-states` | 74 µs | 124 µs | 398 µs | 41 ms, 82 ms | 22 µs | 42 µs | 119 µs |
| uploads, with the sorted compatibility | 55 µs | 89 µs | 277 µs | 30 ms, 63 ms | 21 µs | 38 µs | 59 µs |
| kept worlds, 11 / 14 / sorted | 2.4 / 88 / 58 µs | 4.4 / 135 / 78 µs | 5.6 / 177 / 98 µs | | 4.8 / 25 / 23 µs | | |

A match's uploads cost at most 4 ms of the match thread a minute of play
(5.4 ms before the sorted compatibility). Where an upload's 74 µs went (1,283
uploads of two 11 to 18 place matches): 35 µs making every place's kits
(`MatchProfile::inventory`, mostly `Dictionary::compatible` walking its 356
pairs twice for each ammo stack; 23 µs once they are sorted), 10 µs each
place's grenades and traps (`MatchProfile::held`), 20 µs `world::validate`
(13.5 µs of it the frame, which walks every player's animation stream bit by
bit; 5.4 µs the player states; the objects 0.2 µs). Both kit tables are the
match's own and made again for every upload; making them once a roster would
leave the 20 µs. The late-joiner retries cost nothing measurable: every upload
passed at its first count of installed places. The game's check, built as
the arena builds it (i686, pentium4, opt-level s, fat LTO; on the host, Linux),
takes 5.3 µs median before and 32 µs median, 57 µs p99, 100 µs max now, once a
world load: the animation streams. The first check of a process may also make
`item_params::all()` (about 0.1 s here): `game_data::init` does not make it
(`tables::check` derives its own copy), the first user does.
With `--flips`, the bench also checks 16 one-byte-flipped copies of each
world: the sorted compatibility changes none of the 854,208 verdicts
(217,304 refusals) of the uploads and kept worlds, nor any of the worlds'.
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
- the damage model `body_part_parameters::deserialize` 0x32AFE0: a u8
  `affects_count`, then that many (byte, u32) affects into the fixed `m_affects[8]`.
  The native "affects_count buffer overflow" assert at `> 8` does NOT guard a
  release build (no debugger, assert continues -> the loop overflows the array),
  so the bound is the validator's: `inventory` refuses affects > 8 (type >= 9),
  hits > 64, recent hits that do not fit (0x2E5480 reader). The inventory's
  carried-item and active/target-object ids index 255-entry tables with a null
  check (a u8 is always an entry or guarded none, safe by construction); the
  artefact ids index a 128-entry table with NO null check, so `artefact_index`
  bounds them (< `context.artefacts`), and `slots_begin + A <= 23` bounds the
  write base.
- player-controlled strings (chat, name, clan tag, URL, daily message): the
  engine's `packet_reader::r_string` (0.10b `packet_reader_inline.h`) reads a u8
  length `L`, does a buffer-clamped copy, then `string[L] = 0` -- the NUL write
  is NOT clamped, so `L` must be < buffer or it writes out of bounds. Every path
  enforces that: the lobby server clamps with `lobby::Writer::str`/`::wide` to
  `cut(v, buffer-1)` (NAME 63, CLAN_TAG 15, TEXT 255, WIDE 245), and the match
  0xE4 profile/kit names go through `messages::options::text`, which refuses
  `len >= buffer` (NAME 64 -> <= 63, CLAN 15 -> <= 14).
  - EXCEPTION FOUND AND FIXED: a clan member's display name in
    `read_clan_info` 0x271FD0 goes into a 32-byte record field (stride 0x22,
    name at +0..+0x20, confirmed at 0x272178 `lea esi,[ebx+ecx*2+0x1ec838]` /
    `[esi+0x20]`), the same 32-byte buffer the rank name uses (`RANK_NAME` 31).
    But `social::write_members` and the two inline clan-member loops wrote the
    name at NAME (63), so a clan member whose account name is 32..=63 (NAME
    allows it) overflowed every other member's 32-byte field when they read
    clan info -- a one-player-crashes-another bug. Fixed by a new
    `limits::CLAN_MEMBER_NAME = 31` used by the three writers and the matching
    `client_model` reads; clan tests and clippy pass.

0.10b hard caps match the validator: `body_part::affects` fixed_vector<_,8> ==
MAX_AFFECTS 8; `match_options::player_profiles[20]` == MAX_PLAYERS 20; `inventory
m_slots[max_slots_count]` == SLOTS 23; `artefact_lifebone_core` four protectors.

Evidence this pass: host `world-frame` (14) and `srp-validate` (46) tests; the
i686 patch crate `cargo test --target i686-unknown-linux-gnu` (173, the client
twin: patch 87 -> `world-frame`/`srp-validate`); the animation corpus (177,672
blocks, 0 refused); and the client/host drift gate `check_world_check_026g.py
--drift` run end to end under Unicorn on 1,101 kept worlds (501 old version-4 +
600 current) with their sixteen mutations each -- 19,818 checks, 0 differ, so
the i686 arena build (with `animation_validate`) and the host agree on every
world and mutation. After the animation-float and player-effect checks landed
the arena was relinked (`link_026g.py --write`) and the drift re-run on the 600
current worlds -- 10,800 checks, 0 differ -- so the deployed i686 arena carries
and agrees on the new checks too. The animation stream and the E player-effect
records were the two world reads lacking a bound; both now have one. A third,
in the lobby, was the clan-member display name (NAME 63 written into a 32-byte
client field); fixed with `CLAN_MEMBER_NAME` 31 (above).

### Full reader sweep

Decompiled 235 functions (the cited reader addresses and their closure);
classified each: 65 advance a packet cursor (readers), 170 do not (serializers
-- the `..::serialize` writers that only call the debug-check/write helpers --
plus math, vtable thunks and Scaleform glue, which never consume received
bytes). The 65 readers' loop bounds fall into:

- linked-list / bitmask traversals (`while (x != 0)`, clearing set bits):
  0x2714D0, 0x2F84C0, 0x319470, 0x316D50, 0x330A20 -- not a packet count;
- fixed constants (`< 8`, `< 0x58`, `< 0x18`, `< 3`): e.g. 0x3358E6, 0x31B390;
- object/config counts (`< *(byte*)(this + k)` where `k` is a field set at
  construction from the item/weapon/character config -- body parts, weapon
  curve points, FSM states): e.g. 0x2F5C22, 0x33DCA0, 0x33E7F0, 0x2E3490. The
  packet supplies the values; the count is the entity's own and the validator
  reads the matching bytes against the same config;
- packet counts the validator caps: the 0xE4 options `< players_count`
  (0x27B880, `messages::options`), kd-stats `< roster` (0x323952, `rules`),
  the ammo id / FSM state / affect & hit counts above;
- a byte that indexes a global object table, read then used only after a null
  check -- the 255-entry tables are safe for any u8, the 128-entry artefact
  table is bounded by the validator: 0x301660 (battery portable object,
  `[+0x130][id]`), 0x31C5B0 and the inventory 0x2FE550 (artefact `[+0x18940]
  [id]` < `context.artefacts`), 0x2E3490 (weapon ammo id < list);
- fixed-size records memcpy'd with a constant length (no packet length):
  0x344D70, 0x3053C0, 0x301660 (`memcpy 0x40`), and pointer values used only
  after a null/alignment check (0x338430, 0x344870).

Each in-scope reader above was read in the decompilation and its bound line
confirmed; the account-only lobby readers (status, inventory, prices, skills,
shop, quests, reputations, drops) are server->self and carry no other player's
data, so they are outside the one-client-crashes-another scope.

The covering invariant: the validator reads every player state and section to
its EXACT END against the match config (proven on 50,205 live uploads and
3,919/3,920 local worlds), so it consumes the same bytes as the native reader,
and it caps every packet count to the native fixed-array size (affects 8, hits
64, slots 23, ammo 8, artefacts 128, players 20, FSM state < the list). The
affects cap is load-bearing and shown at the site: `body_part_parameters::
deserialize` 0x32AFE0, read in full, reads a byte `affects_count` and its
"affects_count buffer overflow" assert FALLS THROUGH in release (`DAT_02260BB7`
off, `swi(3)` only under a debugger), so `> 8` overflows the fixed `m_affects
[8]` in the following loop -- the validator's `affects <= 8` refuses that world
first. The player head `base_player::deserialize` 0x2DB490, read in full, reads fixed
angle/position/state fields then the byte-indexed weapon table and the place-
indexed tables (`[+0x115]*0x704`, place < 20) and calls the sub-readers above;
no unbounded packet count or index. The inventory reader 0x2FE550, also read in full, indexes the weapon
table `[+0x18B4C]` and the artefact table `[+0x18940]` by a byte, null-checked
and validator-bounded (artefact id < `context.artefacts` 128). A count
it does not cap is one the native code takes from its own config, not the
packet. The client string readers are the one exception to "read to exact end":
`read_squad_info` 0x274720 reads a member name as `len = r<u8>; memcpy(dst+0x10,
src, len); dst[0x10+len] = 0` into a 0x50-byte member record -- unbounded like
0.10b's `r_string` -- so the server bounds it: `social::squad_info` refuses
more than `SQUAD_MEMBERS` (3) members and clamps each name to NAME (63) with
`Writer::str`, matching the 3 x 0x50 record array and the ~64-byte name field.

### State corruption, not only indices

A crafted value can corrupt a receiving client without being an out-of-bounds
index. The validator handles these classes, verified against the readers:

- **non-finite floats** (NaN/infinity into a transform or physics): `finite`
  (`(bits & 0x7F80_0000) != 0x7F80_0000`) gates every world float -- the player
  head's pitch/rotation, the inventory item condition, the weapon
  recoil/dispersion/aim, the battery motion, the stamina and char-controller
  vectors (`player_tail`/`inventory`), the bullet numbers (`bullets`) and the
  effect template floats (`effects`). The one reader that skipped it was
  `animation_validate`; it now finitizes the floats-table values too
  (`snapshot_rules.md` 7.2.1 #6), so a non-number cannot reach the bone
  transforms the ragdoll and bounding box read.
- **raw pointers / handles, and the table indices behind them**: the effect
  record's two pointer flags (root emitter, time calculator) are forced to 0
  so a serialized pointer is refused, and the index u32 that each flag gates is
  forced to 0 so the native reader (0x2E5C30) cannot read an unbounded global
  table (0xAA2E48 / 0xAA2E74) with a refcount write. Read in full this pass,
  `game_effect_player::deserialize` 0x2DDDF0 shows it exactly: it loops
  `local_c = game_state_E + base_E` records and, per record, when the flag byte
  (at +0x24) is clear with a non-zero index (+0x25) does
  `local_40 = [PTR_DAT_00aa2e74 + index*4]` (the unbounded read), and when the
  flag is set does `(**(code**)(*local_40 + 8))()` (the raw-pointer vtable
  call). `effects::record_is_safe` forces both pairs to 0; `player_tail` caps
  the count at MAX_RECORDS. The effects SECTION always
  bounded this (its template), but the E PLAYER-EFFECT records did not --
  `player_tail` only checked the leading flag and length-walked 67 bytes, so a
  crafted state with E >= 1 (E is 0 in every world seen) reached the unbounded
  read. FIXED: the record's crash-critical fields are modelled once in
  `effects::record_is_safe` (both flag/index pairs zero, the three floats
  finite) and `player_tail` reuses it. The fix changes no verdict on 600 kept
  worlds (E is 0, so it refuses no real world).
- **bytes used as booleans**: every one must be 0x00 or 0xFF (`reader::bool`,
  `TailBool`), not merely non-zero.
- **type / ownership confusion**: ids are checked against the kit, dictionary
  or slot they must belong to; a bullet owner must be an active block, a
  carried battery's user must be this player and its container one of the two,
  an artefact owner a kit with a player.

### Client string readers (the chat/name path)

Every 0.26g client reader of a player-controlled string uses the unbounded
`len = r<u8>; memcpy(buf, src, len); buf[len] = 0` pattern into a FIXED buffer
(decompiled, read in full), and each has a server clamp fitting that buffer:

```
chat text 0x26B6F0: name[64], clan[16], body[256]  <- chat_text str(NAME 63/
                                                       CLAN_TAG 15/TEXT 255)
squad     0x274720: name ~64 in a 0x50 rec, 3 recs  <- squad_info at_most(3) +
                                                       str(NAME 63)
clan info 0x271FD0: clan name[64] tag[80] rank name[32] MEMBER name[32]
                                                    <- clan_info str(NAME 63)/
                                                       str(79)/ RANK_NAME 31/
                                                       write_ranks/write_members
                                                       CLAN_MEMBER_NAME 31 [FIX]
                                                       + wide(245)
elo list  0x271EB0: name[64] per row, count->vector  <- elo_rows str(NAME 63)
kit (0xE4) 0x1F112A: name[64]                       <- options::text(64) refuses
                                                       len>=64
```

The client reader is unbounded, so the server clamp is load-bearing. The lobby
readers otherwise carry only ints (free display values) and sized/`at_most`
counts -- no raw pointer, bool or float surface.

The lobby dispatcher `lobby_client::on_message_received` 0x274A00 is one large
function with every id's case inlined (its jump table at 0x275138 maps ids
0x80..0xA1 to internal labels, not separate functions, so the decompiler's
flow limit rejects the whole body); ChatText 0xA2..0xA4 branch out of it before
the table to `messaging_client` 0x26ABE0 -> text 0x26B6F0. Disassembling the
table and the body, each case calls a named `lobby_client::read_*` reader.
Most of them are the player's OWN account state the server sends it -- status,
inventory, prices, skills, service prices, reputations, elo, quests, match
stats, account services, shop, drops -- which no other player can inject, so
they are outside the one-client-crashes-another scope. The cases that do carry
another player's data are the ones audited: a squad member's profile
(`player_profile::deserialize_static` 0x1F1070 -> name into +8 [64], clan into
+0x48 [+0x48..+0x56], both the `len`+memcpy+NUL pattern, clamped by the server
to NAME 63 / ACCOUNT_CLAN_TAG 14, kits from the validated account), the clan
roster (0x271FD0), the elo list (0x271EB0) and chat (0x26B6F0) -- all with a
server clamp fitting the client buffer.

All eighteen `read_*` readers the dispatcher calls were decompiled and read.
The account-only ones read ints and field/own counts, or memcpy into a sized
allocation: `read_player_reputations` 0x271740 (`memcpy` of `count * 8` into an
alloc of that size), `read_enumerate_inventory_info` 0x270400, `read_player_skills`
0x270570, `read_price_items` 0x271540, `read_shop_items` 0x271650,
`read_account_services` 0x2714D0, `read_match_stats` 0x2718C0 (a growable
buffer), `read_player_match_stats` 0x271A30, `read_player_elo_rating` 0x271D90,
`read_online_stats` 0x271860, `read_drop` 0x271AE0, `read_service_prices`
0x271480, `read_status_info` 0x270100 (fixed `memcpy 0x28`), `process_own_account`
0x270260. One, `read_quest_list` 0x271DF0, reads an unbounded u32 quest count
and grows a buffer to `count * 0x14` -- an unbounded read, but it is the
viewing player's OWN quest list (server->self, from the DB), which no other
player can send, so it is outside this scope; our server sends a real,
bounded list. The line the whole audit rests on: a reader of the player's own
or public data may trust our server; only a reader of ANOTHER player's data
must be bounded, and every one of those is.

All three reader groups -- world sections, player-state deep readers, and the
match-message + lobby readers -- were read in full. THREE unbounded or
corruptible player-to-player values were found without a server bound or native
guard, and all are now fixed: the animation stream (counts, indices, recursion,
tree length, table under-run, and float finiteness), the E player-effect
records' embedded table indices + flags + floats (`effects::record_is_safe`),
and the clan member display name (NAME 63 into the 32-byte client record,
`CLAN_MEMBER_NAME` 31). Everything else was already bounded; the state-
corruption classes above (non-finite floats, raw pointers, bools, type and
ownership confusion) are covered as well as the index bounds.

### Enumerated another-player closure

"Investigate all places" is demonstrable because the SOURCES of another
player's data are a fixed, enumerable set, and the closure is whatever each
dispatches to. There are exactly three entry points, each enumerated above:

1. The world snapshot body `0xE8` (`game_state_history_item::deserialize`
   0x324D80): a fixed six-section mask (bullets, statistics, rules, objects,
   effects, and the per-player state blocks) plus the apply dispatch 0x2D2770.
   Every section reader and every player-state sub-reader is listed and bounded
   above; `Frame::parse` reads each to its exact end against the match config.
   Read in full: 0x324D80 walks the active-player u32 mask (bit indices 0..31
   into the 32-player 0x9054-stride array -- a popcount, not a packet count)
   and reads each section as a u16 length into a GROWABLE (doubling) buffer, so
   the length cannot overflow; 0x33FD80 grows-then-memcpys the per-player state
   the same way, 0x33FEE0 routes the animation to `animation_validate`. The
   0xE4 options native `match_options::deserialize` 0x1F0E70 loops a FIXED 20
   profiles (0x15DEC / 0x117F = MAX_PLAYERS, not a packet count), each the
   clamped `player_profile::deserialize_static` 0x1F1070, then fixed fields.
   Gaps found here: the animation stream and the E records -- FIXED.
2. The match messages `0xE3..0xF6` (`srp_validate::messages::check`): lengths,
   place indices, input rotation/actions, envelopes, lzma2 output caps, and the
   0xE4 static options (kits/items/slots/mods/upgrades/name<=63/clan<=14). No
   gap. The native readers, read in full: the dispatcher
   `on_match_packet_received` 0x27A300 is a `switch(id)` gated by the handshake
   phase `[+0x1F1C64]` (0->1->2), each case a per-id handler; its 128 KB stack
   buffer is the decompression target for 0xE4/0xE8, which `lzma2_bound`
   (`OPTIONS_CAP` 0x578A0 / `SNAPSHOT_STATE_CAP`) caps before the decoder runs.
   The input path reads fixed node fields (0x318400) and applies the rotation
   by ADDING its two IEEE halves to the player angle (0x2D86E0 -> 0x2D8AD0, and
   the merged node 0x2C6640) -- bounded by `rotation_is_sane` (each half finite
   and < 1024) and the `UNUSED_ACTIONS` bit, so no NaN/Inf or stray action
   reaches the angle. `enter_match` 0x2D1470 guards the place byte `< 0x14`
   before indexing the 20-place table (stride 0x45FC) and runs the resync/too-
   old time checks; `items_dictionary::item_by_id` 0x1F0090 is a lookup that
   returns NULL (not an array index) on an unknown id, null-checked by callers.
3. The lobby dispatcher 0x274A00: ids 0x80..0xA1 (jump table) + 0xA2..0xA4
   (chat). All 18 `read_*` cases enumerated above; only chat 0x26B6F0, squad
   0x274720, squad-member profile 0x1F1070, clan 0x271FD0 and elo 0x271EB0
   carry another player's data, the rest are server->self account state. Gap
   found here: the clan member name -- FIXED.

Nothing else can deliver another player's bytes to a client: a reader not
reachable from one of these three sources reads the player's own or the
server's data, which it may trust (the scope line above). So the closure is
closed, and every member of it is bounded or fixed. The one thing not shown as
a single decompiled function is the dispatcher 0x274A00 itself -- its 70-byte
body computes a jump into inlined cases, which defeats the decompiler (the
process dies even at a 4M-instruction cap); it is read instead by disassembling
the whole 0x274A00..0x275138 range, every case shown.

### Deep-state readers read in full

The deep weapon/battery/stamina/artefact readers, read in full in the
decompilation (not grep lines):

- `weapon_core::deserialize` 0x2E3490: the ammo id `[+0x5ac][bVar2]` and the
  next-ammo id are bounded by `inventory`'s ammo-list check, the fire-mode
  byte `[+0x5EC]` is an `!= 6` enum, the recoil/dispersion/aim floats are
  finitized, and the weapon FSM goes through 0x38CCF0 (native guard +
  `inventory` state bound). No unbounded value.
- `player_stamina::deserialize` 0x329400: five free u32 times, a bool, and two
  floats (validator-finitized); the record slots `[+0x54]`/`[+0x64]` are
  pre-set fields, not read from the packet. No index from the packet.
- the artefact cores 0x3119E0 / 0x3124B0 / 0x312AF0: read their own bytes (a
  u32, or Onyx a u32 and a finite f32) only when the state the base reader set
  is 4 (active), which `objects` bounds to <= 5. No index.
- the double-weapon sub-state 0x33E7F0: its `while (.. < [+0x290])` loop reads
  nothing from the packet -- it initializes `[+0x290]` pre-sized sub-records
  (a construction count, not a packet count).
- the animation-channel state 0x334FB0: the outer count is an engine vector
  size; the inner loop reads a bitmask then one value per set bit into a
  config-sized 0x40-dword record, the bit count `[+0x577B5]` a global config.
- the battery `victory_item_core::deserialize` 0x301660: the container byte
  indexes `[+0x130]` and is bounded by `player_tail::battery` (< the two
  containers) with a null check; a fixed `memcpy 0x40`; state/floats/ownership
  via `battery`.

More, read in full: the damage-model entry 0x2E5480 is a bool, four u32 and a
LINKED-LIST walk of the character's config body parts (`while (p) p = *p;`
calling `body_part_parameters::deserialize` 0x32AFE0 each) -- the count is the
list length (config), not a packet count; the hit-effects reader's format
(serializer 0x3402C0) is `H` records bounded by `player_tail`'s
`H <= MAX_RECORDS`; the portable object 0x2988A0 delegates to the selector
reader 0x2FF970 (states bounded). Note 0x2FAB70, flagged by the advance-cursor
heuristic, is not a reader at all: it is the player-logic state-machine
constructor (allocations + vftables, no packet cursor), so some of the 65
"readers" are constructors the heuristic over-counts.

The section readers and their serializers (which show the wire format and its
counts) were read in full too: the rule cores (tdm 0x316340, respawn 0x316F10,
timelimit 0x317F30/0x317FD0, gather 0x313CA0, match-time 0x3149B0), statistics
(shared 0x2F5AC0 -> player 0x335750, victory-item event 0x334DC0), the
victory-item containers (0x31B8E0/0x31CE40), the weapon sub-states
(0x32FD60/0x32FF70/0x337510/0x338430/0x33DCA0), battery motion 0x344870,
grenade 0x33A570, the artefact base state 0x313760. Every loop bound in them is
either a fixed constant (7, 3, the 23 selector states in 0x2FC7D0,
0x58/0x18 event tables in 0x335750) or a config/vector size (the player count
`[+0x31E0]`, respawn points, the victory-item vectors, the `[+0x577B5]` channel
count) -- never a u32 count read from the packet. Byte fields are stored as
enums (compared, not used as array indices) or index >=256-entry null-checked
tables; state bytes dispatch through the FSM vtables guarded by patch 113.

The per-player snapshot apply dispatch 0x2D2770 is bounded by construction:
every index is a bit position of the active-slots u32 mask `[+0x18FD8]` (0..31)
into the 32-entry slot table (`* 0x30`) and the 32-player state array
(`* 0x9054`); it reads state bytes from the already-parsed, already-validated
per-player buffers (not the packet cursor) and dispatches them through the
patch-113-guarded FSM, running downstream of `Frame::parse` with its effects
tail going through 0x2DDDF0 (the fixed E-record path). Note 0x336E60, 0x303820
and 0x31AD50, like 0x2FAB70, are list-registration/constructor/empty functions,
not deserializers -- the advance-cursor heuristic over-counts them among the 65.

The validator also cites four sub-addresses that are NOT reader entry points
(so a decompiler keyed on function starts skips them): 0x2F5B08..0x2F5BCB is a
range inside `shared_statistics::serialize` 0x2F5AC0 (its `m_team_points[0]`
call site and its closing `w<u32>` + `ret`); 0x313D84 is the carrier-exists
branch (`((int*)*i)[2] == 0`) inside `gather_victory_items_rule_core::serialize`
0x313CA0; 0x2D2BB0 is inside the apply dispatch 0x2D2770 (objects.rs calls it
"the walk 0x2D2BB0.."). Each containing function is read in full above.

So the goal's actual scope -- readers of another player's data -- is read at
the site in full and bounded; the only two that were not are the animation
stream and the E player-effect records, both now fixed.

The lobby dispatcher 0x274A00 cannot be decompiled as one function: its own
body is 70 bytes (the `cmp al,2; ja` head and the `jmp [ecx*4 + 0x275138]`),
and the case blocks are reached only through that computed jump, so the
decompiler's flow follows into them and the process dies even with the
instruction cap raised (a tool limit, not a coverage gap). Read instead by
disassembling the whole 0x274A00..0x275138 range: the head, then each case,
which reads a few fixed fields and calls one named `read_*` reader (all read
above). It has exactly one inline string read -- 0x274F44, `len = r<u8>;
memcpy([esp+0x84], src, len); buf[len] = 0` -- the server's "operation denied /
important message" text, a server->self system message clamped by `TEXT` /
`STATUS_TEXT`, not another player's data. No case has an inline unbounded
player-to-player read.

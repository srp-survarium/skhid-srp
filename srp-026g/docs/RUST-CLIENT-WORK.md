# Rust client transition work log

Canonical plan: [PLAN-RUST-CLIENT.md](PLAN-RUST-CLIENT.md). Newest session first.
Statuses distinguish implementation, host/native testing, real play and release.

## 2026-11-04 — resync retry clock PR reviewed and merged

Reviewed PR #1157 against main `7f73a839e`; no blocking findings. Preserving
the assigned-clock bound across unanswered generations prevents repeated
100 ms safety-margin accumulation. Added regressions applying timing payloads
through the client clock/generation model in all six delivery orders, and
rejecting ordinary/joined sends with an existing bound and full reply history.
The proposed production behavior is unchanged by this review.

Integrated head `58873b55b` passes 572 match-server tests (19 existing ignored),
one doctest, targeted match-server Clippy with `-D warnings`, and the diff
whitespace check. Merged as `5cc8ed055` at 03:07:23 UTC. Evidence and limits:
`private/clock-retry-review-20261104/`. These are server/client-model tests;
no native client, played-match or live deployment claim. Merged, not deployed;
no migration gate or plan decision changes. #1156 remains separate.

## 2026-11-04 — battery-marker PR reviewed and merged

Opened and reviewed PR #1193 on the owner's request, isolated from the dirty
workspace onto current main `1bb06e094`. Applied only the carry-state change,
the permanent regression and three native-observation rows; retained main's
newer research-marker code. No blocking review findings. Merged as
`7f73a839ecb16a26f607934da2c5737a0d11cf29` at 02:43:49 UTC.

The exact PR movie passes all 17 Ruffle assertions; current-main baseline fails
only hidden drop/pickup. Movie SHA256:
`09e8e8a5fe4a6194e41ded756ab33fd2b16bbcfaf625094433e5bd67059d71a8`.
ShellCheck and whitespace checks pass. Full `release/build-content 46` passes
on the isolated checkout, including all 18,773 manifest files, a local patch
and server game data. The earlier workspace-only Serbian failure is absent.
Evidence: `private/kube-battery-20261103/pr-regression-{baseline,fixed}/`,
`pr-content-build.log`, and `pr-content-validation-46/`. Scratch number 46
does not reserve or release a revision. The generated content also includes
other changes already on main; it is not a battery-only deployment package.

Merged and tested; not played, uploaded or released. Kube's native trigger
remains unconfirmed. No migration gate or plan decision changes.

## 2026-11-04 — battery-marker UI fix integrated locally

On the owner's go, moved `modifier_visible` consumption in
`HUDIconLayer.set_player_icon_props` before the visibility/filter gates. Partial
carry-state updates now reach the marker even when hidden. The existing setter
only changes its battery child; it does not reveal the marker or whole HUD.
No executable, simulation, revision or release-manifest changes were needed.

Added `scripts/player-marker/check.sh` and `BatteryMarkerHarness.as`: 17 checks
against the source-built movie in CPU Ruffle/lavapipe. The baseline fails exactly
the hidden-drop and hidden-pickup assertions; all 15 controls pass. The fixed
movie passes all 17. ShellCheck and Bash syntax pass. Evidence:
`private/kube-battery-20261103/regression-{baseline,fixed}/ruffle.log`;
fixed movie SHA256
`21862deb5cc9789d8f36a65714d045173d7c3dd3e00171bc0e95f47b289a898e`.

`release/build-content 46 --out private/kube-battery-20261103/content-validation-46`
rebuilt `player_icons.swf` with that same hash, then failed in the unrelated
`serbian-1.json` edit on missing `st_srp_equipment029_*` translations. Full log:
`private/kube-battery-20261103/content-build.log`. No content patch was made;
46 was only a scratch validation argument, not a reserved or released revision.
Implemented and movie-tested; not played or released. The native reachability
limitation below remains: this fixes the reproduced Flash contract bug, without
establishing Kube's historical cause. No migration gate or plan decision changes.

## 2026-11-04 — battery-marker reachability correction and side-effect checks

The owner challenged the hidden-marker explanation and asked for an in-game
reproduction. Further native inspection shows that ordinary off-screen or dead
players return before the carry-modifier comparison: `player::render_name`
branches at 0x1F2B7C, and failed projection returns at 0x1F2C0C. Successful
projection sets visible at 0x1F2C25 before the comparison at 0x1F2D7E. Thus
looking away while a teammate drops a battery, then looking back, is **not a
verified repro**: the ordinary path can send the current modifier on return.
The earlier harness injects a combined `visible:false,modifier_visible:false`
update. Reachability of that combination in Kube's actual native execution
remains unproved; the candidate must not be described as his confirmed fix.

Expanded Ruffle checks (`private/kube-battery-20261103/MarkerCases.as`) reproduce
both stale-on after hide/clear and missing-on after hide/pickup. The candidate
fixes both. Ten control checks pass on baseline and candidate: ordinary pickup
and drop, reveal with current modifier, hidden player/enemy/object visibility,
and whole-HUD hiding. The real object renderer has neither textField nor
icon_box, so the candidate's additional setter call there is a no-op. Recreating
an existing marker id leaves the old child attached on both versions: a separate
UI lifecycle observation, not a demonstrated cause of Kube's moving marker.

`ManualBattery.as` and `repro.sh [baseline|candidate]` provide an interactive
UI-only repro, keys 1/2/3 and 4 to explicitly clear. The candidate is still
private, not applied, packaged, played or released. Next investigation must
establish a native event sequence or capture the actual Flash feed before
claiming an in-game repro. No migration gate or plan decision changes.

## 2026-11-03 — Kube battery marker investigation (no migration gate change)

The owner reported Erik seeing a battery above Kube despite empty hands, and
authorized packet inspection. Match `0x4a96f894`, revision 45, Erik place 8,
Kube place 12: the completed recording has 1,188,694 records, 418 reconstructed
worlds, no decoder errors and a closing dropped-record count of zero. Both
players' final server summaries report zero resyncs and zero unacknowledged
relays/roster changes. Erik's 16,066 and Kube's 15,919 comparable reported hash
ticks all match the majority. This does not establish absence of network loss.

The captured static profiles decode 16 times without errors; Kube has 21 items.
The observed version-8 player prefix and native inventory layout allow 388
Kube states to be walked (zero game/hit effects in these samples), with the
world tick and carried battery's owner/container cross-checked. At ticks
744750 through 789750, all 20 samples show no carried item. The recording
contains simulation state, not Erik's Flash calls or camera visibility.

The native marker feed emits changed properties and clears its dirty flags.
`HUDIconLayer.set_player_icon_props` ignores `modifier_visible` when the same
update has `visible:false`. A subsequent visible update omits the unchanged
modifier, retaining the old battery. Ruffle/lavapipe reproduces that sequence
with the current source's movie. An isolated candidate that applies the
modifier before the visibility gate clears it in the same harness.

Evidence and candidate diff: `private/kube-battery-20261103/`. The candidate
is private, compiled and harness-tested, not applied to production source,
packaged with build-content, played or released. The exact visibility sequence
on Erik's client is not captured, so this is a reproduced mechanism consistent
with the report, not a proven historical UI trace. Plan decisions are unchanged;
no migration gate advances. Next: integrate the candidate with a permanent
movie regression, run the affected movie/content checks, then release only on
the owner's go and verify in a game.

## 2026-11-03 — live match packet investigation (no migration gate change)

Follow-up: reviewed and merged bullet-bounds PR #1154 at `cf008376c` on
2026-11-03 21:13:54 UTC, integrating main through `5dff25a97`. The native
bounds regression, eight world fixtures in three load/save contexts, 22 replay
tests, session Clippy and arena checks pass. The default full-client self-test
finishes with two existing Medic static-check failures; direct native Medic
entry checks verify correct stack cleanup. An existing generated-comment
mismatch and unavailable Windows API checks remain documented. Repeated
embedded-server variants were stopped after the affected default build completed;
no full-matrix success is claimed. Exact evidence and limitations are in
`private/lag-20261103-3b1300f3/review-1154.txt`. Merged, not played or released;
no migration gate advances, plan decisions or live changes.

Investigated match `0x3b1300f3` for three players (A, B and C),
all reporting client revision 45. Final server summaries show 1, 56 and 55
forced resyncs respectively. The complete recording contains 1,069,007 records,
no parser errors and a closing dropped-record count of zero. This is recording
integrity evidence, not proof of zero network loss. At tick 256350, player B
and majority peers have byte-identical worlds except five missing bullets.
Player C repeatedly shares player B's nonmajority hash. Separate shared input
arrival delays also occurred; the packet evidence does not give all three
players one proven cause.

The existing, unshipped `3c5832486` bullet-bounds fix is the strongest candidate.
A native diagnostic classifies precisely the five missing bullets below the
reference map bounds, but uses revision 44 code and revision 43 map resources;
exact revision-45 equivalence and historical runtime bounds remain unverified.
This was operational analysis, with no new production implementation, fix play
test, release, restart or migration gate advancement. Plan decisions are unchanged.
Evidence, scripts, identities, limitations and next validation are retained in
`private/lag-20261103-3b1300f3/report.txt` at repository root (private/ignored).

## 2026-11-03 — T2 safe Rust native projection driver

The native projection loop, history/pool parsing, engine effects, confirmed-input
wrapper and before-send restore now execute in Rust. The C projection/runtime
files contain ABI forwards; the C disconnect/restore bridge is removed. This
completes the next ownership transfer, not the full migration or actual-game
gates. Nothing was played, released, committed or pushed.

### Implementation

- `vostok::native_projection::history` parses constructed input/received-vector
  bounds, history/membership and bounded pools through generated fields. It
  preserves the native absolute-time, grid, horizon, predecessor, duplicate,
  free-capacity and no-eviction guards. Thirty field rows describe the already
  exercised layout; no new engine function or calling convention was invented.
- `native_projection::move_world` is a safe handler. A scoped Baseline abandons
  itself on early return; a scoped Collection owns journal cleanup on exit. It
  drains installed keys in reverse order while the native world remains current,
  including peer-only cancellation. World retirement stops later engine reads/
  calls. Insertion, replay and removal use the existing generated engine bindings.
- Typed `NativePeers` retains the captured connection scope and shares existing
  Round requests/eligibility ownership. The driver calls the safe packet handler
  and typed networking APIs directly, without C callbacks or private byte buffers.
  The projection owner and journal from the preceding checkpoint remain sole
  owners; no second state machine or native history representation was added.
- Native confirmed-input arguments are copied once from a private nine-word ABI
  buffer. The original native call retains all flags, precise/off-grid times and
  return codes. Its `actual_time` output is copied only after native success and
  a surviving world scope, before eligibility callbacks. Before-send copies the
  native core identity before restore. The existing lifecycle forward orders Rust
  projection invalidation before Rust match lifecycle.
- Rust connect/disconnect now sequence restore and retirement directly. The
  pre-restore connection epoch is retained across callbacks, so a replacement
  connection survives old completion. Private plan/finish and owner-step exports
  remain compatibility/test interfaces; production no longer sequences through
  them. C contains no networking/projection state, field reads or effect loops.

### Validation

- The targeted Rust projection/pool run passes its then-existing 23 lifecycle,
  identity reuse, unmapping and peer-loss cases. The final full native run adds
  three direct comparisons with untouched native `change_input` for precise time
  and native flags, plus a before-send restore case. The final native lane passes
  **27 projection cases**, **45 connection, 51 socket, 32 round, 121 login and
  58 crypto cases**, plus peer lifecycle, actual native pool/replay/correction,
  history guards, stall and resync checks. Maximum observed emulator stack is
  **61,768 bytes** within **96 KiB**; this is the bound observed in these cases,
  not a universal game-stack proof.
- Native Clippy with `-D warnings`, formatting/layer checks, eight hook-generator
  tests and hook/engine generated-file checks pass. A lint caught a modulo form;
  the fixed code uses `is_multiple_of`. The layer check rejected a bitmask spelled
  like an address; expressing it as `!(1 << 31)` keeps the meaning explicit.
- Wine 11.8 Staging passes the same real UDP/CNG, input/quarantine, resync,
  eligibility/query release, DNS/receive/close/send callbacks and real OS thread
  gate cases. Restore-reentry now enters the actual Rust restore effect, with
  only engine replay substituted. Both explicit disconnect and outer reconnect
  retain the replacement connection. Wine does not execute native history pools.
- The native pool fixture now constructs actual Rust connection/round owners and
  substitutes only SDK clock availability and the engine player query. It uses
  a controlled wall-zero fallback and an Opening socket; Windows/Wine supplies
  the separate real-I/O evidence. The first full native run exposed an emulator
  fixture regression: installing this setup in default `hint()` changed the
  independent round tests' clock/probe baseline. Making it explicit as
  `hint(native=True)` preserves that baseline. No production code changed in
  response to this failure; no failing assertion was weakened.

### Reproduction

Ignored artifacts:
`private/rust-client-transition/20261103/native-projection-driver-v2/` and
`private/rust-client-transition/20261103/windows-projection-driver-v1/`.
Both capture **309 inputs**, over HEAD `8a72cd9da52b1e4c03cc608a0ef3ce380085e8c1`.
Archive SHA256:

- Native v2: `80342726869bb12bc2925d1f935995e17a4f79cff8256d9db2c2a6497941acda`.
- Windows v1: `d87639d796900b8547d541676971ad3cef6057d219264203381606cc58174c74`.

`native-projection-driver-v2/windows-source-delta.json` records the two differing
emulator fixture files. Windows v1 predates only the explicit projection-fixture
setup fix; its production and Windows-fixture sources match the current/native-v2
inputs. Archive members and those mappings were verified. Failed native v1 is
retained for diagnosis, not acceptance. Concurrent medical changes include a
31st generated hook; the archive captures them, but these tests do not validate
that feature's behavior.

Arena **581,766 bytes**, base `0xa42d000`, SHA256
`b52dd31693bcb347c1022eb8602b44ed2dfa69754df836d1bece25f9050c453e`.
Rust code/data/state: `0x2938c` / `0x2c964` / `0x74`; C peer code: `0x685`,
no C state block. The lock is current. This work did not change a revision,
content manifest or simulation-change row.

### Next session

1. Audit the installed P2P path and actual-game matrix from the canonical plan.
   The source cutover has reached Rust-owned networking and temporary projection;
   do not substitute more helper rewrites for real integration evidence. Retain
   the native engine behind its scoped adapter. Use the isolated local-client
   launcher/capture tools, never a live release or the player's profile.
   Read-only preflight found the default local-client configuration points to
   existing `private/local-clients/rev38-game`, `prefix-template`, and
   `private/wine-keyboard/runtime/bin/wine`. Presence is not executable/resource
   identity or an inactive-prefix check. No local installation/profile was changed.
2. Continue the unblocked T3/T4 work: shared client-direction protocol envelopes
   and reliable match transport. `net-protocol` already contains ACK/framing/time/
   input primitives; inspect their current state and server transport consumers
   before introducing another codec/owner. T1 world-lifetime/storage contracts,
   T5–T8 and actual-game acceptance remain open. Native engine-internal arbitrary
   recursive world mutation is not proved by a current epoch or these fixtures.

## 2026-11-03 — T2 Rust projection state and removal journal

Transferred the temporary native projection's mutable state to Rust. The C
`native` global and `peer_prediction.state` arena block are gone. Rust owns the
pass phase, world-lifetime epoch and bounded reverse-removal journal. C still
parses native history/pool fields and sequences engine effects; that remaining
driver is the next port. T2 and the complete plan remain in progress. Nothing
was played, released, committed or pushed.

### Implementation

- `client-runtime::projection::Machine<W>` is native-independent and has one
  total transition function. Baseline, Collecting, Inserting, Rendering, Removing,
  Discarding, Projected and Restoring carry their phase-specific data. A separate
  private Journal owns at most **160** installed inputs awaiting removal. It
  rejects duplicate/capacity overflow before native insertion, records only
  accepted insertions, and retains a discard until its completion is acknowledged.
  Reverse cleanup completes before Projected can be published or restore begun.
- `vostok::native_projection` holds this non-Copy owner behind the existing
  Runtime/Owner gate. Generated exports copy opaque core identity, typed inputs
  and epoch tokens. No engine pointers or Rust mutation loans survive effects.
  Every phase is invalidated by the existing same-thread core lifetime hook.
  Only the native move/input entry can begin an idle scope; incidental queries
  and teardown cannot bind an uninitialized runtime.
- The C driver checks the captured epoch after baseline move, insertion,
  projected move, removal and restore. It stops before later memory access when
  reset/destruction owns cleanup. Old completions cannot clear a replacement
  owner at the same core address. An interrupted restore also stops an outer move
  or confirmed-input call from entering the retired core.
- Peer-only cancellation is different from native-world retirement: already
  inserted temporary nodes must be removed from the still-live history. The
  driver now drains Rust's journal before leaving this path, instead of returning
  early and leaving a speculative node in confirmed input lists.
- The first enum layout moved the entire journal between variants, increasing
  linked code to `0x3ab70`. Keeping the Journal as an independently owned object
  in place reduces final Rust code to `0x27c00`. Its meaning never changes:
  installed nodes awaiting removal, empty before collection/after cleanup.
  The supported ABI and tests are unchanged by this storage adjustment.

### Validation

- **32 client-runtime tests and two compile-fail doctests** pass on both host and
  i686, including four new projection cases for reverse order/restore sequencing,
  refused inserts/aborted collection, duplicates/capacity and retired-world
  identity reuse. Affected pure/native Rust Clippy with `-D warnings`, formatting,
  layer checks, eight hook-generator tests and generated-file checks pass.
- The targeted native projection run passes **23 cases**. These inject reset and
  destruction after baseline, real input insertion, projected move, real removal
  and restore; each either unmaps the old core/history or publishes a replacement
  owner using the same core identity. Old C continuations neither read unmapped
  memory nor complete transitions against the replacement. A peer-only loss
  after insertion removes the node without an extra projected move. Confirmed
  input handling stops when its preceding restore retires the world.
- Full native lane passes the **23 projection cases**, **45 connection, 51
  socket, 32 round, 121 login and 58 crypto cases**, plus peer lifecycle, native
  pool, confirmed-correction, history guards, stall and resync checks. Maximum
  observed emulator stack remains **61,768 bytes** within **96 KiB**; that is
  the maximum of these executed cases, not every possible game path.
- **Wine 11.8 Staging** passes real UDP/CNG and all prior connection/packet driver
  scenarios. A new actual Windows thread cannot observe an in-flight insertion,
  invalidate the main thread's owner or authorize effects. The main thread then
  completes journal removal and rejects an old restore completion after replacement.
  Wine's networking fixture substitutes native engine effects; the native pool
  suite separately exercises actual 0.26g insertion/removal. Neither is full game
  acceptance or a proof that native engine internals survive arbitrary recursive
  world operations. Callers must honor the scoped adapter's native entry contract.

### Reproduction

Ignored artifacts:
`private/rust-client-transition/20261103/native-projection-v1/` and
`private/rust-client-transition/20261103/windows-projection-v1/`.
Both contain the same **307-input archive**, SHA256
`a0f0b9f9cde1ce7a3250618b602863cb8155fa26f4ba4dd4fd1ab27972651063`,
over HEAD `8a72cd9da52b1e4c03cc608a0ef3ce380085e8c1`. Current selected sources and
archive members match the reports' hashes. Concurrent medical sources are
included; networking checks do not validate their feature behavior.

Arena **573,574 bytes**, base `0xa42d000`, SHA256
`e93dc09035268b3ed3c5870418b9254299b7ec13c468f1461ecbea0f170d9dec`.
Rust code/data/state: `0x27c00` / `0x2c964` / `0x74`; C peer code: `0xf83`,
with no C state block. The arena lock is current. No revision/content/simulation
row was changed by this migration.

### Next session

1. Transfer `peer_prediction_native_026g.c`'s native field parsing and engine
   effects into typed `vostok` operations, and its projection loop into a safe
   Rust handler. Use generated fields/functions and this sole projection owner.
   Preserve native pools, exact insertion-result meanings (including rewrites),
   off-grid confirmed inputs and all current history guards. Do not replace the
   native engine with test models. Existing callback cases must execute through
   the new driver; extend effect coverage where the new boundary requires it.
2. Once native restore runs in Rust, remove the C disconnect/restore bridge while
   retaining its captured connection epoch. Keep the actual-game acceptance
   matrix and the remaining T1/T3/T4–T8 requirements open. General recursive
   native-world mutation is not authorized by a current epoch alone.

## 2026-11-03 — T2 typed connection effects and projection queries

Connect, lifecycle and projection-facing networking decisions now execute in
Rust through generated exports. C's runtime adapter consists of ABI forwards and
the short disconnect/restore bridge. This is another implemented slice of T2;
the separate native projection owner, real-game acceptance and the rest of the
canonical plan remain open. Nothing was played, released, committed or pushed.

### Implementation

- `vostok::peer_connection::Connect` selects the independent login owner, starts
  peer/socket scopes, publishes Opening before DNS and commits Opened only for
  the captured epoch. Typed login selection and socket construction share their
  implementation with the existing private byte exports. The port preserves
  opt-in behavior, low-word native port parsing and mailbox consumption before
  rejecting port zero. An occupied coordinator refuses a new connect.
- Disconnect planning copies only captured epoch8/optional restore core4. C
  invokes the remaining native `pp_restore`, then Rust finishes retirement only
  for that epoch. A restore callback can retire the old world and establish a
  replacement; neither finish nor the outer connect can replace that new scope.
- Typed lifecycle handles matching Binding/Running reset and destruction.
  Binding cancellation retires construction before Ready can read the world.
- Projection queries consume typed player/time/input values and share the
  existing Round and eligibility owner. Confirmation copies its private input
  before eligibility callbacks. Frame output is copied only after current-scope
  checks. Capture/frame/disconnect outputs may be uninitialized C stack storage;
  the adapter writes through raw pointers without creating Rust references to
  uninitialized values. Native pointers remain behind the existing effects.
- No engine function, hook site, protocol format or additional state owner was
  introduced. Generated export metadata, hooks and arena lock are updated.

### Validation

Native checks pass **45 connection boundary cases**, **51 socket, 32 round,
121 login and 58 crypto cases**, plus peer lifecycle and native pool/correction
scenarios. The added cases exercise capture/current/empty-frame output, client
selection and wrong-thread refusals with unmapped private pointers; Binding
cancellation now uses the actual lifecycle export. Maximum observed emulator
stack is **61,768 bytes** within **96 KiB**; this measures the executed cases,
not every production path. Wine 11.8 Staging passed with real UDP/CNG and the
established native engine-effect substitutes. Both new restore-replacement cases
pass: explicit disconnect and the initial disconnect inside connect preserve the
new connection, including its match identity and server endpoint. Existing
positive frame/confirmation, DNS cancellation, reset/destroy during Ready,
receive/close/send reentry and bounded driver scenarios pass through the new
Rust effects. Native Clippy (`-D warnings`), formatting/layer checks, eight hook
generator tests and current hook/engine generated-file checks pass.

### Reproduction

Ignored artifacts:
`private/rust-client-transition/20261103/native-connect-v1/` and
`private/rust-client-transition/20261103/windows-connect-v1/`.
Both capture the same **302 source inputs**, verified against archive members
and the current tree, over HEAD `8a72cd9da52b1e4c03cc608a0ef3ce380085e8c1`.
Archive SHA256 values (archive metadata differs):

- Native: `19997cc7bbd4ce4c4c9a5b71037eaedf099bf4d1202bef4e0d8ee5e07f1c2892`.
- Windows: `bcf8cf0b96b4f15ae10652c61a8f1505adadf22cfee431e523b67a8eff5874ef`.

Arena **573,574 bytes**, base `0xa42d000`, SHA256
`aa2da9d8c97d9a1e1ce2b3f95b56c2cfcdad1544c6fee6a246cb633ba23726c7`.
Rust code/data/state: `0x26248` / `0x2c1a4` / `0x74`; C peer code/state:
`0xe2c` / `0x14`. No revision/content/simulation row was changed by this work.
Concurrent medical sources are in the archive; their feature behavior is not
validated by these networking checks.

### Next session

1. Port the native projection/restore owner in
   `client-patches/peer_prediction_native_026g.c`. Its current lifecycle hook
   clears only Projected: prove cancellation during insertion, projected move,
   reverse removal and restore before retiring it. Reuse the Rust connection,
   Round, Socket and native eligibility owners; do not create competing state.
2. Keep the captured disconnect epoch across native restore until that owner is
   ported; the new Wine callback cases must continue to pass after removing the
   C bridge. Do not infer full native world safety from those substitute effects.
3. Resume remaining T1/T3/T4–T8 gates and the actual game acceptance matrix from
   the canonical plan. T2 is still in progress; fixture success is not play.

## 2026-11-03 — T2 safe Rust packet driver

Moved pump and publication loops out of C. Generated `pp_rust_pump` and
`pp_rust_publish` now enter the safe `peer_connection` patch module; C's
`pp_pump`/`pp_publish` are forwarding functions. The preceding engine-effects
transfer was concrete progress; this turn installs the packet driver using those
effects. Native and Wine validation pass against one reproducible source/image.
The complete goal, T2 real-game acceptance and T1/T3–T8 remain open. Nothing was
played, released, committed or pushed by this work.

### Implementation

- The safe handler sends the optional hello, iterates admitted probe players,
  receives at most 32 datagrams, handles control replies and sends admitted
  publication tickets only to native-eligible players. It contains no unsafe
  code, raw addresses or serialized private-ABI buffers.
- `vostok::peer_connection::traffic` holds a copied captured connection scope.
  Each SDK/engine/crypto operation checks that scope afterward; old work cannot
  switch to a new match/socket. Hello is consumed once; per-player signing still
  uses the Round's one-attempt ticket. Failed nonblocking sends are not retried
  or treated as a reason to retire an otherwise current channel, preserving the
  prior behavior. Destruction during send stops the pump before receive.
- Typed round requests and typed socket send/receive share their implementation
  with the existing private byte entry. This keeps credential-loan failure,
  reentrant invalidation, deferred socket cleanup, malformed-source accounting
  and stale-output rejection in one place. The bounded socket datagram exposes
  only its admitted length and optional parsed endpoint.
- Native pump preparation and publication admission are shared typed operations;
  their existing private snapshot exports remain for compatibility/tests. No
  new state owner, wire format, engine function or hook site was introduced.
  C still sequences connect/disconnect/reset and provides projection bridge
  wrappers. Its separate native projection owner remains unported.

### Validation

- Native checks pass **33 connection/effect boundary cases**, **51 socket,
  32 round, 121 login and 58 crypto cases**, peer callback retirement and native
  pool/correction scenarios. Four new driver-entry refusals cover wrong world/
  wrong thread with unmapped native input. Maximum observed emulator stack is
  **61,768 bytes**, within **96 KiB**. This measures the executed native cases;
  it is not a universal game stack bound or a measurement of every Wine path.
- **Wine 11.8 Staging** runs the actual Rust driver with real UDP/CNG and the
  established engine-effect substitutes. A queue of 33 malformed datagrams
  causes exactly 32 receive calls in one pump; the next consumes the remaining
  packet and then observes would-block. A callback inside the hello send destroys
  the connection: the pump returns refusal, does no subsequent receive and closes
  the active socket once on return.
- All prior direct-input, endpoint/quarantine, resync, stale query/final-release,
  DNS, receive/reset, close/reconnect, inaccessible Ready world, inaccessible
  publication input and stale admission cases still pass through the Rust driver.
- Native Clippy with `-D warnings`, formatting/layer checks, eight hook-generator
  tests and current engine/hook generated-file checks pass. The first metadata
  attempt used a void export, which this generator rejects; the final publication
  entry uses its supported `u32` accepted/refused result, ignored by the legacy
  void C caller. No generator extension was needed. The accepted lanes below
  were run only after that correction and the arena rebuild.

### Reproduction

Ignored artifacts:
`private/rust-client-transition/20261103/native-traffic-v1/` and
`private/rust-client-transition/20261103/windows-traffic-v1/`.
Both contain the **same 300-input archive**, SHA256
`bb0d0ec6296f6ff6636c296868107657ee85a82ff75be48b1e9e9f75adc797a9`,
to overlay on HEAD `8a72cd9da52b1e4c03cc608a0ef3ce380085e8c1`. Reports include
commands/tools/member hashes; current selected sources and archive members were
checked against those hashes. This includes the concurrent medical sources,
whose feature behavior is not validated by these networking tests.

Arena **573,574 bytes**, base `0xa42d000`, SHA256
`ece0067d053565c86a440509afcbbb4043da8ecde08aa5c1f9f4dd3dba0493cf`.
Rust code/data/state: `0x25938` / `0x2c1a4` / `0x74`; C peer code/state:
`0x1488` / `0x14`. The generated lock is current. No revision/content/simulation
row was changed by this migration work.

### Next session

1. Transfer the remaining C connection effect sequencing and projection-facing
   capture/current/status/frame/confirmation wrappers to typed Rust APIs. The
   disconnect path still calls the separate C `pp_restore`; preserve its captured
   epoch check after restore until the projection owner is migrated. Keep the
   existing shared Round/Socket/connection owners and safe packet driver.
2. Migrate the native projection/restore owner with lifetime tests for insertion,
   move, restoration and reverse input removal. The current C lifetime hook only
   clears its Projected state; do not assume that proves reentrant insertion or
   restoration safe. Treat copied identities as insufficient for native access.
3. Complete isolated full-game acceptance and the broader T1/T3–T8 work. There is
   no release or player-facing restart authorization in this plan.

## 2026-11-03 — T2 Rust runtime engine effects

Transferred pump state reads, player query/reference cleanup and native input
capture/admission from `peer_prediction_runtime_026g.c` into
`vostok/peer_connection/effects.rs`. The production arena builds and affected
native/Wine suites pass. The preceding turn made concrete Ready-construction
progress; this turn removes the remaining direct engine reads and player effects
from the C networking runtime. Its copied-buffer packet loops and the separate
C projection adapter still remain. No full game, release, commit or push occurred;
T2 and the full T1/T3–T8 migration remain open.

### Ownership and behavior

- Generated `pp_rust_pump_scope` checks the current thread/world, copies
  generation/resync/frozen values, performs required reset/Ready and returns
  captured peer/socket epochs only for a surviving, unfrozen Running scope.
  A canceled Ready cannot lead to a later frozen-field read. C's pump uses only
  those captured epochs and copied packets.
- Generated `pp_rust_available` checks peer status/current scope, invokes the
  generated native player binding, owns its returned intrusive reference in
  `PlayerLoan`, and drops it before the final epoch check. The count is decremented
  atomically and final release uses the engine-function library. No player pointer
  escapes. Replacement during query or release cannot prove stale eligibility.
- Generated `pp_rust_publication` copies all selected native input fields before
  calling the clock/provider, parses the player and captures time/value, then
  admits publication in the existing Round. It returns peer/socket epochs,
  wall time and the existing one-attempt ticket. The precise flag still uses only
  its native low byte. Rejected outputs remain untouched. C's send loop consumes
  the ticket and never rereads the native input.
- Typed peer status/publication helpers share the same underlying transition
  implementation with the existing byte entry. There is no second peer state
  owner. Six generated field rows reuse the prior C offsets; no new native
  function/ABI was discovered. The private ABI is in the runtime header.

### Validation and fixture boundaries

- The complete affected native suite passes **29 connection/boundary cases**,
  **51 socket, 32 round, 121 login and 58 crypto cases**, peer callback lifecycle
  and native pool/correction checks. Added effect-entry cases refuse wrong world,
  wrong thread and invalid player before accessing deliberately unmapped memory;
  private output remains untouched. Maximum observed stack: **71,080 bytes**
  within the fixture's **96 KiB** budget, not a universal game stack bound.
- **Wine 11.8 Staging** exercises the actual Rust effects with real UDP/CNG.
  The old query/final-release replacement scenarios now execute the Rust
  reference owner. It also verifies frozen-pump refusal, protects a publication
  input with `PAGE_NOACCESS` from the clock callback and verifies its copied wire
  packet, and destroys a connection during that clock with no admission output.
  Existing Ready, DNS, receive/reset and close/reconnect scenarios still pass.
- The Wine fixture substitutes engine effects at their generated calling
  boundary: direct thiscall query at its metadata address, and the arena C ABI
  release entry. It does not execute the real game's player query/destructor.
  The separate **55-function engine ABI** suite passes, as do eight generator
  tests, current generated-file checks, layer checks, formatting and affected
  pure/native Clippy with `-D warnings`.
- Two Wine setup attempts failed before behavior acceptance: the first assumed
  a direct thiscall had an arena symbol; the second correctly refused to overwrite
  an occupied native address. The final runtime fixture is linked at `0x100000`
  with a reserved PE buffer covering that query and the IAT before CRT allocation.
  It checks actual buffer bounds before installing a stub. Other Wine fixtures
  keep their earlier image base. Production binding/arena code was unchanged by
  those fixture fixes.

### Reproduction

Accepted reports under ignored `private/rust-client-transition/20261103/`:

| Directory | Source archive SHA256 (299 selected inputs) |
|---|---|
| `native-effects-v1` | `33abbffaab7baeea8c5cb9daab89728030a9e0cde57de38715b9612a10265cdd` |
| `windows-effects-v3` | `42132e0072c221b34243224ce422c8e99b3b18c323a1a99dbade7779d094577a` |

Overlay each archive on HEAD `8a72cd9da52b1e4c03cc608a0ef3ce380085e8c1`.
Archive/member hashes were checked. The native run predates three Windows-only
fixture corrections (`check.py`'s Windows branch, `peer_windows.h` and the Wine
runtime test); all production/native-test sources still match it. The accepted
Wine archive matches every current selected source. `effects-current-mapping.json`
records this distinction. No native rerun was needed for Windows-only corrections.

Both lanes use arena **569,478 bytes**, base `0xa42d000`, SHA256
`8c4ec67fee0d5e2287fdd2f39b8c5739e9c7de178957a7c9150e6d15a5acbf50`.
Rust code/data/state: `0x24ccc` / `0x2c1a4` / `0x74`; C peer code/state:
`0x19d8` / `0x14`. The lock was regenerated. Concurrent medical code remains in
the combined image; these networking fixtures do not validate medical behavior.
No revision/content/simulation row was changed by this work.

### Next session

1. Transfer C's remaining packet orchestration and connection effect sequencing
   to typed Rust APIs, preserving captured epochs, receive budgets and admitted
   tickets. `peer_prediction_runtime_026g.c` now has no direct engine field read
   or engine-function call; its cross-component `pp_restore` dependency belongs
   to the separate projection adapter. Do not rebuild completed state owners.
2. Migrate `peer_prediction_native_026g.c`'s projection/restore owner and native
   input/event pools. Extend actual native tests for lifecycle changes during
   insertion, move, restore and removal before claiming replay lifetime safety.
   The effects tests above do not discharge those obligations.
3. Complete isolated real-game acceptance, then continue reliable transport,
   protocol/synchronization and timeline work (T1/T3–T7), followed by T8 login/
   lobby. No release or player-facing restart is authorized by the plan.

## 2026-11-03 — T2 Rust Ready construction and callback cancellation

Moved native Ready setup into `vostok::peer_connection::NativeReady`, exposed by
generated `pp_rust_ready`. C's `pp_ready` is now one forwarding call. This is
implemented and tested in the installed arena; no full-game play or release was
performed. The preceding turn was concrete progress (Rust connection ownership),
and this turn closes a construction-lifetime gap while retiring its C setup code.
The full migration remains active, including T1/T2 and T3–T8.

### Change and invariant

Previously Ready read the PVP pointer before RNG, then dereferenced it again
afterward. The connection did not expose a core identity until Running, so a
core destructor during RNG could not cancel construction. The new Binding state
carries the world identity during construction. Core lookup recognizes Binding;
ordinary running queries still refuse it. Reset/destruction in Binding cancels
the partial connection and socket, while an ordinary Running reset retains the
socket as before. No new native lifecycle hooks were added.

Rust reads PVP, local place, core, generation and resync once, before external
calls, through five generated fields in `fields_026g.tsv`. These are the existing
C offsets documented in `match.md` and exercised by the native fixtures, not new
function/ABI discoveries. It parses the local place into `PlayerId`, publishes
Binding, requests entropy and time, then checks the binding before constructing
the Round. No engine reference or later native read crosses the provider calls.
Failed entropy, teardown or replacement cannot publish stale Running state.
Ready must match both the bound world and captured connection epoch.

The private connection ABI adds command 8 (Bind) and snapshot phase 5 (Binding);
the runtime header documents them. The pure transition remains total. Native
Round replacement now has one typed helper shared by the existing byte entry and
Ready, checking that the replacement survives before returning its epoch.
Random/clock typed helpers reuse the existing platform implementations. This
preserves independent login, round and socket owners and their callback rules.

### Evidence

- Five pure connection tests pass on host and i686. The new test requires Bind
  before Ready, rejects repeated Bind/wrong-world reset, and proves teardown
  cancels construction. Existing invalid-event and stale-completion cases include
  Binding. Pure/native Clippy with `-D warnings`, formatting and layer checks pass.
- **22 installed native connection cases** pass, including Binding core lookup,
  reset/destruction cleanup and refusal to read an unmapped wrong/retired client.
  The actual generated Ready setup is exercised with real mapped objects by the
  Wine lane; unmapped-identity refusal alone is not setup evidence.
- The complete affected native run passes **51 socket, 32 round, 121 login and
  58 crypto cases**, peer lifecycle and native pool/correction checks. Observed
  maximum stack: **71,080 bytes** against the **96 KiB** fixture budget. This is
  executed-path evidence, not a bound for every game callback.
- **Wine 11.8 Staging** passes all existing UDP/CNG and lifecycle scenarios plus
  reset/destruction inside `BCryptGenRandom`: the callback marks the old PVP
  allocation `PAGE_NOACCESS`, and Ready returns without reading it or publishing
  Running. Pump subsequently refuses before touching it. RNG failure also
  retires the partial connection and closes its socket exactly once.
- Eight hook-generator tests pass; engine/hook generated-file checks are current.
  `link_026g.py --write` refreshed the lock. An initial compile used a nonexistent
  `PlayerId::from_raw`; it was corrected to the existing roster parser before
  acceptance. No failing attempt is counted as passing evidence.

Both accepted reports are under ignored
`private/rust-client-transition/20261103/{native-ready-v1,windows-ready-v1}/`.
They contain the commands, logs and **same 298-input source archive**, SHA256
`ec2630344e349cb60bf39700dcf3de771c4662e856299e78dbfb0794875a6540`, to overlay
on HEAD `8a72cd9da52b1e4c03cc608a0ef3ce380085e8c1`. Selected sources remained
stable during both runs. The combined arena includes concurrent medical work;
these fixtures validate networking behavior, not the medical feature.

Tested arena: **569,478 bytes**, base `0xa42d000`, SHA256
`34431af2ab0f269d6c1d7eed945248d58238ceca941f3c072204bd10026cecd5`.
Rust code/data/state: `0x2431c` / `0x2c1a4` / `0x74`; C peer code/state:
`0x1be8` / `0x14`. No revision/content/simulation rows, commit, push or release
were made by this migration work. Earlier arena evidence remains historical.

### Next session

1. Keep the installed Ready/Binding construction path. Transfer the remaining
   native pump generation/resync/frozen reads, input-publication reads and owning
   player-query/release effects from `peer_prediction_runtime_026g.c` into scoped
   Rust operations. Preserve captured epochs across calls and release the player
   reference before checking whether callbacks replaced the match. The existing
   Wine fixture currently substitutes these engine calls in C; port its engine
   stubs to the actual Rust binding boundary when migrating those calls.
2. Transfer the separate `peer_prediction_native_026g.c` projection owner and
   input/event-pool adapter. Its insertion/move/removal/restore callback lifetime
   obligations remain open; Ready's no-access test does not prove them. Extend
   the actual native fixtures before claiming those operations safe.
3. Perform isolated local-game acceptance, then continue T1/T3–T8. No release or
   player-facing restart is authorized. Do not repeat completed peer/socket/
   connection/Ready ownership transfers or treat this checkpoint as the goal.

## 2026-11-03 — T2 installed Rust connection phase owner

Moved the remaining mutable connection phase, client/core identity and
generation/resync snapshot out of `peer_prediction_runtime_026g.c`. The installed
Rust owner and the C effect adapter pass native and real UDP/CNG Wine fixtures
against the same combined arena, including the concurrent medical additions.
These tests exercise networking, not medical behavior or a full game. T2 and
T1/T3–T8 remain open. No play, release, revision change, commit or push occurred.

### Ownership and boundary

- `client-runtime::connection::Machine<C, W>` has one total transition API and
  Off, Opening, Connecting, Resetting and Running variants. Only Running carries
  world identity, generation and resync. Client/world identities are generic
  copied values; the pure crate has no native references. Login, match and socket
  epochs are distinct types, including valid zero epochs.
- Generated `pp_rust_connection` calls `vostok::peer_connection`. It gates the
  runtime thread and parses private bounded messages once. Begin requires current
  peer/socket resources before binding the coordinator. Ready requires the
  expected Connecting identity and a current replacement peer epoch. Snapshot
  refusal leaves output untouched. The private ABI is documented in
  `peer_prediction_runtime_026g.h`; these are not network packet formats.
- Retirement sets Off before stopping the captured peer and closing the captured
  socket. Reset removes world identity before retiring peers, then completes into
  Connecting with a new Preparing peer epoch while retaining login/socket.
  Resource effects run outside the connection mutation loan. A pending match
  replacement during a provider callback can become Running; old callback output
  is discarded. Stale completions cannot revive or retire a newer connection.
- C now supplies bounded native snapshots and effects with no global connection
  state. Pump/publish retain the captured socket and match epochs across every
  callback. Eligibility query/release and disconnect restore are followed by
  checks against the captured match. `peer_prediction_native_026g.c` still owns
  projection and native input/event-pool interaction. Moving phase ownership
  does not prove arbitrary engine pointers or speculative replay lifetimes safe.

### Validation

- All four pure connection tests pass on host and i686: construction/reset,
  independent resource lifetimes, invalid-event matrix, failed reset and stale
  completion after cancellation/replacement. Affected pure/native Clippy with
  `-D warnings`, Rust formatting and layer checks pass.
- **20 new installed native connection cases** pass through the generated entry.
  They cover wrong thread/identity, stale transitions, repeated Begin, socket
  retention on reset, resource retirement and complete replacement during CNG.
  Client/world identity numbers deliberately point to unmapped memory; this
  boundary must never dereference them. This proves the identity-only boundary,
  not validity of C's engine reads.
- The same native run passes **51 socket, 32 round, 121 login and 58 crypto
  cases**, peer callback lifecycle and native pool/correction checks. Deepest
  observed stack is **70,936 bytes**, inside the fixture's **96 KiB** budget.
  The increase includes nested connection reset/Ready during signing. It is
  measured fixture coverage, not a universal native stack bound.
- **Wine 11.8 Staging** passes actual UDP/CNG setup, direct inputs, endpoint
  binding, quarantine, resync, stale query/release eligibility, DNS cancellation,
  reset/destruction during receive and replacement during socket close. No full
  game or native Windows run was performed.
- Eight hook-generator tests and the current **55-function engine ABI** check
  pass. `link_026g.py --write` refreshed the combined arena lock. A first C build
  rejected misleading indentation in snapshot decoding; it was corrected before
  these accepted runs. No new native function/ABI was discovered in this cut.

### Exact artifacts

Both accepted lanes ran in the shared tree with stable selected source hashes:
HEAD `8a72cd9da52b1e4c03cc608a0ef3ce380085e8c1` plus **298 archived inputs**,
including untracked code. Overlay a source archive on that HEAD to reproduce it.
The reports under ignored `private/rust-client-transition/20261103/` contain
commands, tools, member hashes and logs:

| Directory | Source archive SHA256 |
|---|---|
| `native-connection-v1` | `46a7e5e60ed9b91dbfbef90a522a0c327d2efa4e7efc35ec4a4ccaabaa5aaf79` |
| `windows-connection-v1` | `5b4416f70a78b042cccbd38f8d17b883eb5e27482302407222d1f1783b3838eb` |

The tested arena is **565,382 bytes**, base `0xa42d000`, SHA256
`0e16a622daa2fb3be48f6e36547dae922c018506fdb42bc05addc690e22a1c50`.
Rust code/data/state: `0x236dc` / `0x2c1a4` / `0x74`; C peer code/state:
`0x1e8e` / `0x14`. The remaining C state is the projection adapter. Generated
entry placement comes from the lock, not hard-coded addresses. The older socket
snapshot remains historical evidence; do not copy its older generated files
over this combined tree. The standalone probe was not rerun: the new connection
owner is exercised by the installed native and Wine lanes instead.

### Next session

1. Preserve the installed connection machine; do not recreate connection state
   while porting engine access. Move C's native reads/effects into scoped
   `vostok` operations with construction, reset and teardown proofs. In particular,
   audit Ready's native reads around RNG/provider callbacks and world lifetime
   changes during construction; the opaque Rust identity alone is no lifetime
   proof. Keep login, round and socket epochs independent.
2. Transfer temporary projection/restore coordination and native pool adapters.
   Reuse the original native fixtures, extending them for lifecycle events
   during insertion, move, restore and removal. A straight syntax translation
   does not discharge the outstanding replay/lifetime obligations.
3. Complete isolated local-game join/play/resync/exit acceptance. No release or
   player-facing restart is authorized by this plan.
4. Continue reliable match transport, protocol/synchronization and timeline
   ownership under T1/T3–T7, then login/lobby under T8. This checkpoint completes
   one ownership transfer, not the complete migration goal.

## 2026-11-03 — T2 installed Rust socket resource owner

Moved optional-channel DNS, socket construction, nonblocking I/O and close
ownership into `vostok::peer_socket`, exposed by generated `pp_rust_socket`.
The implementation is in the shared tree. Its **isolated, reproducible networking
snapshot passes native, Wine and freestanding checks**; the later shared arena
also includes concurrent medical-skills work and does not inherit those binary
acceptance results. No real-game play, release, revision change, commit or push
was performed by this migration work. T2 and the complete plan remain open.

### What now belongs to Rust

- A `Runtime<Opening | Ready(Socket)>` owns an independent socket epoch. Match
  resets retain it; disconnect/destruction invalidates it immediately. An active
  socket/provider call retains its resource until return, then closes once.
  Recursive ordinary I/O refuses without borrowing the resource twice. A close
  callback can replace the owner; Begin checks its returned attachment is still
  current before exposing a token. No OS socket handle crosses the boundary.
- DNS results have a scoped `freeaddrinfo` owner; partial socket construction
  has a `closesocket` destructor, including ioctl failure and cancellation.
  Endpoint and receive outputs are copied only after the socket epoch survives
  the call. Zero is a valid socket handle and epoch; INVALID_SOCKET is decoded
  at the SDK boundary. Failed close is not retried against a possibly reused
  handle. Socket calls retain the existing IPv4/nonblocking/128-byte semantics.
- The adapter retains the process-long Winsock startup and system DLL reference,
  with sticky initialization failure as the old C adapter. SDK signatures are
  generated from `platform_026g.tsv`; the generator now permits ws2_32 and gives
  lowercase SDK exports legal Rust type names. `ADDRINFOA` layout and startup
  were checked against [Microsoft's structure definition](https://learn.microsoft.com/en-us/windows/win32/api/ws2def/ns-ws2def-addrinfoa)
  and [WSAStartup contract](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsastartup).
  The hostname is a scoped, terminated native input; it is never retained and
  needs no libc `strlen`. The native caller must keep it alive through the call.
- Opt-in command-line parsing and GetTickCount are Rust platform operations.
  The `-peer_prediction` token's whitespace/suffix behavior is preserved. The
  crypto initialization cache was moved unchanged to `vostok/platform_cache.rs`
  and reused; it publishes immutable capabilities without waiting on callbacks.
  The freestanding probe includes that new dependency too.
- `peer_prediction_runtime_026g.c` no longer owns Winsock pointers or a DLL
  helper. It keeps native client/core phase, generation/resync and copied public
  configuration. Its new Opening variant publishes construction before DNS,
  so client teardown can cancel it. Connecting/Running carry a copied server
  configuration; reset during an active socket call therefore needs no second
  socket borrow. C sockets/control-crypto implementations remain test oracles;
  the production crypto bridge is now only forwarding functions.
- `Handle::is_current` distinguishes an authorized Ready/Executing/pending
  attachment from a revoked lease still awaiting retirement. The old
  `Owner::current()` identity alone was insufficient for that distinction.
  Socket and peer native epoch queries now use it. The new test covers immediate
  invalidation, pending replacement, rejected reentrant access and cancellation.

### Validation and exact scope

- **51 native socket cases** pass through the actual generated entry and Rust
  owner: opt-in parsing, SDK lookup/startup/DNS/create/ioctl failures, malformed
  addresses, zero handles, server/direct sends, bounded receives, wrong-thread
  calls, cancellation at construction stages, reentrant receive/replacement and
  a replacement from close itself. OS calls are controlled stdcall fixtures.
- The same run passes **32 round, 121 login and 58 control-crypto cases**, peer
  callback retirement and original native input/event pool and correction
  checks. Maximum observed stack remains **61,160 bytes**, within the fixture's
  **96 KiB** budget. This measures executed paths, not every possible game stack.
- **Wine 11.8 Staging** passes real UDP/CNG, peer routing/quarantine and login
  sequence continuity, stale player-query/final-release eligibility, DNS
  cancellation, match reset and destruction inside `recvfrom`, and reconnect
  from `closesocket`. The fixture tracks its actual Rust-created sockets and
  resolver lists; reset retains the socket, destruction closes once and the new
  connection survives the old destructor. This is not a full game or Windows run.
- **5,933 freestanding cases** pass after the cache extraction (including 704
  peer traces, 36 rendezvous cases and 548 controlled native-crypto cases).
  Probe size: **42,220 bytes**. The installed socket is exercised by the separate
  native/Wine lanes, not by that standalone probe.
- All **11 Owner tests** pass on host and i686. Affected pure/native Clippy with
  `-D warnings`, five crypto/cache tests, two mailbox tests, eight hook-generator
  tests, layer checks and the **52-function networking snapshot** engine ABI
  check pass. Shared-tree Clippy later reported an unrelated `collapsible_if`
  in the in-progress `vostok/medical.rs:188`; it was left to that work.

The first build exposed the unwanted `CStr::from_ptr` libc `strlen` dependency;
using a scoped native hostname contract removed it. The layer checker rejected
an opaque FIONBIO numeric literal; the final code expresses the SDK `_IOW`
components explicitly. `native-socket-v2` passed every execution case, but its
runner correctly rejected source stability after concurrent binding/re-kit
edits. The first Wine fixture expansion had a duplicate `client` field name;
that fixture was corrected before accepted runs. No passing claim rests on
those failed attempts.

### Reproduction and artifacts

To avoid building another session's half-written changes, a local clone at
`/tmp/skhid-socket-validation` was restored from HEAD
`8a72cd9da52b1e4c03cc608a0ef3ce380085e8c1` plus the captured networking inputs,
then given the corrected Wine fixture and the probe's shared-cache import.
The three accepted lanes below ran there using the original repository's Nix
shell. Their reports contain exact commands/paths; overlay their source archives
on that HEAD in any separate checkout to reconstruct the tested source. The
archive is durable under `private/`; `/tmp` is only a convenience.

Under ignored `private/rust-client-transition/20261103/`:

| Artifact | Source archive SHA256 |
|---|---|
| `native-socket-v3` | `03cc62aed691ae3ec7cbd119a8b0a2350557fed4b1df995d02fc470466989503` |
| `windows-socket-v3` | `2081c12edf05f1e6c03f55ee0173220ad2f5d15e9a027286c13fa830940eb106` |
| `arena-socket-v3` | `572d23eb2583b196e0dd0b071a6b3124e306a58b221f5985c0efad26dfdadb63` |

Each archive contains **290 selected inputs**, including untracked code; archive
and member hashes were reverified against the immutable checkout. They are not
a complete game-data backup. The tested networking arena is **553,094 bytes**,
base `0xa42d000`, SHA256
`91d29de3fbf853b4e08ff2553e6697330e4d213f679bd2d6ec14ec7722ee706c`.
Rust code/data/state sizes are `0x20ab8`/`0x2c148`/`0x74`; C peer code/state are
`0x1cbe`/`0x54`. Entry locations are generated, never hand-maintained.

A subsequent shared-tree `engine_026g.py --write`, `hooks_026g.py --write --exe`
and `link_026g.py --write` succeeded with **55 engine functions and 30 hooks**,
including concurrent medical additions. That combined arena was **557,190 bytes**,
SHA256 `2ebdf1f524e49103ec65c737220342d0cf7390245b6a03905fe4c80107427462`.
This is build evidence only. Read the current lock next session: another owner
may update it again. Do not copy the tested snapshot's older generated files or
lock over shared medical work, or claim the isolated tests validate the combined
medical build. Socket/crypto/owner implementation files match the tested snapshot;
module registrations, generated bindings and unrelated medical inputs differ.
`socket-current-mapping.json` records 19 networking implementation/boundary/test
files verified byte-for-byte equal to that accepted snapshot; their focused
whitespace checks and the updated documentation checks pass.

### Next session

1. Preserve the installed Round and Socket owners; do not repeat either swap.
   Reconcile current generated files with all current sources and validate the
   combined build once concurrent medical work settles. Keep acceptance evidence
   tied to exact source/image hashes instead of repeatedly testing moving input.
2. Move the remaining C native connection/core phase adapter into Rust, with
   scoped engine reads and explicit construction/reset/destruction events. Retain
   distinct login, match and socket lifetimes and the already-proved callback
   rules; do not expose raw engine pointers in the pure runtime.
3. Port temporary native projection/restore coordination using the real
   input/event-pool and replay fixtures, then complete isolated local-game
   join/play/resync/exit acceptance. No release/restart is authorized here.
4. Continue the full T1/T3–T8 plan, including reliable match transport, protocol,
   synchronization, timeline ownership and later login/lobby replacement.

## 2026-11-03 — T2 installed Rust rendezvous owner

Replaced the installed C rendezvous state with `Round<LoginKey>` behind the
existing generated `pp_rust_peer` entry. This is production-arena code and
native/real-UDP fixture progress, **not full-game play or release**. T2 remains
open for socket ownership, native projection and playable acceptance. T1 and
T3–T8 remain open. No revision, content/simulation row, release, commit or push
was changed by this work; unrelated shared-tree edits were preserved.

### Ownership and boundary

- `vostok/peer_runtime.rs` now owns `Match::Preparing | Running { login, round }`.
  Preparing has no Bank. Ready replaces it with a complete Round and returns a
  new epoch; a complete replacement can also be queued while the previous round
  is inside a CNG callback. The previous epoch is invalid immediately; resource
  retirement waits for its active loan. There is exactly one live peer Bank per
  round, with the existing bounded pending-replacement owner semantics.
- `login::with_login` lends the persistent Login only to control operations and
  checks its captured epoch after provider callbacks. A failed loan/result
  invalidates the match scope too, discarding control mutations made with a
  replaced credential. Match reset continues the login request counter. Publish,
  Probe, Encode, Frame and Confirm need no credential loan. Recursive ordinary
  match access retires speculation instead of losing an already-native confirmed
  input while continuing the old view.
- C no longer stores endpoints, introduction expiry, hello/challenge, control
  replay/rate/receive budgets or publication admission. It retains socket/DLL
  setup, public connection metadata, native generation/core identity and player
  reference adapters. Each pump copies datagrams and source addresses into Round;
  malformed addresses still count against its receive limit. Returned sends
  carry a copied endpoint and bytes, never a Rust pointer or key.
- Publication and probes carry both match epoch and DispatchId. Encode uses the
  stored admitted input. C checks the current **Rust** epoch after crypto, socket
  calls and active-player query/release callbacks; cached C ownership alone is
  insufficient. The old raw INTRO and per-player RECEIVE injection commands are
  gone. ABI layouts/return values are documented in
  `peer_prediction_runtime_026g.h`; all caller buffers remain private and bounded.
- The native workspace permits `large_enum_variant` once with the same rationale
  as the pure workspace: fixed inline match storage with no allocator. It does
  not box or allocate match state. Generated NASM entries and hook addresses did
  not change their calling convention; their linked locations were refreshed.

### Verification

- `native-round-v2` passes the installed generated entry against the exact stock
  executable: **32 new combined round cases**, the existing **121 login** and
  **58 control-crypto** cases, peer callback retirement, actual native input/event
  pool insertion/removal, correction, quarantine, stall and resync checks.
  Peer setup now uses TLS publication/selection, Ready, BeginPump and a control
  introduction through the real Rust CNG adapter. The test supplies controlled
  AES plaintext; it does not pretend that this authenticates a real GCM packet.
- New native cases verify hello/challenge bytes, endpoint expiry, receive/control
  budgets, invalid introductions without replay advancement, cleanup failure,
  single-use/cross-kind/stale dispatch, complete Ready replacement during crypto,
  preserved login sequence and credential replacement/clear during hello/decrypt.
  Failed/stale output remains untouched. Observed maximum stack: **61,160 bytes**
  within the fixture's **96 KiB** budget; this is exercised-path evidence, not a
  proof of every possible native stack or callback chain.
- `windows-round-v2` runs the installed Rust arena with real UDP/CNG under
  **Wine 11.8 Staging**, retaining the C socket adapter and controlled native
  roster. It covers setup/challenge, direct input/source binding, quarantine,
  reset/resync, reconnect and fresh-credential request sequences, teardown, and
  new replacement during both active-player query and final-reference release.
  Each callback creates and introduces a peer into the new round before the old
  eligibility returns. No stale publication is sent; a fresh publication then
  succeeds with sequence zero. No intrusive-reference leak remains.
- The first Wine attempt (`windows-round-v1`) correctly tripped a fixture
  assertion: the simulated query reset overwrote an outstanding reference count.
  The fixture now creates a replacement owner only after final release and keeps
  the query's outstanding loan. Production code was unchanged by that fix.
- Native i686 workspace/all-targets Clippy with `-D warnings`, two mailbox unit
  tests, five crypto unit tests, generated engine/hook checks and layer checks
  pass. Focused whitespace checks pass. The whole dirty-tree diff check reports
  pre-existing trailing empty TSV columns in `engine_026g.tsv`, left intact.
  Pure Round/probe logic is unchanged; its previously passing host/i686
  comparisons and 5,933 standalone arena cases were not needlessly repeated.
- Rebuilt production arena: **548,998 bytes**, base `0xa42d000`, SHA256
  `e4930524207c6352130d041b513fd39edf20bc257c96e8aa3e4053c85a1cf56b`.
  Rust code `0xa462000`/`0x1ed60`, data `0xa481000`/`0x2c110`, state
  `0xa4ae000`/`0x48`; C peer code `0xa4af000`/`0x212e`, state
  `0xa4b2000`/`0xb8`. The old C peer state block was `0x318` bytes. Resolve
  entry addresses from the lock, never embed these diagnostic locations.

### Preserved evidence

Ignored `private/rust-client-transition/20261103/` contains logs, commands,
selected input hashes and source archives, including untracked implementation.

| Artifact | Source archive SHA256 |
|---|---|
| `native-round-v2` | `04c28b4d620becc9d86afebb7bcda1471372f9bc74a9fd892e083b71c1d57250` |
| `windows-round-v2` | 0298f581b7ba677fce2f89d5e72be8861d8fc281b469b867b48d07b079db9d53 |

Both final archives contain **284 inputs**. Archive and member hashes were
reverified. The native archive differs from current selected source only in the
subsequently corrected Wine C fixture; installed code and native tests are exact.
The Windows archive matches current selected source. These archives are not a
complete game-data checkout and do not make the whole dirty repository reproducible.

### Next session

1. Continue from this installed Round; do not rebuild a second owner or repeat
   the previously planned rendezvous swap. Keep frozen C oracles test-only.
2. Move socket ownership and connection lifecycle into a Rust platform owner.
   Establish one owner for DNS/socket startup, close-once and pending reset versus
   destruction. Preserve the opt-in flag, IPv4 binding, nonblocking I/O, low-u16
   port ABI and bounded pump. Keep engine reads/native references scoped; closing
   a socket or replacing a match must never retain stale core/player pointers.
3. The native temporary projection adapter is still C. Port it behind explicit
   engine access/lifetime contracts using the existing real input/event-pool and
   replay fixtures. Preserve confirmed input/history and snapshot semantics;
   investigate remaining ABI confidence gaps and add actual new findings to re-kit.
4. Validate concrete cuts with focused native/UDP checks and source artifacts,
   then perform real opt-in join/play/resync/exit acceptance on isolated local
   clients. No player-facing release or restart is authorized by this plan.

## 2026-11-03 — PvP latency and bullet-restore investigation

Independent diagnostic work; no migration owner or production behavior changed.
No release, live restart, revision change, simulation row, commit or push.

- Implemented `tests/latency_reconnect.rs` in match-server: 17 scripted clients,
  player 5 reconnects at 240/260 ms RTT, other clients at 40 ms, inputs every
  50 ms, 180 seconds after reconnect. A third case uses six ACK bits and slow
  ACKs. All three pass (27.86 seconds), with the expected one TDM first-load
  repair and no subsequent resyncs. The five existing `a_player_back_in` tests
  also pass. These use synthetic world simulation, not native bullet physics.
- `cargo clippy -p match-server --all-targets` under `nix develop ./srp-026g`
  passes without Rust/clippy warnings. No full pre-push suite was run.
- Retained October 31 PvP snapshots of match 0x21471ba4 show same-tick worlds
  differing only in the bullet manager. At 309650, p0 and p15 are byte-identical;
  p12 differs solely by three 68-byte bullet records and the count/length.
  These disagreements precede player 5's reconnect. Player 5 has four logged
  resyncs at stable ~243 ms RTT, but no retained wrong world for that interval.
- `check_bullet_restore_026g.py ORIGINAL` executes the stock constructor,
  serializer, ammo-index helpers and native range decision in Unicorn. With an
  unchanged serialized bullet, switching the owner's slot from A to B before
  restore changes the resolved ammo. Native range outcome: original=continue,
  same-slot restore=continue, changed-slot restore=retire. This is a synthetic
  native reproduction of an identity defect, not a replay of the incident.
- A private fixture round-trips 1,041 retained 68-byte bullet records through
  stock native restore/serialize with fixture resources; all match. Two poisoned
  buffers differ only at offsets 81–83, 89 and 8C–8F after construction. This
  does not test actual map collisions,
  the released patched executable, resource equivalence or multiplayer play.
- The nearby retained shooter states all name kit 0. A kit switch therefore
  is **not established** as the explanation for the observed missing bullets.
  Full packet recording and a converted replay were not found for this match.

Artifacts: ignored `private/diagnostics/<player>-20261103/`, including
`world-differences.jsonl`, `latency-reconnect-tests.log`, `latency-clippy.log`,
`native-bullet-kit-switch.log` and `native-bullet-synthetic.log`. Stock executable
identity is ac33d9937015c2299bf62cc6b127cb308b48a3475b30fff2ddd65479ae3bf153.
Native field/ABI evidence is in snapshot_rules.md §8 and the re-kit.

Handoff: obtain a complete native PvP trace around the first differing bullet:
its emit, owner/weapon/ammo identity, restore, collision/range retirement reason,
and confirmed input sequence. Compare uninterrupted and restored simulation with
the same real resources and inputs. A future fix must preserve the original
weapon/ammo identity across restore and lifecycle changes; changing canonical
snapshots or simulation requires compatibility/replay handling. Do not suppress
bullet hashes or loosen input acceptance on the strength of these results.
Implemented diagnostics and native fixture tested; not fixed, played or released.

## 2026-11-03 — T2 independent match scope and admitted send batches

Implemented the ownership seam needed for the remaining rendezvous cutover:
`Round<K>` owns only match state and its one peer Bank; `Login<K>` owns the
persistent credential/request sequence. This is implementation and host/i686/
freestanding execution progress. **The production client still uses C for the
remaining rendezvous decisions.** No new native round boundary is installed yet.
The installed Rust peer/crypto/login cutovers remain intact. No revision,
content/simulation row, release, commit or push was changed.

### API and invariants

- Round construction takes session, match, server, local player, instance and
  wall time. Retirement drops its peer keys, endpoints, challenge/replay/rate
  state and pending sends, without touching Login. `RoundEvent` has no lifecycle
  variants: a reset retires this owner rather than sending an invalid lifecycle
  event to a running state. The existing Rendezvous lifecycle wrapper delegates
  to the same Round and remains the frozen-C comparison driver.
- BeginPump and Datagram carry an explicit temporary mutable Login loan. Frame,
  Confirm, Probe, Publish and Encode need no login loan. No loan or game pointer
  is stored in Round. Wrong-session control events are refused before consuming
  request numbers, control budgets or calling crypto. The native coordinator
  must additionally check the **credential epoch**, not just public session ID.
- Publish now stores the admitted input and returns an opaque DispatchId. Encode
  accepts that ticket and a player, not another caller-supplied input. A newer
  publication invalidates the previous ticket. Probe work likewise gets a ticket
  from its due BeginPump; the next BeginPump retires an unfinished probe batch.
- Each ticket permits at most one signing attempt per peer. An attempt is consumed
  before signing, so failure cannot create an unbounded retry loop or reset the
  peer sequence. Probe and publication tickets use one nonwrapping issuer, so they
  cannot be substituted for each other. Exhaustion refuses new send batches while
  the independent login hello counter continues normally. Legal C pump/publication
  ordering produces the same wire bytes; stale/duplicate internal dispatch is now
  refused explicitly.
- Tickets, including zero, are valid only within the captured **match epoch**.
  Different Round instances can issue equal numeric tickets. The native boundary
  must carry/check both scope epoch and ticket; a raw ticket is not authorization.
  Do not retain a native eligibility result across replacement and then look up
  whichever match happens to be current.

### Verification

- All **22 client-runtime tests and two compile-fail doctests** pass on host and
  i686. The eight rendezvous groups include stale/duplicate/cross-kind tickets,
  failed signing, counter exhaustion, wrong-login refusal, key retirement and
  match-only replacement during an active lease. Rejected effects still consume
  their login request number; replacement does not reuse it or destroy login.
- The frozen C/OpenSSL comparison remains **9,958 events / 1,198,251 bytes** on
  both targets. The peer comparison remains **302,009 / 51,494,739 bytes**.
  Affected host/i686 clippy and native workspace all-targets clippy pass with
  `-D warnings`.
- The freestanding rendezvous probe now uses the actual native Runtime<Round>
  thread gate/Owner with a separate Login. Its **36 cases** hash hello, challenge,
  probe and publication bytes; reject duplicate publication; replace a match while
  an old effect exists; reject that effect and continue the login sequence in the
  new match. Python supplies HMAC and explicit plaintext fixtures. This is not
  CNG authentication or the final native socket/engine callback adapter.
- All **5,933 arena cases** pass. The probe remains within the existing 64 KiB
  mapped stack, but that is not a production reentry stack measurement. Probe:
  **42,220 bytes**, SHA256
  `5f27627b7af76ef141c09fcaf8e4d577929b229516ad11cb09f113ed6ed6b9d6`.
  The first probe attempt used a publication time which wrapped to 9 ms, correctly
  violating C's initial 25 ms budget. The final fixture uses 50 ms elapsed and
  reports failed vector context. The production policy was not loosened.
- A fresh `link_026g.py --write` proved the production arena is **byte-for-byte
  unchanged**: 540,806 bytes, SHA256
  `ae7d11780a013defb79508119bd99f980e7c6a7d9683902e8b035aedf9b983ef`.
  Production C sources/fixtures are unchanged too. Their prior native/Wine
  acceptance evidence therefore remains applicable; those broad lanes were not
  repeated for unused pure-core/probe changes. No full-game play occurred.

Preserved under ignored `private/rust-client-transition/20261103/`:

| Artifact | Source archive SHA256 |
|---|---|
| `pure-round-scope-v1` | `7b24841c238147d63343a2309ba4eaee489227d17e0db000989a1210f1fb5c9b` |
| `arena-round-scope-v2` | `6bf9dda17f7a6a590f50df9993ee74104680be94d9b5d6a8cbc5e2c862ec94ee` |

Both preserve **282 inputs**, exact commands/logs and hashes; archive/member
hashes were reverified. The final arena archive matches selected current source.
The pure archive differs only in the subsequently corrected probe and Python
probe driver; pure implementation/test sources are identical. The failed arena
v1 is diagnostics. Prose is outside these source archives.

### Next session: production round cutover

1. Replace the peer-only native resource with one match resource containing the
   Round and its credential epoch. Keep the installed LOGIN/Mailbox owner. Add a
   private scoped Login borrowing function for control operations, with results
   checked against both credential and match handles. If login was replaced while
   a control operation ran, retire the old match too; do not retain its mutations
   merely because its own match handle was otherwise still current. Ordinary
   recursive match operations must retire speculation as the current peer helper
   does, so a recursive confirmation cannot be lost.
2. Preserve reentrant reset/replacement before readiness: the existing bounded
   Owner can queue a replacement but cannot mutably borrow that pending resource
   during the old callback. A viable boundary is Preparing (no Bank) at connect,
   then a complete Ready request with copied config/instance creating a replacement
   Running Round and returning its new epoch. Validate the old epoch before that
   replacement. Never leave both a prepared Bank and the Round's Bank alive.
   Bind/read the login only when the operation needs it; retaining the already
   selected credential epoch during readiness needs no secret copy.
3. Switch C's pump/control/endpoint/publication code to copied typed round events
   and send effects. Carry both match epoch and DispatchId through active-player
   query/release callbacks; pass the ticket to Encode afterward. Check current
   Rust scope, not only C's cached epoch. Retire the C hello, control replay/rate,
   endpoint and publication state, and obsolete raw INTRO/RECEIVE entry commands.
   Update the native fixtures to introduce peers through the actual control path
   instead of retaining a production test-only key-injection back door.
4. Extend actual generated-boundary native checks and real UDP/CNG Wine tests for
   reset/replacement, credential replacement, stale native eligibility, source
   routing, cleanup and stack depth. Refresh the production arena lock and record
   new hashes only after this swap. Socket ownership and native projection remain
   subsequent seams; T1/T3–T8 and playable acceptance are still outstanding.

## 2026-11-03 — T2 production credential and TLS mailbox cutover

The production arena now uses Rust for the TLS credential mailbox, persistent
login master key and nonwrapping request sequence. The C runtime no longer
stores any of those values. It retains only a public session ID and an opaque
login epoch in its connection, alongside the separate match/peer epoch. This is
an implemented and native/Wine-tested source/build cutover, **not played or
released**. Rendezvous decisions, endpoints, sockets and native projection still
belong to C; T2 and the overall migration remain incomplete.

This turn made progress, preserved unrelated working changes, and changed no
revision/content/simulation row, commit, push or release. The arena lock was
refreshed through link_026g.py --write.

### Ownership and boundary

- `vostok::login` owns a bounded, nonblocking Mailbox<Update>. TLS publication
  works without binding or accessing the game-thread runtime. Concurrent/busy
  publication is dropped; a busy consumer observes no update, matching C's
  try-lock policy. Acquire/release protects the cell, T: Send permits transfer,
  guards release on unwind, and values retired by replacement drop outside the
  lock. There is no exposed pointer or borrowed mailbox value.
- Connect consumes the mailbox into Runtime<Login<LoginKey>>. Malformed TLS
  replies publish Clear, consumed on the next Connect; an empty mailbox preserves
  login. A replacement resets request numbering, including an identical secret.
  A session mismatch refuses the connection but retains the selected credential,
  as the original C did. Reset/disconnect do not clear login or reset requests.
- `client-runtime::rendezvous::Login` is the same sequence owner used by the pure
  rendezvous implementation; its constructor/session/key/reservation API now also
  serves the native credential boundary. This does not instantiate a second
  rendezvous owner in production: only the separate native Login is active there.
- `crypto::LoginKey` owns erasing Secret storage without loading a DLL or
  initializing CNG on receipt/selection. HMAC initialization remains lazy and
  sticky on actual failure; RNG/AES keep their existing retry semantics. The
  shared provider acquisition was extracted from Key::from_secret, not duplicated.
  The key supports a typed ControlKey implementation using owned plaintext and
  Introduction::parse for the forthcoming pure rendezvous integration. That full
  typed control path is not yet installed/exercised as a native match owner.
- The generated `pp_rust_login` export has TLS, Select, Reserve, Sign and Open
  operations. Exact buffer layouts are in peer_prediction_runtime_026g.h. No
  master key crosses back to C. Sign publishes only a complete current tag; Open
  zeroes output on failure, including stale/wrong-thread calls. Authenticated
  plaintext stays in an erasing owner until the epoch check succeeds, then the
  temporary C adapter receives a copy and wipes it after introduction admission.
  Whole-process/historical-stack erasure is not claimed.
- Credential epochs and match epochs have separate lifetimes. Replacing or
  clearing login during a loader/provider callback immediately invalidates old
  crypto results; the old key remains alive until its borrow returns, then drops.
  Pending replacement uses the existing bounded Owner mechanism. Ordinary
  recursive access is refused without destroying the persistent credential.
  In particular, a recursive Select with no update cannot borrow the executing
  Login and refuses; it does not revive or reset its sequence.
- Epoch zero is valid. The return status determines whether an output epoch
  exists; callers must never interpret its numeric value as a sentinel. The first
  native test attempt caught an erroneous nonzero assumption in the fixture;
  production already used the proper status/token contract.

### Verification

- **121 native login cases** execute the production generated entry: malformed
  TLS lengths/signatures, cross-thread publication versus game-thread use,
  pending clears/replacements, session mismatches, persistent/reserved sequences,
  no premature CNG initialization, provider failures, and clear/replacement during
  both HMAC and decrypt callbacks. Stale tags/plaintext are withheld and subsequent
  replacement credentials work. These emulate OS calls, not AES authentication.
- Existing **58 control-crypto cases**, peer-owner callback/replacement checks,
  native input pools, corrections, quarantine and teardown pass. Largest observed
  stack depth remains **62,904 bytes**, below the existing 96 KiB check budget.
- Real CNG/UDP fixtures pass under isolated **Wine 11.8 Staging** with the actual
  linked Rust login entry. The runtime fixture now verifies signed hello request
  sequences 0 → 1 on explicit reset → 2 on resync → 3 on reconnect, then 0 after
  consuming a new credential. It also retains direct input, endpoint binding,
  quarantine, stale-intro rejection and inaccessible-core teardown checks.
  Legacy/Rust crypto fixtures pass the independent OpenSSL vector, 896 single-bit
  mutations, RFC HMAC and RNG smoke checks. Wine is not real-game/Windows-host
  acceptance, and the RNG smoke test is not an entropy proof.
- **Two native mailbox unit tests** cover busy reentry, unwind and 8 concurrent
  publishers/consumers transferring 8,000 owned values without duplicate drops.
  **Five crypto unit tests** pass. Native workspace all-targets clippy passes
  with -D warnings. The four rendezvous test groups pass on host and i686,
  including **9,958 events / 1,198,251 bytes** compared with frozen C/OpenSSL.
  Affected pure-crate clippy, generated engine/hooks checks and Rust layers pass.
- The isolated freestanding probe passes **5,933 cases**, including 36 rendezvous,
  704 peer and 548 HMAC-provider cases. Probe: **32,028 bytes**, SHA256
  `e4544318af3baace1f4d3128dfbfc065fbeb1e5a3b380146c0dfbe00d5e62c17`.
  This turn did not change the generator itself or rerun unrelated server suites.

Final production arena: **540,806 bytes** at `0x0a42d000`, SHA256
`ae7d11780a013defb79508119bd99f980e7c6a7d9683902e8b035aedf9b983ef`.
The original C mailbox/login state is absent; public connection metadata remains.
No new engine functions/addresses were identified, so no re-kit row was needed.

### Preserved evidence and next steps

All three final artifacts under ignored
`private/rust-client-transition/20261103/` passed and preserve **282 source
inputs**, exact commands, logs and hashes. Every source archive/member was
rehashed and matched the current selected source after the final run.

| Artifact | Source archive SHA256 |
|---|---|
| `native-login-owner-v3` | `8160876559e32f0b24d98cff88b688a327616adc707b6fe76d41dbaf53406b11` |
| `windows-login-owner-v3` | `dbe18171e8815fb0963b2249946c7d114ce12a7cdbc3ede51a10f286e2d52e0f` |
| `arena-login-owner-v3` | `85db797f34bd9fd2112ef3f8528ca8d2b2a0b344e4c4340b976ce290ef048781` |

Use check.py native/windows/arena with fresh output directories. Preserve v1 as
fixture diagnostics and v2 as intermediate evidence; v3 includes stale decrypt
output and real-socket sequence-continuity checks. Prose is outside the selected
source archive. Test artifacts and test-only secrets are not production login
credentials.

Next, install the remaining `client-runtime::rendezvous` transitions and retire
C's hello/control replay/rate/endpoint/publication state. Reuse the installed
mailbox/LoginKey/sequence owner: **do not reintroduce a second master key or
request counter**. The pure model currently owns Login and match state together;
its native integration needs deliberately coordinated persistent-login and
replaceable-match scopes. Either refactor that model into separately owned types
under one coordinator or implement a combined resource with separate scope
invalidation. Do not invalidate/drop login merely to rotate a match, and do not
keep two Banks. Handle callbacks and stale native eligibility/publication results
before applying effects. Keep the C socket/native projection seams until their
own transfer, then repeat actual production native/UDP checks. General T1 event
storage, T3 envelopes, T4–T8, real play and release gates remain outstanding.

## 2026-11-03 — T2 pure rendezvous owner and frozen control oracle

Implemented `client-runtime::rendezvous`, with one transition API and bounded
state. This is implementation and host/i686/freestanding execution progress.
**It is not installed in the production client yet.** The existing C rendezvous,
TLS mailbox and socket owners remain active. No production arena/lock, release,
revision/content/simulation row, commit or push was changed in this turn.
Unrelated work was preserved.

### Ownership and behavior

- Credentials and the nonwrapping per-login request sequence survive disconnect,
  reconnect and world reset. A consumed credential replacement resets the
  sequence; a consumed clear retires the key. An unchanged/busy mailbox must not
  replace login state. The platform mailbox itself is deliberately not duplicated
  in the pure core; `CredentialUpdate` is consumed by Connect only.
- Off/Connecting/Running are enum variants. A running state owns the instance,
  pending hello/challenge, intro replay number, endpoint timestamps, control
  credits, publication times and the existing twenty-slot Rust peer Bank. Reset
  retires all match state/peer keys together while retaining the login owner.
- SessionId, MatchId, WallTime, GameTime, PlayerId and IPv4 Endpoint keep distinct
  domains. Introduction::parse validates decrypted context and addressing before
  importing a peer key. ControlKey's native implementation must authenticate into
  erasing plaintext storage, call this parser with its supplied match/local
  context, and erase on all exits. Test providers are not production crypto.
- BeginPump caps receive work at 32 packets and server control work at two
  packets, with the original 32,000-credit/64-per-ms refill. Even malformed socket
  addresses consume receive slots (`source: None`). Malformed server control
  packets consume control credit. Hello uses the original one-second interval;
  failed signing still consumes its request number and replaces the cached hello.
- Server endpoint binding, challenge matching, intro instance/sequence checks,
  ten-second endpoint expiry, quarantine-preserving refresh and reverse-direction
  peer signing use the same owner. Public peer status includes endpoint expiry.
  Publication admission preserves the original 25 ms/grid/precise-input rules.
- Native effects occur outside the transition. The integration still must guard
  every effect with its captured runtime epoch, preserve active-player eligibility
  queries, and coordinate publication/Encode and periodic Probe dispatch. Those
  events currently expose the same individual operations as the existing native
  adapter; before installing, prove that reentry cannot apply an old eligibility
  result or issue duplicate/currently inadmissible work. Pure tests alone do not
  establish that lifecycle protocol.

### Baseline and verification

`crates/client-runtime/tests/rendezvous_baseline/` preserves the original C runtime
unchanged (SHA256 `40987495deda514ac31bb47eaba653e0112268c0ddf876a833a5d44283061b6c`),
its two headers and an oracle harness. See its README for exact hashes and scope.
The harness runs original TLS/connect/disconnect/pump/control code with OpenSSL
HMAC, controlled decrypt results and a peer admission stub. Ready/reset state is
constructed directly, avoiding the native pointer layout on a host process.
It is not a native hook, AES, real-socket, lock-contention or reentry test.

- **9,958 rendezvous events / 1,198,251 snapshot and output bytes** match frozen
  C/OpenSSL on both host and i686 Rust. Cases include every challenge bit,
  introduction header mutations, plaintext admission, provider failures, malformed
  TLS replies, mailbox timing, replay, endpoint refresh, work/credit exhaustion,
  wraparound, request exhaustion and reset/reconnect behavior.
- Four new Rust test groups also check owned-key retirement, expiry, source
  routing, quarantine/refresh, receive limits and publication rules. All **18
  client-runtime tests and two compile-fail doctests** pass on both targets.
  The unchanged peer oracle still matches **302,009 events / 51,494,739 bytes**.
- A new generated cdecl probe exercises the actual rendezvous owner in the
  freestanding i686 arena: **36 cases**, including hello/reply/probe wire bytes,
  rejected contexts, replay, reset, retained login sequence and clock wrap. Python
  supplies HMAC; its plaintext fixture explicitly does not authenticate AES.
  All **5,933 arena cases** pass (including 704 peer and 548 native HMAC cases).
  The probe fits the existing 64 KiB mapped stack; no production callback/reentry
  stack bound is claimed. Probe size **31,980 bytes**, SHA256
  `5f77e981c15c4fa147e92a9dd58908665c9a4b26c50e4a6d984737f02a2e388e`.
- Affected host/i686 clippy passes with `-D warnings`. Workspace clippy also
  passes (`cargo clippy --locked --workspace --all-targets -- -D warnings`).
  The workspace permits `large_enum_variant` once, with the reason: the native
  owner uses bounded inline storage without an allocator; separating tags from
  payloads would weaken lifecycle ownership. No new allocation was introduced.
- The first arena attempt exposed libc relocation from a test's derived
  whole-Effect equality. The probe now checks variant patterns and explicitly
  hashes emitted bytes; the actual owner links and executes without new runtime
  helpers. Formatting the entire probe crate also hits its existing path-imported
  test-module resolution problem, so the added probe file was rustfmt'd directly.

Preserved under ignored `private/rust-client-transition/20261103/`:

| Artifact | Source archive SHA256 |
|---|---|
| `pure-rendezvous-v1` | `d5c80bc52ecfbc4328a8b2854610006f5b484d8d934d4f8d6f715bd1d93a4486` |
| `arena-rendezvous-v2` | `8442b165be026c9346ad3ee3c995add3e92eea9e692a183d12c5a22da4674a26` |

Both archive **278 inputs**; archive and member hashes were rechecked. The final
arena archive matches current selected source. The pure archive differs only in
the subsequently fixed/formatted standalone probe file; the pure crate and test
sources are identical. Exact commands/logs are in result.json. README/work-log
prose is outside these selected-source archives. Keep the failed v1 arena result
as diagnostics, not acceptance evidence.

### Next session: install this owner

1. Read the new module and frozen-oracle README. Implement the persistent native
   credential owner and atomic TLS mailbox, including contention and retirement
   tests. Do not initialize the HMAC provider just to retain a login secret:
   current C accepts credentials before any crypto call, and actual initialization
   failure has sticky semantics. Reuse the existing erasing Secret/CNG owners;
   avoid creating another provider cache or serializing secrets for inspection.
2. Implement ControlKey through the existing CNG AES/HMAC code, keeping plaintext
   owned until Introduction::parse consumes its peer key. Add safe typed APIs
   inside vostok instead of calling generated raw C export shims from pure code.
3. Replace the native peer-only Bank resource with the combined rendezvous owner
   (no second Bank or overlapping C key/endpoint/sequence state). Preserve login
   across world-epoch replacement. One approach is an explicit persistent login
   resource with a separately invalidatable match scope, but its transitions and
   pending callbacks need a single coordinator; do not independently reset it
   through both old and new mechanisms. Keep the C socket/native projection
   adapters until their own transfer.
4. Install generated boundaries and remove the corresponding C state/logic,
   then extend native and real-UDP Wine checks to that actual production owner.
   Prove mailbox races, callback reset/replacement, stale results, eligibility,
   cleanup and stack use before claiming cutover. Refresh the arena lock only
   after production code changes. Real play and all later T1/T3–T8 gates remain.

## 2026-11-03 — T2 RNG, AES-GCM and rendezvous HMAC cutover

The production client now performs all P2P/rendezvous cryptography through Rust
and the OS CNG provider. `peer_prediction_crypto_026g.c` is excluded from both code
and state arena blocks and retained as a test reference. Its replacement C file
contains compatibility forwarding functions and the existing system-DLL helper
still used by Winsock. Credentials, rendezvous decisions, sockets and native
projection remain C-owned; T2 and the broader plan are **not complete**.

This turn made implementation/test progress and preserved the unrelated working
tree. No revision/content/simulation row, commit, push, release or player prefix
was changed. This build has not been played or released.

### Ownership and interfaces

- `platform_026g.tsv` adds generated SDK signatures for BCryptGenRandom,
  BCryptCloseAlgorithmProvider, BCryptSetProperty, BCryptGenerateSymmetricKey,
  BCryptDecrypt and BCryptDestroyKey. Existing OpenAlgorithmProvider is reused.
  These are OS exports, not invented game addresses or re-kit findings.
- `vostok::crypto::control` owns per-operation AES algorithms and symmetric keys.
  A key borrows its algorithm, preventing closure before key retirement. Explicit
  close consumes the owner; Drop handles early exits. Cleanup is attempted once
  per acquired resource, including failure, and errors reject plaintext. A
  reported cleanup failure does not prove the OS actually released its resource;
  the adapter does not retry a possibly consumed handle.
- Windows x86 authenticated-mode layout is explicit: 64-byte size, 8-byte
  alignment, the internal gap before cbData at offset 48, and initialized tail
  padding. The SDK's NULL/zero object storage asks CNG to own key storage, paired
  with DestroyKey ([SDK ownership](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgeneratesymmetrickey),
  [authenticated-mode structure](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/ns-bcrypt-bcrypt_authenticated_cipher_mode_info)).
  This matches the original C boundary; no AES or RNG algorithm is implemented here.
- HMAC, RNG and AES share one immutable process-owned bcrypt.dll loader reference.
  The generic initialization cache distinguishes Loading from Unavailable and
  has explicit sticky/retry policies. Failed DLL loads acquire no reference and
  RNG/AES may retry later, preserving C behavior. Actual HMAC initialization
  failure stays sticky. A loader callback cannot poison an otherwise-new HMAC
  cache merely by observing the dependency in Loading; it returns immediately.
  Missing AES exports do not disable functioning RNG/HMAC capabilities.
- `Secret<N>` erases live owned buffers, including 64-byte plaintext. The derived
  32-byte encryption key is explicitly erased at the exact address supplied to
  CNG before decryption. Moves/stack reuse can leave historical copies, as noted
  in the earlier key-owner entry; this is not an all-process erasure claim. The
  final native gate records zero stores to every byte of that CNG input buffer,
  rather than treating a later read of a retired/reused stack slot as proof.
- `peer_crypto.toml` declares generated `pp_rust_hmac`, `pp_rust_random` and
  `pp_rust_decrypt` entries; safe dispatch lives in `peer_crypto.rs`. Boundary
  calls snapshot the C-owned master key before any loader/provider callback.
  Borrowed messages and output storage have explicit synchronous/non-aliasing
  contracts. HMAC exposes only a complete tag; RNG clears partial output on
  failure; decryption clears the output on every failure and returns plaintext
  only after authentication, exact-length verification and successful cleanup.
  The previous C code did not clear output on every early setup error. These
  output guarantees affect rejected operations, not valid protocol bytes.
- Rust does not consume the C login credential in this transitional interface.
  It owns and erases its temporary snapshot. The existing C epoch guards still
  decide whether a cryptographic result belongs to the current match. Moving
  persistent credentials/rendezvous state into Rust remains the next transfer.

### Verification and limitations

- `check.py native` passes **58 new control-crypto cases** through the actual
  production generated entries: missing DLL/functions, failed/null algorithm or
  key creation, property/hash/decrypt errors, short plaintext, failed cleanup,
  no partial outputs, Windows stdcall/struct layout, exact derived-buffer erase
  writes, independent capabilities, retry policy and loader reentry. A C
  credential is also retired during a loader callback; the result still uses
  the key captured before it. Existing peer epoch/reentry, real native input
  pools, correction/quarantine, guards and teardown checks pass. Largest observed
  stack depth remains **62,904 bytes** for nested peer replacement. AES itself is
  deliberately stubbed in this lane; it is a boundary/ownership check.
- `check.py windows` loads the production arena into isolated Wine 11.8 Staging
  processes. Both the legacy C crypto fixture and its Rust-backed counterpart
  pass the independent OpenSSL AES-256-GCM vector, **896 single-bit packet
  mutations**, RFC4231 HMAC, and RNG smoke checks. The real-UDP fixture now links
  the C compatibility bridge, so its RNG, challenge HMAC, AES introductions and
  peer MACs all execute in Rust/CNG. Setup, direct input, endpoint binding,
  quarantine/refresh, reset/resync, teardown and credentials clearing pass.
  These are Wine tests, not Windows-host or full-match acceptance.
- **5 i686 native crypto unit tests** pass, including generic-cache concurrency,
  reentry, failure/retry publication, live-secret erasure and hash cleanup.
  Client workspace all-targets clippy passes with `-D warnings`. The commands are
  `cargo test --locked -p patches vostok::crypto::` and
  `cargo clippy --locked --workspace --all-targets -- -D warnings` under
  `nix develop`, with cwd `srp-026g/client-patches/rust`.
- The standalone arena probe passes **5,897 cases**, including the existing 704
  C/OpenSSL peer traces and 548 HMAC-provider adversarial/comparison cases. It
  imports the real crypto module via an explicit child-module path. Final probe:
  **20,924 bytes**, SHA256
  `4efceeb97cd5ca3080781602fe8e9a728522f15bf9077d59bdaac0a9132e5d7b`.
- Generated engine/hooks are current; layer checks and **8 generator tests**
  passed. The shared generator itself was not changed this turn. Relevant tracked
  diffs have no whitespace errors. Pure protocol/server/gameplay code was not
  changed and its broad suites were not rerun. Real play, native world-reentry
  safety and frame-cost acceptance remain outstanding.

Final arena: **536,710 bytes** at `0x0a42d000`, SHA256
`ec8c45062ac33d1beb57af6b95d65c6068fb62bb73fa254bc8f32ab52e02be38`.
`link_026g.py --write` refreshed the arena lock; exported addresses are read from
linked symbols. The retired C HMAC provider state is no longer linked.

### Preserved evidence

Under ignored `private/rust-client-transition/20261103/`, all three final runs
contain exact commands, logs, hashes and source archives with **268 inputs**.
Every archived member and the archives themselves were rehashed successfully;
the selected source contents still matched the working tree after these checks.
The Windows artifact also preserves the exact production arena and test EXEs.

| Artifact | Source archive SHA256 |
|---|---|
| `native-control-crypto-v4` | `141e4926218b26f4ea6910c663dfb90e9dc7bbe32a6342d0ec9af77310ec5127` |
| `windows-control-crypto-v4` | `093d3c7c9c1b799cef2087aa4bd95d8765871a3936f5f01ca0f57c5f62426322` |
| `arena-control-crypto-v4` | `f88a21f98b390f006d07ad74f8c3fb431aaadc0f8dd9ff4290b7582573c59965` |

Use the existing `scripts/rust-client-transition/check.py native|windows|arena`
commands with new output directories. The native stock EXE and MinGW/Wine paths
are recorded in each result.json. Early attempts remain diagnostics: the first
implementation needed stronger in-place derived-buffer erasure, the standalone
probe needed an explicit child-module path, and the loader dependency/retry
policies were tightened before these final runs. Do not use earlier artifacts
as the final build. Archives cover selected source, not the entire dirty tree or
installed game resources.

### Next session

1. Transfer the rendezvous owner, starting with a frozen C control-state oracle:
   credential mailbox consumption, per-login request sequence, hello/challenge,
   introduction sequence/instance checks, peer endpoint refresh/expiry, work
   budgets and world/generation rotation. The current C runtime plus archived
   real-UDP fixture are the behavioral baseline.
2. Give the persistent credential exactly one Rust owner across connection/world
   lifecycle changes; eliminate temporary C master-key snapshots when that owner
   moves. Reuse the Rust CNG implementation through a safe typed API, rather than
   creating another crypto cache/provider or invoking raw export shims from pure
   code. Keep received introduction plaintext owned until typed admission and
   erasure; no key/debug serialization accessor.
3. Transfer socket lifetime after the rendezvous state. Keep the temporary C
   native projection adapter until the T6 seam is proven. T1's general bounded
   event/storage work, T3 control envelopes, T4–T8 and opt-in playable acceptance
   still remain; neither this crypto cutover nor its passing tests completes them.


## 2026-11-03 — T2 production peer owner cutover

Rust now owns the peer keys, receive/replay/rate state, hints, quarantine and
outbound sequences in the production arena configuration. C no longer stores a
`pp_link`, duplicate send identity/key, or per-peer send counter. This is a tested
source/build cutover, **not a played or released build**, and T2 remains in
progress. The separate startup-memory work below was preserved. This work made
no revision/content/simulation-compatibility change, commit, push or release.

### Implementation and ownership

- `client-runtime::peer_bank::Bank` owns twenty bounded peer slots. The validated
  incoming identity selects its own slot. Refresh preserves quarantine, replay
  and outbound sequence; a changed identity replaces them. Reverse-direction
  encoding uses the same owned key. Reservations happen before hint admission
  and authentication, so failed encoding still consumes the sequence; the final
  u64 value is used once and never wraps. The old pure C machine is excluded from
  `peer_prediction.code` in `patch_026g.py`; it remains a test/reference source.
- `Owner::replace` immediately invalidates old work and keeps at most one pending
  replacement during an active callback. A newer replacement retires the previous
  pending resource. Return/unwind publishes the survivor; destruction can cancel
  it. Retired values are dropped outside the RefCell borrow, after reserving the
  next epoch. Old handles cannot cancel the replacement. This does not implement
  the general transport event queue or larger storage owner still needed by T1.
- `hooks_026g.py` accepts `[[export]]` with C prototype, typed vostok boundary and
  safe handler. It generates `arena_exports_026g.h`, the NASM cdecl entry and Rust
  shim; exports have no fictional original engine callee, site or patch number.
  Invalid signatures/collisions are rejected. Arguments currently occupy one
  stack dword each. Existing generated hook entries retain their contracts.
- `vostok::peer_runtime::Call` parses each private synchronous byte buffer into
  typed lifecycle or player operations. Only player operations carry PlayerId;
  start has no meaningless epoch/player fields. Runtime checks the actual OS
  thread before opening the owner. Every subsequent command carries its captured
  LE64 epoch. Outputs are copied only after the owner accepts the completed
  operation. No Rust layout, mutable peer reference or engine pointer escapes.
  `peer_prediction_runtime_026g.h` documents all command sizes/result codes.
- An ordinary recursive operation retires speculation rather than retaining
  hints after a recursively applied native confirmation. Lifecycle replacement
  can still finish after the old callback. Provider/key construction captures
  its owner handle before any loader callback. Old authentication may finish,
  but cannot publish a frame, packet, identity or result into the new owner.
- C retains endpoint eligibility, credentials, socket/rendezvous and temporary
  native projection. `pp_peer_*` functions exchange copied values and epochs.
  Existing connect/reset/destroy hooks construct/retire Rust state; no competing
  hook owner was added. Native correction captures the epoch before calling the
  original engine. Rendezvous hello signing now uses a local packet and checks
  the epoch before committing/sending; random failure cannot disconnect a
  reentrantly created replacement. The pump checks epoch changes across effects.
  These checks do not settle all native world-step/replay lifetime obligations.

### Verification

- `check.py native`: actual compiled Rust bank/CNG adapter/generated export with
  actual 0.26g input/event pools, timestamp/discard/dirty bookkeeping and C
  projection. Existing correction, quarantine, pool/history guards, long frame,
  same-time reset and destruction scenarios pass. New provider-callback cases
  cover wrong-thread calls, two replacements, stale teardown, pending
  cancellation and recursive confirmation. Retired encoding leaves its output
  untouched; provider handles close. OS crypto, socket/roster selection and
  physics are controlled boundaries in this lane.
- The old 32 KiB emulator stack was insufficient: introduction reached below
  that mapping. The fixture now has a bounded 128 KiB stack below entry and
  measures executed stack depth, failing at its 96 KiB test ceiling. The observed
  maximum is **62,904 bytes** including nested replacement; ordinary scenarios
  previously measured **37,236 bytes**. This is evidence about these compiled
  paths, not a universal game-thread stack bound or frame-cost measurement.
  Avoid copying this by-value design into substantially larger transport state
  without T1's explicit storage owner and measurements.
- `check.py windows`: **real nonblocking UDP and CNG under Wine 11.8 Staging**
  pass challenge/intro, direct packets, endpoint rejection, quarantine-preserving
  refresh, resync, destruction, credential clearing and intrusive-reference
  accounting. The fixture loads the lock-verified production arena and calls its
  Rust peer export. Its C socket/control adapter is compiled separately from the
  same sources; the old C codec acts only as the remote sender. The existing CNG
  tamper/HMAC/entropy fixture also passes. An isolated prefix is used; the loader
  reserves its own IAT storage and allocation-granularity-padded arena mapping.
  This is neither a Windows-host test nor a whole-game match.
- `check.py peer`: **14 tests plus 2 compile-fail doctests pass on host and
  i686**, including **302,009 events / 51,494,739 output bytes** matching the
  frozen C/OpenSSL oracle on each architecture. New bank tests cover reversed
  packets, failed-encode sequence consumption, refresh, quarantine, replacement,
  exhaustion and key retirement. Both targets' affected-crate all-targets clippy
  pass with `-D warnings`.
- `check.py arena`: **5,897 cases pass**, including 704 peer traces, 548 CNG
  failure/erasure/comparison cases and existing protocol/runtime contracts. The
  probe now includes initialized `.data` as well as `.bss`: the expanded owner
  enum's initial representation is not all-zero. Probe: **20,804 bytes**, SHA256
  `bb6d8768923ac858a3bae2bca26576193090b0b861d1626750edb5cebabc3a90`.
- Focused native Rust `cargo test --locked -p patches vostok::`: **20 pass**.
  Client workspace all-targets clippy passes with `-D warnings`. Generated engine
  and hook files are current; `hooks_026g.check_layers()` passes. Export generator
  tests pass. `selftest_026g.test_hook_kinds(original)` passes **24 entry-kind
  checks and 9 generator/equivalence tests**. Relevant tracked diffs have no
  whitespace errors. Full game/server suites and real play were not run.

The final arena is **532,614 bytes** at `0x0a42d000`, SHA256
`95434537ce18cb5c3ead70e5982d27c159f531a491453b6692afa0947743ace9`.
`link_026g.py --write` refreshed the lock. The later clippy correction produced
an identical arena, so passing native/Windows checks were not repeated for it.
The entry address comes from linked symbols, never a fixed address in source.

### Artifacts and exact commands

Under ignored `private/rust-client-transition/20261103/`, every lane below
contains commands, logs, selected-input hashes and a verified source archive
(263 inputs). Archive SHA256s:

| Artifact | Result | Source archive SHA256 |
|---|---|---|
| `native-peer-owner-v3` | passed | `425a701551727d8cea6a8ecb691904afd506d09f5a75d7740fd529ef6078a3bc` |
| `windows-peer-owner-v3` | passed; includes production `arena.bin` | `93fa07957f575ea7a636c537fb32dddc40f2c31af5f59686d074721892af4fb4` |
| `pure-peer-owner-v4` | passed | `936dd855538a14cb0f095df2b45561f4db0139b3daddadd1d325c97135a3eb07` |
| `arena-peer-owner-v5` | passed | `30773c263afae325b2e3be6acf16c8e7612d343e7bc279be585db11b9a9dcec0` |
| `final-peer-owner-source` | final source capture, validation references in JSON | `34be6e4dbde8928b17b5449eebc5d7c2b16507643a0b7816d6ca055b23d617f2` |

All archive members were rehashed and matched their manifests. The final capture
includes the generator's improved invalid-prototype diagnostic, which does not
change generated output. Earlier captures predate that diagnostic, the probe
`.data` fix and/or the byte-identical clippy change. AGENTS.md was also updated
concurrently outside this task; it was reread and preserved. Failed earlier lane
attempts remain diagnostic artifacts, not acceptance evidence.

Run lanes with `nix develop ./srp-026g --command python
srp-026g/scripts/rust-client-transition/check.py <lane> --out <new-directory>`.
Native uses `--stock ../srp/0.26g-original/Survarium/game/binaries/x86/survarium.exe`.
Windows compiler/runtime paths and exact arguments are in its result.json;
`--windows-libdir` is required for this installed MinGW runtime. Supplemental
commands/output are in `peer-owner-final-checks.log` and
`peer-owner-entry-checks.log`. The selected-source archive is not a backup of the
entire dirty tree, game resources or the owner's unrelated work.

### Next session

1. Continue T2 with the rendezvous owner. First capture the remaining C control
   lifecycle/bytes as an oracle: TLS credential mailbox consumption, login request
   sequences, hello/challenge, introduction replay, endpoint refresh/expiry,
   generation rotation and bounded work. Keep only one owner at each cutover.
2. Extend the generated OS boundary for RNG and AES-GCM with explicit secret,
   algorithm and per-operation handle ownership; preserve the real CNG vectors
   and failure behavior. The Rust HMAC owner is already production code. Do not
   introduce raw duplicate peer keys or a substitute cryptographic algorithm.
3. Transfer rendezvous/credential state and then socket lifetime into Rust;
   keep native projection behind copied/epoch-checked values until T6. Repeat
   the real-UDP/native scenarios for affected paths, including failed setup and
   teardown. Larger transport storage needs a deliberate bounded owner, not a
   larger Bank moved through every callback.
4. T2's opt-in playable acceptance, T1's native world/lifetime proofs, T3's control
   envelopes and T4–T8 remain unfinished. Native simulation/replay safety and
   frame cost are not established by these fixture passes. Preserve off-by-default
   behavior and release separately only through the owner's release workflow.


## 2026-11-03 — Startup arena allocation on older Wine

Separate from T1–T8: implemented Rust `startup_memory` (patch 78) in the
existing arena, with generated bindings/hook entry and an exclusive retry
owner. After both native attempts fail, it decreases resource requests by
64 MiB per attempt down to the configured minimum. Success consumes the
owner; genuine exhaustion retains the native fatal check. No revision,
content or simulation-compatibility change, commit, push or release.

The reference source already has fragmentation-aware region selection.
Native Wine 9 testing identified the actual final failed-allocation check
at E9BA2. Removing that conditional alone would accept null arena pointers.
Function/ABI findings are in the re-kit; details in
[startup_memory.md](notes/startup_memory.md).

### Verification and limits

- Two i686 Rust budget tests and client `patches` all-targets clippy with
  `-D warnings` passed. The installed Rust/NASM hook passed
  `check_startup_memory_026g.py`: bounded retries, alignment, explicit floor,
  unchanged success, exhausted/invalid boundaries, descriptor identity,
  stack/nonvolatile/volatile/SSE preservation and the native fatal guard.
- Existing `check_memory_defaults_026g.py` and `check_engine_026g.py` passed.
  Focused self-tests `test_apply` and `test_hook_kinds` passed, including
  24 generated entry-kind checks and 7 generator/equivalence tests.
- The runtime probe executes the copied real client and native Win32
  allocator, stopping before video detection. Each run has an isolated Wine
  prefix and Xvfb display; no player prefix or server was used.

| Runtime | Prefix | Final executable result |
|---|---|---|
| Wine 9.0 (Nixpkgs classic 32/64-bit build) | win32 | Passed after smaller retry: managed 1,073,545,216 bytes, unmanaged 715,325,440 bytes |
| Wine 9.0 (same build) | win64 | Passed after smaller retry: managed 1,237,647,360 bytes, unmanaged 618,332,160 bytes |
| Wine 11.0 WoW64 | win64 | Passed on original first allocation; retry unused |
| Wine 11.8 Staging WoW64 | win64 | Passed on original first allocation; retry unused |

The pre-change Wine 9 win32 control failed at E9BA2 as reported. A second
control made from the final EXE by restoring **only** the five original bytes
at E9B96 also failed there. Thus the result is attributable to this hook,
despite unrelated concurrent work in the checkout. The probe checks both
arena addresses are non-null before reporting success. This is not testing
Ubuntu's packaged Wine binary, Windows, DXVK, login, map loading or gameplay.
The existing warning after a successful page-file fallback is unchanged.

Final full client SHA-256:
`034ad35d1405f7b0f594511e022381b75ad1a472f0f5f3f38c8804d65be21955`.
Arena: 516,230 bytes, SHA-256
`a2f22e8e268d3116cf1e354ee4a0eb62f505d1c91169f2bfe284fd549416840e`.
Runtime commands, source/probe hashes and logs are preserved under ignored
`private/startup-memory/20261103/`; `scripts/startup-memory/check.py` is the
reusable runner. Source fragments accompany the binary evidence, not a
claim that the entire shared dirty checkout has been archived.

### Handoff

Implemented, host/native tested and real Wine startup-allocation tested;
**not played or released**. Full pre-push checks were not run; there was no
push or PR. Before shipping, run the required complete suite and separately
test graphics/login/map loading with a suitable graphics stack. T1–T8
remain at their independent migration statuses below. No release is authorized.

## 2026-11-03 — T2 native CNG key owner

The previous turn made progress on the pure peer machine. This turn added its
production HMAC key implementation in vostok, generated SDK bindings, native
failure/erasure/comparison checks, and real CNG interoperability under Wine.
The live peer/socket/projection owners remain C. T2 is still in progress.

### Implementation and boundary decisions

`client-patches/rust/patches/src/vostok/crypto.rs` defines Key implementing the
pure peer MacKey contract. Key is neither Copy nor Clone and exposes no secret
serialization/debug access. Key::take consumes a writable 32-byte source and
erases it even on provider failure. The owned live bytes are erased on drop.
This does not claim that Rust moves, prior stack slots, OS memory or provider
internals contain no historical copies. A fixed-size comparison uses volatile
reads of every byte, then one equality decision; it cannot become early-exit
memcmp in the tested arena build.

One immutable CNG SHA256/HMAC algorithm and one owned bcrypt.dll loader reference
are process-owned. A New/Loading/Ready/Unavailable atomic publication state gives
one initializer exclusive ownership and acquire/release readers. Loading returns
unavailable immediately, including reentry, rather than spinning across loader
calls. Failure/unwind becomes sticky Unavailable. Key is Send through immutable
Provider: each operation creates its own hash, never sharing a mutable hash.
This module touches OS resources only, not an engine world or game-thread token.

Hash owns each successfully created nonnull CNG handle. Update failure drops it;
finish always closes it; destroy failure rejects output; explicit close cannot
also run Drop. Provider failures never expose partial tags. The SDK supports
provider-allocated hash objects for null storage/zero size on Windows 7+, paired
with DestroyHash ([CreateHash](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptcreatehash)).
The cached algorithm follows the existing C provider lifetime; the new loader
always acquires its own reference, so it cannot borrow another caller's unloading
DLL. No new global allocator, crypto algorithm or unauthenticated fallback.

`platform_026g.tsv` records DLL/export/C prototype/Microsoft source for
GetSystemDirectoryW and the five HMAC CNG calls. engine_026g.py generates their
stdcall aliases and typed GetProcAddress resolvers in raw::platform. Signed int
maps to i32 for NTSTATUS. No SDK function receives an invented game VA/re-kit row;
the existing reviewed kernel32 IAT bindings are reused. Loading/ownership stay
in vostok. rust_patches.md documents this extension. RNG/AES intro decryption
remain C and still require a later responsibility transfer.

### Verification and artifacts

- Native i686: **4 crypto tests** pass (single-owner publication, eight-thread
  initialization race, loader reentry/failure/unwind, secret take/drop erasure,
  create/update/finish/destroy failures and exactly-once hash cleanup). Native
  workspace all-targets clippy passes with -D warnings; the first run requested
  NonNull::dangling in the test fixture and was fixed. Generated files are current
  and hooks_026g.check_layers passes.
- The actual adapter now runs in the freestanding arena probe. **548 new crypto
  cases** exercise generated stdcall resolution/cleanup, missing exports, bad
  directory lengths, provider errors/null handles, sticky initialization failure,
  retry after per-operation failure, no partial output, source and live-owner
  erasure, and every tag/key bit. All 256 one-bit key differences execute the
  same **683 instructions / 95 memory reads** as equality, with identical read
  addresses. This is structural evidence for this compiled comparison, not a
  universal CPU timing proof. OS crypto is stubbed with Python HMAC in this lane.
- Together with the peer/core/runtime checks, **5,897 arena cases** pass. Artifact
  `private/rust-client-transition/20261103/arena-cng-owner-v2/`: 257 source inputs,
  archive SHA256
  `84123d40ce5da673a4f46e193770f50dd5919c185efc567e7d1ea7e74f66daa3`.
- New `check.py crypto-windows` loads that exact linked Rust image into an isolated
  i686 Windows test process. Only the game's kernel32 import slots are supplied;
  the Rust adapter resolves real CNG. **1,539 cases pass under Wine 11.8 Staging**:
  RFC4231 HMAC, agreement with the existing C provider for lengths 0..512,
  tampered tags, key comparisons and source erasure. This is not a Windows-host
  run or a playable-game check. Artifact
  `private/rust-client-transition/20261103/windows-cng-owner-v2/`: 257 source inputs,
  archive SHA256
  `36ef5d368fb30466fa1c77427081e83b25ffecc07846aaf29e1a5c57bf9fe7b2`.
- Both successful artifacts contain the same **20,232-byte** probe, SHA256
  `438e0993be49cb5321d8b1bbcd9f5367b4cc4ae7b4d61974ec75c404d583e8ca`.
  Every archived source was read back and hash-verified. Only explanatory
  generator/doc comments changed after the final capture; generated bytes agree.

Failures retained: rustfmt traversing a path-included module looked for its test
child at the wrong path (format the probe with skip_children=true); initial arena
harness expected absolute IAT symbols in the linker's local symbol list (now
reads the engine manifest); initial Windows runner found the game's IAT address
already occupied by its process. The final runner reserves that range inside its
own fixed-base PE .bss and checks bounds before writing. It never frees or
replaces another allocation. Old failed artifact folders are not pass evidence.

Reproduce from the repository root (use new output directories):

```sh
nix develop ./srp-026g --command bash -c 'cd srp-026g/client-patches/rust && cargo test --locked -p patches vostok::crypto && cargo clippy --locked --workspace --all-targets -- -D warnings'
nix develop ./srp-026g --command python srp-026g/scripts/rust-client-transition/check.py arena --out /tmp/srp-cng-arena
nix develop ./srp-026g --command python srp-026g/scripts/rust-client-transition/check.py crypto-windows --out /tmp/srp-cng-windows --windows-cc /nix/store/0n49ig66cbwnq4n4298dj02j86c3d2s8-i686-w64-mingw32-gcc-wrapper-15.2.0/bin/i686-w64-mingw32-gcc --windows-libdir /nix/store/4j1sgd6l2h8yhkzx7943ylplyc7y2dny-mcfgthread-i686-w64-mingw32-2.3.2/lib
nix develop ./srp-026g --command python srp-026g/client-patches/link_026g.py --write
```

The production arena lock was regenerated and compared byte-for-byte with both
successful source archives: unchanged, 516,230 bytes, SHA256
`a2f22e8e268d3116cf1e354ee4a0eb62f505d1c91169f2bfe284fd549416840e`.
No gameplay hook constructs Key yet. No revision/simulation change, commit,
push, full pre-push suite, real play or release happened.

### Next session: transfer live peer ownership

1. Recheck status; preserve unrelated startup_memory and other dirty work. Use
   the native Key, not the peer/arena test providers. Do not keep adding isolated
   replacement primitives when the next dependency is the live ownership bridge.
2. Replace the embedded C pp_link and pp_peer pointer API together. C currently
   retains receive and send copies of the same peer key, checks receive.tag for
   probes/publish, and returns its address to projection/confirmation. Rust should
   own peer identity/replay/hints and outbound encoding; C may temporarily keep
   endpoint/socket/native-memory adaptation. Return bounded values through a
   generated arena entry, never expose/transmute Rust enum layout or retain a
   second mutable peer machine. RustEntry already supplies the cdecl stack shim;
   the declarative hook generator needs an arena export interface without a
   fictitious original engine callee.
3. Preserve control() semantics: same epoch/key refreshes endpoint/introduction
   age without resetting replay/quarantine/outbound sequence; only a new identity
   replaces it. The native bridge must erase transferred plaintext/key buffers.
   A reverse-direction encoder using the owned identity can avoid duplicate raw
   key storage. Preserve original pp_encode/probe bytes/provider failure behavior.
4. Tie construction/reset/destruction to the existing C lifecycle sites and the
   Runtime/Owner gate. Reentrant initialization/teardown must revoke stale work;
   do not install a competing hook. T1's bounded deferred lifecycle handling still
   needs implementation where a replacement arrives during an active callback.
5. After cutover, run the existing adversarial, real-socket, crypto and native-pool
   scenarios against Rust. RNG/AES/control/transport and T3–T8 remain open; HMAC
   evidence alone does not prove live projection, rollback or gameplay safety.

## 2026-11-03 — T2 bounded peer machine and C equivalence

The previous turn made progress on T1 ownership. This turn began T2 by porting
peer_prediction_026g.c into safe, allocation-free Rust and exercising the port
on host, i686 and the freestanding arena. T2 remains in progress: the live client
still uses the existing C peer machine, runtime and projection adapter.

### Implementation

`crates/client-runtime/src/peer.rs` owns Closed/Open/Quarantined state, identity,
replay sequence, rate credit and up to eight hints. Only transition mutates it;
frame expires and copies a bounded owned view together. Pair and Hint constructors
apply admission at the boundary. ConfirmedInput deliberately preserves arbitrary
server action bits so confirmation can contradict a speculative hint. New
GameTime wrapping_since/is_after helpers centralize nearby clock ordering.

MacKey is an owned provider contract, not a replacement crypto implementation.
Identity/Link are not Copy or Clone. Repeated introduction drops its incoming key,
quarantine retains only the identity, and replacement/reset/drop retire the old
key. Production integration must implement constant-time comparison, exact
HMAC-SHA256, provider failure and secret erasure in the native adapter. Test-only
keys do not establish that Windows contract. No unauthenticated fallback exists.

Preserved details include the stricter hint action mask 0x9bfff3ff, half-float
rotation bounds, exact 76-byte encoding, two HMAC calls for probe encoding,
header checks before rate charging, credit consumption on authenticated-path
rejection, monotonic u64 sequences without wrap, repeated-identity quarantine,
no live-hint eviction, off-grid times and unsigned game-clock wrap. An Ignored
or Rejected outcome can still update rate/probe/floor state exactly as C did.

`tests/peer_baseline/` freezes the original untracked C machine/header byte for
byte; README records their hashes. oracle.c supplies real OpenSSL HMAC and
serializes every relevant live state field, ignoring only dead union/array bytes.
Do not update the oracle to follow Rust. SHA2 is a test-only dependency; the
production core adds only the path dependency on net-protocol. Both Cargo locks
were refreshed. No native gameplay hook or raw engine function was added.

### Verification

- Host and i686: all 10 client-runtime tests and 2 compile-fail doctests passed.
  The differential test compares **302,009 events / 51,494,739 output bytes** on
  each target: outcomes, crypto call counts, identity, floor/rate/credit, sequence,
  ordered hints, and exact encodings. It includes every wire bit, truncation,
  authenticated malformed values, provider failures, replay, probes, quarantine,
  replacement, capacity, clock boundaries, 65,536 adversarial samples and mixed
  persistent traces. A separate test checks key retirement ownership.
- i686 net-protocol: all 14 tests and its doctest passed. Host/i686 targeted
  all-targets clippy passed with -D warnings. The first lint run found one manual
  is_multiple_of in a test; fixed. Native client runtime tests (3) and native
  workspace all-targets clippy also passed after the dependency change.
- `check.py peer` now archives and runs the host/i686 differential and lint gates.
  Use `private/rust-client-transition/20261103/peer-core/`: passed, 252 inputs,
  source archive SHA256
  `e91d06040dc668023c1b4c6bbb3fbfe36375ce4d3024352f06fbdce47c03b3d8`.
- The arena probe instantiates the actual peer machine through a generated cdecl
  entry and compares **704 traces** with the frozen C/OpenSSL oracle. Python
  supplies real HMAC at a test-only cdecl stub. It does not exercise Windows
  crypto, sockets or projection. Together with previous checks, **5,349 cases**
  pass, including caller stack/nonvolatile registers and ownership type checks.
- The initial arena build found a real portability problem: slice equality on
  the probe's twelve zero payload bytes emitted a libc bcmp / R_386_PLT32
  relocation. The bounded byte scan now matches C and links without that symbol.
  Keep the failed arena-peer-core artifact as evidence; use the final
  `private/rust-client-transition/20261103/arena-peer-core-v2/`: passed, 252 inputs,
  archive SHA256
  `038df13ea1d4e2c7eb12e6bd398f1cec189c1ff2f6c060658e051796b92321bc`.
  Probe size **18,704 bytes**, SHA256
  `16d78fcba43111b24cebd7fb382633e86ecef06853768b02298842e282147a72`.
  Every source in both final archives was read back and hash-verified.

Reproduce from the repository root:

```sh
nix develop ./srp-026g --command python srp-026g/scripts/rust-client-transition/check.py peer --out /tmp/srp-peer-check
nix develop ./srp-026g --command python srp-026g/scripts/rust-client-transition/check.py arena --out /tmp/srp-peer-arena
nix develop ./srp-026g --command python srp-026g/client-patches/link_026g.py --write
```

The production arena lock was regenerated and is identical to this turn's
pre-regeneration archive: 516,230 bytes, SHA256
`a2f22e8e268d3116cf1e354ee4a0eb62f505d1c91169f2bfe284fd549416840e`.
This differs from the prior turn's a438 hash: separate startup_memory work and
its generated bindings appeared in the shared tree between captures and are
included in these source archives. They were preserved, not claimed as this
port's changes. No revision/simulation change, commit, push, full pre-push run,
real play or release occurred in this turn.

### Next session

1. Recheck shared status and current plan. The original C machine remains in the
   production arena. T1 still needs real lifecycle hooks and bounded deferred
   events/storage; safe generic ownership alone does not prove world lifetime.
2. Implement the native secret owner/provider behind vostok using existing
   platform crypto, including failure, constant-time comparison and erasure.
   Test providers in peer/tests.rs and arena-probe/src/peer.rs are never production
   implementations. Keep raw crypto/FFI out of the pure machine.
3. Replace live C peer ownership through one generated bridge. The C runtime
   embeds pp_link at peer_prediction_runtime_026g.c:77, tests its tag at :328/:386,
   and returns its address from pp_peer (:360). The native projection calls
   pp_frame and pp_transition on that pointer. Replace these ownership/API uses
   together; do not transmute Rust state into the C union or keep competing
   mutable copies. Existing RustEntry supports generated cdecl exports, whereas
   hooks_026g.py callback hooks currently require an original engine callee.
   Determine the minimal generated arena-to-arena interface before live cutover.
4. Existing reset/destructor hooks already own the sites. Integrate their
   construction/reset/destruction and owning-thread policy without a second hook
   owner. Then run real Windows crypto/socket and native-pool checks against Rust.
   T2 completion requires those integration checks and opt-in playable behavior.
5. T3 control/static-info/snapshot decoding and T4–T8 remain open. No timeline,
   native move scheduler or world simulation ownership transferred in this turn.

## 2026-11-03 — T1 native thread gate and ownership contracts

The previous turn made progress on shared codecs and freestanding compilation.
This turn added the actual native thread gate over the non-copy owner, linked it
as a client dependency, and exercised its generated Windows import and callback
entry paths. T1 remains in progress: no gameplay hook constructs this owner yet.

### Implementation and review

`client-patches/rust/patches/src/vostok/runtime.rs` defines Runtime<T: Send>.
A verified initialization hook must bind explicitly. An incidental unbound
callback cannot claim the runtime. Every access checks the actual generated
GetCurrentThreadId binding before forming a reference to its Owner. Foreign
threads cannot access or rebind it. Same-thread reentry shares only Owner; its
existing transition machine rejects recursive mutation but accepts invalidation.
No lock spans external calls. Cleanup remains after the active callback returns.

An initial version allowed owner handles to escape a checked callback. Review
tightened this: callback lifetimes now prevent that escape, and entry/exit use
atomic acquire-RMW/release publication. This also synchronizes Rust state if the
OS reuses a terminated thread's ID. T: Send is deliberate: it permits safe Rust
resource handover/drop in that case, not native-world access. World references
and thread-sensitive engine destruction still require separate scoped effects.
Do not reuse the existing unconstrained Sync Lock or Copy-only GameCell.

`client-runtime::Owner::current` captures a handle for a new synchronous entry,
so state can persist between checked native callbacks. Delayed/reentrant work
must retain its original attachment identity, not reauthorize against a new
current owner. During revocation, current still names the old attachment but
run/effects reject it; after callback cleanup there is no current attachment.

The client Cargo manifest/lock now includes client-runtime. The native gate is
in vostok, where raw access belongs; pure owner code remains unsafe-forbidden.
No function/ABI discovery required a new re-kit row: the existing reviewed
engine_get_current_thread_id binding at IAT 0x870118 is used directly.

### Verification

- i686 client workspace: 3 new runtime tests passed. They cover explicit binding,
  actual host threads with distinct test identities, an eight-thread first-bind
  race, reentrant invalidation, exactly-once cleanup and stale replacements.
- i686 client-runtime: 8 owner tests and 2 compile-fail doctests passed, including
  the new current-epoch test. Both the client workspace and client-runtime passed
  all-targets clippy with `-D warnings`.
- The arena probe now includes the **actual** native gate, hook types, generated
  engine bindings and engine-function library. The GetCurrentThreadId provider is
  stubbed through its real IAT slot; this is not a Windows API implementation.
  Twenty-five new native lifecycle cases cover persistent resources across
  callbacks, wrong/unavailable IDs, reentrant invalidation, stale effects,
  replacement and failed-attachment cleanup. With the existing codec checks:
  **4,645 freestanding cases passed**. Caller stack/nonvolatile registers pass.
- The probe also compiles one valid use and rejects two invalid programs against
  the actual adapter: escaping an Owner reference and storing non-Send resources.
  The initial probe import put hook.rs outside crate::vostok and failed its
  restricted visibility; the probe hierarchy was corrected. No production
  visibility was relaxed to make the test pass.

Commands from the repository root:

```sh
nix develop ./srp-026g --command bash -c 'cd srp-026g/client-patches/rust && cargo test --locked -p patches vostok::runtime && cargo clippy --locked --workspace --all-targets -- -D warnings'
nix develop ./srp-026g --command bash -c 'cd srp-026g && cargo test --locked -p client-runtime --target i686-unknown-linux-gnu && cargo clippy --locked -p client-runtime --all-targets --target i686-unknown-linux-gnu -- -D warnings'
nix develop ./srp-026g --command python srp-026g/scripts/rust-client-transition/check.py arena --out private/rust-client-transition/20261103/arena-runtime-v3
nix develop ./srp-026g --command python srp-026g/client-patches/link_026g.py --write
```

Use `private/rust-client-transition/20261103/arena-runtime-v3/`: passed, 240
source inputs, archive SHA256
`97ace6ca1db775ee7db426aebb7bb57d6c86836583ee4b567195884311b6b683`.
Every archived source hash was read back and verified. Probe size **11,304 bytes**,
SHA256 `e5ebf102b33f21211048fabcd3406e049b6bb88aaa9d21eaffd7dd70b2c760d9`.
The v2 run passed the earlier interface; v3 is the final scoped interface proof.

The final arena lock was regenerated and compared with the archived pre-runtime
lock in server-input-codecs/source.tar.gz: **identical**, 516,230 bytes, arena
SHA256 `a43800aa9e62d03e2520b26303697f15635f6a908545b4ea00494d097c198b4c`.
The generic runtime is not instantiated by a gameplay hook yet and emits no live
arena bytes. This is intentional evidence of current scope, not a client cutover.
No revision/simulation change, commit, push, full pre-push suite, real play or
release occurred.

### Next session

1. Recheck status. T1 still needs bounded deferred events/storage and actual
   construction/reset/destructor integration; the generic gate does not establish
   native world-effect safety. Real wrong-thread teardown must never be silently
   ignored while an engine effect could still access the world.
2. Begin T2's bounded peer machine port against the archived C fixtures. The pure
   C machine is only about 220 lines in peer_prediction_026g.c, with 8 hints per
   peer and 20 peers; preserve HMAC authentication, rate limits, quarantine,
   timestamp wrap and action mask 0x9bfff3ff. Its runtime/native adapter remains C.
3. Determine the generated callback bridge needed to replace C entry points
   without adding hand-written engine bindings. Existing pp_core_reset_hook /
   pp_core_destroy_hook already precede native release and overlap those sites;
   do not install a second independent owner/hook at the same site. Retire each
   C responsibility as its Rust successor takes it.
4. Keep T3 control/static-info/snapshot decoding and T4–T8 open. The allocator-free
   probe is not evidence of real sockets, a game-thread hook, rollback or play.

## 2026-11-03 — T3 time/input codecs and freestanding execution

The previous turn made progress by extracting ACK/framing. This turn added the
time/input codecs, connected production server paths to them, implemented the
client's input-event decoder and executed the exercised codec paths as actual
freestanding i686 arena code. The transition remains active; no client socket,
native match machine or timeline owner has been replaced yet.

### Implementation and compatibility

- `net-protocol/src/time.rs`: GameTime, TickByte, validated TickOffset and
  grid/precise WireTime variants. `resolve_checked` rejects outside-u32 times;
  `resolve_wrapping` preserves native unsigned wrap explicitly. Signed half-range
  ambiguity resolves backwards. Server tick helpers now re-export this code.
- `net-protocol/src/input.rs`: DC/DD parsing and bounded encoding, raw half-float
  rotation/action values and the existing server rotation-admission predicate.
  E9–F5 decoding includes membership, single and columnar batch relays,
  move/discard, and profile events. Batch storage borrows validated columns and
  yields players in mask order without allocating. Input-event decoding requires
  a validated 1–20 roster; player IDs cannot be constructed from unchecked bytes.
- `survarium::v026g::match_msgs`: production DC/DD parsing, single-relay value
  encoding and rotation admission now consume the shared core. The public server
  WireTime/PlayerInput/message APIs are preserved. Raw rotations and all action
  bits remain unchanged, including bit 26 and bit 31; this does not fix the
  separate native quick-slot defect or change simulation/replay behavior.
- `survarium/tests/input_protocol.rs`: comparisons with the independent client
  model, plus the pre-extraction client-input contract and tick arithmetic.
  No production code imports the client model as its decoder.

The old server WireTime compatibility struct still permits invalid public `ms`
and its old `resolve` addition is unchanged. New typed callers cannot construct
invalid offsets and select checked/wrapping overflow behavior explicitly. The
zero-roster helper discrepancy is also preserved; input events are decoded only
after a valid roster exists. Remaining control/static-info/snapshot envelopes
still need shared decoding and bounded decompression/native effect contracts.

### Tests and limitations

Passed on host: 14 net-protocol tests + its compile-fail doctest, 115 existing
match protocol tests, 4 new input equivalence tests, and 33 match-server tests
selected by `input` (relay, late correction, admission, profile, resync and bots).
Host `cargo clippy --locked -p net-protocol -p survarium --all-targets -- -D warnings`
passed. The input test covers all 131,072 tick/offset combinations for DC/DD,
all binary16 patterns in the core, sparse/full batch columns and malformed event
fields. An initial test compile failed on an ambiguous integer type; corrected.

On i686, all 14 core tests and its doctest pass. Attempting the survarium model
integration test on i686 fails before tests in existing
`binary-config-parser/src/lib.rs:36`: its assertion requires BinaryValue alignment
8, but this target gives 4. This server/resource crate was not changed to make a
test pass; the new client core has no dependency on it. Do not claim the entire
survarium comparison ran on i686.

Commands run under `nix develop ./srp-026g --command`, with Cargo in `srp-026g`:

```sh
cargo test --locked -p net-protocol
cargo test --locked -p survarium --lib v026g::match_msgs
cargo test --locked -p survarium --test input_protocol
cargo test --locked -p net-protocol --target i686-unknown-linux-gnu
cargo test --locked -p survarium --test input_protocol --target i686-unknown-linux-gnu # fails as described
cargo clippy --locked -p net-protocol -p survarium --all-targets -- -D warnings
cargo test --locked -p match-server --lib input
```

`private/rust-client-transition/20261103/server-input-codecs/` records the
credential/TLS/rendezvous checks (4 tests), match protocol regressions (115) and
input-model comparison (4), all passed. Its source archive has 588 inputs, all
hashes read back and verified; SHA256
`d00e8d639a2cd3d4a740bee6b7f9802b9973c6df22bb964e8b24d7593baa8aa9`.
`match-input-integration.log` there records the additional 33 server tests.

### Freestanding arena proof

Added `scripts/rust-client-transition/check_arena.py` and `arena-probe/`.
The probe uses the actual link_026g.py compiler/sysroot/linker, the arena's real
`vostok::rt` memory routines and generated RUST_CDECL entries. It imports the
shared codec as a normal path dependency; no allocator or libc is linked. Only
the panic diagnostic logger is stubbed. The probe is never installed in an EXE.
No arena lock or client-patch source was changed.

Unicorn passed **4,620** cases: variable u128 ACK shifts and all tails, valid and
malformed datagrams, 252/253-message limits, individual and columnar inputs,
precise times and wrap at time zero. Calls deliberately start with a dword-aligned
game-style stack; generated entries restore the caller's stack and nonvolatile
registers. Linked probe size is **6,444 bytes**, SHA256
`e075ad534410537387655cd97078e31f4f00ffd053f6048c1c9d1c2f3a529375`.
No undefined compiler helper was required by the exercised paths. This is not
yet a test of Owner, real engine hooks, Windows sockets or world ABI operations.

```sh
nix develop ./srp-026g --command python srp-026g/scripts/rust-client-transition/check.py arena --out private/rust-client-transition/20261103/arena-codecs
```

`arena-codecs/result.json` is passed, with 238 source inputs; archive SHA256
`386b6d442a09a493f5782c7576d4609772e2dbd5d0362f65532d9ec2018df516`.
The runner now archives its sibling scripts/probe sources too. The first direct
probe attempt declared empty data/state blocks and the linker rejected them;
those declarations were removed. `arena-probe-v2` passed the earlier, narrower
ACK/framing probe; use `arena-codecs` for the expanded proof.

### Next session

1. Recheck status and preserve unrelated dirty work. No real game, release,
   commit, push or full pre-push suite has run.
2. Prioritize T1's actual native owning-thread/container adapter and lifecycle
   proofs before adding more pure libraries. `engine_get_current_thread_id`
   already has an engine-library binding (IAT 0x870118). `vostok::Thread` is only
   a non-Send hook-scope token, not a runtime thread-identity check. Existing
   `Lock<T>` has unrestricted Sync and a test thread ID of 1; do not reuse it as
   a resource owner. `GameCell` is Copy-only. Establish construction, reentry,
   invalidation and destruction, with tests using distinct real/test identities.
3. The arena linker already walks path dependencies recursively (`build_crate`);
   no hard-coded dependency list needs extending. When integrating real native
   modules, run their focused native checks and regenerate the arena lock.
4. T3 remains open for control messages and static-info/snapshot envelopes. T2
   P2P migration, T4 transport and T5–T8 ownership transfers remain unimplemented.

## 2026-11-03 — T3 ACK and datagram framing extracted

The previous session made progress: the baseline was reconstructed and the pure
owner and sequence/table core were introduced. This session moved the production
server ACK codec and datagram parser into allocator-free `net-protocol`. T3 remains
in progress; no native connection or client hook has changed ownership yet.

### Implementation

- `crates/net-protocol/src/ack.rs`: unchanged decoder semantics, bounded encoder
  with a validated `TailLen` and inline tail bytes. The sender cap remains 17;
  the reader accepts 18 and preserves the legacy reported count of 127.
- `crates/net-protocol/src/wire.rs`: both tables/directions, fixed/variable
  framing, multipart, low-level messages and patched E0 carriers. A caller owns
  `MessageBuffer`; only successful whole-datagram parsing exposes ACKs/messages.
  Descriptors hold offsets rather than receive-buffer references. Rust borrows
  prevent scratch reuse while a parsed view remains live. `Delivery` and
  `Fragment` enums replace combinations of class/optional sequence/part metadata.
  A following fragment's index is nonzero; only the first carries a count.
- `udp-transport::{ack,wire}` preserve their public allocating APIs, including
  error variants and payload offsets. Existing server call sites now use the
  shared production parser. The independent client model is unchanged.
- `table::player_inputs_len` uses checked size arithmetic: impossible caller
  mask/head sizes return None instead of overflowing. Supported packets retain
  their existing acceptance and bytes. No gameplay/replay computation changed.
- Added a bounded u16 length-prefix encoder for the client. The server's public
  `push_prefix(usize)` retains its historical casting behavior, including outside
  representable wire lengths; do not accidentally change that during extraction.

Preserved deliberately: FE is decoded but must never be dispatched through the
native jump table; zero first-fragment counts remain a reassembly concern;
variable-message minima are checked in the original order. MAX_DATAGRAM remains
a transport receive/send bound, not a new codec rejection. The codec still
supports larger buffers used by logs and extensions.

Measured `size_of` using the built rlibs and pinned Nix toolchain: MessageBuffer
is 4,032 bytes on i686 / 6,048 on x86-64; EncodedAcks is 24 / 32 bytes. The client
adapter must provide the storage deliberately. Neither crate is linked into the
freestanding arena yet; test binaries linking libc do not establish that proof.

### Verification and archived evidence

Host and i686 both passed 9 net-protocol unit tests, 84 udp-transport unit tests,
4 differential integration tests and 2 doctests. The core tests cover storage
reuse after failed parsing, full capacity, typed fragments, extreme size inputs
and tail bounds. The new compile-fail doctest prevents concurrent scratch reuse.
Host clippy for both crates/all targets passed with `-D warnings`.

The temporary test-only fixtures at `udp-transport/tests/protocol_baseline/`
preserve the pre-extraction ACK/parser implementation from
`8a72cd9da52b1e4c03cc608a0ef3ce380085e8c1`. Compared with that revision: identical
after restoring the test import paths and ignoring formatting. The gate checks
9,216 ACK encodings, 19,712 ACK decodings, all 65,536 u16 prefixes and **249,252**
datagrams, comparing full compatibility values and exact rejection errors.
It covers every ID/fragment shape in both directions/tables, deterministic random
packets and mutations/truncations of valid shapes, including patched carriers.
Retire only these frozen implementation copies at T7; keep the independent model.

Commands from the repository root (inside Nix, working directory `srp-026g` for
Cargo):

```sh
nix develop ./srp-026g --command bash -c 'cd srp-026g && cargo test --locked -p net-protocol -p udp-transport'
nix develop ./srp-026g --command bash -c 'cd srp-026g && cargo test --locked -p net-protocol -p udp-transport --target i686-unknown-linux-gnu'
nix develop ./srp-026g --command bash -c 'cd srp-026g && cargo clippy --locked -p net-protocol -p udp-transport --all-targets -- -D warnings'
nix develop ./srp-026g --command python srp-026g/scripts/rust-client-transition/check.py transport --out private/rust-client-transition/20261103/transport-framing
nix develop ./srp-026g --command python srp-026g/scripts/rust-client-transition/check.py server --out private/rust-client-transition/20261103/server-framing
```

Both archived lanes passed. Transport now includes net-protocol tests as well as
the server facade. `transport-framing/` has 231 selected inputs; source archive
SHA256 `01d29f0cfea48583c729e20952b15e39ebee43a29e01a38dfdb43bae513fa74d`.
`server-framing/` has 584 inputs; source archive SHA256
`187055ad086fe433b3f066504d7a7d34fcd2944eb84d18348046192189f97435`.
The server lane passed the 4 existing credential/TLS/rendezvous tests; it is not
the entire server suite. Each lane records exact commands, logs and source hashes.
No full pre-push suite, client arena checks, game play, commit, push or release
occurred in this session. Revision and simulation-change files were not edited.

### Next session

1. Read current status and this entry. The transition remains active: T1 native
   integration, T2 P2P migration and T4–T8 are still open.
2. Continue T3 at `survarium/src/v026g/match_msgs.rs`: wire time and input codecs,
   then production server-to-client message decoding. Its `client_model` remains
   an independent test oracle, not a production implementation to import.
3. Preserve precise/off-grid time and rotation bytes. Investigate before unifying
   duplicate helpers: match_msgs::mask_bytes(0) returns 1, table::mask_bytes(0)
   returns 0. WireTime currently allows public invalid milliseconds and its
   resolve adds without checking u32 overflow near the end of time; define typed
   boundary behavior explicitly rather than silently changing valid traffic.
4. T1 still needs native owning-thread checks, deferred events and hook-lifetime
   proofs. Integrate the pure owner without the copy-only GameCell or unconstrained
   Sync lock. Prove freestanding linkage and runtime storage before client cutover.

## 2026-11-03 — T0 reconstruction verified, T1 and T3 started

The previous session made progress (durable plan, harness and client baseline).
This session captured the server baseline, established local game identities,
reconstructed the baseline EXE from the archive, added the first Rust runtime
ownership primitive and moved shared protocol logic into its own crate.

### Baseline evidence

`private/rust-client-transition/20261103/server/` contains 571 selected source
inputs, including untracked prediction code, logs and source.tar.gz. Archive
SHA256 `fbac4cfec07fbf1227e8b4982e7203e699ffb5a8fd56385dae0501d2cb796a6b`.
Its focused tests passed: session credentials (1), TLS negotiation (1), match
rendezvous (2). The runner refuses a test filter with zero matches.

Built `native-baseline.exe` from the current client source with the exact stock
EXE and `--server=127.0.0.1:1234`. Then extracted Git revision
`8a72cd9da52b1e4c03cc608a0ef3ce380085e8c1` into
`/tmp/rust-client-rebuild-20261103-v1`, overlaid server/source.tar.gz, verified all
571 input hashes and rebuilt independently. The two EXEs are byte-identical:
`8285ab3d0314a8a32b4d4303e72f328374cdb542062bd8df2ec39005a993afb4`.
`reconstruction.json` records the result. These EXEs were not installed or played.
They retain the source's revision 36; no release revision was reserved or changed.

`game-identities/inputs.json` identifies 30 actual mounted inputs (stock resources,
numbered patches and the selected test40b executable) from the local launcher game
directory. The selected existing EXE SHA256 is
`086cd35f33e15dbcdd9ce22a2cdb151f02e9d1a5df391bc5a2d8c736e56c6b5e`.
Its source provenance is unknown; use the newly built baseline for code comparisons.
The local launcher has several older per-account executable overrides: do not
assume its configured clients form a matched baseline.

Added `capture_game.py` to hash exactly the mounted numbered patch set and detect
changes during capture. Reflink capture was attempted but the filesystem refused
it; `game-inputs/inputs.json` records that failure. Identification succeeded
without duplicating the roughly 13 GiB installation. Resource paths/hashes are
recorded, not backed up or validated as release-manifest content. The existing
installation and stock EXE must remain available to repeat whole-game tests.

### Runtime ownership implementation

Added allocation-free, `no_std`, unsafe-forbidden `crates/client-runtime`.
`owner::Owner<T>` owns non-copy resources; handles bind to both owner and epoch.
All slot mutations pass through one total transition function. A callback leases
the resource without retaining a RefCell borrow across effects. Invalidation
revokes immediately and releases resources after an active callback returns.
Stale effect/callback results are discarded. Recursive mutation is rejected.
Delayed invalidation of an old handle cannot destroy a replacement. Epochs never
wrap; exhaustion refuses a new attachment. Unwinding retires the leased resource.

Seven lifecycle tests pass on host and i686: ownership/cleanup, reentrant reset,
recursive mutation, replacement/stale callbacks, exhaustion, panic cleanup and
reentrant destruction. Two compile-fail doctests pass on i686: the owner is not
Sync, and a callback cannot return its mutable resource borrow. Focused clippy
with `-D warnings` passes on both host and i686.

This is not yet installed in the client. The native adapter must establish the
owning thread, handle deferred ordinary events, and prove each engine effect.
Resource destructors may close owned resources, but must not dereference a world
already destroyed by invalidation. The owner does not make native calls safe.

### Shared protocol implementation

Added allocation-free, `no_std`, unsafe-forbidden `crates/net-protocol`.
Moved sequence windows/arithmetic and message tables from udp-transport, leaving
public compatibility re-exports there. Table Duration uses core instead of std;
tests import vec only under cfg(test). A source comparison against HEAD confirms
the moved logic/tests are otherwise identical. No packet or gameplay change.

All 89 original unit tests pass (5 in net-protocol, 84 in udp-transport), plus the
transport doctest. Net-protocol's five tests also pass on i686. Focused clippy
passes with `-D warnings`. The server baseline was rerun after extraction:
`server-shared-protocol/result.json` is passed. The client arena does not yet link
these crates; freestanding arena linkage still needs checking when they are used.

### Next session

1. Read current Git status and this entry. T0 source reconstruction is verified;
   it is not real-game acceptance. No full pre-push suite or release has run.
2. Continue T1 with the game-thread/native adapter and explicit bounded storage;
   do not use the existing copy-only GameCell or unconstrained Sync lock to host
   the new owner. Capture reentrant reset/destruction ordering at actual hooks.
3. T3 can continue independently: move ACK encode/decode into net-protocol using
   bounded output storage; preserve udp-transport's public allocating API through
   an adapter and keep its independent client model. Framing/input codecs follow.
4. T2's peer machine port can be developed against the saved C fixtures, but no
   client cutover until the owner/thread/native contracts pass. T4–T8 remain open.

Source archives now include the new runtime/protocol crates. Never infer goal
completion from these initial libraries; no native networking owner has yet been
replaced. Keep the full transition goal active.

## 2026-11-03 — T0 baseline work started

The owner approved the transition and implementation. Match runtime first,
compatibility first, P2P migrated early but opt-in. The older source reference is
`../vostok` (0.100b), confirmed by the owner. No deployment authorized.

### Baseline and findings

- Main checkout HEAD at investigation: `8a72cd9da` (not a complete baseline:
  many tracked edits and untracked P2P files are present).
- Reference source HEAD: `../vostok` `2abd75e36`;
  version catalog HEAD: `../vostok-versions` `c706bdb`.
- Stock 0.26g SHA256:
  `ac33d9937015c2299bf62cc6b127cb308b48a3475b30fff2ddd65479ae3bf153`.
  Available at `../srp/0.26g-original/Survarium/game/binaries/x86/survarium.exe`.
- 0.21d identities and local paths are in the version catalog and re-kit identity.
- ABI inputs for `atomic_tick` (0x2d1e20), history serialize/deserialize/hash
  (0x324be0/0x324d80/0x325150) are still inferred. Do not expose them as safe
  world operations without executable contract checks.
- Existing `GameCell<T: Copy>` is unsuitable for owning sockets/native allocations.
  A non-Send thread token alone does not establish callback thread identity or
  prevent reentrant mutable access. T1 must establish both at runtime boundaries.
- P2P uses native history/pools. Reset/destruction hooks precede release; same
  timestamps do not imply the same lifetime. The ordinary input path still has
  the quick-slot bit-26 defect described in peer_prediction.md.
- Snapshot representation is much larger than serialized snapshot bytes. Keep
  native state opaque initially; do not size history storage from the 32 KiB
  compressed-snapshot output bound.

### Ownership inventory

| Boundary | Current owner | Transfer |
|---|---|---|
| Login credential callback | native TLS + P2P synchronized mailbox | T2 mailbox, T8 login |
| Direct UDP socket, endpoint/epoch, crypto handles | peer_prediction_runtime/crypto C | T2 |
| Direct input validation/quarantine/hints | peer_prediction C | T2 |
| Temporary native nodes, projection/restore | peer_prediction_native C | adapter T2, retire T6 |
| Ordinary socket/ACK/retry/fragment queues | native udp_match_connection | T4 |
| Match callbacks, generations, ready/resync/send timers | native network_client | T5 |
| Local/received/profile inputs and membership history | native game_world_core | T6 |
| History allocation, world restore/tick/hash | native core/history objects | T1 proof; T6 scheduling only |
| Lobby data and Flash pushes | native lobby handlers | T8 |

P2P also wraps ordinary input calls and rendering/confirmed moves. Audit these
alongside the late-join ready/disconnect/destructor hooks, snapshot upload, replay,
hash cadence and existing Rust presentation hooks before installing replacements.
The authoritative current list is `PEER_PREDICTION_HOOKS` plus generated hook
manifests, not a copied list of addresses in this log.

### Checks

During planning, from the repository root:

```sh
nix develop ./srp-026g --command bash -c 'cd srp-026g && cargo test --locked -p udp-transport'
```

Result: 89 unit tests and one doctest passed. Sandbox execution initially could
not reach the Nix daemon; the authorized escalation succeeded. This does not
verify the current client arena, P2P or a live game.

### Handoff

T0 remains in progress: the standalone client baseline is now captured, but the
server P2P tests and whole-game/resource baseline still need capture. No Rust owner
has replaced native code yet. No full pre-push checks or release have occurred.

Implemented `scripts/rust-client-transition/check.py` and its README. Lanes:
host, native, windows and transport. Each new output directory stores command
arguments, logs, tool paths, relevant environment, exact selected input hashes
and `source.tar.gz`, including untracked client sources. Source drift invalidates
the result. A caller-supplied stock EXE must match the re-kit hash. Windows tests
use an isolated Wine prefix, not a player's prefix. The Nix shell now supplies
Python Unicorn 2.1.4 explicitly. Existing generated client code was not rewritten.

Persistent artifacts are under ignored `private/rust-client-transition/20261103/`:

| Directory | Result | Evidence |
|---|---|---|
| `host` | passed | 65,536 adversarial cases, ASan/UBSan; leak detection disabled under ptrace |
| `windows-v2` | passed | i686 CNG/OpenSSL interoperability and real UDP lifecycle fixtures under Wine 11.8 |
| `native` | invalidated | native assertions passed, but runner source changed during execution |
| `native-stable` | passed | actual native input pools, corrections, lifetime and ABI contracts; stable source inputs |

The host and Windows archives were read back: all 218 source hashes matched
their manifests. Archive SHA256s:

- host: `0c8fe238f71526e78fc72011aa4a4c36ef89e0e1d8980cbee3b407bffb26a443`
- windows-v2: `80a37c6832521fa8406c5f8728a0b5a4837ab168094a032190411c58b697806e`

Exact commands are in each result.json. For the Windows lane on this workstation,
the installed cross compiler needed an explicit `--windows-libdir` (LIBRARY_PATH
alone did not work): compiler
`/nix/store/0n49ig66cbwnq4n4298dj02j86c3d2s8-i686-w64-mingw32-gcc-wrapper-15.2.0/bin/i686-w64-mingw32-gcc`,
library directory
`/nix/store/4j1sgd6l2h8yhkzx7943ylplyc7y2dny-mcfgthread-i686-w64-mingw32-2.3.2/lib`.
Keep these store paths available or provision equivalent tools before rerunning.
Wine fixture success is not validation on Windows or in a full game.

Failures resolved during harness work: incorrect repository parent calculation;
missing Unicorn; LeakSanitizer's ptrace incompatibility; cross-linker thread-runtime
search path. The sanitizer workaround is explicit in result.json and the README.

Next: finish T0 by capturing the server-side prediction/session/login sources and
running their focused tests, and identify resources/replay inputs for mixed-client
acceptance. Do not treat these selected client archives as a complete backup of
the dirty repository. Then implement T1 lifetime ownership with tests for
reentrant reset/destruction before native effects. Keep this goal active.

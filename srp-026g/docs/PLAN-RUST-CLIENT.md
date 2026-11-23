# Rust client transition

Replace the 0.26g client's internals one complete responsibility at a time,
keeping the game playable. The first finish line is Rust-owned match networking,
synchronization, input history and rollback coordination, with native world
simulation behind an explicit adapter. Then replace login/lobby and the remaining
engine. The owner's one-month target is an ambition, not a measured estimate.

This is the canonical plan approved on 2026-11-03. Read
[RUST-CLIENT-WORK.md](RUST-CLIENT-WORK.md) before starting a session. Update both
documents when a gate passes or a decision changes. Do not infer completion from
code existing: distinguish implemented, host-tested, native-tested, played and
released. No release is authorized by this plan.

## Decisions and evidence

- Preserve current wire formats, deterministic gameplay and replay behavior
  during ports. Make intentional behavior changes separately.
- Migrate P2P early, keeping it opt-in. Complete the match runtime before the
  broader login/lobby and UI integration.
- Use the existing Rust arena and generated hooks, with raw bindings confined to
  `vostok`. Pure logic must remain independent of native addresses so it survives
  eventual removal of the native executable.
- The exact 0.26g executable is the behavioral authority; identify the patched
  baseline too, because it contains extensions absent from stock.
- `../vostok` is the older reconstructed C++ reference the owner confirmed. Its
  version is **0.100b**, not 0.11. Use it for implementation/ownership evidence,
  not as a specification for 0.26g lockstep.
- The 0.21d EXE/PDB are available at
  `/mnt/hdd/Projects/Survarium Builds/survarium_full_021d/game/binaries/x86/`.
  `../vostok-versions/versions/0.21d/builds/2010/` records their identities and
  procedure structure; the catalog also contains 0.11 evidence.
- Record the build, address, source revision and confidence of each finding.
  Add established function/ABI findings to the re-kit in the same change.

Start with [peer_prediction.md](notes/peer_prediction.md),
[rust_patches.md](notes/rust_patches.md), [transport.md](notes/transport.md),
[match.md](notes/match.md), [login_chat.md](notes/login_chat.md), and
[snapshot_rules.md](notes/snapshot_rules.md). Their historical status statements
are not current test results; use the work log for this migration's evidence.

The initial inspection found C/NASM peer prediction with a Rust server, an
allocation-free `no_std` Rust arena, and reusable server transport/client-model
tests. The inspected arena lock occupied 516,230 bytes of the 1 MiB limit; this
was not a fresh build measurement. Several lower world/history ABI entries are
still inferred. The initial working tree contains extensive unrelated changes
and untracked P2P work: HEAD alone cannot reproduce it.

## Ownership and interfaces

```text
OS sockets / TLS / cryptography / native callbacks
                    |
          platform and engine adapter
                    |
          typed events / effect results
                    |
             Rust match runtime
       transport / synchronization / input timeline
                    |
              native world adapter
                    |
              world simulation
```

Add two pure library crates:

- `net-protocol`: shared wire types, bounded parsing/encoding, sequence arithmetic
  and constants; no sockets, engine bindings or allocation requirement.
- `client-runtime`: client transport, lifecycle, clock/sync state, prediction and
  timeline; typed events in, effects out; no raw engine access.

The existing patch crate integrates them. Platform/engine access stays in
`vostok` through generated bindings. Preserve the server transport API while
extracting protocol code underneath it. Keep independent model/captured-byte
tests independent; production must not depend on test models.

Use distinct types for monotonic time, game time, grid ticks, precise event time,
player/session IDs, sync generations and connection/world epochs. Preserve
off-grid precise inputs and profile inputs; a grid-only timeline is incorrect.
Parse untrusted bytes once into typed values. Each machine has one transition
function and variants carrying only valid state.

Introduce a resource-owning runtime container, not a larger `GameCell<T: Copy>`.
Run transitions to completion; release Rust borrows before native effects; return
effect results as events. Reentrant reset/destruction invalidates the world epoch
immediately, before native memory is released. Reject subsequent stale effects.
No engine reference may outlive its valid callback scope.

Large buffers use explicit bounded storage outside the code arena, with an
audited platform allocation owner behind `vostok`, not a new global allocator.
Establish capacity/overflow contracts from the supported native traffic before
transport cutover. Do not silently discard confirmed inputs on exhaustion.
Native snapshots initially remain opaque owned resources with paired
construction/destruction. Their representation is a later replacement.

## Work packages and status

Separate startup fix (2026-11-03): the Rust `startup_memory` hook retries
failed resource reservations through the native allocator. Implemented,
host/native tested, and allocation-tested under Wine 9.0 (both prefix modes),
11.0 WoW64 and 11.8 Staging WoW64; not played or released. This does not
complete a networking migration gate. Evidence: [startup_memory.md](notes/startup_memory.md)
and the startup-memory entry in the work log.

| ID | State | Dependencies | Ownership after completion | Next action |
|---|---|---|---|---|
| T0 | baseline recorded | none | unchanged baseline | Archive reconstructed byte-for-byte; real-game acceptance remains later |
| T1 | in progress | T0 | runtime lifetime and effects | Thread gate and bounded lifecycle replacement integrated with peers; next general event/storage limits and world-lifetime proofs |
| T2 | in progress | T1 | peer protocol and prediction state | Rust networking and projection drivers installed; next integration/retirement audit and playable acceptance |
| T3 | in progress | T0 | shared protocol definitions | Time/input codecs extracted; native-code probe passes; next control and snapshot/static-info envelopes |
| T4 | planned | T1, T3 | match socket and transport | Implement client-direction reliable UDP |
| T5 | planned | T4 | match protocol and synchronization | Replace native match dispatch/lifecycle |
| T6 | planned | T1, T2, T5 | input/history/rollback decisions | Prove stepping seam, then transfer timeline |
| T7 | planned | T6 | complete match runtime | Mixed-client acceptance and retirement audit |
| T8 | planned | T7 | login/lobby networking | Replace connections, then message families |

### T0 Baseline and handoff

Create this plan and work log, link them from agent instructions and the notes
index. Record exact executable/resource identities, source revisions and hashes
of selected dirty/untracked dependencies. Inventory entry points, callbacks,
state owners and overlapping patches. Make standalone P2P checks reproducible.
Preserve unrelated work; do not stash/reset the tree or commit unrelated changes.

Exit: another session can reproduce the selected baseline without this chat or
omitting untracked files. Separate failures from checks not yet run. A hash
manifest identifies a baseline but is not, by itself, a source backup.

### T1 Runtime ownership and boundary proofs

Implement non-copy runtime ownership, lifecycle epochs, event/effect handling and
bounded storage. Verify transport and world capture/restore/step/release seams
against 0.26g, including constructors, failure paths, callbacks and destruction.
Write executable ABI/lifetime/reentrancy checks before using inferred bindings.

`vostok::runtime::Runtime<T: Send>` now gates access through the generated Windows
thread-ID import. Binding is explicit. Owner references cannot escape a checked
callback; synchronous entries capture `Owner::current`, while reentrant effects
retain their original handle. Acquisition/publication also synchronizes Rust
state if Windows reuses a terminated thread's ID. This is not evidence that a
particular game hook may touch a native world: thread identity and engine lifetime
must both be established at integration. Native engine references/destructors do
not belong in a Send Rust resource merely to satisfy the container's bound.

`Owner::replace` now supports one pending replacement during an executing
callback. A newer replacement retires the previous pending value, old work is
immediately stale, and return/unwind publishes only the surviving replacement.
Destruction can cancel that pending value. This is bounded lifecycle handling,
not a general transport event queue. The peer bridge rejects recursive ordinary
operations and retires speculation, since a recursive server confirmation may
already have changed native confirmed history. It never silently drops that
confirmation while retaining conflicting hints.

Exit: each exposed operation has proven calling convention and ownership.
Unproven world operations block T6, not independent protocol work.

### T2 P2P migration

Port parser, peer state, reconciliation, rendezvous lifecycle and platform
integration in small equivalence-preserving steps. Preserve authentication,
limits, quarantine, protocol bytes and opt-in behavior. Keep temporary projection
behind one adapter until T6. Retire each C/NASM logic owner when its Rust successor
takes ownership; retain necessary generated ABI glue.

Exit: existing adversarial, crypto, real-socket and native-pool scenarios pass
against Rust. A host parser test does not prove native projection safe.

The bounded peer machine now lives in `client-runtime::peer`, with one transition
API, validated directed identities/hints, owned secret-provider keys, optional
sequence state, and at most eight owned hints. A frozen C/OpenSSL oracle compares
302,009 events on both host and i686, including internal rate/replay state and
wire bytes. The freestanding arena executes 704 additional peer traces against
that oracle through a generated entry. Its HMAC provider is emulated with Python;
it is not the Windows provider. `vostok::crypto::Key` provides native CNG HMAC with owned key
storage, source/drop erasure, fixed-access comparison, fail-closed hash ownership
and nonblocking cached provider initialization. Generated SDK bindings and the
actual freestanding adapter pass emulator failure/trace checks and real CNG
interoperability under Wine.

The production arena configuration now uses `client-runtime::peer_bank` through
the generated `pp_rust_peer` cdecl export. Rust exclusively owns twenty directed
identities, receive state, hints and outbound sequences; reverse-direction
encoding uses the same owned key. Refresh preserves quarantine and sequence
state. C exchanges copied inputs/frames/wire packets and captured epoch tokens;
it no longer owns a `pp_link` or returns a pointer to one. Its old machine is
excluded from the arena and retained only as a test reference. Lifecycle uses
the existing connect/reset/destroy hooks. No second set of lifecycle hooks is
installed. RNG, AES introduction decryption and rendezvous HMAC now also use
Rust-owned CNG operations through generated exports. One process-owned DLL
reference serves the independent capabilities; per-operation AES keys borrow
their algorithm owner, and plaintext is withheld until authentication and cleanup
succeed. The TLS mailbox, persistent master key and nonwrapping login request
sequence are now Rust-owned through `pp_rust_login`. Import does not initialize
crypto; callbacks can replace/clear the owner without publishing stale output.
Connection metadata and the credential epoch are also Rust-owned. Match reset and
reconnect preserve the credential and its sequence. Rendezvous decisions now
belong to Rust too. The generated `pp_rust_socket` entry now owns DNS results,
nonblocking socket construction, send/receive and deferred close. Connection
phase and opaque client/core identity now belong to Rust through
`pp_rust_connection`. Typed Rust APIs now sequence connect/reset/destroy and
provide projection capture/current/status/frame/confirmation queries. Native
history/pool parsing and engine effects now live in `vostok::native_projection`;
the safe `native_projection` handler orchestrates projection. Its scoped collection
owns cleanup on exit, using the sole `client-runtime::projection` journal. C keeps
ABI forwards only; it has no native field reads, projection loop or state block.
The remaining lifecycle forward orders Rust invalidation before Rust match
lifecycle. Connect/disconnect now call Rust restore directly and preserve the
pre-restore connection epoch across callbacks.

Reset/destruction retires all projection phases; captured old completions cannot
touch a retired world or overwrite a replacement. Peer-only cancellation drains
installed nodes in the live world. Native preconditions still prohibit eviction,
rounding and existing-input replacement for speculative inserts; ordinary native
input handling preserves all flags, precise times and result codes. The confirmed
wrapper and before-send restore also execute in Rust. Generated fields expose
the previously tested layout; they do not confer lifetime permission by themselves.
Restore-created replacement connections survive old disconnect completion and
an outer reconnect attempt. Packet and projection drivers now retain copied
scope tokens through typed APIs instead of private byte-buffer calls.
The retired C crypto implementation is a test reference; only compatibility
forwarding functions remain in the C crypto bridge. Test key providers
never enter the production build. Native-pool and provider-reentry checks now
exercise the production Rust bridge; exact evidence and remaining acceptance
gates are in the latest work-log entry. This configuration has not been played
or released.

`client-runtime::rendezvous::Round<LoginKey>` is now installed behind the
existing generated `pp_rust_peer` entry. It owns the sole peer Bank, endpoint
expiry/routing, hello/challenge, control replay and receive budgets, and admitted
publication/probe batches. The C control machine and direct key-injection ABI
are retired. A Preparing state has no Bank; Ready replaces the match scope with
a complete Round and returns a new epoch, including during an active callback.

The independent Login owner survives match reset. Control operations take a
scoped credential loan and discard results after replacement; failed credential
epoch checks retire the round too. Per-peer send tickets carry the captured
match epoch through native player query/release callbacks. Encode takes the
admitted ticket, not another caller-supplied input. Rust checks the captured
scope before sending or using native eligibility. The production native fixture
introduces peers through TLS selection, Ready and the control/decryption path.

Frozen C/OpenSSL comparisons still establish pure behavior on host and i686;
the independent arena probe is retained. Installed-boundary and real UDP/CNG
callback evidence is recorded separately in the newest work-log entry. This is
an installed code cutover, not full-game acceptance or release. Actual mixed-client gameplay and the broader native world-lifetime gates remain
open.

`client-runtime::connection` now owns Off/Opening/Connecting/Binding/Resetting/Running
through one total transition function. The native coordinator stores only opaque
client/world identities and copied metadata. It retires peer/socket resources
after releasing the connection mutation loan; reset retains login/socket while
replacing the round. The C runtime adapter contains only ABI forwards. Installed native tests cover
replacement during CNG and unmapped identity values; Wine covers the Rust engine
boundary with substituted engine effects and actual UDP/CNG. Ready construction now
uses `vostok::peer_connection::NativeReady` through generated `pp_rust_ready`:
native setup is copied before provider calls, and Binding publishes the world
identity so reset/destruction during RNG cancels construction. Wine makes the
old world inaccessible inside that callback and verifies no later read or
Running publication. Rust now also owns pump generation/resync/frozen reads,
publication input capture/admission and player eligibility. A scoped strong
reference releases the queried player before checking for epoch replacement.
Wine drives the actual Rust engine boundary with query/final-release replacement
and inaccessible publication input during clock callbacks. Packet orchestration
now runs in the safe `peer_connection` handler through typed effect/socket APIs,
retaining captured epochs across every call. Wine verifies the 32-datagram work
limit and cancellation during send. The projection driver now also executes in Rust against actual native input/
history pools in the emulator. These tests do not establish arbitrary recursive
native world safety or actual-game acceptance.

Socket epochs are independent of round epochs. Construction publishes an Opening
scope before DNS/provider calls, so teardown can cancel it; Ready alone owns the
socket. Reset retains the socket and copied server configuration. Destruction
invalidates immediately and closes after the active OS call returns. DNS lists
and partial sockets have scoped destructors, and stale endpoint/datagram outputs
are discarded. No OS socket handle crosses the generated boundary. Process
Winsock startup and the system DLL reference retain their previous process-long
lifetime. The shared initialization cache refuses recursion without waiting.
`Handle::is_current` distinguishes an authorized pending attachment from a
revoked lease still waiting for cleanup. The new native/real-UDP lifecycle tests
and exact acceptance limits are recorded in the work log.

### T3 Shared protocol core

Extract framing, ACKs, message metadata and time/input codecs. Supply the
server-to-client decoding a real client needs. Preserve existing server APIs
through adapters and keep the independent client models as test oracles.

ACK/framing now live in `net-protocol`; `udp-transport` retains allocating
compatibility wrappers. Parsing writes private descriptors into a caller-owned
`MessageBuffer` and exposes a borrowed view only after the whole datagram passes.
Do not replace this with an iterator that lets callers apply early messages or
ACKs before a malformed later message is found. The buffer is 4,032 bytes on the
tested i686 build (6,048 on x86-64); place it in the native runtime's explicitly
owned storage at integration. Cargo i686 tests do not prove arena linkage. The
standalone arena probe now proves exercised ACK/framing/time/input paths compile,
link and execute through generated entries with the real arena toolchain; no
native connection or game-world operation is installed by that probe.

Time/input codecs now provide validated offsets, distinct game time/tick types,
byte-preserving values and borrowed E9–F5 event decoding. Time resolution exposes
checked and wrapping policies explicitly; native unsigned wrap must not silently
inherit the server's rejection policy. Server compatibility APIs consume the
shared client-input decoder and retain their existing public shapes. Client
timeline events require an initialized, validated roster of 1–20 players.

Exit: server regressions pass; old/new bytes and rejection behavior agree for
supported traffic. Tests still include an independent reading of the format.

### T4 Match transport

Implement client sequencing, reliability, deduplication, multipart assembly,
pacing, timeout and disconnect. Cut over at the match transport interface, first
delivering messages to native match handlers. Rust exclusively owns this
connection's socket and queues; native transport is bypassed.

Exit: real clients complete matches with the existing server; loss, duplication,
reorder, sequence wrap, resource exhaustion and teardown checks pass.

### T5 Match synchronization

Replace port negotiation, static-info delivery, loading/ready, time correction,
generations, snapshots, roster events, relays/corrections, hash reports, uploads,
reconnect and exit. World loading and serialization remain explicit native effects.

Exit: Rust makes every match protocol decision. Mixed baseline/replacement clients
agree on confirmed state through late join, resync and reconnect. No native match
machine also processes the same messages.

### T6 Timeline and rollback

Investigation evidence (2026-11-03, not a cutover gate): PvP world comparisons
isolate repeated bullet-only disagreements; a native fixture reproduces weapon
identity loss when an in-flight bullet is restored after its owner's slot changes.
The captured incident's cause is not established. See snapshot_rules.md §8 and
the work log; passing transport controls does not establish native determinism.

Transfer local/confirmed histories, received-input bookkeeping, profile inputs,
membership events, dirty ranges, retention, rollback scheduling and prediction.
Peer hints become a separate speculative branch under the same timeline owner.
Use opaque native world-state handles and the proven stepping adapter.

Before cutover, establish every native read of input/history structures, event
ordering and externally visible side effect. Prove stepping does not re-enter
native scheduling. The adapter explicitly captures/restores state, advances a
simulation interval, obtains canonical state/hash, presents and releases worlds.
Handles carry an epoch and cannot survive reset/destruction.

Exit: Rust decides what to restore/replay. Native scheduling and temporary peer
node insertion are bypassed. Confirmed hashes, uploads, replays and irreversible
effects agree. Calling native `game_world_core::move` for rollback decisions is
not completion. If the seam cannot be isolated, record the dependency and revise
the boundary contract before proceeding; do not silently widen the implementation.

### T7 Acceptance and retirement

Audit all remaining native dependencies. Remove retired implementations and
migration hooks after their gates pass. Keep behavior fixtures; remove port-only
comparison machinery when its migration work is finished.

Exit: Rust owns networking and timeline coordination; remaining simulation
responsibilities are individually documented behind the adapter.

### T8 Login and lobby

Reuse transport/runtime infrastructure for lobby ordering and lifetime. Replace
login framing/lifecycle, then lobby/chat message families and typed state. Keep
TLS/cryptographic providers behind an adapter initially. Separate UI application
from parsing, preserving the native safe push order.

Exit: Rust owns login/lobby/match connection and protocol state. Every retired
handler has a replacement and compatibility evidence.

## Cutover and verification rules

- Baseline, shadow and replacement modes are selected before construction.
  Support documented stage combinations only. Shadow computes effects on copied
  events without sending or mutating the live world; compare simulation in
  isolated worlds or separate runs.
- One owner per socket, queue, input list and mutable history. Never switch owners
  halfway through a connection or partially mutated world. Roll back with a new
  connection/process or previous tested artifact, not a partial state copy.
- Server confirmation outranks peer hints. P2P failure disables prediction without
  changing confirmed state. Preserve defined behavior, not native memory corruption;
  isolate intentional safety/gameplay fixes with their own tests and compatibility
  treatment.

Fixtures must cover both transport directions, ordered lobby messages, multipart
limits, malformed lengths, wrapping counters, exhausted queues, stalled receivers;
partial login replies, credential expiry/cancellation; ready/first snapshot,
duplicate/older generations, resync with continuing inputs, late join and slot reuse;
grid/precise/profile inputs, local merging, move/discard, membership and finality;
peer contradictions, stale epochs, bad MACs, crypto failure and unavailable routes;
long frames, focus, same-timestamp reset, reentrant destruction and repeat entry;
death/respawn, inventory/weapons, score/completion and sound/UI effects that must
not happen twice during replay.

Compare exact contractual bytes, message semantics/timing where grouping can
legitimately differ, and exact canonical simulation state/hashes (not float
tolerances). Native checks execute actual routines and verify register/FPU state
and lifetimes. Models alone cannot prove native contracts.

Use the isolated local-client launcher for baseline/baseline,
baseline/replacement and replacement/replacement, P2P on/off. Before default
cutover require completed real matches through join/play/resync/exit, no unexplained
confirmed-state/replay differences, bounded memory/queues across lifecycle cycles,
and recorded frame cost, rollback depth, resync rate and input age against the same
baseline scenario. Validate Windows and Wine platform boundaries. Real NAT testing
is required before considering P2P enabled by default.

Iterate with focused tests/native checks and the port equivalence gate. Before
pushing run all selftest variants, every Unicorn check, workspace clippy and
affected tests, including the client Rust workspace. Build content only if changed.
Refresh generated bindings/hooks and the arena lock as inputs change. Feature work
never reserves revisions. Intentional match-computation changes add a `next` row
to simulation_changes.tsv. Release maintenance/restart/swap still requires the
owner's go at that moment through the existing release workflow.

## Session protocol and subsequent engine replacement

At session start read this plan, the latest handoff and Git status. Pick the first
unblocked item and confirm its baseline. Work on one ownership transfer at a time.
At session end record files, exact commands/results, fixtures/hashes, facts learned,
rejected approaches, blockers and the next concrete action. Architecture decisions
belong here; chronology belongs in the work log. Keep large captures outside Git
with retrieval paths/hashes. Never claim played/released from synthetic tests.

After T7, extend this process to action interpretation and deterministic gameplay;
world serialization/history representation; movement/collision/physics, weapons,
inventory, rules and world objects; animation/presentation; resources, rendering,
audio, UI/platform integration and finally the native executable shell. Before
each expansion add a package naming its owner, replaceable interface, native
dependencies and equivalence gate. Those later contracts are not yet investigated.

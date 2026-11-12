# Optional direct peer input prediction (0.26g)

Implemented as an **experimental, opt-in** client path: start a newly patched
client with `-peer_prediction`. Both peers need this option and the updated
login/match servers. No release revision is reserved by this change.

The server introduces currently live match members. Each client then sends its
own fixed-tick input directly to introduced peers over a separate, nonblocking
UDP socket. It still sends the ordinary input to the match server. The direct
path can arrive earlier when the peer route is shorter than the route through
the server. It does not lower server confirmation time. NAT/firewall failures,
missing capabilities and provider errors leave the ordinary match path in use;
there is no prediction relay. Introductions necessarily disclose the peers'
observed public IP addresses and UDP ports to each other.

## Verification and limits

- Login version-field byte 9 advertises protocol 1. Only that capability gets
  `SPK1` and a random 32-byte secret after the normal 11-byte TLS connection
  reply. The client captures credentials only from a complete 47-byte TLS
  reply. A partial/legacy reply disables the optional channel. The stock TLS
  path verifies against the installed login certificate (`login_chat.md` §3).
  The public UDP session ID is never a secret.
- A rendezvous request must authenticate with that secret and name a currently
  live member of the stated match. Its source IP must match that member's game
  connection. A stateless challenge additionally proves receipt at the UDP
  source port. The cookie uses a server-only random secret; knowing one's own
  login secret cannot manufacture a proof for another endpoint.
- Introductions use AES-256-GCM. The encryption key is
  `HMAC-SHA256(login_secret, "peer-control-aead-v1")`, separate from the request
  MAC key. A session owns the monotonically increasing control nonce and
  accepted request counter across cache expiry and match changes. Neither can
  wrap. Restored sessions have no prediction credential until another TLS login.
- Pair keys and epochs bind the match, both sessions, roster places, sync
  generations, random client instances, and observed endpoints. The client
  accepts introductions only from its match server, for its current instance,
  with a newer authenticated control sequence. Resync/reconnect rotates the
  instance and clears peer state.
- A direct packet must have exactly 76 bytes, the supported version/type,
  expected endpoint, sender, recipient and epoch, and a valid HMAC-SHA256 over
  every header/payload byte. Replay/out-of-order sequence numbers, unsupported
  action bits, nonfinite/excessive rotations, stale inputs and inputs too far
  ahead are rejected. Authentication failure never advances a replay counter.
- Inputs are at most 150 ms old or 100 ms ahead of the receiving game clock,
  and newer than the known confirmed input floor. Each peer has at most eight
  pending events and a 40/s token bucket with an eight-packet burst. A frame
  receives at most 32 datagrams and checks at most two server control packets;
  control cryptography also has a 64/s, 32-packet burst limit. Server rendezvous
  work/output is bounded separately. Authenticated probes open/keep NAT mappings
  without becoming inputs.
- A contradictory same-timestamp hint or disagreement with a confirmed server
  input quarantines that peer for the epoch. Repeated introductions cannot
  reset its counters or lift quarantine. No packet can change health, inventory,
  a confirmed input, finality, or the ordinary outbound server input directly.

Authentication proves who supplied an input, not that the player is honest.
An authenticated player can lie about its own prediction. Hints remain bounded
speculation and the confirmed server path replaces them. This is not an
anti-cheat proof of movement or a replacement for server verification.

Memory safety is a separate requirement. A valid MAC does not make an input
safe for the native engine. The review below follows the values into native
indexing, arithmetic, allocation and object lifetime. It found and corrected
several gaps, including an accepted action that reaches a native out-of-bounds
inventory read. Checks execute those specific contracts; they are not a proof
that every engine subsystem is memory-safe.

## Memory-safety review of the direct path

Evidence uses the exact executable in `tools/re-kit/v026g/identity.json`
(SHA256 `ac33d9937015c2299bf62cc6b127cb308b48a3475b30fff2ddd65479ae3bf153`).
Addresses below are preferred-image VAs. Corresponding function/ABI findings
are recorded in the re-kit TSVs.

| Untrusted value or boundary | Established constraint and native consequence |
|---|---|
| Datagram length | The separate UDP socket receives into 128 bytes; the direct parser requires exactly 76 before any field access. Byte decoding produces scalars, not a native struct cast. Peer packets never enter the engine's packet reader. |
| Player identity | Endpoint and authenticated introduction select the link. Sender/recipient bytes must equal its identity. Native indexing uses the adapter's loop index, not an unchecked packet index. Both inplace vectors must have the expected inline bases, capacity ends, stride-aligned constructed ends, and equal nonzero counts at most 20. Capacity alone was insufficient. Constructors/resizers: `2D41B0`, `2D4330`, `2D4050`, `2D41D0`. |
| Timestamp | Wire age/future limits are followed by stricter native conditions: 50 ms grid, `first < T <= last <= now <= maximum`, `now-T <= 150`, `T > resync`, `maximum-now <= 100`, horizon `H >= 500`, and `maximum-T < H-50`. Origin and retained history endpoints share the grid, with `first >= origin`; the oldest membership-history entry must cover `first`. This supplies a predecessor for insertion, replay and removal. |
| Native timestamp arithmetic | Server mode is excluded (it substitutes a zero effective horizon). `H <= (UINT_MAX-maximum)/5` makes both `maximum+5*H` and `T+H` nonwrapping. The age and future branches at `1FAD43`/`1FAFAD` therefore cannot rewrite a hint's time. This matters because native results **2 and 3 insert a rewritten input**; they do not mean failure without mutation. Two constructed engine states exercise those exact branches. They are contract counterexamples, not claims that peers control the horizon or server mode. |
| Rotation | Both half-float exponents must be below 25: finite magnitudes below 1024. The native path converts them with `F66D0`, applies yaw and held-item pitch through `2D8AD0`. Existing timestamps are skipped, avoiding native half-float addition during merging, which can overflow even when both source halves are finite. |
| Actions | Allowed mask is `0x9bfff3ff`, including the separately passed native high-bit flag. Bit 26 is rejected on both send and receive, even with a valid MAC. See the concrete native defect below. Other bits remain native actions; this is not a proof of all physics, item and rule callbacks. |
| Temporary storage | Parser queues contain at most eight scalar inputs per peer. The insertion record has `20*8` entries and is filled only by the corresponding bounded loops. No peer controls a pointer, copy length, object layout or allocation size. |
| Fixed pools | `1FCB60` and `2D3D70` may evict when full. The adapter checks both used/capacity and nonempty free lists before each insertion; the game thread owns the operation. Existing input and received-time nodes are skipped. |
| Removal | `27D820` requires the exact input and preceding history. Its helper `2D1340` also requires the matching event: a missing event reaches a null-based write at `2D137D`. A same-time projection uses the already-retained history, and removal happens immediately afterwards, in reverse insertion order. An unexpected status traps; a trap is a failed invariant and possible denial of service, not successful hostile-input verification. |
| Hook ABI | Inline callbacks preserve GPRs, flags, x87 and SSE state in an aligned `FXSAVE` frame. The C build uses SSE2 and Windows calls may clobber volatile floating-point state. Saving GPRs alone did not preserve the interrupted native instruction stream. The contract check deliberately clobbers XMM0, MXCSR and the x87 control word in callback substitutes. |
| Core lifetime | Reset `2D0550` and destructor `2D02A0` now notify the adapter before native state is released. Reset discards the projection and rotates peer state even if generation/resync values remain numerically equal. Destruction discards projection, closes the channel and clears ownership before later client teardown. Tests make the old core inaccessible before subsequent cleanup. |

**Concrete action defect.** `process_quick_slots` (`2D8BB0`) compares the held
item's slot with table `927DF0 = {7,10}` and uses `0xff` for no match. With
bit 26 set, `2D8D1C` reads `[927DEC + 4*0xff] = 47`. The subsequent inventory
read is at `inventory+1C8`, beyond its `188`-byte allocation; a nonzero value
can then be used for a virtual call. The exact instruction sequence is exercised
for held slots 7, 10 and 13, producing inventory slots 10, 7 and 47 respectively.
The new parser test signs the rejected bit-26 packet with the correct key.
This closes that direct-hint route. The pre-existing ordinary game-input route
still has this native defect; this feature does not fix it globally. Background
analysis and the allocation evidence are in `match.md` section 20.

**Scope of the argument.** Engine-owned pointers and linked-list structure must
already satisfy the native engine's invariants. Checks of counts and spare
capacity do not validate arbitrary corrupted pointers. This review establishes
the parser boundaries and the adapter contracts listed above, and removes the
identified hazards. It does not establish that all action-triggered item,
physics, death/respawn and rule callbacks remain memory-safe under speculative
replay, or prove the absence of reentrant lifecycle events during that replay.
Those are remaining native-code review obligations, not something a MAC or
fuzz-test pass can discharge. The feature remains experimental and off by default.

The repository already has Rust client patches (`rust/patches`, documented in
`rust_patches.md`): safe patch modules, typed `vostok` wrappers, and generated
unsafe `raw` bindings. The production arena configuration now gives Rust sole
ownership of peer keys, replay/rate state, hints, quarantine and outbound
sequences. Rendezvous, socket ownership and connection phases are also Rust-owned
through generated entries. `client-runtime::connection` is the sole phase
machine; `vostok::peer_connection` coordinates independent peer/socket retirement
after releasing the mutation loan. Its client/core identities are opaque values,
never engine references. The C runtime adapter now holds only local snapshots
for connection sequencing and projection bridge calls. Its packet loops have
moved to the safe Rust handler through typed socket/round APIs. Rust also performs
pump field reads, player query/release and publication input capture; the separate C
projection adapter still owns temporary projection. They exchange bounded copied values and epoch tokens,
with no peer-state pointer or OS socket handle.
Ready construction also runs in Rust through `pp_rust_ready`. Its scoped native
field reads precede RNG/provider calls; a Binding phase publishes the world
identity before those calls. Reset/destruction while Binding cancels the partial
connection, including its socket, and stale construction cannot publish Running.
The remaining C `pp_ready` is a forwarding function. The ordinary Running reset
continues to retain the socket. Wine tests make the old world inaccessible during
the RNG callback and exercise both cancellation and RNG failure.
`vostok::peer_connection::effects` also admits pump and publication work only for
the current thread/world. Its `PlayerLoan` releases the owning native query
reference before checking whether query or final release replaced the round.
Publication copies its native input before the clock/provider can reenter. Wine
tests protect that input with `PAGE_NOACCESS` during the clock callback and
verify the copied packet; teardown during the clock leaves admission output
untouched. The safe Rust driver retains one captured connection scope through
hello/probe sending, at most 32 receives, control replies and publication sends.
Callbacks cannot redirect old work to a new round/socket. Wine verifies the
receive bound and destruction during send; C pump/publish are forwarding calls.
The old C peer machine remains a test reference and is excluded from the arena.
RNG, AES-GCM introductions and HMAC now go through generated Rust crypto entries;
the old C crypto implementation is also excluded from the arena. Socket reset
and destruction use an independent Rust epoch; DNS/partial sockets are scoped
resources, and teardown defers close until an active OS call returns. The TLS
mailbox, master key and login request
sequence are now Rust-owned through `pp_rust_login`; C carries only public
session/epoch metadata. Import initializes no crypto, and stale crypto results
are withheld after a callback replaces the login. Follow
`../PLAN-RUST-CLIENT.md` and `../RUST-CLIENT-WORK.md` for migration gates and exact
test evidence. This source cutover is not a release or proof of the native
world-lifetime obligations above.

## Native adapter and ownership

The original client uses deterministic lockstep/rollback, not a separate
remote-entity position predictor. Hints therefore carry exact input events,
not peer-supplied world positions. `0x27EB3E` copies the selected input just
before native serialization, after local input merging. Precise-time events
are left to the ordinary path. Duplicate/resend events are not republished.

Before a rendering move, the adapter restores the previous confirmed view,
runs the ordinary move and polls the socket. It then inserts only hints on the
50 ms grid, with a retained history state strictly before their time. Each is
at most 150 ms old, while the native history horizon must be at least 500 ms.
Existing native inputs/received timestamps are never overwritten. Both fixed
pools must have spare capacity, since native allocation can evict old nodes.

The insertion calls native `change_input` (`0x1FAB50`) in its network mode, but
the scoped hook at `0x1FAB94` skips the received-timestamp write only for the
exact adapter-owned core/player/time/thread. The subsequent rendering move
uses the temporary nodes. Native `discard_input` (`0x27D820`) removes them
before returning and marks the preceding history dirty. No hint nodes remain
in the ordinary input lists between frames.

Before an ordinary core move, confirmed input handling, network send pass or
disconnect, the adapter rebuilds at the previous projection time with the
confirmed lists. This happens before a long frame advances the clock; it does
not carry speculative dirty history past the rollback horizon. A native reset explicitly invalidates the old projection before clearing
history; a newly loaded resync world also supersedes it. Input pool/event removal violating
the checked native contract traps instead of allowing an unowned speculative
node to remain in the confirmed input path.

The temporary projection phase and its bounded 160-input removal journal belong
to `client-runtime::projection`, behind a thread-gated world-lifetime epoch.
`vostok::native_projection` parses the native history/vector/pool layout through
generated fields and owns scoped engine effects. The safe `native_projection`
handler runs the loop. Its scoped collection drains the reverse-removal journal
on early return while the world remains live; reset/destruction instead retires
all phases before native cleanup. The C projection file contains ABI forwards.
Rust also restores before native confirmed input/send and connection retirement.
Old effect completion cannot clear a replacement at the same core address or
connection epoch. The C global, state block and disconnect bridge are retired.
The work log records exact fixture evidence and remaining real-game gates.

The logic thread owns sockets, links and native projection state. The TLS
callback has only a small synchronized credential mailbox. Native player
lookups return owning intrusive references; the adapter releases each one
before returning its own peer state. Disconnect closes the socket and wipes
the match's keys. Core destruction independently clears ownership, so client
teardown does not depend on the core still being alive. The existing late-join destructor hook remains first; the
prediction cleanup follows its prologue.

## Wire format

All integers are little-endian except IPv4 octets and UDP ports (network order).
All listed lengths are exact; packets do not share the ordinary transport's
framing. Their fourth byte is not a valid native match message ID, so the
reserved prefixes cannot consume a valid ordinary match datagram.

| Packet | Layout |
|---|---|
| Hello/proof, 88 B | `SPH1`, session u32, match u32, instance[16], request sequence u64, bucket u32, cookie[16], HMAC[32] over bytes 0..56 |
| Challenge, 88 B | Same layout with `SPQ1`; server-issued bucket/cookie and HMAC |
| Introduction, 112 B | AAD: `SPC1`, recipient session u32, control sequence u64, recipient instance[16]; ciphertext[64]; GCM tag[16] |
| Introduction plaintext | match u32, recipient u8, sender u8, zero[2], IPv4[4], port u16, zero[2], epoch[16], pair key[32] |
| Peer, 76 B | `SPP\x01`, type u8, sender u8, recipient u8, zero u8, epoch[16], sequence u64, time u32, packed half rotations u32, actions u32, HMAC[32] over bytes 0..44 |

Introduction nonce: `SPC1` followed by the control sequence's eight bytes.
Peer type 1 is an input; type 2 is a probe with all twelve payload bytes zero.
Probes share the same replay counter and authentication as inputs. Windows
uses CNG from the absolute system DLL path; the server uses OpenSSL.

## Checks and remaining validation

- `test_peer_prediction_026g.c`: OpenSSL-backed packet verification,
  truncation/bit flips, replay, bounds, ordering, reconciliation, expiry and
  flooding; exact-size receive allocations and 65,536 reproducible adversarial
  samples, including authenticated malformed payloads, run under
  AddressSanitizer/UBSan. This covers the portable parser, not the whole engine.
- `check_peer_prediction_026g.py <stock exe>`: actual native input pools,
  timestamp hook and discard/dirty-history bookkeeping with the compiled
  adapter; server correction at the same time, full pools, missing history,
  long frames, same-timestamp resets, destruction, native timestamp rewrite
  results, constructed-vector bounds, and the unsafe weapon-slot branch.
  Physics/world stepping is a controlled boundary.
- `test_peer_prediction_crypto_026g.c`: Windows/Wine CNG against an OpenSSL
  AEAD vector, all 896 single-bit packet mutations, RFC HMAC and RNG smoke tests.
  The `windows` lane runs it against both legacy C and the production Rust arena.
  `check_control_crypto_native.py` adds production-export failure/cleanup checks,
  Windows struct/stack ABI, key snapshots, retry policy and loader reentry.
- `test_peer_prediction_runtime_026g.c`: Windows/Wine real UDP sockets,
  challenge/intro exchange with a local server fixture, direct inputs,
  wrong endpoints, quarantine, resync, teardown and reference ownership.
- Session, login-server and match-server tests cover credential issuance,
  legacy negotiation, endpoint proof, encrypted pair introduction, replay and
  bounded rendezvous work. The arena/engine calling-convention checks apply.

A full two-client match test is still required before enabling this by default:
compare confirmed hashes/replays/snapshot uploads, exercise death/respawn and
match exit, and measure frame cost and direct-path benefit under loss, delay,
clock correction and real NATs. Synthetic adapter and socket tests do not prove
those whole-game properties. No live server or player download is changed by
this implementation work.

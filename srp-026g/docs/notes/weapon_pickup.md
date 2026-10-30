# Dropped weapon pickup — implementation evidence

Work in progress; no gameplay hook is installed yet.

## Implemented shared logic

`crates/weapon-pickup` is allocation-free Rust, independent of native pointers.
It implements the approved held-slot/skill policy, captured loaded ammo
(including chamber and selected ammo ordinal), and the one-magazine reserve
grant. Its ownership state distinguishes absent, alive and dead places,
keys drops by donor life, consumes each drop once, and keeps borrowed weapon
provenance independent of donor respawn. A recipient's new life clears its
borrowed mapping; a borrowed weapon dropped on death retains the original
template with the newly captured ammo count. Unclaimed drops expire when
the native corpse's owner respawns or leaves.

The `WPK2` codec encodes this state canonically (at most 996 bytes), rejects
truncation, bad values, trailing data and duplicate ownership, and decodes
into a complete temporary state. Tests replay a pickup from restored shared
state and compare canonical bytes. This is **not yet connected** to native
world serialization, native rollback, network inputs or native inventory
instances, so those integration requirements remain open below. No native
geometry, allocation, crate behavior or full-game sync is claimed by these
unit tests.

`History` now couples that ownership with exact private reserve counts and
the displaced original's simulation state for every borrowed slot. Capture
visits owners in canonical place/slot order and fails as a whole if any
native owner cannot supply its state. The `WPA2` payload contains a
length-prefixed WPK2 mapping followed by each slot's compatible-ammo counts
and length-prefixed original state, at most 3434 bytes. It rejects missing
state, invalid count/record lengths, truncation and originals whose place,
kit or slot differ from the recipient's ownership mapping. The unshipped
WPA1 format is rejected. Its 23 original shared-crate tests and focused clippy pass.
This is a component of the eventual history extension, not the full snapshot:
self-contained source templates still need representation, and no native
history record currently stores this payload. The coordinated native slot
transaction now applies both reserves and displaced-original state.

Template identity includes the original donor life (place + native spawn
time), kit and slot. Reusing a roster place with the same kit/slot therefore
cannot silently substitute the new occupant's equipment. Re-dropping a
borrowed weapon changes its drop ID but retains this original source life.
Regression tests cover leave/rejoin, carrying and passing on the old weapon
alongside a new drop from the reused place, and snapshot round trips containing
both. The native bank must retain the profile captured for that source life;
it cannot resolve a template against the place's current profile. `WPK1` was
an unshipped intermediate format and is rejected, not guessed or upgraded.

`client-patches/rust/patches/src/vostok/weapon.rs` now supplies a typed
`Player::pickup_inventory()` read: it checks concrete item vtables, converts
the item subobject to the active-object base, reads handedness from the native
weapon, resolves the held ammo binding, and evaluates modifiers 25/64 for the
current kit. It returns values, never item pointers. Three i686 adapter tests
cover the pointer bases, kit selection, skill restrictions and invalid/stale
bindings. Original ammunition resolves through the eight fixed ammo slots.
Borrowed ammunition must resolve through the matching resource owner supplied
by the eventual match bank, which verifies every external ammo cell and
recipient binding. A separate test captures a borrowed weapon after an ammo
type change and after death clears its user; a stale resource owner and an
unbound held weapon on a live player are refused. No pickup hook calls it yet.

`vostok/resources.rs` supplies the native inventory query boundary. Its
recipe borrows the hook's lifetime and copies only initialized native
fields, including the two byte-sized fields without their padding. A
successful result retains the inventory before the native query releases
its reference; `OwnedInventory` releases that reference exactly once.
Six i686 resource tests cover recipe parsing, result ownership, failure/wrong
resource types, independent profile lifetime, allocation failure and missing
or reusable native cooks. The callback goes through a generated aligned NASM entry
(`callbacks_026g.tsv`), with its logic in Rust. No native player is changed
by this adapter, and no gameplay hook calls it yet.
Resource dispatch remains an explicitly unsafe Vostok boundary until a
concrete native preparation point establishes reentrancy and context
lifetimes. Safe patch code cannot call it on an arbitrary `Thread` token.

The native recipe builder is20284C..2028A9, inside the player cook's config
completion callback202500 (bound by202400, class54). Its27Ch stage contains
24-byte creation data copied from the original query variant atstage+9C by
2058A0. The recipe copies profile fromstage+9C, byte kit from+B0, dictionary
from+1E8, physics_world from+A8, game_world from+A4, and
byte flag from+B1. Stage+A4 is also cached at+1E4 by2025E0. The final u32 at
recipe+14 is the **match seed**, not a context pointer: the builder obtains
match options through world+9C -> game+53AC -> network-client vtable+34,
then reads options+577B0. This is the same scalar seed documented in
static_info.md. Generated fields now name game_world and match_seed explicitly.

The physics-world origin is now identified too: map loading allocates a60h
`bullet_physics_world` at2CC2D2, constructs it at2CC2DE using74950 (vtable
9E2EBC), and passes that pointer through the player creation data to stage+A8
and recipe+0C. The inventory helper2C6F90 reads it at2C7102 in its
data.type2/class49 armor branch. The creation stage itself does not retain
ownership of that pointer; a preparation context must borrow the live match's
world. Player+350 is already exposed as the live physics world. Establishing
the corresponding game_world/dictionary/core lifetime at a concrete dispatch
point is still required; do not retain an old stage merely because it once
contained those addresses.

The match owner is now traced and exposed through `GameWorld::pickup_resources`.
The map cook's bound callback puts physics at callback entry ESP+28 (EBP+2C),
not its scene local at EBP-18. Original constructor argument assembly passes it
as pvp_match_core argument8;2D567B stores it at wrapper+130. Options are argument4,
stored at+124. Wrapper+19238 is a scene pointer, **not** the physics world.
The resource cook's destroy path2CBBE0 invokes the wrapper destructor first,
then finalizes/destroys/frees physics at+130. Core/player teardown therefore
precedes physics destruction.

`MatchResources` checks the concrete wrapper, borrows its current options,
physics, world and game dictionary, and retains the wrapper resource until
the scoped guard drops. It copies the scalar seed and uses the original map
cook's zero recipe flag. Preparation resolves a `Template` only in this core's
profile bank; checkout keeps registry locks and borrows out of resource
callbacks and restores the retained profile on return/unwind. No saved loading
stage or current donor profile supplies these inputs.

`check_pickup_context.py` executes the original bound callback builder/invoker,
match-constructor argument assembly, physics/options field writes, and cook
destruction order. Scene/physics destructor endpoints are fixtures; it does not
load a real map. The i686 context test checks recipe construction, retained
reference balance, current seed/zero seed, missing bank/context refusal and
concrete wrapper rejection. Dispatch is still explicitly unsafe: a verified
preparation phase must establish that callbacks cannot reenter simulation or
invalidate the outer game/world/dictionary. No gameplay hook calls it yet.

`check_pickup_resource_query.py` executes that original builder for all three
kits and both flag values, checks the exact pointer chain and seven output
fields, and verifies that padding beside the byte fields is untouched. Its
match-options getter is a fixture endpoint. This identifies the recipe inputs;
it does not prove that the native creation stage or its pointers may be saved
after the query completes. Native instance preparation is still pending.

`check_pickup_resource_query.py` runs the original E5C20 wrapper, native
boost-function copies, completion proxy and invoker. Submission and I/O
dispatch are fixtures. The actual generated callback entry is linked with
a recording stand-in for its Rust handler. All four native stack
alignments preserve callee-saved registers, give Rust its required stack
alignment, invoke once and return synchronously for null/non-null results.
This proves the dispatch envelope, **not** actual asset cooking, independent
weapon instances, or reentrancy safety at a simulation hook. The arena
build and focused native clippy pass with this boundary included.
The hook generator's six regression tests and the native engine library's
62-function ABI suite pass too, including profile-body copy/destruction,
cook lookup and the installation callbacks. These are focused checks; the full release
gate and real gameplay validation have not run for this feature.

### Independent original-equipment templates

`ProfileTemplate` owns a stable native allocation of a constructed
`player_profile`, with its equipment body copied from the source.
`InventoryRecipe::snapshot_profile` captures it while the
donor's profile is live; `cook_from_template` substitutes this owned profile
and a typed kit into a currently live recipe context. The resulting recipe
borrows the template until the synchronous query completes. Teardown calls
the original profile destructor before freeing through the same allocator.
The class is neither Clone nor Send/Sync, and exposes no raw pointer.

Late join's `lj_context` contains only a 45FCh profile body, so the full
client-profile assignment at 284D10 would read beyond that allocation.
The adapter uses `copy_body` (284DC0), which copies the equipment data
without reading the client-only tail. `check_pickup_profile.py` checks
every native source read stays within that body and demonstrates that the
full assignment fails this boundary check as a negative control.

The fixture executes the original body/full constructors (2F7C50/2F73F0),
body copy (284DC0), item-buffer copies (270390/14AF20) and destructor
(26F3B0). It fills all 69 item descriptions to their two buffer capacities,
copies all three kits, destroys/overwrites the donor and checks every copied
buffer points inside the new profile. The copied data and allocation guards
survive. The copy and destructor execute only their bounded native bodies,
with no hidden allocator/reference-count calls. No memcpy of a self-relative
profile is used in Rust.

The match owner now contains a `ProfileBank`, indexed by exact source `Life`
(place and spawn time), scoped to its native core. Installed death-entry hooks
at1F28E0 (client player) and2D77C0 (base player) capture the original profile
before either path calls remove_alive2D99C0 or inventory on_death2FCA50.
Both entries supply ECX=player; capture allocates/copies only and does not
dispatch resource queries or change native inventory. Invalid/uninserted lives
are ignored. Repeating a death for the same life retains the first snapshot;
respawn or place reuse creates a separate entry. A failed capture is logged,
and missing templates cannot resolve to a newer occupant's equipment.

`remember_profile` constructs the profile and its bank entry outside the
registry lock, then inserts without allocation/native callbacks. A duplicate
candidate is destroyed outside the lock. `with_profile` checks out unique
ownership, releases the lock before running preparation, and restores it with
an RAII guard on success, error or unwind. Nested access to the same source
returns Busy; other sources/cores remain available. Match teardown while a
profile is checked out is an invariant failure. Normal teardown detaches the
world, then iteratively destroys profiles and entries outside the lock.

Retention is currently until match teardown: 17,972 bytes of native profile
plus an entry per captured life. There is no eviction or garbage collection
that could invalidate a carried weapon or older rollback state. A future
collector must account for all such references. This bank is not yet included
in native history/late-join serialization, and no hook cooks or installs a
borrowed weapon from it yet.

Three new i686 ownership tests cover donor mutation/destruction, source-life
and core separation, duplicate capture, missing/invalid identities, nested
checkout, callback errors/unwind, teardown and empty core-address reuse.
The 60 Vostok tests and native clippy with warnings denied pass.

The linked `check_pickup_hooks.py` fixture executes both installed death
entries, the real Rust bank, native profile constructor/body-copy/destructor,
and teardown hooks. It checks replayed prologues/registers/flags, unchanged
donors, profile and entry allocation failures with retry and no leaks,
duplicate lives, two cores, donor mutation/destruction, self-relative copied
buffers, destructor-before-free and empty address reuse. It stops each death
entry after its prologue, before the original remove-alive/inventory callbacks;
this is capture/lifetime verification, not full death or transfer simulation.
Native heap allocation/free, thread identity and failed log-file open are
fixture endpoints. Original-profile and linked transport checks also pass.

`check_pickup_cooks.py` executes registration setup for inventory class 41h
(2C7170), weapon class 46h (30A800) and inventory-item class 48h (31F2E0),
plus original cook lookup (102CD0) and the name-cache gate (108A00).
All three registrations set reuse policy +Ch to
zero and repeated queries bypass name lookup. A policy-one negative control
enters the name path. The Rust query boundary requires all three cooks to exist
with policy zero before submitting, so it cannot silently accept a cook
configuration that would share mutable resources. This verifies cache
policy, not real asset loading or completed native weapon transfer. The
registration labels in the re-kit are descriptive labels from these bodies.
The inventory-item cook creates the mutable ammo item in its 31F7B7 branch:
fresh 2C0h allocation, inventory_item constructor, ammo_data loader, then
vtable A1C9BCh. Class 43h is shared config, not the mutable ammo item;
requiring that config cook to have policy zero would incorrectly reject
normal native resource setup.

### Preparing captured ammunition

`resources/prepared.rs` adds `InventoryRecipe::prepare_weapon`, which cooks
from an owned template and consumes the new `OwnedInventory` into a
`PreparedWeapon`. The latter keeps the weapon and its compatible ammo
stacks alive together and exposes only value reads. There is no preparation
setter that could top up an already installed weapon.

Preparation now makes a private `WeaponProfile` copy before querying. It
validates the requested kit, selected weapon descriptor and retained ammo
indices, then empties every slot except that kit's selected weapon and the
eight native ammo slots8/9/11/12/19/20/21/22. The retained bank profile is never
edited. Item records, attachment/upgrade buffers, ammo slot order and original
allowances are preserved; indices are not compacted or renumbered. Other kits,
the other weapon, clothes, consumables and artifacts do not enter the query.
The temporary profile stays owned through synchronous cooking and preparation,
then is destroyed on either success or failure.

`check_pickup_inventory_query.py` runs the original inventory translator2C7250,
its slot walks, config-type branching, variant construction and request
assembly against filtered profile fixtures for all three kits and both weapon
slots. It verifies one class46 weapon plus the occupied class48 ammo slots,
including missing ammo slots, exact attachment pointers, game_world, seed and
byte flag. A null physics context is sufficient for these synthetic valid
weapon/ammo requests. Extra-item and armor negative controls submit extra
class48/class49 work; armor forwards the physics-world pointer. Dictionary
lookup supplies synthetic configs and execution stops at resource submission:
this proves query composition, not asset loading. Rust tests independently
check the filtering and exercise `prepare_weapon` through query failure,
verifying the private copy remains live until the query returns and is released
without changing the retained source. The API still requires a live native
recipe/dispatch context; it does not yet construct one from a match hook.

Before writing, it checks the concrete weapon/ammo types, native owners and
slots, absence of a player/ammo binding, captured capacity/handedness/chamber
compatibility, unique ammo cells and the original kit's slot descriptions.
It then sets the captured magazine/chamber/type, disables load-on-initialize,
zeros all private ammo reserves (including incompatible donor calibres),
grants one selected spare magazine and
marks inventory weight dirty. Each supply retains its original kit's total
carried allowance for the later explicit crate path. This allowance does not
authorize charging the recipient's unrelated persistent ammo storage.

Three more i686 tests cover empty/partial/full magazines with all chamber
states, selecting the second compatible ammo type and second kit, exact
reserve/allowance, ownership release, rejection of bound/incompatible
weapons, and a failure in the last supply leaving all counts/flags unchanged.
The focused resource suite now has33 passing tests, with native clippy warnings
denied. New checks also cover unrelated-calibre removal and invalid extra ammo
refusal before any loaded/reserve/flag mutation. The
original initialize slice's autoload behavior is covered separately below.
No gameplay hook gives a player these resources yet, and crate,
reload/HUD/weight integration and snapshot ownership remain open.

### Free crate refill for borrowed ammunition

Owner decision: own-team crates refill borrowed ammo for free up to its
normal carried allowance; do not require matching account ammunition.
`PreparedWeapon::needs_resupply` and `resupply` now implement the private
supply path. The latter requires a `CrateCompletion` token from the native
completed-use boundary (call 2F81C1). A read-only `CrateInventory` can query
availability but cannot authorize a refill. Neither hook is installed yet.

The refill first checks that the weapon is installed in the supplied
recipient inventory. It verifies every private ammo cell,
native encoded slot, item owner and selected retained ammo binding. No
address-relative slot is followed until it matches a cell owned by this
resource. It builds the complete plan before any write. Each compatible
ammo supply fills to its template allowance, subtracting magazine and chamber
rounds only from the selected type. It never removes excess rounds, loads
the magazine, accesses a profile, or charges storage. Both cached inventory
weights are marked dirty when reserve counts change. Stowed weapons refill
too: native deactivation clears their user, but they still belong to the
recipient's inventory.

Three additional i686 tests use fixtures of completed attachment bindings:
free refill and repeat use, all chamber modes, unchanged original ammo and
profile, late invalid binding with no partial refill, above-allowance ammo,
and the full u16 count boundary. Including borrowed death capture, there are
13 resource/preparation/refill tests plus three inventory-reader tests.
These are not native transfer tests or proof of
the live crate hooks' team/hold-time integration.

The original profile refill must still see the saved original weapons when
accounting for their magazine rounds. Native 2FDC10/2FDF50 compare each
weapon's selected ammo slot with the profile's inline ammo slots; an external
borrowed slot does not match, so leaving it installed while calling the
original routines overfills the original reserve by its stowed loaded rounds.
The crate bridge below accounts for those originals without changing the
live held/pending selection or exposing a temporary weapon swap to draws.

`resources/prepared/crate_refill.rs` coordinates both paths through
`Player::crate_availability` and `Player::complete_crate`. It builds a temporary
392-byte view containing only the 23 borrowed slot pointers, substituting
each retained original weapon into that view. This is not a cloned inventory
resource: it has no vtable or owning references and is never attached to a
player or assigned as an item's owner. The original native routines only read
slots from this argument and the refill writes its weight-dirty byte. The
bridge propagates that invalidation to the real inventory after the call.
Normal item callbacks still receive their real item with its real owner.

Both borrowed slots are validated and their private refill plans computed
before the native refill can write profile storage. Missing/wrong slot owners
and external selected ammo slots in supposedly original weapons are rejected.
A private supply needing rounds makes the crate available even with empty
account storage. Once private ammo is full, the native availability result
controls the hint. Only a `CrateCompletion` token authorizes the mutation;
the native team/hold-time hook still has to construct it.

The original-exe harness executes both native routines with a temporary view
for each weapon slot, with and without chambered rounds, and with zero,
limited and sufficient account ammo. It checks native accesses to the view
stay within the slot table plus the weight-dirty write, original reserve and
storage accounting, unchanged live slots/private ammo and balanced native
weapon references. These cases have no non-ammo item callbacks. The native
table at 925FD8 lists only artefact/armor slots, not either weapon slot.
The Rust bridge test uses recording native stubs to check the view contents,
unchanged held/pending selection during the native call, free availability,
refill idempotence and refusal before native mutation. All 27 resource tests
and focused clippy pass. Neither bridge is called by a gameplay hook yet.

### Installing and restoring one native slot

`resources/prepared/installed.rs` now consumes a prepared resource into
`InstalledPickup`. It retains the displaced original item before replacing
the inventory's reference. It deactivates and removes animation references
through the concrete native callbacks, binds the private ammunition, reuses
the slot's existing registry ID, sets held/pending together, and calls the
matching client or core set-user/initialize/activate sequence. Preparation's
autoload flag stays clear, preserving the captured loaded rounds.

The owner restores the original inventory slot and registry entry before
releasing private resources. Explicit restoration returns the detached
resource for the eventual match bank; dropping also restores, then releases.
It releases the initializer's retained ammunition binding, so the returned
resource can be installed again. A dead player is not activated. If the pickup
has been stowed, restoration leaves the other held weapon alone and only
retargets a pending switch when it points to the departing pickup.

Five transaction tests cover ordered callbacks, registry/reference ownership,
atomic refusals, repeat installation, death, stowed/pending restoration,
concrete client/core callback selection, and free refill through the installed
owner followed by detachment. These use recording
FFI stubs; they do not prove real animation teardown or gameplay integration.

`InstalledPickup::replace` coordinates a later pickup into the same held slot.
It preflights the incoming resource, the existing installed mapping and both
ammo bindings before any write. It deactivates the departing borrowed gun,
swaps the private mappings, returns the old gun/ammo to their private inventory,
and activates the incoming gun. The same owner retains the **true original**
throughout; the old borrowed gun never becomes its saved original, and the
original is neither briefly activated nor given an extra retained reference.
Failure leaves the installed owner in place and returns the incoming resource.
Success returns the detached old resource with its current ammo unchanged,
for the future instance bank/history owner. It does not grant pickup ammo again
merely because a resource was detached.

Two additional transaction tests cover three successive borrowed guns,
different incoming ammo type/loaded count, already-used/restocked departing
ammo, release/reference balance, final restoration alive or dead, and six
failure cases before any callbacks or mutations. All 16 installation/snapshot/
crate tests and focused native clippy pass. These use FFI fixtures; live
repeated pickup and the match-owned instance bank still need integration.

The generated engine library's original-exe ABI gate now follows two reviewed
controller tail dispatches: `weapon_core::set_user` at 2E2A24 and
`deactivate` at 2E2EF8. The client constructor installs controller vtable
A04C34, while the core cook installs A1386C; both store it at weapon+278.
The verifier checks construction/dispatch bytes and vtable entries before
following every concrete target's returns. Unknown targets, changed dispatch
slots, inconsistent cleanup and unresolved target tails are rejected by
negative controls. This verifies return conventions, not full callback behavior.

Installation remains an unsafe Vostok boundary: the match owner must keep
player/inventory/core alive and exclusively own the mapping until cleanup,
restore before respawn/destruction/snapshot replacement, and prevent reentrant
transfers. No gameplay hook currently calls it. A match bank and history-record
ownership/serialization remain required.

`Player::decode_pickup_slots` is now the separate snapshot attachment boundary.
It validates both prepared resources/original slots before mutation, retains
the originals and installs only slot, registry and private-ammo mappings. It
leaves held/pending untouched while its callback runs the complete native
player decoder, then returns `DecodedPickups` containing the two installed
owners. Refusal leaves resources in the caller's array; success moves them
into those owners. No pickup grant, initialize or activation runs in this
attachment path. Each input is a `SnapshotWeapon`: a prepared resource paired
with the complete decoded `SlotState` (reserves and original). Pairing rejects a mismatched number of ammo
types without changing or losing the resource. After both slots validate,
attachment restores every private count (including zero and reductions),
marks both inventories' weight dirty, then runs the native reader to restore
loaded ammunition and active state. After that decoder deactivates the old
original, the transaction applies its compact state through the generated
`engine_weapon_core_deserialize` binding. The callback cannot forget either
restoration step. `InstalledPickup::snapshot_reserves` validates the
installed binding before capturing counts.

Three additional Rust tests cover a held transition with changed ammo type,
loaded rounds and reserves, a stowed pickup, and a failure in the second slot
before any first-slot write or decoder call. The decoder in these tests is a
fixture. `check_pickup_registry.py` separately executes the real native
held-ID transition with recording virtual callbacks and proves that prewriting
held suppresses deactivate/set-user. `check_weapon_pickup.py` runs the actual
2E3513..2E3594 ammo-rebind slice and its real intrusive assignment, for both
directions of encoded external slots and with/without a previous ammo binding.

Two further tests cover mismatched supply counts and the combined shared/native
path: install a claimed drop, capture ownership and private counts to bytes,
refill at a crate, detach, decode those bytes, and restore the earlier counts.
Saving again gives identical bytes, without granting another magazine or
preserving the later crate refill. Stale native bindings fail capture. These
tests use native callback fixtures and do not prove actual rollback hooks.

### Displaced original state

The displaced original is deactivated (`weapon+8 == 0`), so its native
simulation codec omits active-controller/animation state. For a non-stackable
weapon owned by a player inventory, `weapon_core::serialize` writes 19 bytes,
or 20 with a chamber: condition bits, owner place/kit/slot, the word at 5BCh,
last tick time, autoload flag, magazine, fire queue type, ammo ordinal, and
optional chamber flag. Native serialized booleans are 00/FF, not 00/01.
The client subclass adds a separate presentation byte to its second writer;
it is not part of the simulation payload.

`Original` parses this compact shape without pointers. It validates owner,
slot, canonical flags and bounded ammo ordinal; `ammunition(capacity)` checks
the loaded count against a concrete template. `InstalledPickup::snapshot_original`
captures those exact bytes from its retained original through generated raw
fields, refusing an active original, stale ownership, stackable item or
incompatible ammunition. Scalar capture avoids the native writer's optional
debug-annotation allocations. It does not mutate the original or inventory.
`snapshot_slot` captures it with the private reserves as one `SlotState` for
`History`. `SnapshotWeapon` requires that complete state, and the transaction
validates both originals before either slot changes: place/kit/slot,
non-stackable entry, concrete magazine capacity, chamber shape, compatible
ammo ordinal/cells/owners, retained selected ammo and the core roster lookup
that the native decoder will follow. It then applies each original after
the native player reader returns, through the original core codec with a null
client reader. Applying the stowed encoding while user is nonzero would make
the decoder read past it into active-controller state; the transaction checks
that contract before calling native code and refuses no inputs after mutation.
There is no independent public original-state setter.

Two added Rust cases verify changed condition/timing/magazine/ammo/chamber
restoration after the callback returns, correct ammo references, and rejection
before mutation for wrong place/kit/slot, missing roster owner, incompatible
ammo type, over-capacity magazine or wrong chamber shape. Native callback
fixtures establish transaction ordering; the original-executable codec test
establishes the actual reader/writer and reference behavior.

`check_pickup_original_state.py` runs the original client/core serializer and
deserializer, including real item ownership resolution and intrusive ammo
assignment, with chamber/no-chamber variants and two ammo types. It verifies
the exact 19/20-byte simulation payload, separate client boolean, switching
selected ammo with balanced references, repeated core decoding without a
client reader, and byte-identical reserialization.
The Rust capture test produces the same payload shape and checks refusal and
no mutation. The native fixture supplies a minimal inventory-holder getter;
active weapons and controller transitions are deliberately outside its scope.
Repeated decode keeps counts unchanged and balances references. These checks
do not cover a complete native player restore or animation lifecycle.

The snapshot boundary remains unsafe and unhooked until the match owner can
provide prepared resources for the target history record, restore old pickup
owners before roster/kit cleanup, then call it after the target kit is chosen.
Native remove (2DA790) with argument zero leaves the old held weapon but clears
pending; argument one deactivates/clears held and resets inventory from profile.
This is why snapshot attachment cannot enforce the live pickup's pending ==
held or alive preconditions. It still refuses an old held original without
its live user, since the native decoder may deactivate it.

## Required behavior

- The picking client targets the bounding box of the weapon on its cosmetic
  ragdoll. The action identifies that particular drop; the other clients do
  not repeat the geometry test against their own cosmetic poses.
- All clients resolve availability and inventory changes deterministically.
  The server relays inputs and compares existing world hashes; it does not
  own another drop registry or check distance/position/bounds.
- Keep the weapon's loaded rounds and give one full spare magazine. Preserve
  a chambered round too: it is ammunition already in the weapon.
- The recipient's own ammo crate must restock the picked-up weapon for free
  up to the normal allowance, including loaded rounds in that allowance.
- Ownership, drop generation, ammunition and any new input payload must
  survive rollback, snapshot loading, late join and replay. Two simultaneous
  picks must resolve to one owner. A new death cannot reuse a stale drop ID.
- This is an in-match transfer; it must not grant permanent account equipment
  or charge the wrong player's or wrong ammunition's persistent storage.
- On death/respawn restore the player's original loadout. In particular, the
  donor respawning must not reclaim an instance still held by a recipient;
  the recipient respawning must not retain their borrowed equipment.
- Pickup replaces a weapon slot rather than growing the inventory. Slots 7
  and 10 are both weapon slots, not fixed primary/pistol slots. The original
  proposal to replace "the primary" is ambiguous when the dual-primary
  skill is enabled and must not become a hard-coded slot assumption.

## Proven native ammo behavior

`tools/re-kit/v026g/check_weapon_pickup.py <original survarium.exe>` runs
the original x86 routines under Unicorn, with synthetic weapons/inventories.
The routines below run unmodified; no ammo or resupply callback is stubbed.

There are two different bases. An inventory slot contains `inventory_item*`.
For a weapon this is **active_object + 0x10**. `weapon_core` functions generally
take the active-object base, but the item virtual death callback takes the item
subobject. Do not reuse offsets without adjusting that base.

Relative to the active-object base:

| Offset | Meaning | Evidence |
|---|---|---|
| +538 | retained ammunition stack pointer | load/unload below |
| +5AC | array of inventory slot indices for compatible ammo | initialize 2E2A99..2E2ABA; crate routines |
| +5B0 | count of compatible ammo slots | 2E41E5 |
| +5B1 | selected ammo ordinal | 2E2AA3; 2FDCFB |
| +5E4 | magazine capacity, u8 | load_magazine 2E1830 |
| +5E5 | loaded magazine rounds, u8 | load_magazine; serialize 2E31BD |
| +5E9 | double_handed, bool | config loader 30AB8D..30ABC0 |
| +5F0 | weapon uses a chamber | 2E2858; serialize 2E3209 |
| +5F1 | chambered round | unload_ammo; serialize 2E3226 |
| +5F4 | load on initialize | 2E2B71..2E2B81 |
| +5F5 | automatic reload flag | initialize 2E2A81; death below |

`load_magazine` (2E17D0, ESI = active object) moves
`min(capacity - loaded, stack)` rounds from the retained ammo item into the
magazine and marks its inventory weight dirty. `unload_ammo` (2E18E0,
EAX = active object) returns both the magazine and chamber to that stack.
The stack count is u16 at item+260; its inventory pointer is item+250.

The native harness also executes the original `double_handed` lookup branch
with absent/false/true config values, proving the field's base and offset.
The containing loader is named descriptively in the re-kit at 30AB40:
stack arguments retained config resource / weapon base, `ret 8`.

The initialize section 2E2A99..2E2B88 can bind a dedicated external ammo cell
using an encoded inventory-relative slot (the existing artefact scheme).
The harness runs both positive and wrapped-negative offsets and proves that
loaded rounds, chamber and reserve survive unchanged when +5F4 is clear,
including an empty magazine. This is a candidate way to isolate borrowed
ammo from the recipient's original calibre. It is **not yet installed**:
the allocation must have a native lifetime owner, reserve counts must enter
snapshots, weight must include it, and crates must refill it explicitly.
The normal crate cannot infer its supply allowance from an external slot.

Native item-slot ownership and weapon-user ownership are separate:
243590 sets item+250/+254 only. Client `weapon::set_user` 2445E0 calls
2E2990 to update the user and dependent pointers; it assumes a non-null
client player at its own +F4E3 read. The normal switch at 2D76D0..2D772F
deactivates the old held object, sets the new user, initializes, then activates
the new weapon. A transfer should follow that path with autoload disabled,
not independently overwrite just player+128 or item+250.
The original `weapon_core::deactivate` with argument one clears its user
at 2E2EE0 after clearing animation callbacks/state. The client wrapper 244860
also stops/releases its +80Ch effect object. Do not replace that native
deactivation with an isolated user-pointer store. The client initialize
wrapper 2449E0 calls core initialize and clears +181Eh.

The native creation path now has owned profile templates and verified
non-reusing inventory/weapon/ammo cooks. The normal inventory cook creates weapon
resources and binds compatible ammo (2C7250/2C75B0).
There is a synchronous resource-query wrapper at E5C20: it copies the query
parameters, wraps the callback, submits at 101F30, then dispatches resource
callbacks until completion. That observation does not establish that it is
safe to call from a simulation hook. Capturing the live context, choosing the
preparation point and a match-owned instance bank remain unimplemented.
Installation/restoration now has the owner above but is not wired into gameplay.
Do not substitute a byte copy of a live weapon:
it contains intrusive references, animation state and internal pointers.

## Weapon-slot eligibility

The data already defines weapon handedness: `ItemParams::double_handed`
(`crates/survarium/src/v026g/item_params.rs`, parsed from the item's config),
also consumed by the HUD/lobby `ItemFactory`. Skill modifier 25 is
`use_two_double_handed_weapons`; modifier 64 is `disable_second_weapon`
(`crates/survarium/src/v026g/skills.rs`). Read the effective modifiers rather
than assuming a particular learned skill node or the weapon's slot number.

Owner-approved replacement rule: replace the held weapon when the
resulting pair is legal. Without the dual-primary modifier, a rifle pickup
while holding a pistol beside another rifle requires switching to that rifle
first. With the modifier, either held rifle (or the held pistol) can be
replaced. `disable_second_weapon` must not be bypassed by pickup either.
The shared Rust rules implement this policy; the native pickup hook is still pending.

The death callback 2E3390 receives **ECX = item subobject**. Its store at
2E3407 to `[esi+5E5]` clears active-object+5F5, not the magazine. Loaded ammo
is still available to capture at death. The focused harness proves loaded
rounds and chamber survive the no-player branch; the player-present branch
has additional animation callbacks and still needs a lifecycle fixture.

## Crate path and the profile mismatch

Labels and ABIs are now in the re-kit. The descriptive names were assigned
from these bodies, not recovered from an original 0.26g PDB.

- `resupply_zone_core::can_use` (2F8080) compares zone+54 with the player's
  profile team at +45EC; team 2 permits both. It then calls check_resupply.
- `use_hint` (2F80C0) uses the same team check and maps availability to
  `st_use_resupply_zone`, `st_resupply_check_empty_inventory`, or
  `st_resupply_check_full_ammo`.
- `update_use` (2F8120) requires the selected zone and the player's Use bit
  (input mask+330, bit 21); after zone+50 milliseconds it calls vtable+48.
- `finish_use` (2F81A0) gets the player through its holder, then calls
  `inventory::resupply_from_profile` (2FDC10) with inventory, profile and kit.
- `inventory::check_resupply` (2FDF50) returns 0 available, 1 empty storage,
  2 full. Its args are AL kit, stack inventory/profile.

The native refill walks the **profile's slot descriptors**, not a weapon's
calibre. It counts the rounds already in each weapon's magazine and chamber
against the configured total carried allowance, adds reserve rounds up to
that allowance, and subtracts those rounds from the corresponding profile
item's storage. It does not fill the magazine directly.

The harness verifies ordinary and chambered weapons, exhausted storage,
repeated full-crate use, and a deliberately different profile item in the
ammo slot. That last case still fills the current ammo object while consuming
the unrelated profile item's storage. Therefore merely swapping the weapon
and its ammo stack is insufficient and would charge the wrong ammunition.
Borrowed weapon supplies need an explicit simulation-owned mapping/restock
path, with the native interaction's team and hold-duration behavior retained.

## Snapshot constraints

`inventory::serialize` 2FE360 and `deserialize` 2FE550 iterate the existing
slots without encoding arbitrary replacement weapon identities. The reader
2FE7B0..2FE7D4 invokes each **currently installed** item's deserializer. The
artifact slots have a separate identity mapping; weapon slots do not.

A pickup implementation must restore the weapon/ammo ownership mapping
**before** those readers run, and must keep an object alive independently of
the donor's respawn/profile setup. A raw pointer swap alone satisfies neither
condition. Any extension must also reach the server's snapshot parser and
replay format; an un-hashed local static cannot own this state.

The mapping must actually be restored earlier than the inventory call:
`base_player::deserialize` first reads held and pending active-object IDs
at 2DB626 and 2DB796, resolves them through core+18B4Ch, and writes player
+128h/+12Ch. Only afterward does 2DB7D8 call inventory::deserialize.
An ID byte of FFh means null. Restoring only at the inventory reader leaves
the already decoded held/pending pointers on the wrong weapon.

There are two native stages, so extending only the byte-stream reader is
insufficient. `game_world_core::serialize` (2D2570) captures live state into a
history record after `clear_buffers` at 2D257E. The record's wire writer is
`game_state_history_item::serialize` (324BE0), also called by the CRC routine
at 3251D7. The wire reader (324D80) clears its record at 324D90, then fills it.
Rollback applies a record through `game_world_core::deserialize` (2D2770),
without necessarily going through the wire reader again. Extra pickup state
must be owned by the history record across all these paths, included in its
wire bytes/hash, and cleared/released with the record; a current-world-only
side table cannot restore an older tick.

The record constructor is 324510 (ESI input, EAX result, ret 0), destructor
324910 (stack record, ret 4). Pool checkout calls construction at 2D3FEF;
pool return calls destruction at 2D4034. The in-place rebuild at 3252A7/3252AC
destroys and constructs the **same address**, then captures live state at
3252B3. Debug comparison also constructs/destroys a temporary record at
2D2446/2D254D. Therefore an address-keyed extension must release/reset on
native lifecycle events, not assume that an address identifies one snapshot.
`check_pickup_history.py` executes original construction, clear and destruction
through three same-address generations, verifies the self-relative section
buffers, player-buffer clearing, stack cleanup and allocation guards. Its
cases use inline buffers and empty animation references; heap-overflow and
nonempty reference teardown are not covered.

The fixture also demonstrates that `clear_buffers` preserves the cached CRC
flag at C0BB8 and value at C0BB4: calling native hash afterward returns the
cached value without serializing. Any extension mutation must invalidate
that cache explicitly. The constructor resets it. The hash writer's fixed
8000h-byte scratch buffer also needs a capacity audit before appending the
full extension (templates and displaced state can exceed the ammo payload).

The apply routine reconciles roster membership, then at 2D2A88 selects the
snapshot's kit in its roster slot and at 2D2AA0 calls that player's virtual
deserializer. Restoration must detach old borrowed mappings before native
roster/kit cleanup, then attach the decoded mapping before the player reader
resolves held/pending IDs. Native weapon decoding at 2E3513..2E3536 resolves
its selected ammo ordinal through weapon+5AC and inventory+10C, so the private
ammo cells must already be bound there too. These are inspected ordering
constraints, not installed hooks or proof of snapshot lifecycle completion.

The live installation sequence cannot be reused verbatim for this restore.
At 2DB62B the native reader saves the old held pointer, then resolves the new
one; when they differ, 2DB662..2DB67B deactivates the old object and calls the
new object's set-user. Prewriting held to the replacement would suppress
that transition. The snapshot attachment transaction above establishes
slot/registry/ammo bindings while preserving the old held pointer until the
native decoder performs its own transition.

`check_pickup_registry.py` executes the original weapon binder 2E30F0
(ECX = item subobject, stack core). It appends active-object pointers via
2D4540 to the core's fixed-capacity registry, writes core into active+4 and
active+258h, and writes the byte ID at active+Ch. The fixture then runs both
original ID-decoding slices, showing repeated original/replacement mapping
restoration with stable IDs and a negative control for restoring too late.
No transfer hook is installed by this fixture; the coordinated native
transaction above still needs match and snapshot integration.
Appending a new ID for every pickup is not a viable solution:
the native registry is capacity bounded and IDs are bytes.

## Targeted input transport (server side implemented)

A pickup now travels atomically with an ordinary input. Client ID D7 has
15 fixed payload bytes: tick, precise milliseconds (FF means on-grid;
00..31 hex means precise 0..49 ms), rotation4, actions4, donor place1,
donor spawn time4. Server ID F7 adds the picker place in front (16 bytes).
The full DropId is retained; it is not truncated to a token. Existing
ordinary input packets remain byte-identical. `udp-transport` now includes
both IDs with the same lifetime/retry policy as their ordinary input peers.

The server runs both through the same input acceptance path. Retiming changes
only the input time; rejection drops the target with the input. Deduplication
is still by the original timestamp, so retransmissions and a normal input at
the same time cannot cause a second pickup. Both kinds occupy the history
ring budget and are retained for clients loading snapshots. The server checks
target roster bounds but does not infer ragdoll geometry or whether a drop
exists: deterministic client state must decide that.

Solo PvE records accepted pickup in record kind 6: full accepted time4,
rotation4, actions4, donor place1, donor spawn4 (17 bytes). This is after
acceptance/retiming, not the original untrusted inbound request. Existing
multiplayer datagram recording retains F7. A temporary explicit conversion
error prevents publishing `.sur` files that silently omit an accepted pickup.
That guard **must be replaced by actual replay encoding and native ingestion**;
playback is not implemented, and the guard is not a completion criterion.

Verified with scripted clients using the real server and UDP transport:
timely relay, Move/Discard/Relay late policies, retransmission and mixed-normal
deduplication, malformed rotation rejection, and target retention before a
joiner's snapshot. Recorder test writes/reads a real solo PvE `.srprec` and
checks full accepted time/target bytes. The 117 match-message tests and focused
clippy of match-server, survarium and udp-transport passed. The broader
80 server game-state tests and 12 replay-conversion tests also pass. These are server/
codec checks, not native client or gameplay verification.

Native input evidence, reread from the original executable:
- 318400 decodes time at +8, place at +C, rotation at +0 and actions at +4;
  action bit31 becomes the flag at +D. Bytes +E/+F are not read or written.
- 3184A0 serializes time, rotation and actions with that flag restored.
- 2C5B60 merges exactly 8-byte input states, adding half-float rotations via
  2C6640 and ORing actions. Padding in the larger input object is therefore
  not a reliable carrier for the pickup target.
- Replay record reader 1F0BE0 reads type/time, then uses only the low four
  bits of type-minus-five for native fields. Its output contains masks at
  +4/+8/+C/+10, 20 eight-byte inputs at +14 and kit choices at +B4. A type
  extension cannot be emitted until this reader, its end-time scan and the
  playback input ingestion path all preserve its target.
These four functions and their observed register/stack ABI are in re-kit.
Native input-journal lifecycle and timing hooks are installed (patch78 below).
Native F7 framing and dispatch now feed the installed target journal. Native D7
sending, replay ingestion and pickup execution are not connected. Discard
retirement follows the installed native completion hook.

### Native transport and F7 dispatch (installed)

The original27A300 dispatcher has EAX=unsigned message ID, ESI=network client
and EBX=buffer reader at27A322. The new hook consumes F7 there and exits through
common completion27A6FC, retaining the native packet-recording tail (including
resetting the reader to its saved cursor). Ordinary messages retain the original
switch. State must be2 at client+1F1C64; client+1F1C40 is the world wrapper,
wrapper+1F8 its embedded core and wrapper+191B8 the ordinary input time base.
Outside a match or without a wrapper, F7 consumes the payload without placing
input. Inside a match, exact length, picker, target, sub-tick, clock and rotation
are parsed before native placement. Duplicate/refusal cannot overwrite a target.

Four transport hooks precede dispatch:2792DC extends the constructed match
getter's receive maximum toF7 and send minimum toD7;279694 handles their fixed
lengths15/16 before the native jump table;279A60/279B10 route their info lookup
to the existing ordinary-input branches (500/50ms request,250/50ms relay,
flags9F). Length checks validate available bytes before returning success,
including multi-message datagrams where the original parser otherwise trusts
fixed lengths. No lobby getter is changed. D7 sending is connected at the
native history sender as described below; local target selection is not.

`check_pickup_transport.py` executes these linked Rust hooks, original table
getters and native3FA240 parser. All30 existing message IDs retain their length
and info. It checks real single/multi F7 parsing, mixed EB/F7 order, truncated
and extra bytes, and range rejection. A separate native-dispatch run executes
actual Rust receive plus original input placement, duplicate handling and
populated move/discard hooks. It also checks outside-match consumption,
ordinary EB dispatch and arrival at the native recording tail. Only the recorder
endpoint is stubbed to capture its arguments: this does not prove recording or
replay of targets. Twelve Rust input tests include retained target contents after
successful dispatch, duplicate/refusal, malformed inputs and full capacity.

Observed upstream behavior:379930 uses12-byte inline response storage only for
EB/EC. F7 takes the generic pooled-payload path, which copies its full payload;
it must not be routed into that undersized optimization. The generic queue and
statistics path are inspected but not yet exercised end-to-end by this fixture.
Statistics select helpers3527E0/352830 use id-min+5 and strides6168h/6174h;
the findings are in re-kit. Transport parsing and game dispatch are tested as
separate boundaries, not as a live network match.

Malformed F7 and placement-boundary errors produce an explicit patch-log line;
they do not insert an action without its target. **A full journal currently
rejects and logs the relay; transport may already have acknowledged it.** A
recovery/backpressure policy is still required before release. The installed
receive path is not evidence that local insertion,
pool reset/resync or pickup execution is complete.

### Native outgoing targeted input

The sender27E9F0 selects each changed native input from the event history,
including its original versus combined rotation/actions. The hook at27EAFF
keeps that selection: ESI is still the node, ECX the selected rotation,
ESP+24 the selected actions and ESP+2C the picker. EBP identifies the sender
context, whose +4 is the core and +0 points to its transport holder. The native
sent flag has already been set; queued continuation27EB4D advances the original
loop and eventually records its sent mask.

`InputHistory::original_target` reads the exact node/place/time generation
without consuming it, ignores corrected targets and refuses ambiguous original
targets instead of choosing one arbitrarily. Without a target, the native
DC/DD serializer remains unchanged. With one, the Rust owner releases its
journal lock, constructs D7 through the concrete match transport279FE0,
writes the shared15-byte codec through native buffer_writer7AC10, and hands
ownership to enqueue27A2A0. The transport's native +4 queued flag is preserved.
Packet construction, writing and queue transfer are one private operation;
callers cannot forget a separate commit or send a differently encoded packet.
The concrete interface vtableA08864 is checked before packet allocation.

The original precise flag distinguishes on-grid from precise zero, and the
native node's flag at+27 restores action bit31. Sender tests cover nonzero
rotations, all three timestamp forms, original-component selection after a
corrected contribution, repeat serialization, corrected-target exclusion and
unchanged native loop accounting. They execute the linked Rust, the real
history sender, packet constructor278D00, metadata initializer378080, writers
and frontend wrappers. Packet-pool checkout, statistics and async queue
submission are fixture endpoints: this is not an end-to-end socket/retry test.
The fixture seeds targets through F7 ingress, not through player selection.

Local selection/placement still must attach one original target to its native
input and define coalescing in agreement with the single Use action that input
can represent. The send lookup deliberately does not silently drop additional
targets. Ragdoll selection, local prediction and live request creation remain
unconnected; installing the sender alone does not make pickup playable.

### Native placement receipt and shared wire codec

`weapon-pickup::input::WireInput` is now the single allocation-free parser/
encoder for D7's payload, used by the server wrapper and the native adapter.
`SubTick` makes invalid precise milliseconds unconstructible and keeps
on-grid distinct from precise zero. Time reconstruction uses the native
signed tick-byte difference, rejecting overflow/underflow; rotations use
the server's finite, below-1024-radian half-float bound. Three additional
shared tests cover malformed bytes, precision, signed tick wrap and invalid
rotations. The shared crate currently has 41 tests, including the input-journal
transitions described below.

`vostok::pickup_input::InputCore::receive` uses the generated
`engine_change_player_input` binding to the original 1FAB50. It separates
bit31 from actions into the native flag byte, validates native 20-place
bounds, and passes the native network/precise flags. It returns a typed
`Acceptance`: only `Placed` carries an `AcceptedPickup` with the unchanged
DropId and the engine's **effective** GameTime. Duplicate/refusal cannot
manufacture a second target. F7 dispatch calls it through the journal owner;
pickup execution is still unconnected.
The adapter now coordinates placement with journal insertion: it checks room
before invoking native mutation, reconciles surviving input nodes, and stores
the target at the effective time. Callers no longer have a separate receipt
recording step. The native list is checked for cycles and the 27-node bound;
a network insertion that reuses a node at a different time retires its old
targets before recording the new one. Ordinary replacement, local insertion,
discard and pool retirement still need their hooks; this adapter alone cannot maintain the
journal across all native mutations.

Original function evidence is in re-kit and
`tools/re-kit/v026g/check_pickup_inputs.py`. The fixture executes the real
1FAB50 and list insertion. The real input-node pool checkout, eviction and free-list reuse execute on
preallocated 27-node pools. Seen-input allocation, history-dirty notifications
and logging are fixture endpoints. It verifies:
- accepted precise input returns status0 and writes its timestamp;
- duplicate returns1, leaves output at the native -1 sentinel and does not
  change the stored input or dirty history;
- an old input at423 with clock1000/horizon500 moves to1023 (status2);
- a too-future input at4007 moves to3457 (status3);
- before-last-resync and reject-inputs paths return6/4 without writing output;
- local/replay inputs at901 then912 can become one node at912, with combined
  actions, dirtying both old and new ticks. A journal keyed only by the
  incoming time would therefore be wrong.

The fixture also executes the real move (27D2D0) and discard (27D820):
- move transfers only the original portion to the destination's corrected
  portion, leaving corrected input at the source;
- self-move combines the portions into corrected input; a second move finds
  no original portion;
- destination placement can retime again (800 -> requested423 -> actual1023);
- **move can remove the original portion and then refuse the destination**:
  moving800 to550 with last-resync600 returns6, but the original portion is
  already gone. A final status alone does not distinguish early source refusal
  from late destination refusal. The installed completion hook observes the inner
  placement result at27D802, rather than inferring extraction from the outer return;
- discard removes both portions. Nonzero discard statuses1/4/5/6 preserve the
  input; these represent no exact time, no preceding history, missing source,
  and at/before last resync respectively.

The output pointer is stack argument9, written on successful tails at
1FB35F/1FB607; ret24h consumes all nine arguments. The stock relay initializes
it to -1 at27C314. Rust tests verify target preservation, exact arguments
(including action bit31), duplicate/refusal handling and no native call for
invalid inputs. The native argument7 (`corrected`) both selects the corrected input portion
and bypasses network duplicate rejection; it is separate from the input's
bit31 flag. Standalone receive passes0; the native move's inner call passes1.

The private placement transaction accepts both ordinary and targeted relays. `RelayInput` validates native time/rotation once; an
ordinary relay carries no target and requires no spare journal entry. When
native placement replaces the original portion it removes that portion's
pickup, preserving corrected targets. Duplicate/refusal leave targets intact;
retimed/merged ordinary inputs add no new target. The original-executable
fixture verifies original/corrected component replacement followed by a move.
The effective native action mask can retain older action bits until the move
splits the components; target replacement follows the original component,
not a diff of the effective mask. Local prediction/relay convergence still
requires end-to-end verification when those callers are connected.

`CompletedDiscard` observes the success-only native epilogue at27DCB7 and
retires both journal portions. It replaces the separate native-call/discard
adapter so ordinary native callers cannot omit target cleanup. It needs no
spare capacity; refusal bypasses the hook. Sixteen i686 adapter tests cover receive,
ordinary replacement at full capacity, pool reuse, invalid arguments,
discard/move/local-retiming receipts, independent journal owners and receive ownership restoration after nested refusal/error/unwind. The original
executable fixture covers the native semantics; Rust fixture tests cover the
adapter's coordination. Neither is a gameplay or complete rollback test.
The generated 66-binding ABI suite and arena relink pass. Current arena
SHA-256: `2118145413e52b8cf870f779a4e7a3c220978126bf974d48c92eafc9a2e935ab`.

Ordinary relay completion is observed at the shared statistics entry27C1F0,
before its prologue. Return27C32D identifies EB/EC, with effective time at
entryESP+14; return27C5CD identifies each ED/EE member, with effective time
at callerEBP-10. Other statistics callers are ignored before reading their
client or input. This observes native decoding/placement without replacing it.
The completed node's corrected flag determines replacement versus merge;
network placement leaves that flag unchanged on the non-server status0 paths.
The native transport fixture exercises EB, ED and EE, including repeated
batches and the original disabled-statistics path for duplicate inputs.

### Native move completion boundary

`CompletedMove::at_end` reads the original move's receipt at27D802: source
node identity inEBX, final status inEAX, core/place atEBP+8/+C, and effective
time at siteESP+1C. The output sentinel is parsed immediately. Early source
refusals never reach this site; reaching it proves extraction happened, even
when the final destination status is4/6. The native fixture observes these
registers/locals during real accepted, retimed, and refused moves.

The receipt borrows the hook thread and consumes itself when applying the
journal transition. It transfers the extracted original before reconciling
live nodes, so source unlinking cannot discard that pickup too early. Native
placement is not called again, and native results/stack remain untouched.
The installed move-completion hook routes this receipt to its match-owned journal.
An eventual generic placement observer must defer the nested move's journal
mutation to this completion boundary, avoiding double application or premature
source retirement.

Pool checkout1FCB60 evicts the oldest node through2D48B0 when full. The fixture
now executes both bodies with27 preallocated nodes, including a move whose
corrected source is evicted while its original is being placed, reusing the
same address. `MoveOriginal` retires stale destination generations instead of
retiming their targets; only the extracted original survives source reuse.
A move to the same node/time still preserves its corrected targets.

The installed eviction observer at2D48C7 recognizes an exact per-place input
list base belonging to a registered core before reading its head. The generic
routine also serves seen-input and other lists, which it ignores. For a ready
journal, eviction retires both portions before native unlink/pool return.
At27D7F5, after extraction but before inner placement, an active journal enters
`Moving { history, source }`. Eviction defers to its completion at27D802:
otherwise evicting the source would destroy the original target before it could
be transferred. Completion checks source identity, applies the move and restores
`Ready`. Early source refusal reaches neither hook. A checked-out `Receiving`
journal similarly reconciles native survival and destination generation inside
its placement transaction. Nested receive while moving is refused.

The Rust owner test checks unrelated/misaligned list filtering, removal of both
portions, preservation of other inputs, checked-out receive, and source reuse
through the begin/eviction/completion sequence. The linked native fixture fills
the local player's 27-node pool, moves its original through source eviction,
then forces further ordinary and targeted evictions. It also checks that
destination refusal and retiming finish the active move transaction, permitting
a subsequent receive and teardown. No native list mutation is substituted in
those cases. Pool clear/resync paths remain unconnected.

### Local merge and tick snapping

Native local merge1FB31C changes the complete node's time. Separately,
`snap_player_inputs`2D4E40 can move a local input to the simulation grid without
calling change_input or move_player_input. It waits when the next tick is
beyond its limit, retaining the pending time at core+18FCC. When permitted,
it dirties the old/new timeline positions and writes the effective time at
2D4F3B (EBX core, EDI node, ESI new time). The original-executable fixture
covers912 remaining912 with limit925, then becoming950 with limit1000.

`RetimedInput` observes either local merge at1FB349 or snapping at2D4F3B,
borrows the hook thread, and consumes its receipt through `Change::Retime`.
Both original and corrected targets move together without adding requests or
requiring capacity. It does not replay native mutation or change registers.
`CoreId` gives the match owner an opaque identity for routing these receipts
and completed moves. Installed creation/destruction hooks retire it before
native address reuse; the local-merge and tick-snap hooks invoke these observers.

Lifecycle evidence for the installed owner integration: core constructor
2CFE90 initializes two input-list vectors at core+2FCC and+89D8. The verified
pickup-input path uses the first. `snap_inputs`2D2E10 invokes separate snapping
functions for both banks; the second bank's purpose is not established here.
Destructor2D02A0 establishes EBP=core at2D02A2, before cleanup at2D02A6, and
later destroys lists/history at2D0478/2D0483. The core is embedded at wrapper
+1F8, followed by wrapper fields at+19238/+1923C; appending storage at core
+19040 would overwrite those fields. Native object sizes/layouts remain unchanged;
the journal is a separate owned allocation.

### Installed match-owned journals (patch78)

`dropped_weapons.toml` now installs twenty generated Rust lohooks: the five
transport/dispatch hooks above, three presentation hooks, two death-profile
capture hooks, plus ten lifecycle/input hooks:

- core constructor completion2D020A registers a separate journal owner;
- destructor entry2D02A6 detaches it before native resource cleanup;
- local merge1FB349 and tick snap2D4F3B route whole-node retiming;
- move begin27D7F5 preserves its extracted source until completion27D802;
- generic eviction2D48C7 retires targets for recognized input lists, deferring
  active receive/move transactions to their completion;
- ordinary relay completion27C1F0 observes single and batch input replacement;
- outgoing history selection27EAFF emits D7 when that original input has a target;
- successful discard27DCB7 retires the removed node after unlink, pool return
  and dirty notification. EBX remains its opaque identity; EBP+8/+C are core
  and place. The five replayed bytes restore EDI/ESI, set EAX=0 and restore
  EBX. Refusal exits never reach this site.

`InputJournals` owns a locked registry of independently allocated worlds keyed
by opaque core identity. Allocations/frees happen outside the registry lock;
each detached node is dropped and freed with its original native allocator.
No fixed number of simultaneous cores is assumed. Duplicate construction and
unregistered mutation/destruction are invariant failures. Core address reuse
gets a fresh owner, and destroying one world preserves other worlds' inputs.

The target storage budget is4096 entries per world, with explicit full refusal
in the ingress adapter rather than silent eviction. Journal contents remain
`Tracking::Dormant` until the first targeted input; ordinary gameplay does not scan thousands
of empty slots. F7 ingress activates its world
journal. `InputJournals::receive` checks out the history into an owned
guard and marks the registry entry `Receiving`; it invokes native placement
outside the registry lock, then restores `Ready(history)` on every return.
A nested receive for the same core returns `ReceiveInProgress` without replacing
its history; another core remains independent. Rust tests also verify restoration
on unwinding. Low-level placement adapters are private to the input boundary,
so external callers cannot substitute an unrelated history. The guard does not
roll back native input mutation: placement still follows the native status and
capacity rules above. Unexpected timing mutation or teardown while checked out
is an invariant failure, not a silently dropped event.
Remaining local-input insertion, pool clear/retirement and
resync paths must also be covered before treating the journal as complete.

`check_pickup_hooks.py` runs the actual linked Rust, generated NASM entries and
original input functions in Unicorn. Only heap allocation/free, thread identity,
and the input fixture's timeline/logging endpoints are substituted. It checks
construction/destruction entry registers and stack, native local merge/snap/move,
successful discard plus all four refusal statuses (1/4/5/6), two independent
worlds, non-head removal, and reuse of a core address with three
allocations freed exactly once. It supplies completed native constructor frames;
it does not execute resource-heavy world construction or a real game. Its
journals are dormant; the transport fixture also exercises populated journals
through receive/move/discard and full-pool eviction/reuse, and Rust tests verify
their contents and isolation. No weapon transfer or restock readiness is claimed.

Pickup targets do not yet execute a weapon transfer. The simulation_changes
`next` entry is due when pickup execution
is connected, before such a build can be released.

### Shared input journal

`weapon-pickup::input_history::InputHistory<N>` is a bounded, allocation-free
journal keyed by opaque native node identities. Its single `apply` mutation
API models accepted original/corrected portions, replacement/merge, extracted
originals with either a placed destination or a refused destination, whole-node
retirement, surviving-node reconciliation and lifecycle reset. Capacity failure
and ownership mismatch are atomic. Moves and removals need no spare capacity.

World rollback retains this input journal, just as it retains native input
lists. Reading events at a time does not consume them. Events are deduplicated
and sorted by picker and donor life so packet arrival and native node addresses
do not affect simulation order. Local node pulls update the entire node's
target timestamps. The nine journal tests include multiple requests coalescing,
local tick snapping, partial moves, rejected destination after extraction, self-move, whole discard,
capacity failure, destination recycling (including source==destination) and reset. Node identities are never serialized;
this journal is separate from saved ownership/ammo state in WPA2.

Still required: native local insertion, remaining pool
retirement/clear and resync paths; local request creation and receive-capacity recovery;
simulation capture/restore/hash; and replay ingestion. A moved input's final
status is insufficient evidence for a journal update, as described above.
The server conversion guard remains until replay preserves and applies targets.

## Client weapon bounds and draw-frame ownership

Three installed Rust hooks now collect presentation geometry, separately from
the deterministic input journal and ownership snapshot:

- `pickup_draw.begin` at1F7C6C starts an empty frame, before either native draw
  path. EBX is game_world, whose match wrapper at+4D4C contains core at+1F8.
- `pickup_draw.weapon` at2477C8 copies the dead player's held weapon's model
  bounds and EAX's exact selected draw transform. This follows patch76's
  ragdoll Weapon-bone change and native composition with the player transform.
- `pickup_draw.complete` at1F7E0B publishes the frame after the parallel draw
  task join at1F7D7F or the synchronous draw loop, including zero-player frames.

`InputJournals` owns a separate `targeting::Frames` for each native core. The
registry lock serializes draw workers. No native pointers escape the reader;
no registry borrow survives a native call. Draws outside an open frame are
ignored. A new frame clears every old observation, and selection is disabled
until completion. Culled players and missing/nonweapon models cannot leave a
clickable previous-frame box. Core destruction frees the views with the input
journal; reconstructing the same native address starts empty. The native task
join is the lifetime boundary: teardown cannot race an outstanding mt_draw.

The model path is client weapon+1810 -> wrapper+108 -> skeleton render
instance+62B4 -> original resource+108 (minXYZ/maxXYZ). Only the concrete client
weapon vtable is accepted; a weapon_core, grenade or battery cannot be cast
to this layout. The resource box is rotated Y(pi) into renderer-local space
(x/z mirrored with min/max exchanged), then positioned using the exact draw
matrix. The reader does not read mutable cached renderer bounds at+62B8:
those are updated by queued render commands, can lag this draw, and include
a generic fallback cube and animation-locator expansion. The authored model
box deliberately excludes that extra culling volume. Coverage of real model
resources and separately drawn attachments still needs gameplay verification.

`WeaponBox` rejects nonfinite/reversed boxes and singular/projective transforms.
Segment intersection occurs in model coordinates, retaining the weapon's
orientation, reflection, nonuniform scale and shear. A finite segment bounds
interaction reach. The nearest currently available DropId wins, with place as
an equal-depth tie break. Claimed drops, old lives and departed donors fail
the simulation-world lookup even when a view still exists. Geometry remains
client-only and never enters packet payloads, snapshots or hashes.

Verification: six shared targeting tests (41 crate tests total) cover reach,
parallel/inside/end-point hits, rotated-box empty corners, scale/reflection/
shear, invalid/extreme floats, nearest selection, life/claim rejection and
frame publication/expiry. Two additional i686 boundary/owner tests (14 input
adapter tests total) cover asymmetric coordinate conversion, the selected
draw pose, native type/life rejection, independent worlds and address reuse.
Focused shared/native clippy passes with warnings denied.
`check_pickup_bounds.py` executes the original renderer's resource-box copy,
Y(pi) transform, fallback and bone-origin expansion; it verifies the authored
resource remains unchanged. `check_pickup_hooks.py` installs all 18 pickup
hook entries and executes draw begin/capture/complete alongside input/lifecycle
hooks, checking their native register/flag continuations. The F7/D7 transport fixture
also passes against this linked arena. These are synthetic fixtures, not a
rendered match or successful gameplay pickup.

Local camera-ray construction, occlusion/reach policy, Use-edge attachment
and actual resource transfer remain unconnected. Capturing frames does not
yet make pickup playable or change ammunition during a live game.

## Completed local input attachment boundary

`CompletedLocalInput::at_end` reads game_world::tick at1F7928, after the
ordinary wrapper1EEDE0 has placed the input and popped its three arguments.
The caller still owns effective-time output at siteESP+14 and player_input
at+20 (rotation), +24 (actions), +28 (requested time), +2C (place). EBX is
game_world, EAX the native return code. The native local player at siteESP+18
must be alive/inserted and belong to this core/place. Its input handler+F470
still holds the preceding action mask at+344 until native code updates it at
1F7944. The boundary requires a rising Use bit, preventing held Use from
generating a pickup every rendered frame without another key-state cache.

`InputJournals::attach_local` coordinates target insertion and native timing
under the match owner's lock. The receipt requires an unretimed successful
local input and finds its exact live node. Native local coalescing may have
preserved movement bits; those are retained. A different original target on
the same node is refused, while repeated attachment of the same target is
idempotent. A clone preflight reconciles retired node generations and checks
capacity before committing the journal and changing native fields.

The native `precise` argument does not affect local placement:1FB2AE derives
precision from FIRE transitions alone. After attachment, the adapter sets
the completed node+24 precise byte and clears its matching core+18FCC pending
tick-snap cursor. Both changes are required:2D4E40 can move a precise node
when walking the pending timeline. Later local inputs skip this node as a
merge candidate at1FB256 and retain their ordinary coalescing/snapping behavior.

The adapter refuses corrected nodes (the sender chooses a separate original
component), retimed placements, another outstanding pending input, missing
Use edges, unavailable local-player state, conflicting targets and full
history before changing target/native timing. The eventual producer must
handle these refusals rather than report a successful pickup. It must also
select against the current match's completed presentation and ownership state.

Two i686 tests cover the caller stack layout, Use edge, accepted/idempotent
attachment, preserved coalesced movement bits, and atomic refusal paths;
all 16 pickup-input tests and native clippy with warnings denied pass.
`check_pickup_inputs.py` executes the original caller1F7903..1F7928 and its
post-completion input-mask update, proves the ignored local precise argument,
and tests the two promotion writes against original subsequent input and
tick-snap code. The Rust adapter and original consumers are checked separately;
the fixture does not simulate a selected weapon or live client request.

This adapter is not installed as a local-input hook yet. Camera/reach/occlusion
selection and the match-owned live drop/resource state must be connected first.
It does not yet add a pickup target during an actual match. Full local input ->
D7 -> server/F7 -> predicted transfer convergence remains to be verified.

## Remaining implementation and verification

1. Establish safe lifetime and creation/reuse of native weapon instances,
   preserving attachments and appearance without donor respawn reclaiming a
   weapon already picked up. The profile owner and native query boundary are
   implemented, as is one native installation/restoration owner. Death hooks
   populate a match-owned profile bank which retains exact source lives after
   donors leave. Live resource-query context now comes from the current match
   with a retained native owner. A verified preparation dispatch point and the
   match-owned instance bank remain unimplemented.
   Serialize retained templates for history/late join, resolving the source
   life in the shared template key rather than the place's current occupant.
   Isolate all raw access in the Rust Vostok layer.
2. Add pointer-free shared drop/ownership state and its serialized mapping,
   including restore-before-inventory ordering and teardown.
3. Complete the targeted-input protocol integration with local request creation,
   ordinary input mutations, pool reset/resync and capacity recovery; implement replay encoding/ingestion and replace
   the temporary conversion guard. Apply pickup in deterministic player order.
   No server spatial check.
4. The authored model bounds/exact draw pose and frame ownership are exposed.
   Connect the local camera segment and Use input to the completed-frame
   selector, verify real weapon/attachment bounds, and remove the dropped
   visual after a successful transfer.
5. Prepared resources now initialize loaded rounds unchanged and reserve to
   one magazine exactly once; validated borrowed supplies have a free refill
   path. Connect those resources to native transfer and the native crate
   eligibility/completion hooks through the implemented crate bridge; integrate HUD,
   reload and weight without charging unrelated profile supplies.
6. Verify native transfer/reload, simultaneous pickup, donor/recipient death
   and respawn, rollback before/after pickup, snapshot/late join, replay and
   restock. Add the simulation_changes `next` row when pickup changes simulation.
   Run the complete native and Rust gates before pushing; request a fresh
   owner go before starting clients for the required gameplay verification.

# Held artefact transfer: engine investigation

Status: research for Shift + artefact number to offer an artefact in the hand.
No executable hook or gameplay feature is installed by this branch yet.
The requested interaction is another character pressing the normal Use key;
the recipient must have a free container. Full inventories must refuse the
transfer without replacing an artefact. The game must not run a local-only
inventory mutation: input, ownership and offer state must survive rollback,
joining and replay on every client.

## Inventory ownership and quick slots

Disassembled from the original 0.26g executable, and exercised with
`tools/re-kit/v026g/check_inventory_transfer.py` under Unicorn:

| Function | Contract |
| --- | --- |
| `inventory::set_item` 0x2FC620 | ESI inventory, EDI slot, stack reference to item pointer, `ret 4`. Retains incoming item, replaces slot reference, calls item virtual +0x28 with inventory and slot, marks inventory dirty. |
| `inventory::remove_item` 0x2FC680 | ESI inventory, EAX slot, `ret`. Requires an occupied slot. Calls item virtual +0x34, then +0x28 with null owner and slot 23; clears and releases slot reference. |
| `inventory::take_artefact` 0x2FC850 | EAX inventory, stack reference to incoming pointer, `ret 4`. Finds the first free artefact slot. If none is free, **removes the first artefact, shifts the remaining artefacts, and inserts the new one last**. A zero-container inventory is not a supported caller. |
| Quick-slot lookup 0x2FE7F0 | Stack inventory and zero-based quick ordinal, `ret 8`, EAX inventory slot or 23. Walks the regular item order, then the artefact containers. Empty artefact containers still consume an ordinal. |
| `base_player::process_quick_slots` 0x2D8BB0 | EAX player, stack previous input, current input, simulation time; `ret 12`. Input action masks are at input +4. Bits 14..19 address the six quick slots. Calls `inventory::action` on rising and falling edges. |

Inventory slots are 23 resource pointers at +0x10C. The pointer at +0x168 is
the beginning of the artefact slots, **not the end of all inventory slots**;
the artefact slot count is the byte +0x16C. Item owner and slot are +0x250 and
+0x254; the intrusive reference count is +0xD0. The inventory owner interface
is +0x174; inventory dirty is +0x184.

Unicorn verifies: empty-container quick-slot mapping; insertion into the
first empty slot; full-container eviction and shifting; removal followed by
pickup while retaining the item across the gap. Item lifecycle methods are
recording stubs in that test, so it does not prove real passive-effect
unlinking/relinking, world integration or presentation.

Artefact virtual +0x30 is 0x313470: enable passive, set state 3. Virtual +0x34
is 0x313490: if state 4, disable active; disable passive; set state 0. The
native transfer sequence therefore must retain the resource, remove from
the donor, then take into the already-validated empty recipient container.
Calling take first would overwrite item ownership before the donor's
passive unlink. Cloning the artefact is also incorrect: it remains in its
original anomaly's resource list and is serialized by that anomaly.

The observed pickup routine checks capacity, not an artefact-kind/container
compatibility table. Any additional restriction needs evidence from the
container setup/configuration code, rather than a hard-coded kind list.

## Why assigning the held pointer is unsafe

`inventory::action` 0x2FCAA0 checks item category +0x24C. Artefacts are category
1, which calls their activation method. Category 2 selects a held object
through a separate active-item interface at item minus 0x10. Artefact
constructors do not establish that interface.

The player's current and desired active objects are +0x128 and +0x12C.
`base_player::tick_active_object` 0x2D7650 calls the current object's virtual
+0x2C without a null guard at 0x2D767F. It negotiates readiness/deactivation,
updates animation objects, activates the target, and checks continued
usability. `process_quick_slots` also calls active virtual +0x70 and reads
active +0x0E. Neither a raw artefact pointer nor a null pointer is a valid
replacement for a living player's active object.

`set_new_active_item` 0x2D8210 only writes the desired pointer; it does not
construct an interface, register an object, or validate its lifecycle.

The current and desired objects are serialized as bytes from their +0x0C
IDs (0x2DB15x and 0x2DB196). Deserialization at 0x2DB626..0x2DB654 resolves
those IDs through world +0x18B4C. A patch-local pointer or per-player array
therefore cannot stand in for a registered native active object. Its
registration, reconstruction and teardown must be explicit.

The registry's concrete layout and construction are now traced:

- World +0x18B4C/+0x18B50/+0x18B54 are begin/end/capacity-end pointers.
  The constructor 0x2CFFF0 initializes their storage at world +0x18B58
  with 0x3FC bytes: **255 active-object pointers**, leaving ID 255 for none.
- Append 0x2D4540 takes ESI vector header and a stack pointer to the pointer
  being appended, `ret 4`. It checks capacity through the engine assertion
  path but does not reliably refuse the write after an ignored assertion.
  A typed registration owner must reject a full table itself.
- Weapon registration 0x2E30F0 takes ECX inventory subobject and stack world,
  `ret 4`. It appends ECX minus 0x10, stores world at active +4 and item
  +0x248, and stores `(end-begin)/4 - 1` as the active object's byte ID +0x0C.
  The Unicorn test now executes this registration and verifies two IDs,
  pointers and world references. Do not call it on an empty-hands object:
  its inventory-specific +0x248 write is outside that smaller allocation.
- Gather-rule setup 0x313F50 registers batteries through the same append
  at 0x313F8B and writes their world/IDs. World teardown 0x2D0330 resets
  end to begin. The registry does not own an intrusive resource reference.

There is a legitimate native `empty_hands` implementation, vtable
0xA0BA6C, constructed by the lobby resource path at 0x259245. Its constructor
0x299830 takes EDI destination, stack animation-pointer array and count,
`ret 8`; the object is 0x168 bytes before external animation-array storage.
It sets ID 255 and is not registered by the constructor. Its set-user
0x299900 records the player and copies the player's transform. It has real
selected-animation and bone-computation implementations (0x299930 and
0x299B00), but selected-animation indexes the supplied animation array;
constructing it with a null/empty array is not a valid shortcut. It does
not by itself provide an artefact model or a holding pose. Reusing its
implementation requires tracing and loading suitable animation resources
for a match, supplying registered identity and snapshot state, and adding
the artefact's model attachment. It is a better starting point to
investigate than modifying battery objective objects, but is not yet a
proven complete carry adapter.

The portable core constructor 0x2FEBC0 creates animation-selection and
animation-object machinery. The battery has its own take/carry/put states,
hand expressions and serialized carried-item representation. Reusing a
battery pointer as an artefact risks objective-rule ownership and battery
serialization; that reuse has not been established as safe.

## Integration requirements still to resolve

1. Establish a dedicated carry adapter's active-object ABI, world
   registration, lifetime and snapshot reconstruction. Obtain the artefact
   model and first-/third-person hand attachment through existing resource
   and animation systems. The native inventory artefact wrapper alone is
   not an active item.
2. Encode Shift + quick slot in deterministic input without also activating
   the artefact or changing equipment presets. Chain `input_builder.end`
   with Blackout filtering; do not read keyboard state from simulation.
3. Persist an explicit offering state and the previous active selection.
   Cancellation, weapon selection, death, disconnect and rollback must
   remove the adapter and restore normal ownership/selection exactly once.
   Initially allow offering passive artefacts only; active effects need an
   explicit gameplay decision before crossing owners.
4. Use the engine's line-of-sight/range selection and normal Use interaction
   with a clear prompt. `detect_usable_object` 0x2D84C0 raycasts and returns
   a usable-object interface; `apply_new_input` 0x2D88C3..0x2D898F starts Use
   on a rising edge after calling its can-use virtual. No remote transfer
   should be possible through walls or by client-only targeting.
5. Serialize contested takes by simulation order: the first successful
   ownership transition consumes the offer; later attempts see no offer.
   Recheck donor identity, offer generation, item ownership and recipient
   free capacity at the single commit point. The transfer guard owns the
   temporary resource reference and must never expose a half-transfer.
6. Cover active/passive modifier transfer, full/no containers, cancellation,
   death, disconnect, competing takers, repeated Use, snapshot restore and
   replay. Add a `next` simulation-change row with the actual gameplay
   implementation. These research-only changes do not change a match.

HUD QuickSlots/QuickItemRenderer and the native quick-slot description are
deliberately outside the anticipated hook surface; the separate consumable
activity-ring work owns those areas.

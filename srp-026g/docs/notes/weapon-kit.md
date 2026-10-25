# The weapon kit: six upgrades on a weapon in three records

**Version scope:** client 0.26g (build 2777), srp's lobby and match servers, client patch 68
(`../../client-patches/weapon_kit_026g.asm`, `weapon_kit_026g.c`). Tags: PROVEN (read at the
address or run with Unicorn), [srp] for srp's own choice.

## The ask

The owner (2026-10-25): weapons should hold two decals, a camouflage, a scope, a grip ("hand") and
a muzzle device ("fire podavitel'"), "at least 6 slots there for weapons only". Stock 0.26g gave
weapons no upgrade slots at all; srp's camouflage (patch 44), decal (patch 47) and scope (#153) use
the armour's upgrade records.

## Why three records

- The client keeps at most three upgrade records an item: the item record (0xB4 bytes, built at
  0x2F7C10) holds them inline at +0x6C..+0xB4, its buffer_vector at +0x60; the reader
  (inventory_item_descr 0x1F1350 → 0x1F13D0, the lobby's and a match profile's alike) sizes it
  at 0x1F147F, and more than three is the fatal assert at 0x1F1655 / 0x1F16C4. PROVEN.
- The item config builder (0x2F14C0, six callers) keeps the records' values in a stack array of
  three (0x2F157F..0x2F161F) for its cache key's checksum. PROVEN.
- Growing the record is out of reach (its size is a stride at about a hundred sites and the
  profile's item table is inline, profile+0x1568).

## The slots and the record [srp]

A weapon's upgrades are kept by the server in **logical slots**, one upgrade a slot:

| slot | what | on the wire |
|---|---|---|
| 0, 1 | a decal (its value its place) | a record of its own, as now |
| 2 | the camouflage | named by the kit, or the kit's own |
| 3 | the scope (a variant, `weapon_scopes.md`) | the kit's own |
| 4 | a muzzle device (flash hider, brake) | named by the kit, or the kit's own |
| 5 | a grip | named by the kit, or the kit's own |

The account stores them so (no rewrite: slots 0, 2 and 3 are those weapons had before). They go
to the client packed (`survarium::v026g::weapon_kit::pack`, `session::v026g::wire::upgrades_descr`):
the decals of slots 0 and 1 in that order, then one **kit** record in slot 3. The kit is the first
of scope, camouflage, muzzle, grip the weapon wears (its item id and record dict id, as that one's
own record would be); its value:

```text
bits  0..15  the camouflage's dict id (0: none, or it is the kit's own)
bits 16..31  the muzzle device's dict id
bits 32..47  the grip's dict id
bits 48..55  the kit's own logical slot (2, 3, 4 or 5)
bits 56..63  0xA7, the kit's mark
```

So a weapon is sent at most three records whatever it wears. Armour is sent as before.

## The client (patch 68)

- **The config.** item_config_cook::translate_query (0x2F1750) builds an item's config from the
  records its query points at: 0x2F2060 copies the query's {dict id, modifications*, upgrades*,
  dictionary} to [ebp+38h..44h], and the function reads the upgrades' buffer_vector at [ebp+40h]
  twice only, its size at 0x2F1799 and each record's dict id and value in its loop
  0x2F18D0..0x2F195A (each record: its item's `parameters.modification`, the value as the
  modification's `<low_param>`/`<high_param>`, 0x3584E0). PROVEN. Patch 68 jumps from its `call
  0x2F2060` (0x2F1771): when a record is a kit, a list on translate_query's own stack (below its
  frame, as its allocas; its epilogue takes esp from ebp, 0x2F2041) holds the records and one
  `{0, dict id, 0, slot}` for each dict id the kit names, and [ebp+40h] points at it. At most six.
  A dict id is left out when the dictionary lacks it, when the list holds it already (its
  modification would add a second `camouflage`: the config's duplicate-name assert), or when a
  record that is no kit is in its slot. Without a kit nothing changes (the original's stack too);
  nothing is shared between two cooks.
- **Every copy carries the kit.** The item record is never written: the lobby's inventory and
  kits, a match's static info (its profiles, a late join's), a replay's recorded static info all
  carry the three records as they are. The config builder's cache key checksums the kit's value,
  so each combination is its own config.
- **The lobby movie.** The upgrades to Flash (0x288180) give each record's object `id` and
  `param`; patch 68 (0x2882E3) gives a kit's object `kit_lo` and `kit_hi` (its value's halves).
  `UpgradeWindow.kitUpgrades` makes the window's slots of them: the kit's own item in its own slot,
  each dict id it names as the inventory's item of that id (`InventoryList.getInventoryItemOf`).
- **What a cosmetic is attached to.** setup_inventory_data (0x288520) maps each record's item id
  to its host ({id, host} pairs pushed at 0x2885C9, an alloca of 50h bytes an item) and gives each
  item its `is_attached_to` from them. Patch 68 pushes, after a kit's own pair, one for each dict
  id the kit names: the inventory's item of that id with the lowest item id (as the movie's
  `getInventoryItemOf`). A scoped weapon's camouflage shows worn on it.
- **The preview (patch 48).** Its `apply` calls `weapon_kit_preview_apply` through a dword the
  build writes: the queued change on the logical slots, packed again as the server packs.
- **The client never writes a weapon's record itself**: srp answers a weapon's `46 01` and `46 02`
  with the inventory and the account only (no `83 46 01` / `02`, whose own copy of the record would
  be a fourth, or take the kit off for its own).

## What it changes

- Two decals on a weapon (`WEAPON_DECAL_SLOTS` 0 and 1, the window's `WEAPON_DECALS` 2): visual
  only. Patch 47 bakes both.
- The camouflage on a scoped weapon now travels in the kit: visual only.
- Slots 4 and 5 have no item yet (later steps: flash hiders and brakes, grips).
- No row in `simulation_changes.tsv` for this step: a camouflage, decals and a scope change
  nothing a match computes. A muzzle device's or grip's modification will (its own row).

## Old clients

A client without patch 68 shows a kit's own (the scope, or the camouflage when there is none) and
the decals, not what the kit names (a scoped weapon's camouflage): never a crash, never a fourth
record. The release that ships the server change requires patch 68 (`SRP_MIN_PATCH_REVISION` the
release's revision; the exe and server ship together).

## Not done

- A player holding two items of one camouflage sees the lower item id as the one worn (the kit
  names the dict id, not the item).
- Not run in a game.

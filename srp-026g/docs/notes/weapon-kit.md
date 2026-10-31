# The weapon kit: six upgrades on a weapon in three records

**Version scope:** client 0.26g (build 2777), srp's lobby and match servers, client patch 68 (the
Rust patch `weapon_kit`, `../../client-patches/rust/patches/src/weapon_kit.rs`, over the
`weapon-kit` crate, `../../crates/weapon-kit`). Tags: PROVEN (read at the address or run with
Unicorn), [srp] for srp's own choice.

## The ask

The owner (2026-10-25): weapons hold two decals, a camouflage, a scope, a grip and a muzzle device,
"at least 6 slots there for weapons only". The steps (2026-10-31):

- **A (this):** the kit transport and a second decal on weapons. No muzzle devices, no grips.
- **B:** muzzle devices (flash hiders, brakes; no silencers) in slot 4.
- **C:** grips in slot 5.

## Why three records

- The client keeps at most three upgrade records an item: the item record (0xB4 bytes, built at
  0x2F7C10) holds them inline at +0x6C..+0xB4, its buffer_vector at +0x60; the reader
  (0x1F13D0) sizes it at 0x1F147F, and a fourth is the fatal assert at 0x1F1655 / 0x1F16C4. PROVEN.
- The item config builder (0x2F14C0) checksums the three records' values on its stack for its
  cache key. PROVEN.
- Growing the record is out of reach: its size is a stride at about a hundred sites, the profile's
  item table is inline (profile+0x1568).

## The slots and the record [srp]

A weapon's upgrades are kept by the server in **logical slots**, one upgrade a slot:

| slot | what | on the wire |
|---|---|---|
| 0, 1 | a decal (its value its place) | a record of its own |
| 2 | the camouflage | named by the kit, or the kit's own |
| 3 | the scope (a variant, `weapon_scopes.md`) | the kit's own |
| 4 | a muzzle device (step B) | named by the kit, or the kit's own |
| 5 | a grip (step C) | named by the kit, or the kit's own |

- The account stores them so; no rewrite (weapons' slots 0, 2 and 3 are those they had).
- On the wire (`weapon_kit::Logical::pack`; the lobby's inventory and kits, a match's static info,
  a late join's, a replay's): the decals of slots 0 and 1 in that order, then one **kit** record in
  record slot 3. Its item id and dict id are the kit's own: the first of scope, camouflage, muzzle,
  grip the weapon holds. Its value:

```text
bits  0..15  the camouflage's dict id (0: none, or it is the kit's own)
bits 16..31  the muzzle device's
bits 32..47  the grip's
bits 48..55  the kit's own logical slot (2, 3, 4 or 5)
bits 56..63  0xA7, the kit's mark
```

- A weapon is sent at most three records whatever it holds. Armour is sent as before.
- A record is a kit only in record slot 3 with 0xA7 on top. **Malformed** (refused by the server's
  unpack and by the client, which then builds the item from its records as they are): an own slot
  outside 2..5; a kit naming its own slot; two things in one slot; a name beside a record of the
  same dict id (the config would get that modification twice, its duplicate-name assert); two kits;
  more than three records. Two records of one dict id (the same decal twice) are fine.
- One encoding, one crate: `crates/weapon-kit` (`no_std`, no `alloc`, no `unsafe`, no panic) is
  linked by the servers (`survarium::v026g::weapon_kit`) and built into the client's arena.

## The client (patch 68)

PROVEN in the code, run with Unicorn (`check_weapon_kit_026g.py`); not run in a game.

- **The config.** item_config_cook::translate_query (0x2F1750) builds an item's config from the
  records [ebp+40h] points at: 0x2F2060 copies the query's {dict id, modifications*, upgrades*,
  dictionary} to [ebp+38h..44h]; the function reads [ebp+40h] at 0x2F1799 (the count) and in its
  loop at 0x2F18D0 (each record's dict id and value: its item's `parameters.modification`, the
  value as `<low_param>`/`<high_param>`) and nowhere else before 0x2F1F1F reuses the local.
  - `weapon_kit.query_room` (the one hand-written cave, `weapon_kit_026g.asm`, 16 bytes): the
    site's `call 0x2F2060`, then `sub esp, A0h` (room for six records and their vector below the
    frame, as translate_query's own allocas; its epilogue takes esp from ebp, 0x2F2041), on at
    0x2F1776. Every cook takes the room; nothing is shared between two cooks. A lohook cannot do
    this: its entry restores ESP.
  - `weapon_kit.query_records` (lohook, 0x2F1776): for records holding a well-formed kit, the room
    holds the records (the kit's own valued 0, as its record was before the kit) and one
    `{0, dict id, 0, slot}` for each dict id the kit names that the items dictionary has, and
    [ebp+40h] points at them. No kit, a malformed one, or 0x2F2060's refusal (AL 0): untouched.
- **Every copy carries the kit.** The item record is never written. The config builder's cache
  key checksums the kit's value, so each combination is its own config.
- **The lobby movie.** The upgrades to Flash (0x288180) give each record's object `id` and `param`;
  `weapon_kit.flash_upgrade` (lohook, 0x2882E3) gives a kit's object `kit_own` (its own slot) and
  `kit_camo`, `kit_muzzle`, `kit_grip` (the dict ids it names, 0 none), through the AS3 object
  interface's SetMember (vtable 0x8FC288, slot 5 0x7C6A20; an object of another interface gets
  nothing). The movie (`UpgradeWindow.kitUpgrades`) makes the window's slots of them: the kit's own
  item in its own slot, each dict id as the inventory's item of that id with the lowest item id
  (`InventoryList.getInventoryItemOf`). The movie never decodes the value.
- **What a cosmetic is attached to.** setup_inventory_data (0x288520) maps each record's item id
  to its host ({id, host} pairs, pushed at 0x2885C9, an alloca of 50h bytes an item) and gives
  each item `is_attached_to`. `weapon_kit.attached_pair` (lohook, 0x2885CE) pushes, after a kit's
  own pair, one for each dict id it names (the inventory's item of that id, the lowest item id;
  only while the pairs have room). A scoped weapon's camouflage shows worn on it.
- **The preview (patch 48).** Its `apply` calls `weapon_kit.preview_apply` through the dword at +23h
  of its code, which the build writes: a generated entry (the framework's `entry` kind, aligned to
  16 as a lohook's: rustc's i686 code keeps 16-byte locals with movaps, and the game's stack is
  aligned to 4 only), whose handler applies the queued changes on the logical slots as the server
  applies them (`Logical::put`, `take_off`) and packs them again. Malformed records stay as they
  are, and only dict ids the record held or a change named are written (the cook would read a
  dict id the dictionary lacks from a null entry).
- **The window** (the lobby movie): two decal slots on a weapon (`WEAPON_DECALS` 2), as armour's.
- **The client never writes a weapon's record itself**: srp answers a weapon's `46 01` and `46 02`
  with the inventory and the account only (no `83 46 01` / `02`, whose own copy of the record would
  be a fourth, or take the kit off for its own).

## The simulation

No `simulation_changes.tsv` row: a match computes the same.

- Before the kit, srp sent a scoped, camouflaged weapon with a decal as records (decal, value its
  place), (scope variant, 0), (camouflage, 0). With the kit, the cook gets (decal, place),
  (decal 2, place), (scope variant, 0), (camouflage, 0): the same dict ids with the same values (the
  kit's own valued 0), so the same modifications with the same params (`check_weapon_kit_026g.py`:
  "the modifications and values the four records of before would give").
- Their order may differ; the keys they touch do not meet: the scope variant sets
  `addons.rifle_scope_dict_id` (`weapon_scopes.md`), the camouflage `data.camouflage`, a decal adds
  to `data.decals`, so the order changes nothing.
- The second decal is drawn only: patch 47 bakes it into the weapon's textures.
- Only the config builder's cache key changes (the kit's value): which cache entry, not what it
  holds.

## One client version

The release that ships this sets `SRP_MIN_PATCH_REVISION` to its revision (every release does):
an exe without patch 68 is refused at sign-in, and recordings of older games are not offered
(release.sh). There is no old-client path.

## Steps B and C (not here)

The transport is complete for them: slots 4 and 5, the kit's muzzle and grip fields, their
expansion in the cook, their members to the movie (`kit_muzzle`, `kit_grip`), the attached marks
and the preview carry them already. B adds a muzzle device's content (items, their modification
adding the device's model to the weapon's `data`), its drawing and its slot on the window's
Attachments page, and a `simulation_changes.tsv` row when a device changes a shot (a brake's
recoil). The device is drawn at retail's `muzzle_point` locator (our 0.26g gun meshes are
retail's, so the locator fits unchanged), the gun's built-in device taken out of its mesh, and the
shot's flash moves to the device's own `fire_point`. Not at `barrel_point`: that is 0.5 to 25 cm
off the muzzle, and #420's and #766's devices were drawn there, in the wrong places. C adds the
grips the same way.

## Not done

- A player holding two items of one camouflage sees the lower item id as the one worn (the kit
  names the dict id, not the item). Taking it off works: on a weapon the server takes off the item
  of that dict id the weapon holds (`items_rules::detach_upgrade`).
- Not run in a game.

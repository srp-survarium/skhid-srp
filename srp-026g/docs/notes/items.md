# 0.26g items: modifications, defects, cosmetics, reroll, craft, repair

**Version scope:** client 0.26g (build 2777). Read from its exe, its
`lobby_menu.swf` (decompiled ActionScript) and its `resources.db`
(2026-10-12). Nothing here was tested against a running client. Tags:
PROVEN (read at the address or in the file named), LIKELY, GUESS, and
[srp] for srp's own choice where neither the client nor the data says.

The code: `crates/survarium/src/v026g/modifications.rs` (data),
`crates/session/src/v026g/items_rules.rs` (rules),
`servers/lobby-server/src/v026g/items.rs` (requests and answers).

## 1. The data

| file in `resources.db` | what | who reads it |
|---|---|---|
| `gameplay/items/modifications/modifications.options` | 158 `modifications`, 20 `defects`, 3 `defect_slots` | client: `id`, `modifiers`, `ui_desc`, `lobby_info`, a defect's `fix_cost_general` / `fix_cost_gold` / `fix_time_general` (loader 0x2C9E31.., strings at 0xA02264..). **Server only:** `drop_weight`, `type_mask`, `type_requirements_mask`, `required_modifications`, `defect_slots` (none of these strings is in the exe) |
| `gameplay/items/craft/craft.options` | `craft_price_general[16]`, `craft_price_premium[16]`, `craft_probabilities_general[3][2]`, `craft_probabilities_premium[3][2]` | server only; the lobby gets them in 0x9B |
| `gameplay/items/reroll/reroll.options` | `reroll_price_general[16]`, `reroll_price_premium[16]`, `reroll_modificator_multiplicator` 1.1 (not used: [flat-reroll.md](flat-reroll.md)) | server only; the lobby gets an item's two prices in `83 46 06` / `08` |
| every item config | `available_modifications_mask` (40 characters), `available_defects_mask` (20), `default_modifications[]`, `camo_available`, `decal_available` | client: the last three (0x2C97B7, 0x287703, 0x28777D). The two masks are server only |

PROVEN by the data:

- A modification's `type_mask` (41 characters) has one character set: its
  **type**, 1..=40. Five strengths share a type (ids `xxx0`..`xxx4`) with
  `drop_weight` 10, 8, 6.4, 5.12, 4.096. `type_requirements_mask` equals
  `type_mask` in every row.
- **An item's mask is one character shorter: its character `j` is type
  `j + 1`.** Only so do bolt-action rifles (`1110111…`) get "chamber a
  round" (type 6) and no rate of fire (type 4), automatic ones the other
  way round, and the defects' masks (21 characters against the items' 20)
  agree in the same way.
- Types with `drop_weight` 0 never roll: 11 (weight, unused), 15 pockets,
  16 artefact slots, 17 ammo bags, 29 pocket size, 33..40 (the boosters'
  effects and the premium items' silver, experience and reputation
  bonuses). They are only ever `default_modifications` in the data; srp gives
  the golden weapons their silver one (1230) and the premium armour its
  experience and reputation ones (1240, 1250, 1260) as modifications of their
  own ([golden_weapons.md](golden_weapons.md), [premium_armour.md](premium_armour.md)).
- Three defects have `required_modifications: [low, high]` (4030 pockets
  `[1030, 1039]`, 4040 artefact slots, 4050 ammo bags): the id range of the
  modifications that give what the defect takes.
- Every defect: `fix_cost_general` 100, `fix_cost_gold` 50,
  `fix_time_general` 360, `drop_weight` 10.
- `defect_slots`: `{condition [0.9, 0], probability [0.1, 0.5]}`,
  `{[0.5, 0], [0.3, 0.7]}`, `{[0.1, 0], [0.5, 1]}`.
- A modifier is `{path[], value, type}`: type 1 with absolute steps
  (armour +0.02, a slot +1), type 2 with small fractions (damage +0.01,
  weight −0.05, wear −0.05). LIKELY 1 adds and 2 multiplies by `1 + value`.
  PROVEN (2026-10-23): the match's item cook applies a path's summed
  modifiers once, type 1 by 0x357F80 (an int value, as every item's
  `*_modifier` 0, becomes a float first: +0.05 is kept), type 2 by 0x358010
  (x max(0, 1 + sum)).
- **A gear modification of a player modifier** (`parameters/<name>_modifier`
  on a vest, trousers or backpack): the cooked config's 68 `_modifier`
  floats (0x2C83E0 from the inventory_item ctor 0x2FFB80) -> armor::armor
  0x33EAA0 makes a node of each nonzero one -> 0x33F3A0 links them into the
  kit's lists -> the getter 0x2D5AE0. Run with Unicorn on the original exe
  (T16, 2026-10-23) for type 21, Regeneration (1090..1094,
  `health_regeneration_speed_modifier` +0.05..+0.15): index 2's only reader
  is the pool update 0x32A4B0, speed = the pool's `regeneration_speed` x
  (1 + sum) (index 2 is not in the additive table 0x927E08), so +5..+15 % of
  0.03/s (body, head, arms, legs; pain 0.1/s), with the same 8 s timeout and
  cap (body 50 %). Body 0 -> 50 %: 16.7 s, 15.9 s at +5 %, 14.5 s at +15 %,
  11.5 s with three +15 % pieces. It works as its card says; it is small.
- 47 items (every vest and every pair of trousers) have `camo_available`
  and `decal_available`; `items_compatibility` pairs no cosmetic with
  anything.

## 2. An item on the wire (corrects lobby.md §2.6, static_info.md §3.2)

`n1 × {u32 id, bool state}` — **state 0 is a modification, anything else a
defect.** PROVEN: the UI fill 0x229A40 looks the id up in the dictionary's
modifications table for state 0 (`+0x140`, count `+0x144`, 0x229F0D) and in
its defects table otherwise (`+0x148`, count `+0x14C`, 0x229AA0), 0x2C
bytes a row, and **reads the row it did not find in both cases** (`xor
esi,esi; mov eax,[esi+4]` at 0x229AC2). So every id must exist in the
table of its kind. (The earlier note had "state 0: no lookup"; wrong.)

- The lobby shows a state-1 entry as damage (`MessageRepair`, "critical
  damages", with its `fix_*` values), counts the state-0 entries as the
  item's **rank** (`InventoryList.getItemRank`; the exe's `rarity_level =
  min(3, base_rarity_level + count)`), and lets only those be changed.
- `default_modifications` are **not** sent: the client adds them from the
  item's config (`modifiersDefault`, `isDefault`).
- The match client applies an item's modifications to the item's config by
  their `path` (strings `modifiers`, `path`, `modification` used at
  0x2F1978..0x2F1A9A). LIKELY: nothing but the ids is needed from the
  server.

`n2 × {u32 upgrade_id, u32, u64, u8 param}` — PROVEN from the attach
handler (0x275671..0x2756A1), which builds the record: **`upgrade_id` is
the item id of the cosmetic** (it stays an item of the inventory: the
upgrade window finds it there, `UpgradeWindow.create_item`), **the second
u32 its dict id** (`movzx eax, word [item+4]`), **the u64 the value the
decal editor sets** (`46 05`), **`param` the slot**: 0 and 1 for decals
(the lobby shows two for `decal_available`, `and esi, 2` at 0x2877A6), 3
for the camouflage (`DragRegion.as`: `relocate_upgrade_item(item, camo,
3)`). Three at most (the inline capacity): one camouflage and two decals. A
weapon's (srp's, client patch 68) are sent packed into those three: the
weapon kit, `weapon-kit.md`.

`repair_time` and `reroll_time` are **seconds left** (PROVEN in the UI:
`InventoryItem.updateRepairTime` counts `repairTime - delta` down with its
own timer; `ItemControlPanel` shows `rerollTime / 60` minutes).

## 3. Requests and answers (corrects lobby_requests.md)

| request | body (PROVEN) | Flash call |
|---|---|---|
| `46 01` | u32 active kit, **u32 host item, u32 cosmetic item, u8 slot**, u64 0 | `relocate_upgrade_item(item, upgrade, slot)` (0x29A984: args 0, 1, 2 in order) |
| `46 02` | u32 active kit, u32 cosmetic item; **u32 host item** with patch 30 (§6) | `drop_upgrade_item(item, upgrade)`: the stock exe sends only the second (0x29AAD5 → 0x2708C0; 0x29A9E1 reads the first for a log line) |
| `46 05` | u32 active kit, **u32 host item, u32 cosmetic item**, u64 value | the decal editor 0x263950; the client then finds the host and the cosmetic's record and writes the value itself (0x270A1A..0x270AC6) |
| `46 06` | u32 item | `item_reroll_opened` |
| `46 07` | u32 item, u32 modification id (`item.mods[index]`), **bool gold** | `reroll_item_modifier(item, index, money type)` |
| `46 08` | u32 item, u32 old modification id, **u32 the place of the choice (0..2)**; srp's window sends 3 to keep the old one ([reroll-keep-old.md](reroll-keep-old.md)) | `modifier_choosed(item, index, modifiersList2.selectedIndex)`: the third is an index into the three offered, not an id |
| `46 09` | **u32 item, u32 donor item, bool gold** | `craft_item(slot1.itemId, slot2.itemId, premium)` |
| `47 03` | u32 item, u8 kind, u32 kit item if kind 2 | as before |

`83 46 <ev>` (0x275520):

| ev | body | notes |
|---|---|---|
| 01 | u32 host, u32 cosmetic, u8 slot, u64 value, bool detach_first | `detach_first` takes **the cosmetic** off the first item of the inventory holding it (0x275589 → 0x275220 with the second u32), not the host's upgrades; with patch 30, off the host (§6). Both items must be in the inventory or nothing is done; with `detach_first` 0 the cosmetic stays on any other item holding it |
| 02 | u32 cosmetic; **u32 host** with patch 30 (§6) | the stock client takes it off the first item of the inventory holding it (find_if 0x277CF0); patch 30 off the host named |
| 06 | u32 item, **u32 regular_cost, u32 premium_cost**, bool more; if more: **u32 the modification changed before, u32 reroll_time** | 0x2757CC → 0x29F430 → `root.fill_item_reroll_data {reroll_time, regular_cost, premium_cost, modifiers[]}`; each modifier gets `was_rerolled_before = (id == that u32)` and `reroll_available = state 0 and (none changed before or this one)` (0x29FC8D..0x29FD1A). The item must be in the inventory (no check, 0x29F58B) |
| 07 | u32 item, 3 × u32 modification, u32 reroll_time | each is looked up as a modification (0x29F680 → 0x229FD0): an unknown id crashes. `item.reroll_time` is set |
| 08 | u32 item, u32 old, u32 new, **u32 regular_cost, u32 premium_cost, u32 reroll_time** | the client replaces old by new in the inventory and the kit copies, then fills the window again with `new` as the one changed before (0x2759F0..0x275A16) |
| 09 | u8 mode, u32 item; **mode 0xFE ends here**; else u32 consumed item; if mode ≠ 0: u32 modification | 0x275A31: for 0xFE nothing more is read (the earlier note had a modification id; wrong). Mode 1 adds, another non-zero mode removes. The window shows **the consumed item's dictionary entry** as the upgraded item (0x275B84 → 0x29F770 `dict_id`) and the modification from its table |

`83 47 03`: as lobby_requests.md §4. The client sets the condition to
full and `repair_time`; it does **not** take the defects off its copy:
send the inventory after it.

Important message type 6 (`social.md` §1.4): the text is
**`#ir[<item id>]`** (PROVEN: prefix check 0x226851, then the bracket
parser 0x227120 that the quest texts use, `[` 0x9DB57C and `]` 0x9ECA34;
the client finds the item and sets its `repair_time` to 0, 0x22693A).

0x9B (PROVEN in the UI, `MessageCraftItem.updateCraftChance`): the twelve
floats are four for each difference in rank between the item and its donor
(0, 1, 2): **success in silver, failure in silver, success in gold, failure
in gold**, in percent; the two u16 tables are the prices by **item level**.
The window has no values of its own: without 0x9B it shows nothing.

## 4. The rules

What the lobby checks or says itself (PROVEN, `lobby_menu.swf` and the
0.26g localization):

- Reroll: "You can change only one modifier on an item but an unlimited
  number of times. When you change the modifier you will be prompted to
  choose one of three random modifiers." "Only one attempt in 6 hours is
  available when paying with silver." Silver is disabled while the item's
  `reroll_time` runs; gold never. [srp, the owner's rule of 2026-10-22] No
  wait: srp sends `reroll_time` 0 and the window shows no six-hour line.
  [srp, the owner's rule of 2026-10-22] Any modification, not only one:
  the window locks none ([reroll-keep-old.md](reroll-keep-old.md)).
  Not for a damaged item
  (`st_reroll_message_broken_item`). The window can not be closed between
  the offer and the choice. [srp, the owner's rule of 2026-10-21] Its Cancel button
  then keeps the old modification ([reroll-keep-old.md](reroll-keep-old.md)).
- Craft: both items not damaged and not in repair; both with fewer than
  three modifications; the item with no fewer than the donor
  (`st_craft_message_modifiers_difference`); not an equipped item while in
  the queue. Price = table[item level of the item]. Outcomes: "+1 mod.",
  "-1 mod.", or "you failed to upgrade the item". [srp, the owner's rule of
  2026-10-23] Never "-1 mod.": a failure leaves the item as it was
  ([craft-menu.md](craft-menu.md), "A failed upgrade").
- Repair: silver = `(1 - condition) * repair_cost_ratio * cost` + each
  defect's `fix_cost_general`, and takes the defects' `fix_time_general`
  together; gold ("quick repair") = the defects' `fix_cost_gold`; kits = one
  `repair_voucher` a defect. Gold and kits are offered only with defects.

srp's reading of what only the server knew:

| rule | tag |
|---|---|
| an item gets modifications of the types its mask names, one of a type, by `drop_weight` | LIKELY |
| a dropped weapon or armour of rarity `n` gets `n` modifications | LIKELY (the rank shown is their count) |
| a reroll offers three that fit in the replaced one's place (its own type too, another strength): of different types while the item has them, then other strengths of those, so an item of few types (a backpack, a balaclava) is offered three too | [srp] |
| the payment is taken when the three are offered, whatever is chosen, the old one kept too; an offer not chosen is kept (saved) and shown again when the window opens | [srp] |
| every change of an item costs the same: the level's price, `reroll_modificator_multiplicator` (1.1) not used (until then each change made the next 1.1 times dearer, a GUESS) | the owner's rule of 2026-10-25 ([flat-reroll.md](flat-reroll.md)) |
| the donor must be another of the same item | LIKELY (the result window shows the donor's dictionary entry as the upgraded item); a golden weapon and its plain weapon count as the same item ([srp], the owner's rule of 2026-10-21, [golden_weapons.md](golden_weapons.md)), and so do a premium armour piece and its plain piece (2026-10-22, [premium_armour.md](premium_armour.md)) |
| the craft's chance row is `rank(item) - rank(donor)`; success, then failure, then nothing | PROVEN for the row (the UI indexes it so); the roll is the server's |
| a gained modification is rolled as a drop's | [srp] |
| a failure takes no modification: it does what "nothing" does (price and donor spent, the item as it was); 0x9B sends every failure chance as 0 and the window shows no failure row | the owner's rule of 2026-10-23 ([craft-menu.md](craft-menu.md)) |
| no roll (a drop, an upgrade, a reroll's offer) gives a modification that only changes wear ("reliability"); owned ones stay; masks may roll Armor, oxygen packs Weight limit and Energy costs, the Pilgrim container Energy costs, so every item rolls three types or more | the owner's rule of 2026-10-23 ([no-reliability-roll.md](no-reliability-roll.md), `third-mod-1.json`) |
| a repair takes the defects off at once; the wait only locks the item in the lobby | [srp] |
| a defect slot opens under its first `condition`; its chance goes from the first `probability` there to the second at condition 0 | LIKELY |
| one defect roll for each match that wore the item, for its first free slot | [srp] (`SRP_ITEM_DEFECTS=off` switches defects off). Since 2026-10-13 a match wears nothing unless `SRP_ITEM_WEAR=on` (the owner's choice, `results.md` §12), so only a found item has a defect |
| a defect needs its type in `available_defects_mask` and, with `required_modifications`, such a modification on the item (a default counts) | LIKELY |
| wear is multiplied by `1 + value` of the item's "reliability" modifications | LIKELY |
| the kit's items' `money_bonus` / `experience_bonus` / `reputation_bonus` (modifications 1230..1280, defaults of the premium items) add that share to a match's pay, shown in the screen's "premium items" column | LIKELY for the meaning; that the three bytes of the column are percents is GUESS |
| any camouflage goes on any vest or trousers | [srp]: the data pairs none |
| a weapon's decal in slot 0 (one, with `decal_available`: the client keeps three upgrades an item), its camouflage in 2, its scope in 3 | [srp] (`weapon-camo.md`; client patches 44, 45 and 47 bake and place them) |
| one cosmetic (camouflage, decal) is worn by any number of items at once, each with its own slot and value | [srp], the owner's rule (2026-10-16), with patch 30 of the client: §6 |

## 5. Not done

- Whether an item in repair may be played with: srp lets it [srp].
- `fill_craft_data {craft_time, …}` (`MessageCraftItem.fillCraftData`): a
  craft timer the window has a clip for; no message that sets it was
  found, and srp crafts at once.
- The camouflage's u64 (the decal editor serves both): kept as sent.

## 6. One cosmetic on many items (2026-10-17)

The owner's rule: a player who owns one camouflage or decal can have it on
any number of items at once, and it is one item of the stash. The same is
meant for weapon attachments later: nothing below is of the cosmetic's
kind (any upgrade, any slot).

What the stock client does with one id in the upgrades of several items
(read 2026-10-16/17 in the exe and the retail `lobby_menu.swf`):

| what | PROVEN at |
|---|---|
| each item keeps its own records `{upgrade id, dict id, u64, slot}` (+60h: begin, end, capacity; 3 inline); nothing in the inventory model forbids one id in several items' records | 0x275671..0x2756F1, 0x276EB0 |
| the full inventory (`0x89`) builds every item from its own records; `is_attached_to` comes from a list of (upgrade, host) pairs made from all of them, the first pair of an id winning: the stash shows the cosmetic once, as worn | `fill_inventory_contents` 0x288520 (pairs 0x28B0F0, room for ten an item; search 0x28865A) |
| the upgrade window of an item fills its slots from that item's records, each with the inventory item of the id (`create_item`): every item shows the cosmetic | `UpgradeWindow.updateData`, `create_item` |
| the decal editor works on the item it is opened for: start (0x2636B0) finds the host by its id and the cosmetic's record in the host's records; its place (`46 05`) names the host | 0x29A830 → 0x2636B0, 0x263950 → 0x270950 |
| a match takes each item's camouflage from its own record (`[record+4]`, the dict id) | 0x2F18D0..0x2F18FA |
| `83 46 01` with `detach_first` 0 adds the record to the host and leaves the others | 0x275641..0x275747 |
| **`46 02` names no item**: `drop_upgrade_item(item, upgrade)` (the window's drop and cancel, and `on_item_sold`) sends `46 02 <kit> <upgrade>` | 0x29A9E1 (`[eax+8]`, the item, only logged), 0x29AAD5 → 0x2708C0 |
| **`83 46 02` and `detach_first` take it off the first item holding it**: `process_detach_upgrade` sets the cosmetic's `is_attached_to` to 0, then `find_if` over the inventory | 0x275220..0x2753A1, 0x277CF0 (its only call, 0x27534D) |
| `on_item_sold` runs for a sold camouflage (category 25) that is worn: `drop_upgrade_item(is_attached_to, camo)`, after the sale's answer | 0x27654B..0x2765DB → 0x228FF0; `LobbyMenu.on_item_sold` |

So the client holds one cosmetic on several items as it is; only taking it
off one of them is ambiguous. **Patch 30** (`client-patches`,
`shared_upgrades_026g.asm`, 181 bytes at `.srp`+700h, four sites; `client_patch.md`
§3.28) makes `46 02 <kit> <upgrade> <item>` (the item the window shows),
`83 46 02 <upgrade> <item>` take it off that item alone, and `detach_first`
take it off the host of the answer (it was in another slot of it). No
change of `lobby_menu.swf`.

The server (`session::v026g::items_rules`):

- `46 01`: the cosmetic goes into the slot of the host; what was in that
  slot comes off the host; every other item keeps what it wears. Already
  in that slot: nothing changes (its value kept). In another slot of the
  host: it moves (`detach_first`), its value (a decal's place) with it, in
  the answer too. Only the kits wearing the host get new revisions. (The
  lobby movie of a drag from slot to slot sent `46 02` for the slot left
  after the `46 01`, taking it off; the movie no longer does, 2026-10-21.)
- `46 02 <kit> <upgrade> <host>`: off that host alone. A cosmetic that is no
  item any more (the `on_item_sold` that follows a sale) is off: `83 46
  02`, not a refusal. A host that does not wear it: refused, then the
  inventory.
- `46 05`: the host's own record (each item its own decal place).
- A cosmetic that goes (sold, used up as a donor) comes off every item
  (`drop_gone_upgrades`), and is paid once. No new item is made for any
  of this: the stash has the one cosmetic.
- Persistence: as before (each host's records in its `upgrades` column);
  any account loads unchanged.

A game without patch 30 sends `46 02` without the host. Until 2026-10-17
the lobby could not read it and refused it (`84 46
st_cannot_relocate_item`, "неможливо перемістити ...", seen in a local test
with a weapon scope); now it reads host 0 and takes the cosmetic off the
first item of the inventory holding it, which is what that game's own
`83 46 02` reader does (find_if 0x277CF0; the host after the cosmetic is
not read), so the two agree. With the cosmetic on several items that may
not be the item the window showed; patch 30 names it. The release asks for
the patch's revision (`SRP_MIN_PATCH_REVISION`), so such a game does not
sign in to the players' server.

**Weapon attachments** would use the same records and the same patch, but
the client has no window for them: `UpgradeWindow` has the three decal
slots and the camouflage slot only (`configUI`), `lobby_menu.swf` has no
attachment UI, and what a match does with a record of another kind is not
read. That is a Flash change and more reading, not part of this.

Not yet seen in a running game: the stash with one camouflage on several
items, the upgrade window's drop on each, and a match showing each item's
camouflage.

**Buying an item owned already** (the owner, live rev40, 2026-11-01: "when I buy camo, I don't
get warning that it was already bought"; "it shouldn't just be camo, it should also be scopes and
other things"; "in shop it shouldn't, it should only skip from rmb on item in your inventory, since
there it is a voluntary action"):

- Every buy in the lobby goes through `LobbyMenu.try_buy_item` (`GangShop.buyFunction`,
  `MainScene.buyFunction`). An item owned already that is no stack is asked about there first,
  with the stock box "This item is already purchased by you. Would you like to buy anyway?"
  (`st_already_in_inventory`): OK opens the buy dialog, Cancel buys nothing.
- Not asked: Buy chosen from the right-click menu of an item the player owns, an inventory row or
  a kit slot (`ItemCard.buyFromOwnMenu`, `try_buy_item`'s `ownItemMenu`): the player's own wish
  for another, a stock copy of a modified item too (`buy-modified.md`).
- Asked: everything else. The shop tab (card Buy, double click, right-click Buy on an entry,
  a drop on the character), the card's Buy or a double click on an owned row, and the craft and
  attachment windows' Buy (`InventoryList.buyFromCraftMenu`).
- Never asked: a bundle (its id is no item's) and a stack (ammo, medkits add to theirs).
- What lacked the box until 2026-11-01:
  - a camouflage, from anywhere: the stock movie left camouflages out (LIKELY: a copy was worn by
    one item then);
  - the card's Buy, a double click and the windows' Buy on an owned row: revision 25 asked only for
    a buy from a shop entry (`sourceSlot == SHOP`).
- `ItemCard.acceptAction`, a second buy with its own check that nothing called, is gone.
- A second camouflage or decal is of no use: one copy is worn on any number of items (above).
  Weapons take the same category 25 items (`weapon-camo.md`).
- Ownership comes from the inventory the movie has (`InventoryList.hasItem`). The server buys as
  before.
- Check: `scripts/buy-owned/check.sh <lobby_menu.swf>` (Ruffle).

## 7. The lines of an item's card (2026-10-18)

The lobby's item hover card shows text the data keeps, not values it
computes (read in the exe and patch 20's `lobby_menu.swf`):

| what | PROVEN at |
|---|---|
| an item's lines are its `ui_desc/props_list` entries `{prop_id, prop_value}` (a string, the card's text), sent to the movie in file order, all of them | 0x286F87..0x2871AD |
| what a line is: `gameplay/static_game_parameters.options` `ui_properties`, one entry an id (57 in 0.26g, ids 1-24 and 101-133); `prop_id`, `min_value`, `max_value` read always, `prop_icon`, `prop_name` (a localization key), `description`, `postfix`, `comparable`, `direction`, `mod_type` when there | 0x2211E0..0x22174E |
| the movie keeps them in a dictionary by `prop_id` (`PropertiesDescriptor`); an unknown id is a stack trace and an empty line | `getPropertyDescription` |
| the card (`InventoryItemHint`, sprite 1547) draws the `comparable` lines in its `itemCompare` (`SrvItemCompareHintG`, sprite 1538, a dynamic `ItemCompare`): one row each (`diff_1`..`diff_8`, sprite 1537, 22.6 px apart) in `props_list` order, and stops at the last row | `ItemCompare.updateData` |
| the card's height follows `itemCompare.height` | `ItemInfoRenderer.set data` |
| a row's bar runs over the line's `min_value`..`max_value` (the damage line 0 to 150); the number is the real value, the bar full above the top (until 2026-10-25 CLIK's `StatusIndicator.value` clamped the number too: the VSSK's 175 damage read 150); a line's text is made a number before its modifications are added | `DiffIndicator`, `ItemCompare.updateData` |

Every 0.26g weapon has eight comparable lines (damage, armour piercing,
aiming time, stopping power, dispersion, distance, rate of fire, weight),
so a ninth needs a ninth row. Weapon balance 1 adds the magazine: line 134
(`st_inv_magazine_size`, the bullets-in-pack line's keys, 0 to 100), a
`props_list` entry after the rate of fire of each of the 83 weapon cards
(the weapon's `magazine_capacity`), and in the movie a `diff_9` row in
sprite 1538 and `ItemCompare`'s constructor adding `getChildByName("diff_9")`
to its rows when there is one (`scripts/content-edit/README.md`).

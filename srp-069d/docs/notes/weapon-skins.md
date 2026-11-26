# Weapon skins (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Status words: PROVEN = read from the exe / lobby movie / game data (address or file given); LIKELY = inferred; CHOICE = a
server rule no source states; OPEN = unknown, nothing invented. Nothing here was run against the real client.
VAs: exe base 0x200000000. Code: `crates/game-data/src/skins.rs` + `data/skins_069d.json` (`crates/data-extract`),
`servers/lobby-server/src/skins.rs`, `workshop.rs` (B3), `crates/accounts` (schema v15 `skin_unlocks`, v18 `item_skin_base`).

## 1. What 0.69d has (PROVEN)

* A skin is a dictionary weapon item. 115 weapon rows have a `base_item_dict_id` different from their own id; 40 of them carry
  `parameters.skins_to_unlock`, a list holding the base weapon (always the row's own base: `sr2m_premium` 1240 -> `[1225]`,
  `glock_17` 1155 -> `[1156]`, `neostead` 1213 -> `[1214]`, `mauser_camo` 1360 -> `[1250]`). The other 75 variants (event
  weapons: `*_newyear`, `*_hw2016`, ...) have no list and are no skins. The dictionary reader stores the list as a `u16`
  vector at item `+0x268` and `base_item_dict_id` at `+0x25C` (0x2002DA64D).
* `45 11 u32 item, u16 skin` is `change_weapon_skin` (handler 0x20028D530: two Flash arguments, item uid and skin dict id),
  `45 12 u32 item, u16 skin` is `preview_weapon_skin` (handler 0x20028E360, log "process_preview_weapon_skin %d %d").
* The weapon context menu (AS `WorkshopWindow.skinSelected`, `ItemFactory.fillContextMenuSkinItems`) lists skins; a skin that is
  `enabled` sends `change_weapon_skin(weapon uid, skin dict id)`; a locked one opens the shop on the item's category (the skin
  is bought as the variant weapon: `canBuy` = the shop sells it); an observe button sends `preview_weapon_skin` (also for locked
  skins).
* The C++ `root.set_weapon_skins_availability(unlocked[], locked[])` (0x2002873E7) splits dictionary items into unlocked and
  locked by whether their dict id is in a `u16` list kept with the workshop view; that list is the LAST field of the B3 push
  (`u32 n, n x u16`, reader 0x200289C08). The `Workshop::unlocked` field of the protocol crate is therefore the list of unlocked
  skin dict ids.
* `83 45 11` and `83 45 12` carry an item record. 0x11 (0x2001904CD): reads the record, finds the item with that uid, logs
  "changing item %d skin from %d to %d" (uid, old dict id, new dict id) and replaces the item by the record: the skin is the
  item's dict id. 0x12 (0x2001906CD): reads a record and shows it in the weapon view when the viewed item has that uid; nothing
  is stored.

## 2. Server rules

| rule | status |
|---|---|
| A weapon of dict `d` has base `b = base_item_dict_id(d)` (or `d`); it can wear skin `s` when `s == b` or `b` is in `skins_to_unlock(s)` | PROVEN data |
| Applying a skin sets the weapon row's dict id to `s`; the +0xC value (the base id) stays; modules, modifiers and slots stay. The match loadout then names dict `s`, so the skin shows in a match like a bought variant | PROVEN mechanism (the record is replaced), in-match look LIKELY |
| Unlock = the account owns an item of the skin's dict id (bought in the shop, dropped, rewarded, or already applied). The unlock is stored and stays when the variant weapon is sold | CHOICE (no source says how the original unlocked or whether it kept the skin) |
| B3 `unlocked` = the base weapon (so the skin can be taken off) + the account's unlocked skins of that base | the base entry LIKELY, rest PROVEN consumer |
| `45 11`: refused silently (no client message exists) when the item is not a weapon, the skin does not fit, or it is not unlocked. Success: `83 45 11 <record>`, then an 87 with a bumped version (the 3D model rebuilds only then) | answer PROVEN, 87 LIKELY |
| `45 12`: any skin of the base, locked or not; `83 45 12 <record with the skin's dict id>`; nothing stored | PROVEN consumer |
| Skin prices | none: skins are bought as the variant weapon through `46 00` at its shop price (`gold` / `cost` of the dictionary) |
| Reward bonus of skins (silver / experience / spare parts of the worn weapon) | the bonus is the one of the weapon underneath, a skin adds none: LIKELY, CHOICE. Evidence for: a skin is named "skin" and listed under the base weapon's own selector, the client replaces only the record's dict id, nothing in the exe or the data computes a worn bonus (the server does). Against: every skin dict carries the premium variant's `default_modifications` (+25 % silver; 1137 / 1155 / 1360 / 1637 also +25 % experience), so a client that read the record would show them. OPEN whether the original paid it. Consequence if the bonus were paid: "an unlock outlives the sale" would let a buy-sell cycle (variant for gold, sold back) give the plain weapon the premium bonus for good, so paying it would need a different unlock rule |
| Real item under a skin | `skin_base` (schema v18, `item_skin_base`): item uid -> the weapon's own dict id while a skin is on. `boosters::worn_bonus` reads the modifications of that real item, `46 01` sells it at the real item's price (a skinned base weapon sold for the skin variant's 60 silver instead of 5000). Taking the skin off, or selling the weapon, removes the entry. A bought variant weapon (its own row, no entry) keeps its bonus and price | CHOICE |

## 3. More exe findings (PROVEN)

* The selector (0x200286220) lists every dictionary item with the same `base_item_dict_id` as the open weapon (the item
  field `+0x294`, 0x200287351; the dictionary reader's base is `entry + 0x38`, 0x2002DB368), including event variants
  (e.g. a New Year SVD); the B3 unlocked list decides enabled vs greyed. So B3 `unlocked` may list every owned member of
  the family; the server lists the base and the account's unlocked skins.
* No client text says skins unlock by kills or achievements (exe strings, movie, localisation searched).
* A refused `45 11` sets no lock and has no reply id: silence is safe. A `45 11` refusal could go through the generic text
  channel `84 x 45 01 <key or text>` (display 0x200176B38, localised by 0x20013E4B0), which would show a system message and
  clear UI locks 0 and 5; no skin-specific key exists, so the server stays silent.

## 4. OPEN

* Whether the original unlocked skins by kills (the UI fields `kills_to_unlock`, `bonus_steps` belong to module slots) or by
  achievements: nothing in the data says so. CHOICE: owning a variant unlocks its own skin; the selector lists the whole
  `base_item_dict_id` family, the rest greyed.
* Refusal feedback for `45 11` (no message known).

## 5. Checks in the game

* Open the workshop (weapon view) of a weapon whose variant you own: the context menu lists the skin as selectable; pick it, the
  weapon's name / model changes and survives re-login; pick the plain entry to take it off.
* A skin whose variant you do not own appears greyed; clicking it opens the shop; the observe button previews it.

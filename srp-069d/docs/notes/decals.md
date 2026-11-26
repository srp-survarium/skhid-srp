# Camouflage and decals (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Status words: PROVEN = read from the exe / lobby movie / game data (address or file given); LIKELY = inferred; CHOICE = a
server rule no source states; OPEN = unknown, nothing invented. Nothing here was run against the real client.
Code: `crates/game-data/src/decals.rs`, `data/decals_069d.json` (made by `crates/data-extract`),
`servers/lobby-server/src/decals.rs`, `crates/accounts` (schema v12 `item_decals`).

## 1. What 0.69d had (PROVEN)

* Items: the `camo_decal` section of `gameplay/game.options` (volume 75) has 280 dictionary items: category 25 = camouflage
  (52 rows), 26 = decals (228 rows). `type` prefixes `stk_` (shop, silver), `prm_` (premium, gold or event), `drp_` (drop),
  `unl_` (unlocked); `camo_decal_type` 0 / 1 / 2. Every row has `default_modifications` (group, grade) pairs: the reward bonus
  groups 123 (silver), 124 / 141 / 185 (experience), 139 / 143 (reputation) and the spare-parts groups 147 / 148.
  Example: dict 718 `char_decal_5_29`: `prm_decl`, 50 gold, `[(124, 6)]`.
* Decal sets: `camo_decal_sets` has 11 sets, each with `items` (two dict ids) and `bonus_tiers` `{items_needed 2, modifiers}`.
* Armour that takes them: only torsos (category 3) and legs (category 5) have `camo_available` / `decal_available` true
  (65 + 65 rows of the `equipment` section). Boots, hands, helmets, masks, backs do not.
* Client: lobby classes `survarium::lobby_character` (`lobby_menu_character_decals.cpp`: preview and bake of camouflage / decal
  textures), Flash `DecorateControl` with `getCamoDecalById`; the editor Flash calls `start_decal_edit`, `cancel_decal_edit`,
  `camo_decals_list_opened`, `exit_decal_edit`; the inventory lists camouflage as "camo" (category 25) and the rest as "decal".
  The results screen has the components `camo_and_decal_bonus_rep / _money / _exp` (strings at 0x95aba8, 0x95ad18,
  0x95ae28): decals / camouflage give a reward bonus that the client shows as its own line on the results screen.
* The record: every item record (89, 87, `83 45 0F`, `83 46 00`) carries `n3 <= 3` entries `{u32 id, u16 param, f64 x, u8 y}`
  (Flash `camo_and_decals {id, param}`), the inline vector holds 3 (reader 0x2001263D0).

## 2. Requests and answers (PROVEN builders / readers)

| request | wire (after `45 <sub>`) | builder | answer |
|---|---|---|---|
| 45 01 `relocate_upgrade_item` | `u32 item (armour piece), u32 id (uid of the decal item), u8 slot, u64 x` | 0x2001875D0 | `83 45 01` `u32 item, u32 id, u8 slot, f64 x` |
| 45 02 `drop_upgrade_item` | `u32 item, u32 id, u8 slot` | 0x2002CABA0 | `83 45 02` `u32 item, u32 id, u8 slot` |
| 45 04 editor save | `u32 item, u32 id, u64 x, u8 slot` | 0x200187740 | none: the builder first sets `x` on the entry (id, slot) of its own copy of the item (0x2001878B4..0x20018794E), then refreshes the UI |

`83 45 01` (reader 0x200190238): both items must exist in the client inventory; the entry of `item` with the same slot byte
is erased first, then `{id, dict id of the decal item, x, slot}` is appended (more than 3 asserts). `83 45 02` (0x200190422):
erases the first entry with the same `id` and slot. The u8 is therefore an entry key (slot), the f64 a placement value the
server never interprets (LIKELY: position / angle / scale packed by the editor; kept as raw bits).
The item-to-owner map the client builds from the n3 lists (0x24A130) is keyed by the decal item's uid: one owner per decal item.

## 3. Server rules (decals.rs)

PROVEN client facts: target = torso / legs with the matching flag; at most 3 entries; same slot replaces; one owner per decal.
CHOICE: a decal item on another piece, or the same item twice on a piece, is refused (the client would show it wrong); a refusal
is silent (the client has no deny for 45 01); attaching and removing are free (no price anywhere in data or client); a
decal item removed by a sale leaves its piece (the piece's record is pushed again); an armour piece that is sold takes its
entries with it (the decal items stay in the inventory).

Buying: decals are dictionary items bought with `46 00`. The faction shop table lists 19 camouflages (silver); other camouflage
and decals are sold at the dictionary `cost` / `currency` when the type prefix is `stk_` or `prm_` and the cost is > 0
(`DecalData::dictionary_price`). LIKELY: the prefixes' meaning; `drp_` (drop) and `unl_` (achievement) items are not sold
here; achievement rewards already hand out decals (achievements.rs).

## 4. Reward bonus

`boosters::worn_bonus` adds, per worn armour piece (an item in a profile slot), the `default_modifications` of every decal /
camouflage attached to it, and for every decal set whose `items_needed` is reached by the distinct decal dict ids worn (over all
worn pieces: CHOICE; the data gives no scope) the tier modifiers. They resolve through `reward_modifiers` to experience /
silver / reputation fractions and are added to premium and booster percentages (LIKELY additive). Test:
`decals::tests::worn_bonus_follows_the_worn_piece`.

Faction reputation pairs: groups 125..128 write `parameters/reputation_bonus_modifier` like group 139, but
their lobby_info / ui names are per faction (`scav_rep_booster`, `bm_rep_booster`, `ren_rep_booster`,
`edge_rep_booster`, props 128..131; +10 / 1 / 5 / 2 % by grade 0..3). About 320 faction armour pieces and 12
camouflages / decals carry one (e.g. 184
`camo_fabric_grey` (128, 1), 163 `camo_denim` (126, 1)). They are not in `reward_modifiers` (keyed on the generic
names); `faction_reputation_modifiers` of `data/progression_069d.json` maps them to factions 1..4
(reputation_leveling.options: 1 scavengers, 2 black market, 3 renaissance, 4 border; PROVEN by the data: the armour pieces with group 125 / 126 / 127 / 128 have `faction` 1 / 2 / 3 / 4), and
`Progression::reward_bonus_for` counts a pair only when the match reputation goes to that faction (the active one, the
faction `play.rs` / `online.rs` credit), so the starter scavenger kit pays +2 % scavenger reputation. LIKELY rule
(the per-faction names; the original may also have paid it with another active faction: OPEN); test
`decals::tests::faction_reputation_camouflage_counts_for_its_faction_only`.

Camouflage and decals in the match itself: the only modification a camouflage / decal row carries is visual
(`data/camouflage` 52 rows, `data/decals` 228 rows); it reaches the match through the armour's n3 list
(`match-modifiers.md`). No camouflage changes a match stat.

## 5. Open / absent

* Spare parts bonus, group 147 `details_bonus_modifier` (built): `gameplay/game.options` `modifications` rows (type 17) give the
  fraction of the grade (0 = 7 %, 1 = 10 %, 2 = 15 %, 3 = 25 %, 4 = 50 %, 5 = 75 %, 6 = 100 %, 7 = 5 %, 8 = 9 %, 9 = 20 %; the lobby
  shows it as "+10 %" from `lobby_info`). The data has 19 decals with (147, 1), one each with (147, 7) / (147, 9), 22 weapons and
  8 decal sets (tier modifiers). Official notes agree (prestige-rank decal +10 % spare parts, 0.66; Keeper decal +10 %, 0.68b;
  Christmas Ornament +10 %, 0.69b). `details` (dict 925) is the spare parts item (`st_details_descr`: spare parts), so the bonus is a
  percentage of the match's spare-parts drop. The percentages of the worn items add (LIKELY, "bonuses are additive") to the
  premium +50 %: `Drops::roll_with_parts_bonus` = `round(base * (1 + premium_factor + percent / 100))`; `boosters::parts_percent` collects
  the worn items, their decals and the decal sets' tiers (the same list as `worn_bonus`); `play.rs` (local match) and `online.rs`
  pass it to the roll. Data: `parts_modifiers` of `data/progression_069d.json` (`crates/data-extract`).
  Test: `game_data::progression` (`item_modifications_resolve_to_reward_bonuses`), `game_data::rewards`.
* Group 148 `details_add_win_bonus_modifier` (one row, value 13, ui "13" without a percent sign): no item, decal, set or workshop roll
  carries it (`drop_weight 0`), so there is nothing to apply and its meaning (LIKELY a flat +13 spare parts for a win) stays OPEN.
  Searched: `game.options` (all `default_modifications`, `bonus_tiers`), the exe strings (only the name 0x9671B8).
* How a decal shows in a match: the match options' item records ARE
  read by the lobby's own item decoder (`inventory_item_descr::deserialize_impl` 0x2001263D0, called from the options profile
  reader 0x200125B0F), so they carry the n3 list (`u32 decal uid, u16 decal dict, 8 placement bytes, u8 slot`), and the
  match's item cook (`item_config_cook::translate_query` 0x200335120) applies each entry's decal `parameters.modification` with
  the 8 bytes (the modification's `<low_param>` / `<high_param>` = the two u32 halves, 0x2005AD960; that the value is the 8 bytes
  is LIKELY). Built: `HumanLoadout::decals` from the stored rows, written into every human's options items. That the
  match draws them is LIKELY (not run). The lobby preview comes from the n3 list.
* Editor rules: from the lobby movie `DecorateControl` / `ListDecorItem`, see section 6.
* The placement value is not an IEEE double (section 6); its meaning stays opaque, the server keeps the 8 bytes.
* Weapon skins (`45 11` / `45 12`): see `weapon-skins.md`. No reward bonus is attached to skins in any data read.

## 6. Editor rules (PROVEN, lobby movie `DecorateControl`, `ListDecorItem`; exe 0x2002484A0)

* Slots: `decalSlot1.index = 0`, `decalSlot2.index = 1`, `camoSlot.index = 3` (`configUI`). A piece therefore wears at most one
  camouflage (slot 3) and two decals (slots 0, 1): the three-entry record cap. The exe sends `decal_available` to the movie as
  the number 2 and `camo_available` as a flag, the movie shows decal slot 1 when `decalAvailable > 0`, slot 2 when it is `> 1`
  and the camo slot when `camoAvailable > 0`; all torsos and legs of the data have both flags, so all three slots show.
* A slot's list holds only its kind: the camo slot offers camouflage items, the decal slots decal items; the list's first row
  is a "remove" pseudo item (sends `drop_upgrade_item`, 45 02); picking an item sends `relocate_upgrade_item` (45 01) with the
  item uid and the slot index.
* Placement: the "put in" button is shown only for a filled slot whose index is not 3, so only decals are placed
  (`start_decal_edit` item, decal uid, slot); camouflage has no placement and no 45 04.
* The 8 placement bytes of 45 01 / 45 04 are not a double: the native editor (0x27A660 -> 0x61A460) packs six floats of its
  transform into 8 bytes. Opaque; stored and echoed verbatim (OPEN what the fields are, not needed).
* Server (`decals.rs`): a camouflage outside slot 3 or a decal outside slots 0 / 1 is refused (`Refused::WrongSlot`). Whether
  the same decal item may stand in both decal slots is not enforced by the movie; the server refuses it (CHOICE, the client's
  item-to-owner map keys by item uid).

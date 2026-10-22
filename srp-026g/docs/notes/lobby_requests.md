# Lobby client -> server requests (0.26g vs 0.21d)

Version scope: Survarium **0.26g (build 2777)** client, compared with **0.21d (build 2010)**. Static analysis only: the annotated 0.26g disassembly and the PDB-labelled 0.21d disassembly. VAs are image-base VAs, as in both dumps. The 0.21d layouts come from `srp-021d/docs/notes/lobby_spec.md` sections 1.2, 2.2, 2.3 and 4.

Tags:
- **PROVEN**: read instruction by instruction in 0.26g. The 0.21d match is cited by PDB symbol and VA where one exists.
- **LIKELY**: the byte layout is proven, but the meaning or trigger is inferred from strings, call sites and Flash names.
- **GUESS**: the meaning is not established.

The server -> client side (0x80..0xAA) belongs to the sibling note `lobby.md`. It appears here only where a request expects a reply.

---

## 0. Conventions (PROVEN)

- **Building a message.** `msg = 0x37A0E0(push id, eax = lobby_client+8)`. The payload is appended to the byte vector at msg+0x2BC (0x7AB30 resizes it). `0x379640(lobby_client+8, msg)` sends it on the UDP match transport, which the lobby now shares (see the `lobby.md` sign-in section).
- **Booleans** are written as `00` / `FF` (`setne;dec` or `neg;sbb`), as by 0.21d `w<bool>` 0x155EF9. Fields marked "raw u8" copy a Flash number's low byte (0 / 1), not 00 / FF.
- **Strings.** `0x26AB90` writes `u8 len` and then `len` bytes, with no NUL. len is `strlen & 0xFF`, so a string of 256 bytes or more is sent with a wrapped length. Nickname and comment strings are cut to 63 chars before sending (0x2A5F30 / 0x28CC10, `min(len+1, 0x40)`).
- **The old `0x21 <type>` query is gone.** Each query now has its own id. The generic query sender `0x26FE20` (al = id) writes `id 00`. Queries with arguments write the same `00` first and then the arguments. **Servers must skip that `00`.**
- **The lobby_client fields used as guards:**
  - `+0x1E57A4` is the match id. It is set by the 0x85 client_state reader 0x270100 for states 1 and 2. **-1 means not in a match.** It replaces 0.21d +0x1C8.
  - `+0x1E57A8` is the order id: the queue / play order, set by the same reader. **-1 means not queued.** It replaces 0.21d +0x1CC.
  - `G55C8`: `[[lobby_client+0x1E55C8]] + 0xB01F4` must be 0 or 4. This is a game-world state check: no world, or a lobby world. [I]
  - `+0x1EA964` is the active profile id (written by the `83 49 02` ack).
  - `+0x1E639B` is the profile count. `+0x1EA998` is the chosen faction.

---

## 1. Sign-in

### 1.1 `0x2E` sign_in. Replaces 0.21d `0x26`. PROVEN
Sender: `lobby_client::connect` 0x26F650, writes at @0x26F69F. 0.21d: `lobby_client::on_connected` 0x19D04F.

| off | type | field | source |
|---|---|---|---|
| 0 | u8 | `2E` | |
| 1 | u32 | session id | lobby_client+0x1E5738: the first dword of a 0x50-byte struct copied (`rep movs` 0x14 dwords) from the login reply. The struct also holds u16 port at +0x1E573C and the host at +0x1E573E. |
| 5 | bool 00/FF | need_shop_items | `FF` when lobby_client+0x1EE5F4 == NULL. That field is the shop-container array, allocated by the 0x95 shop-items reader 0x271650 (@0x2716E9). |

The message is **6 bytes** and is sent as the connect payload of `match_client::connect` 0x379090.

Removed compared with 0.21d `0x26` (11 bytes):
- `just_left_match` now travels in `0x31` (section 2.3).
- `pending_match_stats_id` is gone. `#mstats [N]` sends `0x41 N` directly (section 3).

### 1.2 `0x32`, sent after signed-in `0x80`. Layout PROVEN, meaning GUESS
The 0x80 handler 0x26FAB0 sends it at @0x26FBEE. Before it, the handler:
- stores `u8 client_state` at +0x1E5790;
- calls 0x26FA00;
- calls 0x26C320 (messaging sign-in, `0x4F`).

| off | type | field |
|---|---|---|
| 0 | u8 | `32` |
| 1 | u8 | `00` |
| 2 | bool 00/FF | the second byte of `0x80`, sent back as `!= 0` |

0.21d had no equivalent: after `0x30` the client sent nothing. This is probably the initial-data request. The reply is not established; the candidate is 0x87 profiles/account. The server must accept `0x32` and not drop the connection.

---

## 2. Action requests

### 2.1 Summary table

The "0.21d" column gives the 0.21d message and the PDB symbol of its sender. Status: **=** means same body, **chg** means changed body, **NEW** means no 0.21d equivalent.

| 0.26g id / sub | body after the id (and sub) byte | sender VA | trigger (Flash callback / event) | guard | 0.21d | status | tag |
|---|---|---|---|---|---|---|---|
| `2F` | u32 active profile_id (lobby_menu+0x6A0 -> +0x4620) | 0x26FD80 | `play_button_clicked` 0x28C320 @0x28C512, when the state (0x270660) is 0, +0x1EE6D4 != 1 and the profile count +0x1E639B != 0 | see trigger | `20` play, `set_status_ready_for_match` 0x19D4D6 | = (new id) | PROVEN |
| `30` | u32 profile id ([profiles + idx(+0x4632)*0x704 + 0x6FC]) | 0x26FDD0 | `play_button_clicked` @0x28C3E4, when +0x1EE6D4 == 1 (clan-match mode) and +0x1E56BC != 0. Otherwise the client shows `st_must_be_in_clan_team`. | see trigger | none | NEW | LIKELY (clan-team play) |
| `31` | u32 order_id (+0x1E57A8), bool left_match | 0x2717E0 | `left_match = 00`: `play_button_clicked` @0x28C532 when state == 1 and +0x1E579C != 0 (leave the queue), and `important_message_read` @0x28C76E (leaves the queue before joining a clan team). `left_match = FF`: `network_client::close_current_match` 0x27A790 @0x27A93C, and 0x27C690 @0x27C75A (leaving a match). | none | `27` discard order, `discard_playing_order` 0x19E562, **plus** the `just_left_match` flag of `26` | chg | PROVEN layout, LIKELY flag meaning |
| `45` | u8 language (english 0, russian 1, ukrainian 2, german 3, polish 4, anything else 0). **No leading 00.** | 0x2768A0 | right after the 0x9D reader 0x271340 (case @0x274DAA), when +0x1E6144 (the video-tutorial link string, filled by the 0x9E reader 0x2713A0) is empty | +0x1E6144 empty | none | NEW | PROVEN layout, LIKELY reply 0x9E |
| `46 00` | u32 profile_id, u8 n, n x 16 B {u32 item_id, u16 dict_id, u8 src_slot, u8 dst_slot, u32 count, u32 count_in_inventory} (descriptor writer 0x26E3A0). **`apply_profile_items` (the ammo dialog; PROVEN 0x2A5850):** the list starts with one entry for each occupied slot of the 14 ammo/pocket slots (table 0x9C2598): `{item, dict, src = that slot, dst = 100, count = the slot's count, 0}`, then the dialog's rows `{id (0 for a stack not owned), dict, src, dst, count = rounds in the slot, count_in_inventory = rounds wanted in all}`: first the ammo rows (`AmmoShareLite.relocateAmmo`, src always 100: its `source_slot` is set nowhere), then the pocket rows (`relocateItem`, **src = the item's `descriptor`, which for an item of the kit is the slot it is in** (`LocalPlayerProfile.createItem`: `descriptor = slotId`), 100 when it is 0). So an item already in pocket slot 13 is named `13 -> 13` after the list's own `13 -> 100`: the server must take a source slot that an earlier entry of the same request emptied of that item (srp refused it as `NotInSlot` until 2026-10-12, seen live: the dialog could not be accepted with anything in a pocket). **The exe renumbers the ammo rows' targets** (0x2A5A68..0x2A5A98): a row whose `targetSlot` is one of the 8 ammo slots (table 0x9C2578: 8, 9, 11, 12, 19, 20, 21, 22) gets the next of them in order, the first such row 8, the second 9, and so on, whatever Flash named; a row with no rounds in the slot and none wanted is left out but still takes its turn. The profile id sent is the active profile's (+0x6FC), and the first entries are read from the active profile too. No `47 00` goes with it: rounds beyond the stack owned are the server's to sell, as in 0.21d. | 0x2706F0 | `relocate_item` 0x288D60 (@0x28A26B), `apply_profile_items` 0x2A5850, tutorial 0x2A4A20 | match == -1 | `23 00`, `move_item` 0x19DE6D | = (new id). The count n is u8 (`(end-begin)>>4`) | PROVEN |
| `46 01` | u32 active profile_id, u32 a0, u32 a1, u8 a2, u64 0 (always 8 zero bytes: locals zeroed @0x2707CB) | 0x2707C0 | `relocate_upgrade_item` 0x29A8B0 (log `ui: attach_upgrade`) | none | none | NEW | PROVEN: a0 = the host item, a1 = the cosmetic's item id, a2 = the slot (0x29A984; `UpgradeWindow.as`). items.md §3 |
| `46 02` | u32 active profile_id, u32 upgrade_item_id; u32 host_item_id with patch 30 (items.md §6) | 0x2708C0 (patch 30: `.srp`+700h) | `drop_upgrade_item` 0x29A9D0 (log `detach_upgrade[%u] from [%u]`) | none | none | NEW | PROVEN |
| `46 03` | u32 a0, u32 a1 | 0x270AE0 | `use_drop_card` 0x288D00 | none | none | NEW | layout PROVEN, meaning GUESS |
| `46 04` | u32 item_id (not sent when 0) | 0x274400 | `item_use` 0x288D30 | none | none | NEW | PROVEN |
| `46 05` | u32 active profile_id (+0x1EA964), u32 item_id, u32 a, u64 value | 0x270950 | 0x263950, from the decal / camouflage editor 0x2641A0 (strings `camouflage`, `decals`, `%I64d`). The client then writes the u64 into its own copy of the item (+8/+0xC of the 0x18-byte entry) without waiting for an ack. | none | none | NEW | PROVEN: `item_id` = the host, `a` = the cosmetic's item id, the u64 the decal's placement (items.md §3) |
| `46 06` | u32 item_id | 0x270DC0 | `item_reroll_opened` 0x29FE50 | none | none | NEW | PROVEN |
| `46 07` | u32 item_id, u32 modification_id (= item.mods[arg1], **index not range-checked**), bool (arg2 == 1) | 0x270E30 | `reroll_item_modifier` 0x29FE80 | none | none | NEW | PROVEN |
| `46 08` | u32 item_id, u32 old_modification_id (item.mods[arg1]), u32 **the place of the choice among the three offered** (arg2 = `modifiersList2.selectedIndex`, not an id; items.md §3), or 3 from srp's window: keep the old one (`reroll-keep-old.md`) | 0x270EE0 | `modifier_choosed` 0x29FF10 | none | none | NEW | PROVEN |
| `46 09` | u32 a0, u32 a1, bool (a2 == 1) | 0x270F90 | `craft_item` 0x29FFA0 | none | none | NEW | PROVEN (`MessageCraftItem.onMessageAccept`): the item, its donor item, paid in gold (items.md §3) |
| `46 0A` | (nothing) | inline 0x2A4678 | tutorial state "choose weapon" 0x2A45D0, after `49 02` | none | **`89`**: the state waits for `inventory_content_filled` (raised by `setup_inventory_data` 0x2886F5) to highlight dict 106 and 107 (`premium.md` §5.1) | NEW | layout PROVEN; that an `89` must follow PROVEN; what else the server did LIKELY (gave the two weapons) |
| `47 00` | u16 dict_id, u32 count (a u16 from Flash, in slider steps: a clip of ammo), u8 faction (when Flash gave 0 the client fills in the faction of the first price record of that dict id, 0x288BC0), u8 `00`, u8 target_slot (100 = the inventory, from `buy_ok_clicked`), u32 active profile_id (+0x1EA964). Flash's 4th argument (`_money_type`) is not sent. | 0x270BE0 | `buy_ok_clicked` 0x288BC0, `relocate_item` @0x2892BA | match == -1 | `24 00`, `buy_item` 0x19DF6A | = | PROVEN |
| `47 01` | u32 item_id, u32 amount | 0x272F70 | `sell_ok_clicked` 0x288C80 | match == -1 | `24 01`, `sell_item` 0x19EB16 | = | PROVEN |
| `47 02` | u32 offer_id (arg0), u32 x, f32 0.0 (always: `fldz` @0x271048) | 0x271040 | `buy_shop_item` / `accept_premium` 0x2A5F80 sends x = 0; `accept_premium_victory_screen` 0x2A5FB0 sends x = arg1 | match == -1 | `24 02`, `buy_shop_item` 0x19E054 (u32 only) | chg (+u32, +f32) | PROVEN layout, x meaning GUESS |
| `47 03` | u32 item_id, u8 kind, then u32 kit_item_id **only if kind == 2** | 0x270D00 | `repair_item` 0x29FDF0. kind (PROVEN in the UI, `MessageRepair.onMessageAccept`): 0 silver ("regular fix": `(1 - condition/100) * repair_cost_ratio * cost` plus the defects' `fix_cost_general`; it takes the defects' `fix_time_general`), 1 gold ("quick fix": the defects' `fix_cost_gold`), 2 a repair kit (voucher type 2, one per defect). The gold and kit boxes are enabled only for an item with defects. | match == -1 | none | NEW | PROVEN |
| `47 04` | u32 amount (not sent when 0): the gold to sell; srp's exchange window (`client-patches/exchange`) sends `0x40000000` + the gold to **buy** with silver (`requests::BUY_PREMIUM`) | 0x274390 | `exchange_premium_money` 0x2A5FE0 (sends the u32 it is given) | none | none | NEW | PROVEN layout, LIKELY gold->silver |
| `47 05` | u32 a0, f32 a1, u32 a2, u32 a3, u32 a4, u8 faction (+0x1EA998) | 0x276AE0 | `accept_premium_with_bonuses` 0x28AA90 | none | none | NEW | PROVEN layout |
| `47 06` | u32 a0, f32 a1 | 0x276BF0 | `accept_premium_with_discount` 0x28B0B0 | none | none | NEW | PROVEN layout |
| `48 00` | u8 n, n x {u16 skill_id, u8 points} (taken from 4-byte {u16, u8, pad} elements) | 0x271100 | `apply_skills` 0x2A5580 | match == -1 | `25 00`, `set_player_skills` 0x19E0CE | chg: **no profile_id, u16 skill ids, no perk list** | PROVEN |
| `48 01` | raw u8 branch, raw u8 payment | 0x271270 | `reroll_ok_clicked` 0x2A5740. When branch == 0 the client first switches off `allow_use_two_double_handed_weapons` and `enable_second_weapon`. **Meaning (PROVEN in the UI, `MessageAskReroll.onMessageAccept` and `CharWindow.rerollChanges`, `lobby_menu.swf`):** branch = the skills window's active tab (0 = skills 101.., 1 = 201..), payment = 0 silver / 1 gold (the dialog's two boxes, priced by the two reroll costs of 0x8D). So a reset frees **one branch**. | match == -1 | `25 01`, `reroll_player_skills` 0x19E272 (u32 profile) | chg | PROVEN |
| `49 00` | u32 profile id, string new_name | 0x26FD00 | `profile_rename` 0x2A6060 | none | none | NEW | PROVEN layout |
| `49 01` | raw u8 on (Flash arg0) | 0x273000 | `autobuy_changed` 0x288CD0 | none | `29 03`, `set_autobuy_for_profile` 0x19EB9F (u32 profile, u8) | chg: **no profile id** (account-wide) | PROVEN | **Corrected 2026-10-12, see premium.md §3: the byte is 0..3, bit 0 autobuy, bit 1 auto-repair.** |
| `49 02` | u32 profile_id (profiles[idx].+0x1E6A98) | 0x26FC90 | `profile_changed` 0x2A6010, tutorial 0x2A45D0 | none | `29 04`, `set_active_profile` 0x19D467 | = (new id) | PROVEN |
| `49 03` | string nickname | 0x2730E0 | `request_nickname_validation` 0x2A5D30 | none | none | NEW | PROVEN |
| `49 04` | string nickname (63 max), u32 a1 | 0x274310 | `change_player_nickname` 0x2A5F00 | none | none | NEW | PROVEN layout. a1 LIKELY the rename-ticket item id (see ack ev 4) |
| `49 05` | string nickname (63 max), u8 a1, u8 a2 | 0x274470 | `comment_about_user` 0x28CBE0 | none | none | NEW | PROVEN layout, meaning GUESS (player report/praise) |
| `49 24` | u8 `00` | 0x274510 | `video_tutorial_hide_clicked` 0x28CDF0 | none | none | NEW | PROVEN |
| `4A 01` | (nothing): destroy | 0x274580 | `disband_squad` 0x2A5CE0, tutorial 0x2A4D40 | order == -1 | `2A 01`, `destroy_squad` 0x19ECA1 | = | PROVEN |
| `4A 02` | string name: add | 0x2745D0 | the messaging "membership accepted" path 0x26B940 @0x26BD8C | G55C8 and order == -1 | `2A 02`, `add_squad_member` 0x19ED0A | = | PROVEN |
| `4A 03` | u32 member_id: drop | 0x274660 | `remove_from_squad` 0x2A5D00 | order == -1 | `2A 03`, `drop_squad_member` 0x19ED8A | = | PROVEN |
| `4A 04` | (nothing): leave | 0x2746D0 | `leave_squad` 0x2A5CC0, tutorial 0x2A4D40 | order == -1 | `2A 04`, `leave_squad` 0x19EE04 | = | PROVEN |
| `4B 00` | raw u8 faction | 0x273070 | `change_quest_faction` 0x28E960 | none | `2B 00`, `switch_quest` 0x19EC23 (u32 caller, u32 selected) | chg: the quest swap is gone; a faction is chosen instead | PROVEN layout, LIKELY meaning |
| `4B 01` | u8 step (constant 1..4 at the call sites) | 0x270B70 | tutorial / survival notes: `survival_notes_readed` 0x28CCD0 (2, 3), 0x21D461 (1), 0x24921D (4), 0x2832EF (3), 0x28334E (2), 0x2A52BE (1) | none | none | NEW | PROVEN layout, LIKELY "tutorial step done" |
| `4B 02` | (nothing) | inline 0x281660 @0x2816EA | `skip_tutorial` 0x28CCB0 | none | none | NEW | PROVEN |

### 2.2 Clan requests `0x49 06..0x23`. All NEW, with no 0.21d equivalent

All layouts are PROVEN. Meanings come from the Flash callback names (LIKELY).

Guards are listed per row:
- **O** means order == -1.
- **G** means G55C8.

| sub | body | sender | trigger | guard |
|---|---|---|---|---|
| `06` | string clan_name, string abbreviation | 0x273150 | `reserve_clan_name` 0x2A1E10 | O |
| `07` | (nothing) | 0x2731F0 | `create_clan` 0x2A20A0 | O |
| `08` | string clan_name | 0x273240 | `request_clan_name_validation` 0x2A1950 | none |
| `09` | string abbreviation | 0x2732B0 | `request_clan_abbreviation_validation` 0x2A1BB0 | none |
| `0A` | u32 id, u8 accepted (0/1) | 0x273320 | `important_message_read` 0x28C5E0, important message of type 7 (`st_add_to_clan_request`) | G |
| `0B` | string player_name | 0x2733D0 | sent automatically on `83 49 0F` (invite accepted by server) @0x275F83 | G, O |
| `0C` | u32 a | 0x273580 | `buy_clan_registration` 0x2A2140 | G, O |
| `0D` | (nothing) | 0x273610 | `system_message_clicked` 0x2A2BA0 for confirm ids 0xD7 and 0xDA. Those ids are raised by `cancel_clan_registration` 0x2A2170 and `disband_clan` 0x2A23D0 (0xD7), and `leave_clan` 0x2A22A0 (0xDA). | G |
| `0E` | u8 page: 0 from `clan_window_opened` / `clan_members_tab_opened` 0x2A2B80; 4 from `clan_team_window_opened` 0x2A2CA0 and 0x21D3C0 | 0x273680 | | G |
| `0F` | string player_name | 0x2734F0 | `invite_player_to_clan` 0x2A20C0 | G, O |
| `10` | u8 rank, u8 value (0/1), u8 permission (0 invite, 1 kick, 2 change rank, 3 daily message, 4 battle commander) (PROVEN 2026-10-12) | 0x273710 | `update_{invite,kick,change_rank,daily_message,battle_commander}_permission` 0x2A2570..0x2A2670 | G |
| `11` | u32 member_id, u8 up (1 = promote, 0 = demote) | 0x2737F0 | `rank_down_member` 0x2A2700 sends (m, 0). `system_message_clicked` confirm 0xD8 sends (+0x1E56B0, 1). | G |
| `12` | u32 member_id (also stored at +0x1E56B0) | 0x2738A0 | `rank_up_member` 0x2A26B0 (a pre-check; see ack ev 0x12) | G |
| `13` | u32 member_id (+0x1E56B4) | 0x273930 | `system_message_clicked` confirm 0xDB, raised by `kick_member` 0x2A2730 | G |
| `14` | string rank_name | 0x2739C0 | `clan_create_rank` 0x2A28E0 | G |
| `15` | u8 rank | 0x273A50 | `clan_remove_rank` 0x2A2B50 | G |
| `16` | text converted UTF-8 -> UTF-16 (`MultiByteToWideChar` CP 65001), written by 0x26E210: `u16 count`, then `count` UTF-16LE units (PROVEN 0x26E210: the count is stored as a word, then `count * 2` bytes are copied) | 0x273AE0 | `update_daily_message` 0x2A24D0 | G |
| `17` | (nothing) | 0x273C70 | `create_clan_team` 0x2A2C80 | G |
| `18` | (nothing) | 0x273CE0 | `request_to_join_clan_team` 0x2A2CC0 | G |
| `19` | string player_name | 0x273460 | `invite_player_to_clan_team` 0x2A2CE0 | G, O |
| `1A` | u32 id, u8 1 | 0x273D50 | `important_message_read`, important message of type 0x0A (clan-team invite). It first sends `31 <order> 00` if the player is queued. | G |
| `1B` | (nothing) | 0x273E00 | `leave_clan_team` 0x2A2D60 | G, O |
| `1C` | (nothing) | 0x273E70 | `disband_clan_team` 0x2A2D80 | G, O |
| `1D` | string name, u8 status | 0x273EE0 | `change_status_clan_team_member` 0x2A2DA0 | G, O |
| `1E` | (nothing) | 0x273F90 | `oust_commander` 0x2A2E20 | G, O |
| `1F` | string name | 0x274000 | `remove_team_member` 0x2A2E40 | G, O |
| `20` | bool (00/FF) | 0x274120 | on `83 30` (clan play permitted) 0x276810, when +0x1E56BD == 0: `FF` when +0x1E56BE == 0, else `00` | G, O |
| `21` | u8 0/1 | 0x274090 | on `83 30`, when +0x1E56BD != 0: 1 when +0x1E56BE == 0, else 0 | G, O |
| `22` | string name | 0x2741B0 | `set_another_clan_team_commander` 0x2A2F00 | G, O |
| `23` | (nothing) | 0x274240 | `clan_match_closed` 0x2A2F80 | G, O |

Not found:
- **No sender** was found for `checkbox_tournament_match_clicked` 0x2A2EC0.
- **No 0x49 sender** was found for `invite_to_squad` 0x2A5C40. It goes through messaging `0x26AFB0 "invite"`.

---

## 3. Queries (replacing 0.21d `0x21 <type>`)

Senders:
- The no-argument queries go through the generic `0x26FE20`, which writes `id 00`.
- The others have their own senders, which also write `00` first.

The reply ids are proven only where a 0.21d query matches one-for-one. Otherwise they are LIKELY.

| 0.26g id | body after id | sender / call sites | when sent | 0.21d query | expected reply | tag |
|---|---|---|---|---|---|---|
| `32` | `00`, bool | 0x26FAB0 | after `80` | none | ? (see 1.2) | GUESS reply |
| `34` | `00`, u32 member_id (squad m+0), u32 profile_id (m+4) | 0x26FE70 | `fill squad` 0x227760 @0x227CE5, for every mate whose cached character (+0x4614 / +0x76C) does not match m+8 / m+4 | `21 10 u32 profile_id`, `query_squad_member_contents` 0x19D603 | `88` squad member profile | PROVEN; the extra member_id is new |
| `37` | `00` | 0x26FE20 @0x226B39, @0x226D49 | important message type 0x0D (referral reward) and 0x0E (skill point reward), handler 0x226020 | none (0.21d never asked for skills) | `8C` skills | LIKELY |
| `39` | `00` | 0x26FE20 @0x22625F, @0x226606 | important message type 1 (quest failed, second branch) and type 4 (reputation level unlocked) | `21 09`. Same call order as 0.21d `important_message_arrived` 0x176A5D @0x176BFA / @0x176DE7 | `8E` reputations | PROVEN |
| `3B` | `00`, u32 a, raw u8 b | 0x270070 | `request_players_range` 0x28C960 | `21 0C` (u32, u8), `query_elo_players` 0x19D75F | `91` elo list. srp's own, for its changed lobby movie only: a start of 2 000 000 000 or more is no ranking page (lobby.md §2.16.1: 2 000 000 000 the matches now, 2 100 000 000 + key the JOIN of a match) | PROVEN |
| `3C` | `00` | 0x26FE20 @0x22633D (end of type-1 quest messages); 0x274EED (on `83 4B`) | quest changes | `21 0D` (0x176C9C) | `92` quests | PROVEN |
| `3D` | `00`, u32 a0, u32 a1 (Flash order) | 0x26FF00 | `victory_screen_request_player_data` 0x29ED70 | `21 0E` (u32, u32), `query_player_match_stats` 0x19D672 | `93` player match stats | PROVEN |
| `3E` | `00` | 0x26FE20 @0x226205 (new quest), @0x22627B; 0x274EF6 (on `83 4B`) | quests / services | `21 0F` (0x176BA7, 0x176C09) | `94` account services | PROVEN |
| `40` | `00` | 0x26FE20 @0x226243, and @0x2265F6 only when faction bit (f-1) of +0x1EE610 is clear | quest failed / reputation unlocked | `21 12` (0x176BEB, 0x176DC8) | `96` unlocked factions | PROVEN |
| `41` | `00`, u32 match_id | 0x26FF90; 0x225810 (`#mstats [N]` parser, from chat 0x26B873) @0x22587F | `#mstats [N]` chat line | `21 13`, `query_match_stats` 0x19D6F0 | `97` match stats | PROVEN |
| `42` | `00`, u32 drop_id | 0x270000; `on_additional_drop_arrived` 0x225950 @0x2259D9 | chat text kind 4 (`query drop: %s`) whose second number is 0 or 2 | none | `98` read_drop | PROVEN layout, LIKELY reply |
| `43` | `00` | 0x26FE20 @0x2259BE | the same drop text with second number 1 | none | `99` daily bonus | LIKELY |
| `45` | u8 lang (no 00) | see section 2.1 | after `9D` | none | `9E` video tutorial link | LIKELY |

**Not sent by the 0.26g client.** There is no creation site anywhere for these ids:
- 0x33, 0x35, 0x36, 0x38, 0x3A, 0x3F, 0x44.
- 0x2E..0x4B ids other than those listed.

Consequences for the server:
- 0.21d `21 02` (profile) and `21 05` (money) have **no 0.26g equivalent**. The server must push profiles (0x87) and money (0x8B), unasked, after every change.
- In particular, the `83 47` shop acks no longer make the client re-query money. 0.21d sent `21 05` after `34 24` (`process_shop_action` 0x1A2179); 0.26g's 0x276360 sends nothing.

---

## 4. Server acknowledgements the requests expect

### 4.1 `83` operation_permitted. Dispatcher @0x274E92 (0.21d `0x34`)
Layout: `u8 op` (= the client id), then a body that depends on op.

Dispatch:
- The byte table @0x2751FC indexes on op-0x30. Jump table @0x2751E4 (PROVEN):
  - `30` -> 0x276810
  - `46` -> 0x275520
  - `47` -> 0x276360
  - `49` -> 0x275C80
  - `4B` -> sends `3C`, then `3E`
- **Every other op (`2F`, `31`, `48`, `4A` ...) reads nothing and is ignored.**
- An op above 0x4B or below 0x30 is ignored (`ja`).

**`83 30`** (0x276810): no body. It sends `49 21` or `49 20` (section 2.2), then refreshes the play button.

**`83 46`**: `lobby_client::process_succeeded_inventory_action` 0x275520 (string at 0x275607). The event byte = the client sub-type. Jump table @0x275C54 handles ev 1..9; ev 0, 3, 5, 10 and higher read nothing.

| ev | body | client action |
|---|---|---|
| 01 attach | u32 host_item_id, u32 upgrade_item_id, u8 slot, u64 v, bool detach_first | If detach_first, it calls 0x275220 with **the upgrade's id** (takes it off the first item holding it; corrected 2026-10-12; with patch 30 off the host, items.md §6). It finds both items (0x26FC60); if either is missing, it stops. The upgrade goes into the host's upgrade vector (host+0x60, 0x18-byte entries, +0x10 = slot); an upgrade already in that slot is swapped out. It logs `attach upgrade: upgrade_id: %u, item_id: %u`. |
| 02 detach | u32 item_id; u32 host_item_id with patch 30 (items.md §6) | 0x275220: off the first item holding it; patch 30: off the host |
| 04 item_use | u32 item_id | When id != 0: count (+8) -= 1, and the item is erased at 0. Then refresh. |
| 06 reroll window | u32 item, u32 a, u32 b, bool has_more; if has_more: u32 c, u32 d | 0x29F430(item, has_more, a, b, c, d): a = `regular_cost`, b = `premium_cost`, c = the modification changed before, d = `reroll_time` (items.md §3) |
| 07 reroll result | u32 item, u32 m1, u32 m2, u32 m3, u32 v | Shows the 3 choices (0x29F680); item+0x10 = v. |
| 08 modifier chosen | u32 item, u32 old_mod, u32 new_mod, u32 a, u32 b, u32 c | Replaces old_mod with new_mod in the inventory copy and the profile copies (+0x1E78AC, count +0x1E78A8), then 0x29F430(item, 1, a, b, new_mod, c): a, b = the two costs, c = `reroll_time` (items.md §3). |
| 09 craft | u8 mode; u32 item; **mode 0xFE ends here** (0x275A31; corrected 2026-10-12); else u32 consumed_item_id; if mode != 0: u32 mod_id | mode 1 adds mod_id to the item; another non-zero mode removes it (both inventory and profile copies). consumed_item_id is then erased from the inventory. Then 0x29F770 (result UI; mode 0xFE is reported as -2). |

**`83 47`**: shop (0x276360; 0.21d `process_shop_action` 0x1A2179). Event byte; table @0x2767EC.

| ev | body | client action / difference from 0.21d |
|---|---|---|
| 00 buy | u8 n, n x {u16 dict_id, u32 item_id, u32 value} | Same as 0.21d. For a non-stack dict (+0x22A == 0) the value is **replaced by 1,000,000**. It then adds to the existing item or appends a new one. **No `21 05` follow-up.** **Crash:** an unknown dict_id makes 0x1F0090 return NULL, which is dereferenced at @0x276459. |
| 01 sell | u32 item_id, u32 amount | Stack items: count -= amount, erased at 0. Others: erased; if dict +0x228 == 0x19, 0x228FF0 first. **The 0.21d trailer `u32 n, n x u32 profile_id` is gone.** Same NULL-dict hazard @0x27659E. |
| 02 | none read | 0.21d sent `21 05` and `21 0F`; 0.26g does nothing |
| 03 repair | u32 item_id; if found: u32 v (item+0xC = v, item+8 = 1,000,000); **if v == 0**: u32 kit_item_id; if kit != 0 and found: u8 used (kit count -= used, erased at 0) | NEW |
| 04 | none read | |
| 05 | none read | Shows an important message of type 7 (`root.add_important_message`). NEW |

**`83 49`**: profile/clan (0x275C80; 0.21d `process_profiles_action` 0x1A212B). ev - 1 indexes byte table @0x276300, then jump table @0x2762D0. The events not listed below read nothing.

| ev | body | action |
|---|---|---|
| 01 autobuy | u8 on | -> +0x1EA974; squad/profile UI 0x227760. Was `34 29 03 u32 pid u8 on`. |
| 02 active profile | u32 profile_id | -> +0x1EA964; finds its index -> +0x1EA976; UI 0x222330. **NEW: 0.21d had no answer** (spec R8). This ack does not call `profile_changed`, so it does not loop. [I] |
| 03 nickname check | string name, bool ok; if !ok: u8 reason 0..5 = `st_short_clan_string`, `st_few_alpha_clan_string`, `st_already_exist_clan_string`, `st_space_symbol_clan_string`, `st_wrong_symbol_clan_string`, `st_string_pattern_error` | 0x228EF0 |
| 04 rename done | u32 ticket_item_id (0 = none) | The ticket's count -= 1, erased at 0. |
| 06 | none read | +0x1EC99D = 1 (clan name reserved) |
| 08 / 09 clan name / abbreviation check | string, bool ok; if !ok: u8 reason 0..4 (same strings without the pattern error) | Kept only when the string equals the last request (+0x1E5660 / +0x1E56A0). |
| 0B | string name | system message `st_clan_invite_1 %s st_clan_invite_2` |
| 0F | u8 status; if status != 1: u32 id, string a, string b | Messaging sends an important message of type 7 (0x26B0A0), then the client sends `49 0B a`. |
| 12 | u8 ok | When ok, a confirmation (0x21FB20). A yes then sends `49 11 m 1`. |
| 19 | u32 id, string a, string b | Messaging important message of type 0x0A, and `st_clan_team_invite_2` |

**`83 4B`**: no body. The client sends `3C 00` and `3E 00`. Same as 0.21d `34 2B`, which sent `21 0D` and `21 0F`.

**`83 48`, `83 4A`, `83 2F`, `83 31`, `83 45`**: no handler; they are ignored. Skill results must come as `8C`. Squad changes are answered by the next `A1`.

**What answers Play and leaving the queue (added 2026-10-12; PROVEN).** `lobby_menu::on_client_status_received` 0x21D720, for 0x85, clears `pending_for_match_making_answer` (lobby_menu+0x3138, the flag that keeps the Play button off after a click: `mov [esi+0x3138],bl` at 0x21D771 for state 2 and 0x21D7A4 for state 1) and then updates the button (0x21FD40). So a Play (`2F`) is answered by a **0x85** (state 1), or by `84 2F`. For state 1 the handler first tests lobby_menu+0x3139 (`cmp [esi+0x3139],bl; jnz` at 0x21D785) and skips the whole queue update when it is set: LIKELY the "leave sent" flag `play_button_clicked` sets after `31` (0x28C532..0x28C54C), so after a leave the client waits for a 0x85 with state 0.

### 4.2 `84` operation_denied (0.21d `0x35`). Reader @0x274F14, handler `lobby_menu` 0x21D9D0
Layout: `u8 op`, `u8` (skipped, unused), then `string text`. The string goes into a stack buffer of the 0x48AC-byte frame, so 255 chars fit. The text is localised (0x1FF0F0) first. Dispatch is by op-0x2F (byte table @0x21DED8, jump table @0x21DEB8).

| op | behaviour |
|---|---|
| `2F` play, `30` clan play | If text == `st_not_logged_id`: sign out, disconnect (0x26F8C0) and go back to login (0x1ED260(3)). This is the 0.21d `0x35 20` rule. Otherwise: `show_system_message("debug", text)` and reset the matchmaking / play-button UI (0x2206D0, 0x220730). |
| `46` | the message, then inventory refresh 0x229400(…,0,0,0) |
| `47` | the message, then refresh 0x229400(…,1,0,0) |
| `48`, `49` | the message |
| `4A` | silently ignored (as 0.21d `0x2A`) |
| other | logs `Unknown (operation denied) type received %d` |

**PROVEN (2026-10-13), a text that is no key:** `text_translator::translate_text` 0x1FF0F0 asks the localization for the key (0xF6C00) and, when there is none, goes on with the text it was given (`mov [esp+10h],esi`, 0x1FF124): a sentence sent as the text of `84` is shown as it is. The text must stay under 0x200 bytes (the length check at 0x1FF15D; the wire's 255 is well under it). Used until 2026-10-15 by the patch revision check at Play (client_patch.md §6), whose default text was the game's own key `st_login_menu_status_invalid_version` ("You're using an outdated version of the game.", in all six languages); that check now refuses at the sign-in. Still used by `SRP_SQUADS_OFF_TEXT`. Not yet seen in a real game with a sentence.

---

## 5. Full 0.26g client enum (lobby part)

| id | meaning | 0.21d | status |
|---|---|---|---|
| 2E | sign_in (u32 session, bool need_shop) | 26 | chg |
| 2F | play (u32 profile) | 20 | = |
| 30 | clan-team play (u32 profile) | none | NEW |
| 31 | leave order / left match (u32 order, bool) | 27 (+ 26 just_left_match) | chg |
| 32 | after sign-in (00, bool) | none | NEW |
| 33 | never sent | | |
| 34 | squad member profile query (00, u32 member, u32 profile) | 21 10 | chg |
| 35, 36 | never sent | | |
| 37 | skills query | none (06 was push-only) | NEW |
| 38 | never sent | | |
| 39 | reputations query | 21 09 | = |
| 3A | never sent | | |
| 3B | elo list query (00, u32, u8) | 21 0C | = |
| 3C | quests query | 21 0D | = |
| 3D | player match stats query (00, u32, u32) | 21 0E | = |
| 3E | account services query | 21 0F | = |
| 3F | never sent | | |
| 40 | unlocked factions query | 21 12 | = |
| 41 | match stats query (00, u32) | 21 13 | = |
| 42 | drop query (00, u32): sent only after a chat line srp never sends; not answered on purpose (results.md §5.3) | none | NEW |
| 43 | daily bonus query | none | NEW |
| 44 | never sent | | |
| 45 | language / tutorial video link (u8 lang) | none | NEW |
| 46 | inventory actions 00..0A | 23 (only sub 00) | 00 =, rest NEW |
| 47 | shop 00..06 | 24 (00..02) | 00/01 =, 02 chg, 03..06 NEW |
| 48 | skills 00 / 01 | 25 | chg |
| 49 | profile / account / clan 00..24 | 29 (03, 04) | 01/02 chg/=, rest NEW |
| 4A | squad 01..04 | 2A | = |
| 4B | quest faction / tutorial 00..02 | 2B | chg / NEW |
| 4C..4F | messaging (not covered here) | C1..C6 | |

**0.21d client messages with no 0.26g equivalent:**
- `21 02` profile query, `21 05` money query, and every other `0x21` type the 0.21d client could send (the remaining ones were push-only anyway).
- `26` fields `just_left_match` and `pending_match_stats_id`.
- `29 03` per-profile autobuy (now account-wide `49 01`).
- `2B 00` quest swap (`accept_change_quest` no longer exists in the exe).
- `24 01` ack trailer (profile list).
- `0x28` (never sent in 0.21d either).

---

## 6. What srp-021d `lobby-server` (`servers/lobby-server/src/lobby_server/{mod.rs,actions.rs}`) must change

| area | 0.21d code (mod.rs) | 0.26g |
|---|---|---|
| `SignIn` / `SIGN_IN = 0x26` | 11 bytes, `is_empty()` check | id `2E`, **6 bytes**: u32 session, bool need_shop_items. Drop `just_left_match` (derive it from `31 .. FF`) and `pending_match_stats_id`. |
| `0x21` query dispatch | `0x21 => match type` | One arm per id, each **skipping the leading `00`**: `34` -> 0x88 (the new u32 member_id comes first), `37` -> 0x8C, `39` -> 0x8E, `3B` -> 0x91, `3C` -> 0x92 (srp also sent services; keep a separate 0x94), `3D` -> 0x93, `3E` -> 0x94, `40` -> 0x96, `41` -> 0x97, `42` -> 0x98, `43` -> 0x99, `45 <lang>` -> 0x9E. Accept `32 00 bb`. |
| `op::MOVE_ITEMS` 0x23 | `23 00 pid n x 16B` | id `46`, sub `00`, same body. There is still no permitted handler for sub 00, so answer with pushes as today (inventory 0x89, profile 0x87, money 0x8B). |
| `op::SHOP` 0x24 | 00 / 01 / 02 | id `47`. 00 and 01: same bodies. 02: read `u32 offer, u32 x, f32` (x = 0 except from the victory screen). Acks become `83 47 ev`; the **sell ack must drop the profile list**. Push money `8B` after each change; the client no longer asks. New subs 03..06 (repair, exchange, premium variants) need at least an `84 47 text` to avoid a hung UI. |
| `op::SKILLS` 0x25 | `25 00 pid n x(u8,u8) m x u8`; `25 01 pid` | id `48`. 00: `u8 n, n x (u16 skill, u8 points)`, no profile, no perks; apply to the active profile (`49 02`). 01: `u8, u8`. No `83 48` handler: reply with `8C`. Deny with `84 48 text`. |
| `op::PROFILES` 0x29 | 03 autobuy (pid, on); 04 active | id `49`. 01: `u8 on` (account-wide), ack `83 49 01 on`. 02: `u32 pid`, ack `83 49 02 pid` [I: the client stores it; the R8 loop no longer applies]. 00/03/04/05/24 and clan 06..23: at least deny or ignore. |
| `0x27` leave queue | `27 u32 order` | `31 u32 order bool left_match`. |
| `op::PLAY` 0x20 | `20 u32 pid` | `2F u32 pid`. Add `30 u32 pid` (clan team), or deny it with `84 30 text`. |
| `op::SQUAD` 0x2A | 01..04 | id `4A`, same bodies. A denial is ignored by the client. |
| `op::QUEST` 0x2B | `2B 00 u32 u32` swap | `4B 00 u8 faction`; ack `83 4B` (the client then asks `3C`, `3E`). `4B 01 u8 step` and `4B 02`: tutorial; store them or ignore them. |
| reply ids | `34`, `35`, `36 xx` | `83`, `84`, and the per-type ids 0x85..0x9F (see `lobby.md`) |

---

## 7. Crash and robustness notes (client side, PROVEN)
- `83 47 00/01` and `83 46 09` look up the dict id with no NULL check (@0x276459, @0x27659E, @0x275B9A). Send only dict ids known to 0.26g.
- `46 07` / `46 08` index the item's modification vector with a Flash index and no bounds check (0x29FED3, 0x29FF63). That is client-local, but the server receives whatever is there.
- The string writer truncates the length byte for strings of 256 bytes or more. The server must not trust `len` to match the text.
- `83 <op>` for an op with no handler reads nothing, so any body is harmless. An `83 46` / `83 49` event outside its table is ignored.

## 8. Open questions
1. What `0x32 00 bb` asks for, and what the second byte of `0x80` means (0x26FA00 not traced). The candidate reply is 0x87.
2. ~~The meaning of `30` and of `49 20` / `49 21`.~~ Settled (`social.md` section 8): +0x1E56BD is "I command the team", +0x1E56BE the player's own ready state; on `83 30` the client sends the opposite of it.
3. Field meanings for `46 01` (a0, a1, a2), `46 03`, `46 05` (a), `46 09`, `47 02` (x), `47 05` (a0..a4), `47 06`, `48 01`, `49 05`, `49 0C`.
4. ~~`49 16`: the wire form of the daily message.~~ Settled: `u16 count` + UTF-16LE units (0x26E210).
5. `43` -> `99` and `37` -> `8C` replies are inferred from triggers only.
6. Whether the server is expected to answer `46 00` (moves) with anything besides pushes. 0x275520 has no ev 0 handler, as in 0.21d.
7. Clan callbacks with no sender found: `checkbox_tournament_match_clicked`. `invite_to_squad` goes through messaging (`4C..4F`, not covered here).

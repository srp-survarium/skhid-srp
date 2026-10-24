# 0.26g: premium, the shop's offers, daily bonus, tutorial, nickname, autobuy

Version scope: client **0.26g build 2777**. Read from the exe (VAs of that build), the decompiled `lobby_menu.swf` (class names given) and the game's data, 2026-10-12. Nothing here was tried against a running client. Tags: **PROVEN** (read at the place named), **LIKELY**, **GUESS**, **[srp]** = srp's own choice where the client does not say.

Code: `crates/survarium/src/v026g/{shop_offers,awards}.rs`, `crates/session/src/v026g/{premium,bonus,tutorial,names,autobuy,notices,extras}.rs`, `servers/lobby-server/src/v026g/shop.rs`.

## 1. The shop record of 0x95 (corrects lobby.md §2.18)

Record 0xD0 bytes, reader of the UI 0x227F60. PROVEN:

| off | use |
|---|---|
| +0x00 u32 | id: what `47 02` / `47 05` / `47 06` send back |
| +0x04 u32 | **a bundle id of `shop_bundles_dict`**: 0x2CAC10 looks the bundle's config up by it (name, description, icon) and the result is used unchecked. So every record is one of the dictionary's 14 bundles |
| +0x0C u32 | kind: 0 = premium time (an entry of `root.set_premium_store`), 1 = a bundle (`root.setup_bundle_data`), anything else is skipped |
| +0x10 u32 | price |
| +0x14 u8 | item count, at most 5 |
| +0x18 + j·0x24 | item: +0x00 dict id, +0x0C amount, +0x04 / +0x14 / +0x18 three u32 of the discount label, +0x1C u32 (== 1: counted for the label), +0x20 u32 (== 0: an inventory item, listed as the bundle's content) |
| +0xCC u32 | money type: 1 = gold (`isPremium` in the lobby) |

- The discount label of a premium entry (0x228280): `sum` = Σ over items with +0x1C == 1 of `trunc(item[+0x14] / item[+0x18] * item[+0x04])`; shown as `"%d%%"` of `(price - sum) / sum * 100` when that is above 0. What the original server put there is not known. **[srp]** sends premium entries with no items: no label.
- `LobbyMenu.set_premium_store` reverses the list, so srp sends the longest time first and the window shows the shortest first.
- A bundle's contents shown: `InventoryList.setupBundleData` makes an item of each dict id (an unknown one is the usual NULL dictionary entry).

**What the 14 bundles are** (PROVEN, the localization's own texts): ids 1..7 premium time: 3 days "(72 hours)", a week, a month "(30 days)", 3 months "(90 days)", 6 months "(180 days)", a year "(365 days)", a day "(24 hours)". Ids 8..14, by their descriptions: Bandit (jacket, pants, enhanced Uzi, 2000 × 9x19 Parabellum), AK-74N (2000 × 5.45x39 7N6M), MP-27M Lancaster (1000 × 12x70 buckshot), SR-3M Vikhr (2000 × SP-6), PP-19-01 Vityaz (2000 × 9x19), VSS (1000 × SP-5), Survivor (jacket, pants, improved Kiparis, 1000 × 9x18 PM). That "improved" means the dictionary's `*_premium` copies is LIKELY. Their weapons are golden weapons ([golden_weapons.md](golden_weapons.md)), the Bandit and Survivor jackets and pants premium armour ([premium_armour.md](premium_armour.md)): srp gives them their bonuses as modifications of their own.

**Prices are not in the client.** From the sources of the time (`progression_sources.md` §6; changed 2026-10-12):
- Bundles, PROVEN (the wiki's `Bundles` page of 2015-01-22, revision 7866, 0.26h): AK-74N 1,500, Bandit 2,000, Survivor 2,000, MP-27M Lancaster 300, Vityaz 700, SR-3M Vikhr 5,000, VSS Vintorez 5,000 gold (`shop_offers.rs` `Bundle::gold`). Until 2026-10-12 srp charged the premium copies' `base_cost` (1,200 / 1,500 / 1,500 / 500 / 1,000 / 2,500 / 2,500); that `base_cost` is still what the lobby shows for selling such an item back.
- Premium for 90 days: 2,500 gold, LIKELY (official sale news of January 2015: three months at 40 % off save $10, so $25; 1,000 gold was $10).
- The other premium times are srp's own choice (0.21d srp's; no source found; the owner confirmed them 2026-10-12): 250 a day, 600 for 3 days, 1,200 a week, 3,000 a month, 13,500 for 6 months, 24,000 a year (`SRP_PREMIUM_PRICES`). So 90 days cost less than 30 by default.

**Premium items one by one** [srp]: a 0x95 record cannot be a single item, so every premium item with a price that no bundle holds (vouchers, premium camouflages and decals, the AK-74N) is in the price list 0x8A under seller 0 at its gold `base_cost`; `47 00` for one takes gold. The lobby's shop list takes any category from 0x8A and its buy dialog pays gold for a premium item (`MessageAsk`: `_money_type = isPremium`). PROVEN for the window.

**Not sold: the Russian and Belarusian flags and coats of arms, the flag of the USSR and the red star** (the owner's decision, 2026-10-18; `prices.rs` `NOT_SOLD`):
- `decals_4_1` (dict 266) Flag of USSR, `decals_4_2` (267) Flag of Russia, `decals_4_4` (269) Flag of Belarus, `decals_5_1` (282) Red star, `decals_5_2` (283) Coat of arms of Russia, `decals_5_4` (285) Coat of arms of Belarus: premium decals, 300 gold each until then.
- In no price record (0x8A), silver or gold; a `47 00` for one is refused (`st_cannot_buy_item`).
- The Russian and Belarusian ones: found by name in all six localizations of stock 0.26g with patches 12..=20 and checked against their icons; nothing else names Russia or Belarus. The two Soviet ones: the owner's word. The coat of arms of Lithuania (`decals_5_7`, the Vytis) stays on sale, the owner's word too.
- They drop nowhere (`drop_weight` 0) and are in no bundle.
- srp leaves owned copies alone: they are still worn and sold back like any decal. The owner removes them from accounts by hand (`g26_items` rows by `dict_id`, and the `upgrades` entries of other items that name them).

**The third kit is not premium's in srp** (the owner's choice, 2026-10-17): every account uses all three kits, premium or not, and no kit is sent with the client's premium state 2 (`Account::kit_usable`; lobby.md §2.5). The client locks a kit only by that state (`ProfileSelector.setupProfiles`, `LobbyMenu.onProfileContextMenu`: the buy-premium box opens for a state-2 tab), not by the account's premium. The original game's third kit was premium's (PN 0.26g, `progression_sources.md`). Premium's other benefits are unchanged. OPEN: the client's premium windows (`MessagePremium`, `MessagePremiumDiscount`) still list `st_premium_bonus_profile` ("Профиль экипировки", an equipment profile) among premium's bonuses; that text is the game's localization in `resources.db`, which a server change does not reach.

**Gold has no real-money source in srp**: level rewards, a new account's 500 (`no-tutorial.md`), the daily bonus (50 a week), drops, the operator's web page.

## 2. Buying: requests and answers

| request | meaning (PROVEN in the UI) | srp's answer |
|---|---|---|
| `47 02 <id> 0 0.0` | `accept_premium(id)` (the premium window) or `buy_shop_item(uid)` (a bundle) | `83 47 02`, `94`, `89`, `87`, `8B` |
| `47 02 <id> <match> 0.0` | `accept_premium_victory_screen(id, matchId)`: the results screen's button | the same, then the match's premium half (`rewards::Results::claim_premium`): `89 87 8B 8C 8E` and notices |
| `47 05 <id> <f32 discount> <money> <exp> <rep> <u8 faction>` | `accept_premium_with_bonuses` (`MessagePostPremium`), the answer to `A8` | `83 47 05`, then as above. The amounts are the client's echo; the server gives its own |
| `47 06 <id> <f32 discount>` | `accept_premium_with_discount` (`MessagePremiumDiscount`), the answer to `A9`; closing the window sends id −1, discount −1 | `83 47 02` and the pushes; nothing for −1 |
| `46 04 <item>` | `item_use`: sent only for a voucher of type 3 (`usable = category == 27 && (type == 3 \|\| type == 0)`; type 0, the rename voucher, opens the rename window instead) | `83 46 04 <item>` (the client takes one off its copy), `94`, `89`, `87`, `8B` |

- `83 47 02` and `83 47 04` read nothing; `83 47 05` shows the lobby's "premium activated" notice with the three amounts of the last `A8` (`LobbyMenu.add_important_message`, its type 7). After none of them does the client ask for anything, so `94` is pushed.
- **`A8`** (0x274FF8 → 0x28A400, PROVEN; corrects social.md §5): `u32 premium seconds` (÷ 3600 → `hours`, which the window does not use), `u32 money`, `u32 experience`, `u32 reputation`, `u32 time left` (seconds, counted down on the premium button), `f32 discount`. The client lists every kind-0 record at `floor(price * (1 - discount) + 0.5)` (`root.set_premium_discount_store`) and opens `root.show_premium_post_reward(money, exp, rep, hours, timeLeft, discount, faction)`. The window preselects the fourth entry.
- **`A9`** (0x274FBA → 0x28AB20): `f32 discount`; the same list, and `value_old = floor(value / (1 - discount))` in the lobby: a discount of 1 divides by zero.
- When the original server sent `A8` / `A9` is not known. **[srp]**: only when the operator sets `SRP_PREMIUM_DISCOUNT` (percent, 0..90, default 0): `A9` after the sign-in of an account without premium, `A8` with the results of a match whose premium half was not paid, standing for `SRP_PREMIUM_OFFER_SECONDS` (3600).
- The one-day voucher's time is its config's `ttl_in_hours: 24`.

## 3. The two boxes: autobuy and auto-repair (corrects lobby.md §2.7, lobby_requests.md `49 01`)

`49 01 <u8>` carries `LocalPlayerProfile.getAutoBuyState()`: **0 neither, 1 autobuy, 2 auto-repair, 3 both** (PROVEN in the UI). The same number is the "autobuy flag" byte of `87` and the byte of `83 49 01`: the client hands it to `root.set_autobuy` as a number (0x227760) and `LobbyMenu.set_autobuy` ticks autobuy for 1 and 3, auto-repair for 2 and 3. A wire `FF` would untick both.

- **The auto-repair box is gone from the Character panel** (the owner's, 2026-10-23: wear is gone, so it did nothing). Its code is out of the movie; the original movie's graphics still place it, so `LocalPlayerProfile.init` hides it and moves Autobuy into its place (the row's start, no gap). The movie now sends 0 or 1 only; an account whose auto-repair was on is still pushed 3, which ticks Autobuy as before, and its next Autobuy change clears the server's `auto_repair`. The server keeps the bit (with `SRP_ITEM_WEAR=on` it still repairs).
- **Autobuy is on by default** (the owner's, 2026-10-23): a new account starts with it ticked (`Account::with_loadout`). The server keeps only the box's state, not whether the player ever touched it, so the accounts of before were all turned on once, at the first start of an srp with the rule (`GameAccounts::turn_autobuy_on_once`, the `g26_meta` mark `autobuy_turned_on`, written after the accounts; a mark, not a schema step, so an older srp still opens the database after a rollback) [choice: an account that had turned it off gets it on again once; the box shows it, and a box turned off from then on stays off]. Unticking it sends `49 01 00` and is saved as ever.

What a match used: `session::v026g::autobuy` (the player's block of the last trusted snapshot when the match server read it, held to what the counters and the kit allow; else the bullets counter for a kit with one kind of ammo). Autobuy buys back in whole clips while the silver lasts; auto-repair repairs what the match wore by the lobby's silver repair. The three lists that end `89` carry what was spent.

## 4. The daily bonus (`99`, adds to results.md §4.1)

The game's own texts: "Play everyday and get x2 experience and reputation for the first three victories", "On 7th day you will get" the reward, "Complete". The window shows 7 days with a progress of 3 each; a day before the current one is shown done (`LobbyMenu.show_daily_reward_window`); the status bar shows the current day's progress. PROVEN. The reward comes from the data's `daily_bonus_drop` table (always 50 gold).

[srp]: a day is a UTC day; the run grows by one on the first day after the last day the account was seen (sign-in or a finished match), a missed day or the day after the seventh starts a new run; the reward is rolled at the run's start and given when the run reaches day 7; a victory counts when the team won and the player stayed; the first three of a day pay the match's experience and reputation once more (the results screen's "daily" block, bytes 100 = "+100%"). `99` goes out at every sign-in (its window opened once a day), after a match's results, and for `43 00`.

## 5. The tutorial (`9D`, corrects lobby.md §2.21)

`9D`: `bool show, u32 a, u32 b`. With `show`, the tutorial leaves its disabled state by `a` (0x283190, PROVEN): bit 0 clear → `tutorial_state_choose_nickname`; bit 0 set → `tutorial_state_click_inventory` (`b` == 0) or `tutorial_state_equip_drop` (`b` != 0); `a & 0x22` → `default_state`.

**Bit n of `a` is the step n the client reports with `4B 01 <n>`** (PROVEN: step 3 / bit 3 the "prepare to battle" notes 0x2832EF / 0x2833F1, step 2 / bit 2 the "items" notes, step 4 / bit 4 the match's own tutorial 0x24921D / 0x27AE46, step 1 / bit 1 where the tutorial ends 0x21D461 / 0x28E9AD). Bit 0 = a nickname chosen, bit 5 = skipped (`4B 02`): LIKELY.

The awards are the game's `gameplay/awards/unique_awards.options` (PROVEN data; answers data.md question 8: `type` is the lobby's `Reward` constant, 0 experience, 1 reputation, 2 money; for money `param` 0 silver, 1 gold, LIKELY): `tutorial_completed` 1000 silver + 250 experience (step 1), `survival_item_notes_completed` 25 gold (step 2), `survival_prepare_to_battle_notes_completed` 25 gold (step 3), `game_world_tutorial_completed` 50 gold (step 4), `tutorial_change_name` nothing. That the server paid them for these steps is LIKELY.

[srp] **No tutorial since 2026-10-19 (the owner's):** it took a new player through Play into a match, and with rooms nobody is sent into a match they did not choose. Every `9D` has `a` = 0x3F, every step done (`session::v026g::tutorial::ALL_DONE`): `a & 0x22` puts the lobby in `default_state`, bit 4 skips the match's own tutorial, bits 2 and 3 hide the notes' rewards; `b` = 1. The sign-in's has `show` on, the ranking's recounts off. A step a client sends anyway (`4B`, `46 0A`) is not answered. **A new account starts with** (at its first sign-in, before its client hears of it: no window) 60 000 silver, 500 gold, the steps' 250 experience (read from the awards file), 1000 rounds of every ammo, the Scavengers at level 5 and a worn starter kit (`no-tutorial.md`); its quests are the Scavengers' (the tutorial ended in `state_choose_faction`; never neutral). A player counts as new for the first match's drop table while it has no more experience than that. The video link (`9E`) stays hidden. §5.1 and §5.2 are what the tutorial was, kept as knowledge of the client.

### 5.1 The lobby tutorial's states, and the one step the server has a part in (2026-10-12)

The tutorial is a state machine in the exe (state objects 0x2A4240..0x2A5320; each has a name, an enter function and up to two `{UI event, next state}` pairs at +0x30). Entering a state (0x2A3EA0) calls `root.clean_highlight` and **clears every UI handler of the lobby** (0x281580: 112 slots of 0x20 bytes at lobby_menu+0x1C00, in the order of the name table 0x9C56C8), then switches on only the ones the state names (0x219470, which copies a default from lobby_menu+0..0xE00). `root.show_slide(text1, text2, closable)` (0x22A350) puts a sheet over the stage and makes the highlight layer the modal clip (`LobbyMenu.show_slide`: `tutorBack`, `FocusManager.setModalClip`). So in a tutorial state **only what the state highlights can be clicked or dragged** (`highlight_element` / `enable_element` -> `InteractiveObjectEx.setTopmostLevel`), besides "Skip tutorial". All PROVEN from the exe and the lobby SWF.

| state (enter) | what it does | leaves on |
|---|---|---|
| `tutorial_state_choose_nickname` 0x2A43C0 | `root.show_tutorial_choose_name_message` | `tutorial_nickname_ok_clicked` -> click_inventory |
| `tutorial_state_click_inventory` 0x2A44E0 | slide `st_tutorial_click_inventory_message_1/_2`; highlights `root.topMenu.inventoryTab` | `subsystem_activated` -> choose_wpn |
| `tutorial_state_choose_wpn` 0x2A45D0 | slide `st_tutorial_choose_wpn_message_1/_2`; **sends `49 02 <active kit>` and `46 0A`**; registers 0x283470 for `inventory_content_filled` (slot 63, +0x23E0) | `handle_drag_start` -> drag_wpn |
| `tutorial_state_choose_wpn_no_check` 0x2A4840 | the same step without the two requests: calls `root.highlight_inventory_items([106, 107])` itself | `handle_drag_start` -> drag_wpn |
| `tutorial_state_drag_wpn` 0x2A4730 | highlights and enables `root.player_profile.paperDoll.getSlotAt(7)` (weapon slot 1); filters `relocate_item` (0x2A3F70: only a list with dict 106 or 107 reaches the real 0x288D60) and `apply_profile_items` | `relocate_item` -> click_ammo; `handle_drag_end` -> choose_wpn_no_check |
| `tutorial_state_click_ammo` 0x2A4940 | slide `st_tutorial_click_ammo_message_1`; highlights `root.player_profile.btnShowAmmo` | `ammo_container_showed` -> choose_ammo |
| `tutorial_state_choose_ammo` 0x2A4A20 | slide `st_tutorial_choose_ammo_message_1/_2`; highlights `root.ammoShare.getAmmoRenderer(..)`; **sends a `46 00` itself** (0x2A4BB9): the stack in slot 8 of the active kit `{id, dict, src 8, dst 100, its count, 1000}` then `{id, dict, src 100, dst 8, clip_size (twice that for dict 50, the 7.62x25), 1000}` | `ammo_count_updated` -> apply_ammo |
| `tutorial_state_apply_ammo` 0x2A4C40 | slide `st_tutorial_apply_ammo_message_1`; highlights `btnShowAmmo`, `btnSetAmmo` | `apply_profile_items` -> press_play |
| `tutorial_state_press_play` 0x2A4D40 | `root.remove_items_filter`; slide `st_tutorial_press_play_message_1`; highlights the play and the notes buttons; leaves a squad (`4A 01` / `4A 04`) | `play_button_clicked` -> disabled_state |
| `tutorial_state_equip_drop` 0x2A4F70 | after the first match (`9D`'s `b` != 0): slide `st_tutorial_equip_drop_message_1/_2`, the quests button | `subsystem_activated` -> choose_faction |
| `tutorial_state_choose_faction` 0x2A50F0 | the two faction buttons | `change_quest_faction` -> default_state; then `root.show_tutorial_complete_message` and **`4B 01 01`** (0x2A52BE) |
| `state_choose_faction` 0x2A5320 | where "Skip tutorial" goes for an account with no quest faction | `change_quest_faction` -> default_state |
| `default_state` 0x2A4240 | every handler back (0x281480), `root.close_slide` | - |

**Why no weapon could be dragged (PROVEN).** `inventory_content_filled` is in the handler table but **nothing in the lobby SWF calls it**: the exe raises it itself at the end of `setup_inventory_data` (0x288520 @0x2886F5), which runs when an `89` is read (0x274C45 -> 0x21D720, case 0x89 -> 0x21D822). The callback 0x283470 then calls `root.highlight_inventory_items([0x6A, 0x6B])` (0x22A840): the PPSh-41 (dict 106) and the Mosin carbine (dict 107), found by dict id (`InventoryList.filterItems`, `getItemRenderer`), filtered into the list and raised above the sheet. **So after `46 0A` the client waits for an `89`.** srp answered `49 02` with `83` and `46 0A` with nothing (packet log of the session: `49 02` + `46 0A` in, `83` out, then only keep-alives until `4B 02`): the two weapons stayed under the sheet, with every other handler off. The items themselves were right (dict 106 and 107 owned, not locked: `InventoryItem.handleDragEnabling` needs only `descriptor` != 200 / 300, `repairTime` 0).

**[srp] `46 0A` is answered with the inventory, the account and the money (`89`, `87`, `8B`)**; an account still in the tutorial that lacks one of the two weapons gets it first (the highlight needs both: `highlight_element(null)` for a missing one is a script error, LIKELY ending the call). What the original server did at `46 0A` is not in the client; LIKELY it handed the two weapons over there (the twin state "no check" hints that `46 0A` is a check or a grant). srp's new account owns them from the start. The strict model has the rule: `survarium::v026g::lobby::tutorial_model` (held by `client_model::Client`): nothing can be dragged in `choose_wpn` until an `89` with both weapons came.

**Not seen live yet, read from the exe only:**
- The tutorial's text says "drag ... into the empty weapon slot" and the slot it highlights is weapon slot 1 (slot 7). **srp's starter kit has the TT-33 there** (`loadouts::STARTER`), weapon slot 2 is the empty one and stays under the sheet. Dropping on slot 7 replaces the pistol (`rules::move_items` takes it). LIKELY the original new account had the TT-33 in weapon slot 2 (slot 10) and slot 7 empty; srp's kit was not changed (owner's decision: it is also what the match and autobuy were tested with).
- `relocate_item` (0x288D60) adds the ammo moves itself when a weapon is put on (0x289DD0..0x28A1E6: the first owned stacks that pair with the kit's weapons, `count = bags / required_bags x clip_size`, into slots 8 and 9). `choose_ammo` then reads the stack of **slot 8 without a check** (0x2A4AA2: index 0xFFFF for an empty slot, read 0xB4 x 0xFFFF past the item table): if slot 8 were empty there the client would LIKELY crash. With srp's kit slot 8 holds the 7.62x25 before the step, and a Mosin's own ammo is owned.
- The `46 00` of `choose_ammo` asks for 1000 rounds in all; srp tops the free starting ammo up to that for nothing (`rules::move_items`, `top_up`).

### 5.2 "Skip tutorial" (`skip_tutorial` 0x28CCB0 -> 0x281660)

PROVEN: the state becomes `state_choose_faction` when the account has no quest faction (lc+0x1EA998 == 0), else `default_state`, and `4B 02` is sent. Entering either removes the highlights; `default_state` (after the faction is chosen) brings every handler back and closes the slide, which removes the sheet and the modal clip (`LobbyMenu.onSlideClose`). **Everything is usable after it** (seen live 2026-10-12: skip, then the faction choice `4B 00`, answered). One leftover is possible: `root.remove_items_filter` is called only by `press_play`, so a skip between the weapons' highlight and that step leaves the inventory list filtered to the two weapons until the client restarts [LIKELY; not seen: in the live session the highlight never came].

## 6. A nickname change

`49 03 <name>` → `83 49 03 <name> <ok> [reason]`. `49 04 <name> <u32>`: the u32 is the rename voucher's item id, or 0 to pay gold (`MessageEnterName`: `radioButtonPremium.selected ? 0 : voucherId`; PROVEN). The gold price is 0x8D's fourth value. Answer `83 49 04 <voucher or 0>`, then `8F` with the new name.

[srp]: an account signs in with its login and is shown by its name; a rename changes the name only. A new name must pass the name rules, be shown by no other account and be no other account's login. Price `SRP_RENAME_COST`, default the voucher's 250 gold. (Two accounts registered as `a@x` and `a@y` are both shown as `a`, as in 0.21d: only a rename is checked.)

## 7. The level notice: important message type 14 (corrects social.md §1.4)

0x226B5B: the text is four bracketed numbers, read in this order by 0x227120: **`[skill_level][reward_type][reward_id][reward_amount]`** (PROVEN). The lobby shows "Level N achieved" with `reward_amount` of money and one skill point; `reward_id` 0 is silver, else gold (`MessagePlayerRank`). `reward_type` is read by nothing of that window; srp sends the same number as `reward_id` (GUESS). srp sends one for each level a match or a claimed premium half reached. Other types: 1 and 4 were sent already; 5 shows nothing; 6 (item repaired) is the items work's; 13 (referral) has no rule in srp.

## 8. Settings

| variable | default | |
|---|---|---|
| `SRP_PREMIUM_PRICES` | `100,200,400,1000,2500,4500,8000` (90 days sourced, the rest the owner's choice 2026-10-12) | gold for a day, 3 days, a week, a month, 3, 6 months, a year |
| `SRP_PREMIUM_DISCOUNT` | 0 | percent off premium time, offered by `A9` / `A8`; 0 offers none |
| `SRP_PREMIUM_OFFER_SECONDS` | 3600 | how long the offer after a match stands |
| `SRP_RENAME_COST` | 250 | gold for a nickname change |
| `SRP_AMMO_USE` | on | `off`: a match takes nothing off the stacks |

## 9. Open

- Everything above against a live client. The tutorial: its steps after "choose a weapon" (§5.1), and whether the starter kit should leave weapon slot 1 empty for it.
- The original prices, the discount label's item fields, when `A8` / `A9` were sent.
- `9D`'s `b`; bits 0 and 5 of `a`.
- `42 00 <drop>` is not answered, on purpose: the client sends it only after a chat line srp never sends, and nothing waits for the answer (results.md §5.3).
- A premium account running out while its player is in the lobby is noticed at the next push of the account, not at the minute it ends.

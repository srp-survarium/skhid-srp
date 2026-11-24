# Progression audit (0.69d): new player, levels, faction reputation, match rewards, pace

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Status words: PROVEN = read from the exe / AS3 / game data (file + field or address given); LIKELY = inferred; CHOICE = a
server rule no source states; OPEN = unknown, nothing invented. `crates/data-extract` builds
`data/progression_069d.json` from the game files (new-player kit, faction level table, event rewards, reward modifiers,
tutorial awards). Code: `crates/game-data/src/progression.rs`, `levels.rs`, `servers/lobby-server/src/progress.rs`.

## 1. Starting equipment of a new account

Data (PROVEN): `gameplay/items/items_for_new_player.options` (volume 38): 30 rows `{dict_id, buy_amount, wear_amount,
slot, faction_id, profiles_to_wear}`. Rows with `faction_id` 1..4 are the torso (slot 2), legs (4), boots (6) of that
faction (31/45/37 Scavengers, 29/28/24 Black Market, 750/751/752 Army, 745/746/747 Fringe). Faction-free rows: weapon
slot 7 by player profile (profile 0 AKMN 1201 + 200 x 7.62x39 (81); profile 1 Mosin 107 + 30 x 104; profile 2 TOZ-66 17 +
80 x 12mm buck (22)), TT-33 (18) in slot 10 for all profiles with 50 rounds of 1178 in slot 9, medkit 67 (slot 13), RGD-5 (slot 14), and six items
bought without being worn: PPSh 106, rails 1703 / 1867, scope 1768, collimators 1771 / 1772. No money, experience or
reputation value exists in the file, in `game.options`, or in the exe strings (searched `starting`, `initial`, `new_player`
keys): starting money / xp / reputation = 0 (OPEN whether the original server granted any; see the tutorial awards).

Server: `lobby_server::progress::new_player(gd, faction, roll)` builds exactly this kit (the five modules are
stored green with drop grades from the green cells of the general table, LIKELY: workshop.md section 7); every new account of the login server
gets it (`servers/srp/src/lib.rs` `AccountAuth::register`, faction 1 until chosen). The equipment profiles: profiles.md.
Ammo rows of the same item merge into one stack (the three TT-33 rows: 150 rounds).
First faction choice (`4A 00`, the exe stores the faction only while none was set): while the
account has no experience and no reputation, choosing another faction re-issues the faction armour of the kit as that
faction's pieces (same slots, same uids) (`progress::first_faction_choice`; no schema change). A later switch changes only
the active faction. `Profile::starter()` stays for tests only.

## 2. Character level

* Thresholds (PROVEN): `character_leveling` in `game.options` (volume 75), 100 rows `{experience_points, reward_amount,
  reward_currency}` = `game_tables_069d.json` `xp_table` / `level_rewards` (checked equal). Row i is the total experience
  that ENDS level i + 1: level 1 = 0..500, level 2 = 500..1030, level 5 starts at 2170, level 100 starts at 6 331 640 -
  478 390 = 5 853 250. The exe table reads match (`u32 +0x725018` previous, `+0x725014` next threshold).
* Max level 100. Evidence: 100 rows (the last row is the
  bar of level 100); OFFICIAL Steam Q&A 2021-03 (gid 4048146376729246088) "level 100 and Prestige 10 in all factions", patch
  0.66 decals "for character levels 10, 20, ... 100". `Levels::level_of` is capped at `max_level()`.
* What a level-up gives: skill points = level - 1 (PROVEN by `MessagePlayerRank`, progression.md section 2) and
  SILVER `reward_amount` of the row (500 for levels 2-10 rising by 250 every 10 levels to 5000): the client's level-up
  window lists this reward from its own table, and the server pays it: `Levels::level_up_silver(old, new)`
  and `progress::add_xp` (used by matches, quests, chests). Achievement rewards of levels (achievement 413) are separate.
* 8C / 87 fields (`LevelFields`): level, skill points, xp, previous and next threshold from the table; at level 100 `next` is
  the last row.

## 3. Faction reputation and faction level

* Thresholds (PROVEN): `gameplay/reputation_leveling.options` (volume 00), four factions with the same 16 values
  `0, 0, 5000, 20000, 80000, 200000, 350000, 500000, ... 1 700 000` (index = level; levels 6..15 are Prestige 1..10, label
  `st_prestige_level_N`); factions 5 and 6 exist in the file with one level and no use. Equal to `levels::FACTION_THRESHOLDS`
  (test `faction_table_is_the_data`). Max level 15 (OFFICIAL: "Prestige 10").
* Per faction (PROVEN): points are stored per faction (8E record `{faction, unlocked, points}`, exe record `+8` points only).
  Reputation of a match goes to ONE faction: the player's active faction (R record `faction_id` `+0x1A` is a single u32, one
  `faction_reputation` field). Switching the active faction keeps every faction's points.
* New accounts start at 0 points in every faction. Hunt tier, daily quest rows and
  achievements 408-412 read these points, so a new player sees level 1 content first.
* What a faction level unlocks (PROVEN): the client opens an item when `faction level >=
  item_level` of the item's dictionary row (`item_level` 1..5, `faction` 1..4); ammo and premium items have faction 0 and are
  always open. So by the client's own rule level 5 (200 000) opens every item of the faction; levels 6+ unlock nothing in
  the shop (prestige is cosmetic + decals). The fan wiki lists shop "ranks" 1..10 per faction (rank names Rookie .. Legend);
  those are NOT the dictionary levels (AK-74M is wiki rank 7 but `item_level` 2): CONFLICT, the dictionary
  is used. The server refuses a purchase the client would have disabled (`progress::faction_allows` in `shop::buy`).
* Rewards per faction level: none found. The `rank_N_equip_chest` / `rank_N_boosters_chest` items (1115, 1119..1121, 1149..1153)
  are not referenced by any faction table in the files read; they are drop-table chests (items.md section on chests). Bundles
  (`*_starter_bundle.options`) carry `faction_reputation_level 1` for a purchase, not a reward. OPEN: whether the original
  server gave anything at a level change (the client's `MessagePlayerRank` / faction messages show only the unlock text).
* Faction battle: per faction 5 tiers at 10 000 .. 80 000 battle reputation (progression.md section 5); needs a schedule.

### Every way reputation is earned in 0.69d

| # | source | formula | target faction | status |
|---|---|---|---|---|
| 1 | finished match | sum of `events.options` event rows x `reputation_factor` of the match level, then premium / booster / worn-item bonus | active faction | PROVEN rows, LIKELY level index; kills / assists / deaths only when observed |
| 2 | daily quest (each of three) | reward row by level: 100 / 250 / 300 / 400 / 500 reputation | active faction (shown with it, 0x1C2832) | PROVEN (quests.md) |
| 3 | daily quest chest and any chest / hunt / login calendar reward with dict 1164 `reputation` | the chest entry's amount (100..500 by tier) | active faction | PROVEN item, CHOICE target (the item has no faction field) |
| 4 | tutorial awards (`unique_awards.options`) | enum decoded (section 8): `tutorial_completed` = 1000 experience + 2500 silver + 200 spare parts (no reputation: type 2 param 4 is the spare-parts payment, not a faction) | none | PROVEN enum; `tutorial_completed` paid once per account on `4A 01 01`; `game_world_tutorial_completed` item row paid once at the end of the tutorial match (CHOICE trigger); the three `survival_*_notes_completed` rows (silver) OPEN: trigger not decoded |
| 5 | faction battle | accounting only (tier rewards are items); a finished match's reputation counts for the battle of that faction | active faction | CHOICE (progression.md section 5) |
| 6 | decal / item bonus "+x % reputation" | modifier groups 139 (value 0.01 .. 0.5) and 143 (0.05 .. 0.25): decals carry them in `default_modifications`, premium equipment too | the same match reputation | implemented: worn items and worn decals / camouflage + decal set tiers (`docs/notes/decals.md`) |
| 7 | 24 h reputation voucher / premium | group 139 grade 2 = +50 % (voucher 861), premium +50 % | same | implemented (boosters.rs, premium.rs) |
| 8 | achievements | rewards are items only (all rows `reward_type` 0) | n/a | PROVEN: no reputation |
| 9 | clan battles | OFFICIAL 0.65b: training custom matches give no rewards, experience, faction reputation or equipment progress; "However, you will get rewards for clan battles" (kind not stated). CHOICE: the data's normal match reward (`clan-battles.md`) | the active faction like any match | LIKELY / CHOICE |
| 10 | event rewards (Faction War 2019 / 2020 event reputation) | OFFICIAL event-only counter, not account reputation | n/a | not part of 0.69d data |
| 11 | equipment-based reputation split (0.26g: reputation split by the faction of the worn kit) | NONE found in 0.69d | - | ABSENT: strings (`faction_reputation`, `reputation_factor`, `reputation_bonus_modifier`, `player_reputations`), `faction_affinity` (a weapon-module field of 0/5/10/15/20/30, not a kit field), the R record with one `faction_id`, the 8E record, the data tables: nothing splits reputation by item faction. Not implemented (only 0.69d mechanics are built). |

Tests: `progress::tests::every_reputation_source_credits_the_active_faction_only`, `match_reputation_is_credited_to_the_active_faction_on_finish`.

## 4. Match rewards

Real data (PROVEN), `gameplay/awards/events.options` (volume 54): every match event has `score`, `experience`, `money` (silver),
`reputation`, `reward_factor`, and rows per game mode that replace the base row: e.g. `kill` base 10 / 6 / 30 / 3, `tdm` 15 / 9 /
45 / 5, `assist` 5 / 4 / 15 / 2 (tdm 10 / 10 / 30 / 5), `sad_bomb_planted` 15 / 30 / 150 / 20, `capture_the_point` 20 / 30 / 60 / 15
(`reward_factor` 100), `team_kill` -10 / -10 / -30 / 0, `played_first_game` 500 experience. Money is always 3-5 x the score
and experience 0.6 x: this is the "the more points you get, the more silver" of the official Q&A (2021-11-17). Factors: `game.options`
`match_levels_factors` (6 rows: reputation x2 / x4 / x4 / x4 / x4 / x4, experience x1 .. x1.45, silver x1 .. x4); the drop
table `drop_after_combat` (drops.options, volume 72) has a silver row (dict 214 `generic_money`) 100..200 at match level 1 up to 500..1000
at level 5 and spare parts / chests (rewards.rs). The exe only cooks these tables (strings `match_levels_factors`,
`silver_factor`, `experience_factor`, `reputation_factor`, `reward_factor`, items_dictionary_cook.cpp); the formula runs on the server
side, which is not in the exe.

Implemented (`GameData::match_reward`):

    events      = counts from MatchFacts (kill, headshot_kill / melee_kill / grenade_kill / trap_kill extras, assist, death)
    exp, rep    = sum(row * count / reward_factor)  * factor(match level)
    silver      = sum(row.money * count / reward_factor) * silver_factor(match level) + drop_after_combat silver roll
                  (online matches: the silver factor is the server setting `match_silver_factors`, default
                  1 / 1 / 3.5 / 7 / 10 / 14, and the level is each player's own equipment level; docs/notes/online.md 7)
    then        premium +50 %, 24 h boosters, worn-item bonuses (additive percentages)
    leavers / no win-loss marker: nothing (OFFICIAL 2022-01-27)

* LIKELY: the factor row index = the match level (6 rows, levels 0..5); `reward_factor` is the event value that counts as one reward
  (100 for the objective events, so a value of 100 pays the row once); the silver factor does not scale the drop roll (CHOICE);
  extras of headshot / melee / grenade / trap are separate events on top of `kill`.
* LOCAL (bots) TRAINING MATCH PAYS NOTHING (PROVEN): the movie tooltip of the "Training with bots" checkbox
  (`MatchMakingSettings`, l10n `st_match_settings_bots_hint`) reads "Training mode where you fight against bots (computer
  controlled opponents). Rewards in this mode are not awarded." So no experience, silver, reputation, drops or chests and the
  `played_first_game` bonus stays unspent (`game_data::rewards::LOCAL_MATCH_PAYS = false`). `drops.options` and `events.options`
  have no match type; the official 0.65b note about custom rooms is a separate rule. The tutorial map uses the same local path and
  pays no match reward either (CHOICE; it has its own awards). Quest / achievement / league counters still see the match
  (OPEN: nothing proves whether bot matches counted for them).
* Facts the server does not have are `None` and add nothing. When the match server observes kills,
  assists and deaths (`MatchFacts::kills`, `deaths`, `stat_events[1]`) the same function pays the full formula. The synthetic
  kills and score on the 97 results screen are NOT used.
* OPEN: objective events (capture points, batteries, bomb, victory items) need event counts the facts do not carry.
  `played_first_game` (+500 experience once): section 8 (no counter needed: a stored claim). The short-match multipliers
  (OFFICIAL 0.62b: x3 below 5 minutes, x2 5-10, x1.5 10-13 for prematurely ended matches) are not in the 0.69d data or exe;
  the server applies them by duration (section "Web round 2"). The match level of an online match: section 4b. Event drops
  951 / 1138 / 1139 (`service_needed 23`) are event currency: implemented, see `events.md`.
* Worn-item bonuses (`boosters::worn_bonus`): `modifications` groups 123 (silver), 124 / 141 / 185 (experience), 139 / 143
  (reputation) are on 400+ items as `default_modifications`: premium weapons +25 % silver (47 items), 27 weapons such as TT-33 +10 %
  experience, premium armour +10 % experience, decals +1 % .. +10 %; workshop rolls of the same groups count too. Added to the premium
  bonus per component (LIKELY additive: official "bonuses are additive"). Spare-parts group 147 is NOT in this vector: it is a percentage of the spare-parts drop (`boosters::parts_percent`, see
  decals.md section 5); 148 has no carrier (OPEN).

## 4b. Closed and open items

| item | state |
|---|---|
| spare-parts bonus groups | 147 implemented (decals.md section 5: `Drops::roll_with_parts_bonus`); 148 no carrier, OPEN |
| match level of online matches | PER PLAYER, CHOICE on OFFICIAL texts (`online::reward_level`; Guns Race notes 0.69b "The list of rewards does not depend on your maximum equipment tier", developers 2022-01-27 "if you are after rewards, you should play with a higher equipment tier"; docs/notes/online.md 7): the player's own equipment level (`items::equipment_level`, the client's number: pockets and empty stacks do not count), 1..=5 (the reward tables' index). The kit is the one the player has when the match ends. `ONLINE_MATCH_LEVEL` (1) is only the fallback when no kit can be read. The options byte `match_level` (+0x1081) stays 1: the client only logs it (PROVEN) |
| short-match multipliers | OPEN: official 0.62b says "less than 5 minutes x3, 5-10 x2, 10-13 x1.5" and "the reward for a prematurely ended match is proportional to the duration", but not the full length the proportion is taken against, nor what makes a match premature; neither is in the data or exe (`reward_factor` is read from another config). The server applies a CHOICE (section "Web round 2") |
| objective-event counts | OPEN: no source delivers them. The 53 flush has only the win / loss marker; the match server's observer (`Ended::stats`) reports kills, deaths and score per player and nothing about capture points, batteries, bombs or victory items. `MatchFacts::stat_events` is ready for them (keys of `stats_event_type`) |
| reward at a faction level change | PROVEN: `B1` type 4 `#ru[f:l][a][b]` reads only faction, level and the two premium numbers; `MessageFactionRank` lists the unlocked shop items or a fixed "+1 ability"; the reputation levels data has no reward field. No reward is read from any message: nothing to pay (an optional generic `B1` 0x14 reward notice would be a CHOICE). What the player sees is client-made from `B1` type 4 + `8E` (both sent): the window "N NEW ITEMS UNLOCKED" with the item tiles (`GangShop.getUnlockedItems`), a 15 s notification, a shine on the equipment / character tab, the shop's `isNew` marks (`GangShop.updateItemInfo`) and the new-items count on the shop button (`LobbyMenu.updateNewItems` -> `statusPanel.setNewItems`). OPEN: a level that unlocks no item shows "+1 ability" (button to the Character window); our skill points come from the character level only (`level - 1`), so if a faction level also gave a point in 0.69d we do not grant it (the client takes the count from 8C, it cannot tell) |
| wiki shop ranks 1..10 vs `item_level` 1..5 | settled as a relation, not a rule: over 268 shop items with both values `item_level = ceil(rank / 2)` for 190 (71 %); 64 more are one step off, 14 two or three (the cross-table is in `shop_069d.tsv`, columns `wiki_level` / `dict_level`). The client gates purchases by `item_level` only (PROVEN), so the dictionary stays the rule; the wiki rank is not used |
| equipment level of a player | the client's own number is PROVEN structure (0x1C7440 over 0x3330C0): the highest dictionary level byte over the worn non-pocket items of the playable profiles; `items::equipment_level` mirrors it (LIKELY that the byte is `item_level`). Used for the AE `items_level` (pve.md section 9) |

## 5. Pace (computed from the data with the formula above)

Assumptions (CHOICE, labelled): tdm, kills and assists as listed, no premium, one faction active, the data's silver factors.

Per-match reward (tdm, the scenario's event counts; exp / silver / reputation) by match level:

| scenario | L1 | L2 | L3 | L4 | L5 |
|---|---|---|---|---|---|
| weak (4 kills, 1 assist) | 46 / 360 / 100 | 55 / 720 / 100 | 58 / 1080 / 100 | 64 / 1335 / 100 | 67 / 1590 / 100 |
| average (8 kills, 3 assists) | 102 / 600 / 220 | 122 / 1200 / 220 | 128 / 1800 / 220 | 143 / 2175 / 220 | 148 / 2550 / 220 |
| strong (15 kills, 5 assists) | 185 / 975 / 400 | 222 / 1950 / 400 | 231 / 2925 / 400 | 259 / 3488 / 400 | 268 / 4050 / 400 |

Character level pace (average scenario; matches to reach the level from level 1, total experience in brackets):

| level | total XP | matches at L1 | at L3 | at L5 | level-up silver (that level) |
|---|---|---|---|---|---|
| 2 | 500 | 5 | 4 | 3 | 500 |
| 5 | 2170 | 21 | 17 | 15 | 500 |
| 10 | 5640 | 55 | 44 | 38 | 750 |
| 20 | 16710 | 164 | 131 | 113 | 1250 |
| 30 | 38060 | 373 | 299 | 257 | 1750 |
| 50 | 167590 | 1643 | 1314 | 1133 | 2750 |
| 75 | 1027420 | 10073 | 8058 | 6947 | 4000 |
| 100 | 6331640 | 62075 | 49660 | 42810 | 5000 |

Faction pace (average scenario, one faction active, matches from 0 reputation):

| faction level | reputation | matches at L1 | at L3 | at L5 |
|---|---|---|---|---|
| 2 | 5000 | 23 | 23 | 23 |
| 3 | 20000 | 91 | 91 | 91 |
| 4 | 80000 | 364 | 364 | 364 |
| 5 | 200000 | 909 | 909 | 909 |
| 6 (prestige 1) | 350000 | 1591 | 1591 | 1591 |
| 8 (prestige 3) | 650000 | 2955 | 2955 | 2955 |
| 10 (prestige 5) | 950000 | 4318 | 4318 | 4318 |
| 15 (prestige 10) | 1700000 | 7727 | 7727 | 7727 |

Silver for the whole faction shop of a wiki rank (sum of the silver prices of all four factions' items of that rank / 4, 
fan wiki 2022-05, data/shop_069d.tsv) against matches of the average scenario at match level = tier of that gear (rank 1-2 -> L1, 3-4 -> L2, 5-6 -> L3, 7-8 -> L4, 9-10 -> L5; CHOICE):

| rank | items (all factions) | silver per faction | match level used | silver per match | matches to buy the whole rank |
|---|---|---|---|---|---|
| 1 | 26 | 3760 | 1 | 600 | 6 |
| 2 | 33 | 10888 | 1 | 600 | 18 |
| 3 | 25 | 22750 | 2 | 1200 | 19 |
| 4 | 21 | 35350 | 2 | 1200 | 29 |
| 5 | 22 | 62850 | 3 | 1800 | 35 |
| 6 | 19 | 114625 | 3 | 1800 | 64 |
| 7 | 26 | 239250 | 4 | 2175 | 110 |
| 8 | 30 | 390375 | 4 | 2175 | 179 |
| 9 | 36 | 630000 | 5 | 2550 | 247 |
| 10 | 37 | 817000 | 5 | 2550 | 320 |

Reading: with these event rows an average player needs about 20 matches for character level 5, 55 for level 10, 1 100-1 600 for level 50
and ~43 000-62 000 for level 100; faction level 2 takes ~23 matches, level 5 (everything open by the client's rule) ~900, prestige 10
~7 700. Silver: the whole rank-1 shop of a faction costs ~3 760 = 6 matches, the rank-10 shop 817 000 = ~320 matches at match level 5.
The level-100 / prestige-10 totals look too slow against the official "274 players reached level 100 and Prestige 10 in all factions"
(Steam Q&A 2021-03): either the real server paid more per match than these rows (objective events, higher match levels, premium +50 %,
double-XP weekends x2.5 additive, decal bonuses up to +10 %) or those players played tens of thousands of matches. OPEN, not tuned: no
multiplier was invented.

## 6. Web

Sources: the Steam news feed (appid 355840) and web search.

* OFFICIAL: max character level 100 and Prestige 10 per faction; "274 people" have all of it (Q&A 2021-03, gid 4048146376729246088).
  0.66 (2021-03, gid 4059402835305839891): decals for levels 10..100 give +1..9 % experience, level 100 +10 % reputation; prestige-10
  decals +10 % spare parts. Silver depends on match points (Q&A 2021-11-17, gid 4227186657138481237); rewards are "a stimulus to purchase
  higher tier equipment" (2022-01-27); leavers get nothing (2022-01-27, gid 4237326104320437807); premium +50 % silver / reputation /
  experience / spare parts (gid 2415533798132485926); double XP and reputation weekend x2.5 with premium (gid 3658535001295488196);
  0.59f rewards points, silver and reputation for healing an ally, destroying an enemy medkit, disarming a supply; clan battles give no
  experience or reputation (0.65b, gid 3856733252006135358); factions "differ only in equipment" (2022-01-04); renaming for free after
  the tutorial (0.69, gid 4224936215405529242).
* PLAYER claim (2019 feedback, gid 2436926440567360452): three daily missions gave about "3000 silver or 1500 reputation".
* NOT found: experience per level, reputation per level, per-match numbers, a starter kit or starting money, tutorial reward amounts,
  how the faction is chosen. No usable fan numbers on pace (searches returned other games).

## 7. Summary

1. New accounts: data kit, 0 money / xp / reputation, faction 1 until the first choice (kit armour follows the choice).
2. Level cap 100; level-up silver paid.
3. Faction purchase gate = the client's rule; reputation only to the active faction (tests).
4. Match reward from `events.options` + `match_levels_factors` + `drop_after_combat` silver; worn-item bonuses; leavers nothing.
5. Data: `data/progression_069d.json`, `crates/data-extract`.
Open list: objective events, spare-parts bonus group 148, whether the original server gave anything at a faction level
change, the three `survival_*_notes_completed` awards, the second row of `game_world_tutorial_completed`.
Section 8: tutorial awards, first-game bonus, decals.

## 8. Tutorial awards, first game, decals

### Tutorial awards (PROVEN enum, schema v12 `claims`)
The `type` of an `unique_awards.options` row is the client's `survarium.constants.Reward` enum (lobby movie, class `Reward`,
values 0..10), proven by its readers: `ItemFactory.fillRewardItems` handles `ITEM` (param = dict id) and `EXPERIENCE`; the exe
sends the four tutorial rows to Flash with the keys `type`, `param`, `amount` (`root.set_tutorial_rewards`, 0x2001C8330, names
`tutorial_completed`, `survival_item_notes_completed`, `survival_prepare_to_battle_notes_completed`,
`survival_modifications_notes_completed`); `RewardRenderer` reads `param` of a MONEY row as the `Payment` enum (0 silver, 1 gold,
3 event coins, 4 craft details = spare parts, 5..8 vouchers / currency, 9 league) and `param` of REPUTATION as the faction.
Enum: 0 EXPERIENCE, 1 REPUTATION, 2 MONEY, 3 ITEM, 4 RATING, 5 POINT, 6 DETAILS, 7 PREMIUM, 8..10 event currencies.
Result: `tutorial_completed` = 1000 experience, 2500 silver, 200 spare parts (type 2 param 4 is spare parts, not
reputation).
Trigger (PROVEN): the client's tutorial finishes in step type 6 (0x2002CA070): when the account's tutorial flag (9D flags bit 5)
is clear it shows `show_tutorial_complete_message` with those rewards and sends `4A 01 01` through 0x200187970, whose only
caller is 0x2002CA247. The server pays on `4A 01 01` once per account (`awards::tutorial_completed`, claim `tutorial_completed`
stored in the `claims` table). The field that marks a finished tutorial in this code is the claim `tutorial_completed`
(`Profile::claimed(CLAIM_TUTORIAL)`); the 9D gate flags stay with the onboarding work (`onboarding.md`: bit 5 `TUTORIAL_DONE` is
set there by `4A 02` and by the tutorial match played to its end; the awards claim is a separate, once-only payment record, so the
two do not depend on each other. OPEN: the client sends `4A 01 01` exactly when it ends the faction step with bit 5
clear, so `TUTORIAL_DONE` could also be set on that request).
`game_world_tutorial_completed`: row 1 = ITEM 1132 (`drop_card_free`) x1, paid once when the tutorial match (second `5B 02`) ends
(CHOICE: the name says "game world tutorial"; no client string names it, the exe sends only the four keys above). Row 2 is type 4
(RATING in the client enum) with param 975 (`premium_access_voucher_3d`): the enum and the param disagree. The movie's `RewardRenderer` shows RATING as the "elo" icon and never reads `param`; `ItemFactory.fillRewardItems`
builds a reward item only for ITEM (3) and EXPERIENCE; this row is never sent to the client (the exe sends only the four
tutorial keys), no string names a premium gift for the tutorial. LIKELY (the param is an item id, amount 1) a data slip for a
3-day premium voucher, but nothing proves it and RATING with an item id is meaningless: OPEN, not paid (CHOICE).
`survival_*_notes_completed` (500 / 500 / 1000 silver): the encyclopedia notes; the Flash callback `survival_notes_readed`
(`MessageTutorialCompleteReskin`, argument `notesIndex`) has the exe handler 0x2001E9810 (registration idx 0x82, 0x26D43D),
which only calls one method of the movie object and returns: NO request is sent (PROVEN). No client message exists that could
trigger these three awards, so they are not paid (the data rows exist for the client display only). The `4A 01 01` request
is the tutorial-complete one, not a notes request. `tutorial_choose_faction`, `tutorial_change_name`,
`tutorial_change_head_and_name`, `tutorial_select_options` are empty rows (no reward).

### First-game bonus (data PROVEN, definition LIKELY)
Source: `events.options` row `played_first_game` = 500 experience (no money, no reputation, `reward_factor` 1). The client's event
name table (0x200986E10, index 27) lists it among the match events; no other exe string, lobby movie or news item defines the
trigger (searched exe strings, the lobby movie classes, Steam news 2021-2022, web search: nothing). So the definition is LIKELY:
the account's first finished match (a leaver pays nothing): the event counts once (`MatchFacts::first_game`, `event_counts`) and
the claim `played_first_game` is stored with the credit (`MatchOutcome::first_game`, `play::reward_messages`), for local matches
(srp and the default hook) and online matches. The tutorial match is not a game of the account (CHOICE: it neither pays the
bonus nor uses it up). The bonus is multiplied by the match level's experience factor and premium / booster percentages like any event.

### Decals
Built, see `docs/notes/decals.md` (items, sets, protocol 45 01 / 02 / 04, storage, buying at the dictionary price, worn bonus).

### Schema
Version 13: tables `item_decals` and `claims` (`CREATE TABLE IF NOT EXISTS`, idempotent; steps 11 and 12 belong to other branches).

## Web round 2

Era tags: [0.6x] 2019-2022, [0.5x] 2017-10..2019-02 still in force, [OLDER ERA] not adopted.
* Short matches [0.6x, OFFICIAL]: 0.62 (2020-03-04, gid 2579938650920880443) "The reward for prematurely ended match is
  proportional to the duration of the match"; 0.62b (gid 3893864668613542194) "less than 5 minutes: the reward is multiplied
  by 3, 5-10 minutes - multiplied by 2, 10-13 minutes - multiplied by 1.5". Still the rule in 0.69d (later posts only change
  when a match ends early: 0.66 point gap 40 for 12-15 players / 30 for 8-11; Q&A 2022-01-27: the match ends 30 s after half
  a team left; 0.69d: training with bots never ends early). Normal length 15 min (FAN 2019). How the proportional cut and the
  multiplier combine: NOT FOUND. CHOICE: `game_data::progression::short_match_factor` =
  duration / 900 s (the data's `match_time`) x the multiplier, applied to an online match's data reward before the bonuses.
* Objective events [0.6x, OFFICIAL 0.64 / 0.64b]: the score values of the 0.69d `events` rows ARE the official numbers (TDM
  kill 15 / assist 10 / death -2, S&D kill 30 / assist 15, battery carry up to 40 / stolen 60 / found 20 / delivered 10,
  Research kill 5 / attacker 10 / in zone 10, killstreak end 2). Silver / reputation per event exist (0.59f) but their values
  are data only.
* Local bot training counting for missions / achievements / league: NOT FOUND (only "training with bots is a local match").
* Spare parts per win (`details_add_win_bonus_modifier` 13): NOT FOUND; only percentage bonuses and the victory screen's
  "20+5" display (0.60b2).

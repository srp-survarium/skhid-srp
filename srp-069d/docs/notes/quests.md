# Quests, daily missions, daily login reward (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

PROVEN = read from the exe / AS3 / game data (address given); LIKELY = inferred; CHOICE = a server rule with no client
evidence.

## 1. What exists in 0.69d

* The exe loads `gameplay/quests/daily_quests.options`, `badges.options`, `achievements.options` (strings 0x200987A78 /
  0x200987AD8 / 0x200987B48, table 0x2002DACB0). It does NOT load `quests.options`: the old quest system (reroll
  waits, `quests_coef_data`, 54 quests) is dead data. The `92` flag (`set_sybsystem_active("Quests")`) therefore
  stays 0; nothing to implement for `quests.options`.
* "Quests" for the player = three daily quests in the lobby "actions" list (`LobbyMenu.set_daily_quest_data`, AS
  `MessageDailyQuests` / `DailyQuestProgresItem`).
* The daily LOGIN reward is a service record of the `94` offers (section 5), not a quest push.

## 2. Messages (decoded)

| message | layout | evidence |
|---|---|---|
| S->C `AB` (reader 0x2001881A0, block 0x20018E8CB) | `u32 time_to_expire_in_sec, u8 count (<= 3), u8 reroll_allowed, u8 free_reroll_count, count x {u32 quest_id, u32 dict_id, u32 param, u32 progress}, u32 received` (`received` = reward item dict id << 16 \| amount; 0 = chest not opened) | PROVEN; fills +0x79F730, UI 0x2001792A0 -> `root.set_daily_quest_data` (keys at 0x200962C50..0x200962E28) |
| `dict_id` | index into `daily_quests.options` `quests` (client table stride 0x130, filled 0x2002DCF7E..0x2002DD0A5: parameter, value, type byte, must_win byte, min_faction_level, need_item_of_category, name); no bounds check | PROVEN |
| S->C `B1` type 0x0E | `text` = `qg[uid][dict][param]` (granted, ignored when 3 are listed), `qu[uid][progress]`, `qc[uid][progress]` (completed: popup with the three reward lines); every message is answered by the client with request `51` | PROVEN 0x2001C2140..0x2001C28FA, tokenizer 0x2001C4750 |
| C->S `4A 03 u32 quest_id` | reroll (`daily_quest_reroll`, builder 0x2001C7BC0); the Flash dialog `reroll_option` shows price `reroll_premium_cost` gold and `free_reroll_count` | PROVEN (AS `DailyQuestProgresItem.handleRerollClick`) |
| C->S `4A 04 u8 0` | chest opened (builder 0x2001C79D0) | PROVEN |
| S->C `83 x 4A 04` | `u16 dict_id, u32 amount` -> `root.daily_quest_chest_reward_received` -> `MessageDailyQuests.receiveReward` | PROVEN 0x200191880 |
| other `83 4A` subs | sub 0 makes the client re-request `37` and `39`; sub 5 = faction-battle rewards `u8 n, n x {u32,u32,u16,u16,item}` | PROVEN 0x2001914A0 / 0x200191513 |
| C->S `4A 05 u8` | `receive_faction_battle_rewards`: answered (progression.md section 5; needs a battle schedule input) | done |
| C->S `5E u32` | `daily_reward_collect`; by the exe the u32 is the record's `callback_value` (its first u32, 0x2001C6DC2), but a live client sent 2373604912 for id 1 (OPEN, section 5); the server collects whatever id comes (one calendar record per account) | PROVEN sender, OPEN value |
| C->S `34 00`, `35 00`, `39 00`, `3A 00` | sent by the important-message handler (0x2001C3700 / 0x2001C3B05 = refresh the message list; 0x2001C31F9 / 0x2001C31D8 = faction-battle status; 39 = contract info, a `94` service record). None belongs to the daily quests; settled in requests-34-39-3a.md (3A -> 96, 39 -> 94, 35 -> 8E, 34 no reply) | PROVEN senders, replies not traced |
| `BC` | NOT quests: in-game shop actions data (`5C` reply: sub 0 `u32 id, u32 len, bytes`; sub 1 `u32,u32,u8`), 0x200196540 | PROVEN |
| `C2` | NOT quests: PvE (`sub 0 u32 map_id, u32 difficulty` -> `st_pve_settings_changed_by_commander`), 0x20018FB55 | PROVEN |
| `A6, A7, A9, AA` | hero of the day / weekly drop lists, not quests | hero-of-the-day.md |

## 3. Rules and data (`data/daily_quests_069d.json`, `crates/data-extract`)

Source: `gameplay/quests/daily_quests.options` and the drop tables `daily_quest_chest_N_drop` of
`gameplay/drops/chest_drops.options` (volume 00).

* 29 quests; `lifetime_in_minutes` 1080 (18 h), `free_reroll_count` 1, `reroll_premium_cost` 30.
* Reward rows by level (`min_level`..`max_level`): per completed quest experience / reputation / silver
  100/100/250 (levels 0-1), 250/250/500, 300/300/750, 400/400/1000, 500/500/1500 (5-15); all quests done = chest item
  `daily_quest_chest_N` (1157..1160, 1233). Type 0 = experience, 1 = reputation (shown with the ACTIVE faction, 0x1C2832),
  2 = silver (PROVEN from the UI key order 0x2001796xx / 0x1C27C6). Row = the highest faction level, capped at 5
  (PROVEN 0x2001883A0..0x20018840C); `faction_level` of game-data is the same index.
* Chest: weighted entries (`drop_weight`), one item of the entry at random, amount = the entry's fixed range. Rows: glock
  (1), drop card, premium voucher 1 day (id 213, weight 5), boosters, armour of the chest's level, spare parts (925),
  silver (214, 1000..5000 by chest), reputation (1164). The 1-day Premium voucher is therefore a CHANCE inside the chest
  (data), not a guaranteed item as in a 2022 Discord announcement; the voucher is a real stackable item and is
  usable (premium.rs). Ids 214 / 925 / 1164 are wallet entries (silver / spare parts / reputation), everything else an item.

## 4. Server (`servers/lobby-server/src/quests.rs`, accounts schema v7 `daily_quests`, `daily_quest_slots`)

* Assignment: at sign-in and on the first request after the set ended, 3 distinct quests at random from the quests the
  account may have. Set lifetime from the data (18 h from assignment): no calendar day boundary needed. CHOICE: free
  rerolls reset with each new set; LIKELY: `need_item_of_category` means "owns an item of that category".
* Progress is counted from one per-player facts structure, `game_data::MatchFacts` (section 4a). A fact the server does
  not have is `None` and the counter does not move; nothing is ever faked as 0. A leaver's match (no win / loss marker)
  and the tutorial count nothing.
* Completion pays the row at once (experience, reputation of the active faction, silver), then `qc`; progress sends `qu`;
  money / level / reputation pushes follow.
* Reroll `4A 03`: unfinished quest only; free while `free_reroll_count` is left in the set, then 30 gold; replacement not
  already in the set; the answer is a fresh `AB`. `reroll_allowed` = a free reroll is left or the gold is enough (LIKELY
  meaning). Refusals push the unchanged `AB`.
* Chest `4A 04`: only when all quests are done and not opened yet; draws, grants, answers `83 4A 04`, pushes 8B / 8C / 8E
  (+ 89 when an item came) and the `AB` with `received` set (so a reopened window shows the chest open).
* Expiry while online: `quests::tick` (every request) assigns and pushes a new `AB` after the set ended.

## 4a. Match facts: what each condition needs, what the server fills today

Entry point: `stats::record_match(cx, account, &MatchFacts)` (`servers/lobby-server/src/stats.rs`). It counts
achievements (`achievements::record`) and daily quests (`quests::record`) and delivers the messages to the account's
peer (`cx.fx`). The local match path calls it with `MatchFacts::local(map, mode, won, finished, faction)`; a future
match observer calls the same function with the other fields. Counting rules are pure functions
(`DailyQuests`/`QuestDef::gain`, `achievements::figure`) with unit tests that feed full facts.

| fact (`MatchFacts` / `KillFact`) | needed by | filled today |
|---|---|---|
| `mode`, `map`, `won`, `finished`, `faction` | game / mode / faction quests (types 0, 3, 4), win achievements 610-612, 616, 628, `is_won_match` of all | YES (53 flush marker + 5B request + profile) |
| `kills` (list), per kill `weapon_category` | kill with weapon class (type 5), 215-222 | no |
| per kill `kill_flags` (2 melee, 4 bleeding, 8 headshot, 16 ricochet) | event kill melee (13) / headshot (15), 202, 203, 206, 209 | no |
| per kill `inflictor_type`, `inflictor_id` | 205, 207, 226 (kills by one grenade / bullet / mine) | no |
| per kill `victim_state`, `victim_category` | 208 (reload), 211 (anomaly), 228 (sniper in the sight) | no |
| per kill `killer_hp`, `after_own_death`, `grenade_in_hand`, `last_bullet`, `no_scope` | 210, 212, 213, 227, 224 | no |
| `kills.len()` | event kill (type 7 / 0), 201, 601, 602 | no |
| `deaths` | 601 (win without a death) | no |
| `score` | match score (type 6, best match) | no |
| `max_killstreak` | killstreak (type 8), 214 | no |
| `artefacts` | event artefact (type 7 / 5), 407 | no |
| `stat_events` (1 assist, 24 battery, 26 battery stolen, 29 capture points, 47 artefact AH) | 204, 617, 618, 619, 626 | no |
| `badges` (type, state) | 603-607, 622 | no |
| `in_squad`, `in_clan_squad` | squad quests (type 1), 418, 419 | no (a local match has no squad) |
| `match_type` (1 random, 2 rating) | 608, 609 | no (a training match is neither) |
| `score_diff` | 613-615, 627 | no |
| `killed_all_enemies`, `duration_secs` | 223 | no |

Counting: a quest / achievement gets `Add(n)` (counter: games, kills, events) or `Max(n)` (best figure in one match:
score, killstreak, apply-0 achievements). `must_win_to_complete` and `is_won_match` make a lost match add 0. PROVEN
by the names: kill flags, `event_*` parameters, `stats_event_type` numbers; LIKELY: the inflictor / victim state
numbering, "ways" of achievement 229, apply 0 = best figure of one match (601 / 602 / 208 ...), match-type bit numbering.

Not covered by `MatchFacts`: customise / reroll / upgrade (403, 404, 420), daily quests (620), treasure hunt (406), PvE
wins (629, 630) and hero of the day (623-625) are lobby events, counted where they happen (`achievements::on_event`,
progression.md section 1). No fact source at all, so they stay at 0: modules played with (415-417, rule not
established), health regeneration (402), anomaly destruction on low health (414), squad assist (225), squad
best-match badges (621).

## 5. Daily login reward: implemented, reward table is an input

PROVEN: it is a service record of `94` (offers; reader 0x200189F90): per record `u32 id (= callback_value for 5E), u32
(entry+8), u32, u32, u32, u32 len1, u32 len2, u8 type (service id), u8, len1 bytes (<= 0x800), len2 bytes (<= 0x800,
the body)`. Type 0x1B body (reader 0x2001B9040, emitted 0x2001C6B62): `u16 n, n x {u32 amount, u16 reward dict id, u8
award_quality}, u8 has_state; with state: u64 received_mask, u8 current_day, u8, u8`; `today_reward_received` =
mask bit of `current_day` or `current_day >= n`. Type 0x2D = 7 `simple_daily_reward` x 8 bytes (reader 0x2001B92C0, the
"seven days streak" window). `protocol::s2c::quests::BonusBody` encodes / decodes the 0x1B body (tested).
The body travels as a RAW LZMA2 STREAM in len2 (<= 0x400): the
status builder 0x2001C6350 decodes data2 of types 8 / 0x17 / 0x1B / 0x2D / 0x2F with the 8A LZMA2 decoder (output <= 0x800,
0x2001C64DE) before the reader; the reader holds at most 64 days. A plain body makes the client assert
`buffer_vector<daily_reward>::resize`. `tests/client_94.rs` checks the whole sign-in 94 against a model of the client.
Done in `login_calendar.rs` (progression.md section 6): the per-account day counter, `5E` collect + payment, the record
in every 94. The server does not check the `5E` id: a live client sent 2373604912 for record id 1 every time, and a refused
collect reopens the dialog with every 94 (`LobbyMenu.set_actions_data` opens `daily_bonus` while the 0x1B action says
`today_reward_received` false and no `MessageDailyBonus` is open). `5E` has one sender (0x2001E9760, callback index 0xAC) and
an account has one calendar record. Where 2373604912 comes from is OPEN (the traced path gives the record's first u32; the
value changes per client process in bits 16..23, LIKELY the low half of an address).
The window (PROVEN, `BonusDayRenderer.setData`): per day the reward item's icon, `award_quality == 1` -> a star, and as
amount text only the `labelValue` of the reward item's own modifiers (`default_modifications`); the record's `amount` is
not shown (only the 7-day streak window, type 0x2D, shows `"x" + amount`). Currency rewards (214 / 925 / 215) have no
modifiers: icon only. Day 15 is drawn as a secret day.
Default table (CHOICE: boosters and vouchers, not currencies; no source has the original table): the window's icon-map items (`BonusDayRenderer.dictIdToIconFrameMap`; 852 / 860 / 89 are not in the
0.69d data) and the related per-match boosters, escalating: 1 Tornado Energy Drink 340 x1 (+5 % max movement speed),
2 Rations 341 x2 (+2 health regeneration), 3 Radio 342 x2 (respawn time -0.30), 4 Stash map 1132 x1 (free treasure-hunt
drop card), 5 Reputation booster 861 x1 (+50 %), 6 Experience booster 853 x1 (+50 %), 7 Silver booster 857 x3 (+50 %,
star). Collecting puts the item into the inventory (`chests::grant`) and sends its record with the received-item popup
(`83 46 00` flag 1) and 8B; a currency day (configured table) is credited and sent as 8B. The reward table exists in no game file (progression.md section 6): it is the setting
`LobbyConfig::login_calendar` (`[[login_calendar]]` in the config; none = the default table above).

## 6. Open points and checks in the game

* AB in the login pushes shows the three quests with progress, the countdown, the reroll icon; check the chest window
  after all three are done (`received_reward` 0 = "receive" button), and that the quest popups (`qc`) appear.
* LIKELY, not proven: `B1` type 0x0E needs the lobby loaded bits (queued by the client otherwise); `89` replaces the
  inventory; the `AB` re-push after a reroll replaces the list.
* `4A 05` faction battle rewards, `35 00`, the login calendar: implemented with the schedule / reward table as inputs
  (progression.md sections 5 and 6); `34`, `39`, `3A` stay unanswered.

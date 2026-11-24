# Progression: achievements, skill points, queue estimate, quests (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Status words: PROVEN = read from the exe / AS3 / game data (address given); LIKELY = inferred; CHOICE = a server rule
with no client evidence.

## 1. Achievements (done)

Data: `gameplay/quests/achievements.options` (79 achievements, 3 categories) extracted by
`crates/data-extract` into `data/achievements_069d.json`; loaded by `game_data::achievements`.
Each achievement has `type` (the "kind"), `apply_type`, `preconditions` and `states` (condition, score, reward).

Wire (PROVEN, exe):

* `B9` (reader 0x200194F40): `u32 N, N x {u16 id, u32 value, u8 state, u8 lo, u8 hi}`. `value` is the progress
  number; `state` is the client's `current_state` = the number of states reached (0 = none, all = the state count);
  for each `i` in `lo..hi` a "state reached" message `{id, i, 1}` is queued (<= 200 in total, assert 0x1A4). LIKELY:
  `i` is the 0-based index of the state just reached (the popup text shows that state).
* `83 55 02` (0x200194D78): `u32 n, n x {u16 id, u8 state, u32 value}` (same setter, no popups).
* The `state` byte:
  - Both readers store the entry as `{u16 id, u8 state, u32 value}` (byte at +2: 0x20019502C, 0x200194DE6).
  - The window object 0x2001C98C0 (`root.set_achievements`) sets `current_state` = the byte, `progress` = value,
    `achieved` = byte != 0, and `current_description` = state[byte]'s text if byte < the state count, else the
    achievement's own text.
  - In the client descriptor (loader 0x20034CA80) `type` is at +0x04 (0x20034CAF4), and +0x0C is the number of
    `states` (0x20034CC34). The 0x2001956F4 compare ("byte == state count") runs only with the Steam sign-in
    (sign-in type 2 at auth +0x610, start flag `-steam`) and gates only `ISteamUserStats::SetAchievement`.
  - AS3 `AchieveWindowLoader.createAchievement`: `current_state` n shows states[n-1] (states[0] for 0), next
    threshold = states[n].condition, score = sum of the first n states. `AchievementRenderer`: alpha 0.3 unless
    `achieved`, level pips = n of `states.length`.
  - Server: `achievements::entry` sends `Achievement::reached(value)`.
* `83 55 03` (0x200194CE1): `u8 n, n x {u8 category, u16 score}`. The category totals in the file (1665 / 1205 / 2150)
  equal the sum of the state scores of the category's achievements (checked), so the score is the sum of the scores of
  the reached states (LIKELY).

Server (`servers/lobby-server/src/achievements.rs`): progress per account in `Profile::ach` (table `achievements`,
schema v3). B9 is sent at login (all achievements, `lo == hi`, no popups); `55 04` answers with `83 55 02` and `83 55 03`
from stored progress; changes are pushed as B9 with the reached range (popups), plus the inventory `89` when a reward item
was given (LIKELY: 89 replaces the client's inventory list).

The client does not count: it loads the descriptors (loader 0x20034CA80; the switch on `type` at 0x20034D041 reads
the preconditions of each type, e.g. `num_stashes_found` only for type 29 at 0x20034D291, `nomination` only for type 30
at 0x20034D3D1) and shows the values the server sends.

Counted from real facts only (kind -> source; the module docs of `achievements.rs` hold the same table with the
evidence of every rule):

| kind | achievements | source | rule (label) |
|---|---|---|---|
| 2 | 610-612, 616, 628 (wins by mode, win streak; `mode_mask` bit n = game mode n, 32 = Search and Destroy) | a finished match that counts (local bots matches count nothing, section 9): its outcome and mode; tutorial and matches left early do not count | |
| 0-18 (match facts) | combat, stats events, badges, artefacts | `MatchFacts` of a finished match (`figure`) | a missing fact moves nothing; local matches have none |
| 19 | 420 upgrade_item | `45 08` (`workshop::craft_pick`): a modifier was added | best quality an upgrade took an item to: rank after it, 1 green / 2 blue / 3 orange (LIKELY, see below) |
| 20 | 404 reroll_item | `45 06` (`workshop::reroll_pick`): a modifier was replaced by a different offered one | best quality of an item whose modifier changed (LIKELY) |
| 21 | 413 level | `level_of(xp)`, refreshed at login, after a match, a skill change and every lobby event below | |
| 22 / 23 | 408-412 faction ranks | `faction_level(points)`; faction n has `faction_mask` bit n-1 (LIKELY) | |
| 24 | 403 customize_equipment | an accepted `45 01` (`decals::relocate`) | 1 (LIKELY) |
| 25 / 26 | 401, 405 skill branches | learned skills; branch = id / 100 | 401: the fewest points of the branches in `skill_branches_mask`, reached at 5 ("a minimum of 5 points" in every branch); 405: branches with at least the data's `points_*` (20) |
| 27 | 620 complete_gold_quests | `quests::record` | +1 per completed daily quest (LIKELY) |
| 28 | 406 search_all_stashes | `46 07` (`hunt::search_request`) | 1 when the find leaves no item of the page unfound (PROVEN text) |
| 29 | 629, 630 win_pve | `pve::record_run_notify` / `record_run`, only when the match id is stored the first time | +1 per player of a won mission with caches found >= `num_stashes_found` 3 / 10 (LIKELY; every player: CHOICE) |
| 30 | 623-625 hero_of_the_day | `hero::tick`, each title of a newly calculated slot | best place: third 1, second 2, first 3 (PROVEN texts) |

Evidence (0.69d only):

* 420 / 404: state texts "Upgrade piece of equipment to green / blue / orange quality" and "Change modifier on a green /
  blue / orange piece of equipment" (`st_achievement_desc_upgrade_item_1..3`, `..._reroll_item_1..3`). An item's
  colour is `Colors.RARITY_<rank>` (AS `ItemData.getOverFrameColor`; `RARITY_1` 0x92C025 green, `_2` 0x429BF6 blue,
  `_3` 0xFF6825 orange), a module's is its rarity; rank = `min(3, base_rarity_level + modifiers)` (0x24966D). The
  item upgrade is "Upgrade equipment" (`st_item_craft_message_label`), the module merge "Improve module": only the item
  upgrade counts for 420. Both have `apply_type` 1, yet a count of events would reach "orange" after three green
  upgrades; the progress is the best quality, so a state is reached exactly when its text is true.
* 403: "Change appearance of an item"; the camouflage / decal editor is the "Appearance" button
  (`st_btn_upgrade_label`, `st_btn_close_upgrades_label` "Finish changing item's appearance"). Weapon skins (`45 11`):
  OPEN, not counted.
* 406: "Receive all rewards in the Treasure Hunt".
* 620: "Complete %d daily quests"; the "gold quests" of its id belong to the old quest system the 0.69d exe does not
  load (quests.md 1).
* 629 / 630: "Win %d matches in team missions", "Find all stashes in a team mission"; a mission has `chests_total` 10.
* 623-625: `nomination` 1 / 2 / 3 = Best Player / Killstreak / Winstreak (achievement texts, window order 0x2001CA550);
  the state texts give state 1 = third place, 2 = second, 3 = first. Friends / clan rankings are "not rewarded" and
  count nothing.

Choices: the streak figure is the best streak (`value`), the current one is `run`; progress never goes back; a reward item
the account already owns is not given twice (all rewards are one-of decals / camos); `mode_mask` bit n = game mode n of the
client's mode table (gvi 1, tdm 2, capt 4, sad 32; match-stats.md section 7). Lobby events count in the
same profile update as the action itself (upgrade, reroll, decal, hunt find, quest completion); PvE and hero titles in
their own update right after the run / slot is stored, so a repeated report cannot count twice.

**Open (no fact on the server, not faked):** kills, assists, headshots, melee / grenade / mine / bleeding / ricochet
kills, killstreak and every other combat achievement (201-229): the client reports no kills or deaths to the lobby
(the numbers of the 97 screen are synthetic and are not used); batteries (617, 618), capture points (619), artefacts
(407, 626), badges (603-607, 621, 622), health regeneration (402), anomaly on low health (414), squad / clan matches
(418, 419: a local match has no squad), rated / random matches (608, 609: a training match is neither) and
`score_diff` wins (613-615, 627: team scores are synthetic): `figure` counts them once a match source fills the facts.
Items with random modifiers (415-417, kind 8): counted per item at the end of a finished online match, see section 8. Weapon skin changes for 403: see above. All of these stay at 0 and the window shows them at 0.

## 2. Skill points at level 1 (done, choice)

PROVEN (AS3): the level-up message (`MessagePlayerRank`) shows `"+" + int(levelOffset || 1) + " " + getName("ability")`
and `LobbyMenu` sets `levelOffset` to the number of levels the reward list passed: each level GAINED gives one ability
point. The exe takes available = the "unlocked" byte of 8C/87 minus learned, and neither the client nor
`game.options` has a starting bonus. So unlocked = level - 1 (level 1 = 0 points), plus the prestige points of section 7. Rule in
`Levels::skill_points_for_level`. LIKELY, not PROVEN: nothing in the files
excludes a start point that the real server granted outside the level-up message.

## 3. 83 2F estimated time (real queue, formula)

PROVEN layout: `83 2F u32 a, u32 b`; `a` only logged, `b` -> `root.set_mm_estimated_time(b)`; a value > 0 shows the
"estimated time" label, 0 hides it (seconds, LIKELY). `2F u32 server, u8 type (2 = rated), u32 game-mode mask (0x380
seen), u8` starts a search; `85` state 1 / 4 = `u32 order, u32, u32` (the reader 0x200186280 stores them; state 2 =
`u32 order, u32, u8` match found; 0 = idle), `31 u32 order` cancels.
Server (`servers/lobby-server/src/queue.rs`, state in the `Registry`): one ticket per account or squad (accounts, mask,
rated, join time), a log of recent joins and of started matches. `2F` enqueues (a second press shows the running
search), `83 2F order, estimate` then `85 01 order 0 0`; `31` or a disconnect removes the whole ticket (the other members
get `85 00`). Estimate: the mean wait of the last 10 started matches with an overlapping mask, else
`(min_players - players waiting with an overlapping mask)` over the arrival rate of those players in the last 600 s
(`players after the first / span`, span at least 60 s); 0 while fewer than two players ever arrived or nobody is
missing. The match itself needs the match server (`online.md`: with the world host serving the row, a full group becomes a match and every
member gets `82`); otherwise, when the queue holds `min_players` players (the smallest of the
`game_modes` rows; OPEN: the mask-to-mode mapping) the group leaves the queue and every member gets `85 00` (refused),
the log names it. The words after the order in `85 01` are sent 0 (their use is not traced). A hook for the match
server exists (`PvpQueue::group` / `record_start`).

## 4. Quests, daily rewards, chests

Daily quests, their chest and the daily login reward: see `quests.md` (the `AB` push is the daily quest list;
`quests.options` is not used by the 0.69d exe). `34 / 39 / 3A`: requests-34-39-3a.md; `35`: section 5.

## 5. Faction battles (4A 05, 35 00, the 8E status): implemented, schedule is an input

Server: `servers/lobby-server/src/faction_battle.rs`, data `data/faction_battle_069d.json` (the `faction_battle_drop`
table of `drops.options`: per faction 5 reward tiers at 10000 / 20000 / 40000 / 60000 / 80000 reputation: 500 spare
parts, a 3-day Premium voucher (975), then one item each; `item_to_drop_after_win` 2086), accounts schema v8
`battle_rows`.
PROVEN (exe + AS3): the status is the optional 40 bytes of `8E` (`u8 k != 0`, then `[time_left, mask, rep1, got1 .. rep4,
got4]`; reader 0x20018AC70, `root.faction_battle_status_received` 0x2001CAFC0 -> `WinterEventData.setBattleStatus`):
the two factions whose mask bit (1 << faction) is NOT set are fighting; `time_left` counts to the end of the battle, or to
the next one when nobody fights ("Next battle starts in"); a tier is received when `2^tier - 1 < got`, so the server
sends `2^n - 1`. `4A 05 u8 faction (1..4)` (the AS index + 1, sender 0x2001C7AB0) -> `83 4A 05 u8 n, n x item record`
(0x191513, important messages "received: item xN"; the client does not add the items: the server pushes 89 / 8B).
`35 00` (`refresh_faction_battle_status`) wants the 8E again.
NOT in any file: the battle schedule (the window links to the website timetable), so `LobbyConfig::faction_battles`
(`Schedule`, list of `{start, end, factions}`) is an input and None by default: no battle runs, `k = 0`, `4A 05` is
logged. CHOICE: battle reputation = the faction reputation of finished matches played with that faction active while it
fights (`MatchFacts::reputation`); all reached tiers are paid at once, once per battle; a finished battle's row stays
claimable until the faction's next battle replaces it. OPEN: 2086 after a won battle match, the real schedule, whether
`34` has an answer (none expected); `39` -> the `94` and `3A` -> the `96` mask are answered (requests-34-39-3a.md).

## 6. Daily login calendar (5E, the 94 record 0x1B): implemented, reward table is a setting

Server: `servers/lobby-server/src/login_calendar.rs`, accounts schema v8 `login_calendar`. Layout: quests.md section 5.
No game file holds the reward table: the `gameplay/**` files, the localization tables, the lobby AS3 and the exe strings
carry only the window texts (`st_login_reward_header`, `st_daily_bonus_*`; `st_daily_bonus_description` belongs to the x2
victory bonus, a different feature). The only item hit is the weapon `colt_m1911_daily_bonus` (dict 1162, `game.options`): LIKELY
a reward of the old calendar, but no table points at it (the original server authored it in the 94 record). So the table is the setting `LobbyConfig::login_calendar`
(`Vec<BonusDay>`, `[[login_calendar]]`; none = the default table of quests.md section 5). CHOICE: one collection per UTC day, consecutive days continue the
cycle, a missed day restarts at day 0, the day after the last starts a new cycle; the record id is 1 (`5E` sends it).
The record is added to every `94` the server sends (`login_calendar::with_calendar`).

## 7. Skill points from prestige ranks (web sources)

* OFFICIAL 0.56 (2018-12-20, gid 2417769111284025080): "Each Prestige rank rewards your character with a skill point." Prestige
  ranks are faction levels 6..15 (10 per faction, 150 000 reputation each after level 5 at 200 000; the 0.69d thresholds
  match), so up to 40 points over the four factions. No 2019-2022 post removes it. Faction levels 2..5: no source gives a
  point (NOT FOUND). It fits the client: the faction window shows "+1 ability" only when a level unlocks no shop item.
* Character level: one point per level (FAN, plus an OFFICIAL 2021-05 Q&A that assumes it). Feb 2022 Q&A (OFFICIAL): even
  prestige-10-in-all players lack "enough skill points to learn all abilities".
* Server: unlocked points = `level - 1` + one per prestige rank reached in each faction (`game_data::levels::prestige_skill_points`, `build::skill_points`; 8C pushed with the #ru of a prestige level; test `prestige_ranks_add_skill_points`).

## 8. Achievements 415-417 (random modifiers) and other match-data achievements

The client has no achievement evaluator (only the loader and the window); meanings from the data and
the English texts, all LIKELY: 415 / 416 / 417 "equip items with at least one / two / three random modifiers" (per item; the
workshop text says the maximum is 3); 402 "Restore more than %d% health in one life" (best restored in one life, 80..300 %);
225 "Kill or assist in killing an enemy that has taken damage from all of your team members"; 621 "Get the squad all four
post-battle medals in one match"; 414 "Destroy the anomaly having 1% of health". Only 415-417 can be decided by the lobby
(CHOICE: ANY equipped item, not EVERY equipped item, must have the count); the others need match data.
Rule (ANY item): the data's states have conditions 1 / 2 / 3 with apply 0 (a current figure) and the texts say "Play a
match with items having at least N random modifiers", so the figure is counted only at the end of a finished online match
that counts for achievements (not left, not inactive; no clan battle, custom training or local match): the most random
modifiers on one item of the loadout the player played the match with (`achievements::match_mods` -> `MatchFacts::item_mods`,
filled in `online::pay`, counted by `on_match` / `figure`). The loadout is the profile as it was at the match start
(`achievements::start_mods`, one figure per options profile); of it count the options profiles he spawned with (the world
host's `Used` row, protocol 6), or, without that row, options profile 0, the profile the options start him with (LIKELY:
he may switch before his first spawn). Wearing the items counts nothing. The count is the item's stored modifier list
(workshop upgrades, treasure-hunt hidden modifiers); slots hold no module, so no rarity pair (312) or override pair is in it.

## 9. Local matches count nothing

Local training matches with bots count for NOTHING (CHOICE; the client: "Rewards in this mode are not awarded"; no
source says they counted): `stats::record_match` returns before achievements, daily quests, the hunt card, the hero rows,
the league and the clan.

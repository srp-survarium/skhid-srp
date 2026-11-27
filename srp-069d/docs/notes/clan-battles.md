# Clan battles (custom match type 1), lobby side

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Code: `servers/lobby-server/src/rooms.rs` (rooms, clan rules, ready), `online.rs` (`MatchKind::Clan`, start and
results) and `stats.rs` (`MatchFacts.clan_battle`). Tests: `servers/lobby-server/tests/clan_battle.rs` (scripted clients, two
clans, a fake match server like `tests/online.rs`). Labels: PROVEN (exe address / movie / data), LIKELY, CHOICE, OPEN.
Sources of the evidence: the 0.69d exe (`survarium.exe`, x64 Steam build), the ABC of the lobby movie
(`CustomMatchWindow`, `CustomMatchPlayer`, `MatchMakingSettings`), the game's English localization table and the official
posts of the 0.65b / 0.65 era.

## What a clan battle is

* A custom match with `custom_match_type` 1; type 0 is the training match. PROVEN: the create dialog
  (`MatchMakingSettings.as`, handler of the OK button) sends `62 18 u8` with 0 when the training check box is selected, else 1;
  `CustomMatchWindow.setData` tests `custom_match_type == 0` (`bIsTraining`) and the label is `match_settings_clan_match`
  ("Clan match") otherwise.
* The create dialog offers the clan check box only when `PlayerInfo.clanState` is 3 or 4 (member / commander of a clan; set by
  `root.set_clan_window_state`, which `clans.rs` already drives through 9A). PROVEN.
* OFFICIAL (0.65b notes): "you can create custom matches of two types: training and clan. ... you can invite anyone to training
  matches, but only players of your clan and one other clan may join clan matches." Participation cost of a clan battle is a total
  sum that every player may pay in any amount, reduced by each player with a premium account (the same text is the tooltip
  `custom_match_premium_player_tip_clan`). "Clan matches follow the same rules as ranked matches: there's no killcam and no markers
  when hitting characters." Later notes: manual matchmaking was the idea from the start ("the whole idea behind clan battles is to
  provide a tool allowing you to find out who is the best"); no ladder (Q&A 2021-03).

## Wire facts

| Fact | Evidence |
|---|---|
| 98 member `team` is the Flash `role`: **0 team A, 1 team B, 2 unassigned**. The reader stores the byte at member +0xB6 (0x20018BA00), 0x2001C8DA3 puts +0xB6 under the key `role` (string at file offset 0x96A110), `CustomMatchPlayer` shows the "join A" button when `role != 0`, "join B" when `role != 1`; `62 1D` sends `role + 1` (1 A, 2 B, 3 unassigned). | PROVEN |
| `62 22 u8` is the per-player READY toggle: the byte is the NEW state, sent as 0xFF / 0 (0x20018D5F0 turns the flag into that byte; called from the Play button 0x2001E82B0 with `ready == 0` for a non-host, and from `83 30`). | PROVEN |
| 98 member `ready` is stored at +0xB8 and counted per role 0 / 1 by the movie (`readyCountA` / `readyCountB`). | PROVEN |
| The Play button of the host (+0xB7 == 1) sends Start (`62 23`) only when `tokens allocated (L+0x79F727) >= tokens required (L+0x79F728)` and every member that is not the host and has role 0 **or 1** is ready; otherwise the client shows its own text. | PROVEN 0x2001E8161..0x2001E81A6 |
| `83 30` (no payload): the client calls `process_succeeded_check_ready_for_clan_match` (no such function exists in the movies: nothing visible) and then **sends `62 22` ready** by itself (0x20018F337 -> 0x20018D5F0 with 1). So it is the server's "ready check": it makes the receiving client answer "ready". | PROVEN effect |
| In a clan match (type 1) the 98 clan ids / names (`clan_a`, `clan_b`, `clan_a_name`, `clan_b_name`) reach the movie; for type 0 the exe leaves them out (0x2001C8765 tests `type == 1`). The movie writes the two names as the team labels and offers a member the "join A" button only if `clanAId == member clan_id` and "join B" only if `clanBId == clan_id`. Bots (`max_ai_count`) and the global chat box are hidden for non-training matches. | PROVEN |
| Refusal texts the client has for clan matches (`84 62` text variant is client-shown): "The player is not in a clan, so he cannot participate in a clan war", "This player's clan is not participating in the match", "Player is already in a match or looking for a match", "Players in the squad cannot participate in a custom match", "Unable to start the match: some players are not ready", "No free player slots on the team", "To start a match, both teams must have players". Used verbatim. | PROVEN texts (localization table), the rule behind each is LIKELY |

## Rules implemented (`rooms.rs`)

* **Create** (`62 18 1`): the host needs a clan (CHOICE text "A clan match needs a clan") and the clan rank permission "battle
  commander" (`accounts::clans::PERM_BATTLE`, bit 0): LIKELY, the exe has `update_battle_commander_permission` and the clan UI's
  five permission switches, no other user of the bit exists. Not in a squad of 2+, not in another room, not in a queue or a match
  (the game's texts). Team A starts as the host's clan.
* **Invite / accept**: target must be online, not busy (squad, room, queue, match), in a clan, and that clan must be one of the
  room's two clans or fill the free side. The second clan is the first foreign clan that joins; its side frees when none of its
  members is left. Everything is re-checked when the invite is accepted. LIKELY (texts PROVEN, official sentence "your clan and one
  other clan").
* **Teams** (`62 1D`): side A needs a member of the room's clan A, side B of clan B (the movie hides the other buttons: PROVEN;
  the server enforces it), team sizes bounded by the row and the host's setting. A move takes the member's ready back (CHOICE).
* **Ready** (`62 22`): stored per member, shown in the 98; the host counts as ready and cannot toggle. The same server rule applies
  to training rooms (the client's Play button code does not look at the type).
* **Swap** (`62 24`): the sides swap together with their clans (the movie ties side A to the clan A label).
* **Bots**: refused in a clan room (the movie hides them; the 98 offers none: `max_ai_count` 0).
* **Start** (`62 23`, host only): both teams have players (unassigned members are the reserve and stay out), no team larger than
  its setting, no bots, two different clans, each member in the clan of his side, every team member (except the host) ready,
  online matches on and the match server serving the room's row (social.md section 4). Otherwise a client-shown text. When members are not ready the server also sends **`83 30` (ready check)**
  to exactly those members; the trigger is a CHOICE (the stock client blocks Start itself and never sends it with unready members,
  so this only matters for a race), the effect is PROVEN. Then `online::start_room(.., MatchKind::Clan([clan A, clan B]), ..)`:
  team A is side 0, team B side 1; the room closes (`A0` to the players).

## Results

* The clients' win / loss reports and the verdict are the normal online path (`online.rs`: events 5 / 6, majority vote,
  `decide_winner`), because the stock client reports the same way in every online match.
* **Clan experience**: each finished, reported member adds to his own clan (`clans::record`, `game_data::clan::match_xp`:
  20 win, 10 loss, CHOICE values; the formula and the level table are CHOICE too, the 0.69d data has no clan experience source).
  `MatchFacts.match_type` is 3 for a clan battle (CHOICE; no achievement `match_type_mask` uses bit 3).
* **Rewards**: OFFICIAL 0.65b: "custom training matches do not yield rewards (like regular PvP matches). Also, the character's
  experience and reputation with factions are not awarded, equipment progress is not tracked. **However, you will get rewards for
  clan battles.**" The kind and amount of the clan battle rewards are not stated in the exe, the data or the posts. LIKELY: they
  are the ordinary ones. CHOICE implemented: a clan battle pays the same data formula as an online match (`match_reward` of the
  `events.options` rows, the `drop_after_combat` silver roll and the drops), to winners and losers who reported, nothing to leavers
  or when the verdict is unknown. OPEN: if a later find shows other rewards, change `pay` for `MatchKind::Clan`.
* **No rating**: there is no clan rating, ladder or league for clan battles (Q&A 2021-03: "probably not a ladder", matches are
  arranged by hand; `leagues.options` has no clan table; `league::record` only handles `match_type == 2`). ABSENT; not built.
* **Achievements, daily quests, treasure hunt, league**: not counted (`MatchFacts.clan_battle`, `stats::record_match`). Evidence:
  0.69c dev notes "achievements can no longer be earned in custom matches". Nothing says anything about quests or the hunt: they
  are skipped because a clan battle is a custom match and no evidence says otherwise. The faction battle still counts the
  reputation the battle paid.
* **`MatchFacts.in_clan_squad`** is the fact of achievement 418 ("Play N matches in a squad with your clanmates",
  `is_in_squad` + `is_in_clan_squad`), which is about normal random / rated matches played in a squad, NOT about clan battles.
  It is filled for every online match (`online::squad_flags`): the account is in a squad of 2+ and has a clan, and another
  squad member is in the same clan (CHOICE for the exact reading). A clan battle has no squads: false.

## OPEN (not built, nothing faked)

* **Participation cost** (`62 21`, 98 `tokens allocated / required`, `token_cost_gold / silver`): the client has the whole
  payment UI and the official text describes it, but no data file, exe table or post gives the cost of a clan match. The
  server's token rule is a CHOICE (section "Custom match cost").
* **No killcam, no hit markers** in a clan battle (OFFICIAL 0.65b; implemented): the killcam
  (0x2001E47CF), the hit marker (0x2001A546C / 0x2001A547E), the kill marker (0x20023830E / 0x200238320) and the protection marker
  are off when the options match-type byte (+0x107D, stored at 0x2001251C8; the u8 after `b3`) is 3 or 6 (PROVEN). `show_killcam`,
  `show_enemy_damaged_marker` and `enemy_hit_mark_timeout_in_ms` are client settings, not options. All 22 readers traced (PROVEN): 3 / 6 = the HUD switches above; 1 / 3 = the inactivity
  warning after 180 s / 300 s and a per-player inactive flag (no kick, no charge in the client); 4 = unlimited ammo,
  grenades, placeables and medkits; 0, 2, 5, 7+ = no reader. Server: `MatchKind::options_match_type` -> clan battle 6,
  ranked 3, everything else 0 (`protocol::s2c::online::MATCH_TYPE_*`, through `OnlineMatches::create`). OPEN: the
  inactivity fine itself (`inactivity_fine.options`: 1000 money, escalating ban times) is the server's and needs an
  inactivity source; the 97 header `b3 = 3` (`rating_match`: league row, `points_league_progress`) is not sent yet.
* Clan window "ready players" (`root.set_clan_ready_players(value, total)`, text "Ready", 0x27FF45), PROVEN: `value` =
  the number of `9A` members whose byte +0xC is 0 (lobby+0x79C518; the own-clan reader never writes +0xC, so it is the member
  count), `total` = byte 0 of `9C` (lobby+0x7A33B0; Flash default 10). `9C` does not refresh the window: `total` shows at the next
  `9A`. This counter belongs to the clan CREATION window (`CreateClanWindow`), not to clan battles: text `"READY" + value + "/" + total`; its Continue (to payment) button shows only when `value >= total`,
  state 1 and not invited. The server never sends the 9A supporter list of a clan being registered (value 0), so `total` must
  stay 0 or clan creation is blocked: `9C` is all zero. Original total LIKELY 5 (`st_clan_creation_step_2_descr`: "at least 5 players").
  `9C` bytes 1..3 have no reader (NOT FOUND): 0.
* `62 19` join request has no caller; a member kicks with `62 1C` + his id (LIKELY).
* The `3`-or-more clans case, spectators and tournament rules (Q&A 2021-05 asked for them; later than the exe's feature set).
* Who the "commander of the other clan" is: `st_team_already_has_commander` ("The clan team already has a leader") exists, but no
  client request sets it; the host is the only commander.

## To check in the game (two game copies)

Two clans of two players, host (battle permission) creates a clan match, invites the
other clan's players, the buttons for sides appear only for the right clan, players press Play to become ready, the host's Play
starts the match.

## Web sources

* OFFICIAL 0.65b rules (agree with this page): a training match's cost is shown in tokens, payable in silver or gold, premium
  players play free; a clan battle has one total cost anyone may pay part of, reduced per premium player; clan battles give
  rewards, training matches none; no leave / AFK penalty in custom matches. The token amounts and the token price: NOT FOUND
  (operator settings).
* No clan ladder (Q&A 2021-03), no clan squad bonus (Q&A 2022-02), no clan currency or season (OFFICIAL / NOT FOUND);
  tournament prizes (SCL) were handed out by support (decal bonuses listed in 0.68b, gid 4186652357027343755).

## Custom match cost

Custom training match cost [0.6x, FAN: Steam thread 2021-04 with the moderator]: 5 000 silver per player per match,
premium players free. Token rate and the clan-battle total: NOT FOUND.
CHOICE: one token per room member without premium (training and clan rooms); a token costs 5 000 silver
or 20 gold (`rooms::TOKEN_COST_SILVER` / `_GOLD`; the gold price = 5 000 / the 250 exchange rate, CHOICE). `62 21` (signed deltas, gold
first) changes the member's own tokens (CustomMatchWindow `custom_match_change_tokens_count`, PROVEN; refused when he cannot
pay them, no increase once the room's tokens are complete); 98 carries each member's tokens and real premium flag, allocated / required and the prices; Start needs
allocated >= required and charges each payer once the match server accepted the match (8B to each).

## Protocol audit

See `protocol-audit.md` for the `62 21` and clan request layouts.

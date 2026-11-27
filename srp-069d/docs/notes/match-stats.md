# Match statistics of a local (bots) match: what the client reports

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Labels: PROVEN (exe code read or real log), LIKELY (inferred), CHOICE (our decision), OPEN (not known).

## 1. The only report: the `53` event flush

* `53` is the flush of the client's marker tracker, not a match report: `u32 n, n x {u16 id, u16 junk, u32 value, u32 time}`.
  PROVEN (builder 0x200193AA0; no reply exists). The flush is sent at most once a second, only when rows exist, at
  most 50 rows at a time. `junk` is uninitialised stack. `time` is the client's uptime in whole seconds (PROVEN by the
  logs: it matches the log clock, `0x353` = 851 s at the first flush of a session, and rows of one flush differ by 0..1).
* The tracker drops every id not in an id set, which starts as {1, 2}; the lobby's login `B8` push adds {5, 6, 0x3D, 0x3E}
  (PROVEN live: events 5 / 6 appeared after it). Enabling more ids would only add menu noise.
* All 34 producers of a tracker push were read. They are lobby / UI / tutorial / start-up markers (ids 1, 2, 4, 7, 8,
  0xA, 0xC, 0xD, 0xE, 0x17, 0x21, 0x29..0x31, 0x37..0x44) plus the match end. None is in the kill / death / damage code.
  PROVEN by enumeration (the producers at 0x10BC36, 0x117E8C = id 2, 0x118C51 = id 0x2A,
  0x133xxx, 0x145E81, 0x174671, 0x1791BA, 0x187AB2, 0x18F3F5, 0x1960xx..0x1C30xx are all outside the match code).
* The 0.69d event-name table at exe 0x200986E10 (kill 0, assist 1, death 2, suicide 3, team_kill 4, ..., won 12,
  headshot_kill 15, played_first_game 27, multi_kill 36, ...) is the `stats_event_type` table of `events.options` (the
  reward and achievement rows), NOT the tracker's ids. Its indices are what `MatchFacts::stat_events` uses. The client
  never sends it (no message carries them).
* Real data: 51 flushes in client logs hold only ids 1, 2, 5 and 6.
  Id 5 = my team won, id 6 = lost, value 1 (offline) or the match id (online, 0x20011695E). Ids 0x3D / 0x3E come from the
  single player branch (the game-type object answers 2): LIKELY the same meaning, never seen.
* Timing: at match end `game::tick` pushes 5 / 6 (rule state 4 or 5), the flush follows when the lobby is back (together
  with id 2 "lobby loaded"). One second after the server's `97` the client's own 97 handler pushes 5 / 6 again (0x18B2E5)
  and a second one-row flush arrives. It is an echo of our `97`, not a result (the lobby ignores it: the local match
  is already closed).
* A match left early has a flush with id 2 only: no marker = not finished (leaver, nothing is paid).

## 2. Other end-of-match messages

None. During and after a local match the client sends only `FC` keepalives, the `53` flush, and `37 00` after our `97`
(an internal ack, PROVEN 0x20018B47A). `close_current_match(3)` sends nothing and saves only `user.cfg`. PROVEN (exe and logs).

## 3. Kills, deaths, assists, score: not available (PROVEN)

The per-player counters exist only inside the client (`shared_statistics`, one 0xA24-byte block per player, the HUD's
Tab block); the match objects die in `close_current_match(3)` and nothing reads them out. A local match runs
in-process in the client, so there is no message carrying them. Rule: a fact the client does not report stays
`None`.

Therefore for a local match `MatchFacts` carries: `mode`, `map`, `won`, `finished`, `faction`, `first_game`,
`reputation` (what the match paid). Everything else (`kills`, `deaths`, `score`, `max_killstreak`, `stat_events`,
`duration_secs`, `team_place`, ...) is `None`. `crate::match_report::local_report` is the one place that reads the flush
(every row, no truncation: a flush holds up to 50 rows).

## 4. What counts for a local match today

* Reward: NONE. The client's own hint of the bots mode says "Rewards in this mode are not awarded"
  (`st_match_settings_bots_hint`, PROVEN, progression-audit.md section 4), so `game_data::rewards::LOCAL_MATCH_PAYS` is false:
  no silver, experience, reputation, drops, and `played_first_game` is not used up. `GameData::match_reward` stays for online
  matches.
* Quests, achievements, hero of the day, league: they run through `stats::record_match` with `finished` / `won` / `mode`
  / `map`. Counters that need kills, score, streaks or place (kills in a mode, hero titles, league rating) do not move:
  correct, since the data does not exist (a counter on `None` never moves). Win counters (e.g. "win 5 tdm matches")
  and match-played counters do move.
* Online matches (`online.rs`): the `53` flush there only carries 5 / 6 with `value = match_id` (the win / loss report). The
  statistics come from the match server's own observer (`Ended::stats`), which sees the world; the flush has none.

## 5. The `97` results screen of a local match (CHOICE, OPEN)

The screen's player rows (kills, deaths, score, team scores) are synthetic numbers seeded by the match id
(`local-match` `synth_stats`); they are display only, never paid or counted, and the `97` shows the paid amounts. They do
not match what the client saw in the match (the client's kills cannot be read). The screen needs a number in each row;
zeros would be honest but show "0:0" for every match. CHOICE: the player rows and team scores stay synthetic (golden-pinned, and a zero table cannot be proven safe for the Flash screen without running the client; the
original real numbers cannot be had), while the reward block of the `97` shows zeros: it shows exactly what is paid, which
is nothing.

## 6. What would be needed for real numbers

A read-out of the client's `shared_statistics` object at match end would need a patched client (ruled out: the stock client is
not patched) or a headless game client (ruled out too). What is left is the server's own simulation
of the bots match. OPEN.

## 7. Match items in the exe

PROVEN unless marked:
* 97 player row (`fill_match_statistic` 0x2001412F5..0x200141615): +0xA8 s16 score, +0xAA u16 kills, +0xAC u16 deaths,
  +0xAE u8 table place (0 = first; reader default 20), +0xAF bool league top player, +0xBC u8 team, +0xB8 hero place,
  +0xB9 nomination, +0xBA league points (rated only), +0xBB league index. No assists field anywhere (NOT FOUND).
  `ResultPlayer.flag` is the place byte (section 8).
* R block: +0x1C 9 x u16 seconds played per profile, +0x2E 9 x u8 match level per profile, +0x560 badge counts (meaning LIKELY).
* Bot rows of a local match come from the client's own 10-byte-per-player array (kills, deaths, score), not from the 97.
  Where kills / deaths / score sit in the client's 0xA24 per-player statistics block: section 8.
* `2F` mask: bit i = game-mode id i (mode table 0x200987060: gvi, tdm, capt, gunrace, pb, sad, slaughter, pve, dom, ah; the
  lobby loop and the movie's 7 check boxes use the same order): `queue::modes_of_mask` is right. The achievement `mode_mask`
  uses the same numbering (bit n = mode n): its mask 32 (627 / 628) is bit 5 = sad, which their texts name ("Artifact Hunt
  or Search and Destroy", "5 - 0" = the 5 rounds of a sad win). The client loads that mask without comparing it
  (`game_data::achievements::mode_mask`).
* First in-match profile: CB always sends profile byte 0 = 0, so it is index 0 of the player's options record; the HUD can
  switch (`profile_changed`). We write only the current lobby profile (no in-match switch); if several are ever written, put
  the current one first (CHOICE).

## 8. Results and per-player statistics in the exe

### The 97 place byte (+0xAE) (PROVEN)
* Whole table (both teams), 0-based, ties share: place = number of rows with a strictly higher score. The client computes bot
  places the same way (0x20013E8E0, no team test); the movie's `DebugData` sample follows the same rule.
* Use: ordering only. `fill_match_statistic` sets `sort_order = 2 * place + (row != me)` (0x200141494);
  the Flash sorts by sort_order, then score, rank, kills, deaths, then splits by team. Nothing else reads it (hero / league use
  +0xB8..+0xBB and +0xAF). No "left" flag in a row.
* Limits: at most 20 rows (row 21 overwrites the header at +0x1180, the count is not checked); the receiver's own row must be
  present (matched by public id), else row 0 is taken as "me".
* Server: `ResultPlayer.flag` = number of rows with a higher score (`protocol::s2c::results::set_places`). Local matches: bot
  rows are the client's; a small value is safe there.

### The client's 0xA24 per-player statistics block (PROVEN unless marked)
| offset | meaning | writer | reader |
|---|---|---|---|
| +0x000 | `u16 count[74 events][9 profiles]` (event table 0x200986E10: kill 0, assist 1, death 2, headshot 15, ...) | `add_event` 0x20033F030, kill handler 0x20033DCF0 | best player 0x20033FE95, PvE chests, serializer |
| +0x534 | 27 badge slots x 12 bytes | 0x20033F3D0 | serializer |
| +0x678 | `f32[20][9]` damage received per attacker, cleared at death | damage method (slot 10) | assist rule 0x20033E660 |
| +0x948 | `u32[20]` last time this player hit player j | slots 4 / 13 | 4 s teammate-rescue window |
| +0x998 | `u32[20]` deaths in a row by killer j | kill handler | badge at >= 3 |
| +0x9F4 / +0x9F5 | last damaged / last killer (revenge) | hit methods / kill handler | kill handler |
| +0x9F8 | f32 kill/death balance, +-5 | `add_event` | NOT FOUND |
| +0x9FC / +0xA00 / +0xA04 | last kill (7 s multi-kill) / last team kill / last death | kill handler | kill handler, badges |
| +0xA08 | current kill streak (reset at death) | kill handler | `killstreak_end` (71) |
| +0x9E8 / +0x9F0 | last activity time / inactive flag (the relay report gives them from the core: core + i x 0xA24 + 0xA10 / + 0xA18; 0x2003403C0 / 0x2003403E3) | stamp 0x200340380 (6 event call sites) | rule ticks, match end |
| +0xA17 / +0xA18 | carries victory item / victory artefact | slots 6 / 7 | kill handler |
| +0xA1C | score (u16) | score method (slot 35) | HUD, serializer |
| +0xA1E | best kill streak | kill handler | serializer, best player |

* Score per counted event = `trunc(score / reward_factor * amount + 0.5)` from the mode's `events.options` row (0x2002E06A0).
* Death counted for an enemy kill, a kill without killer, or a suicide (death + suicide); a team kill gives the victim no death,
  the killer `team_kill`. Assist: another player of the killer's team with >= 10 % of that team's damage to the victim. Match end
  adds `participating`, `won`, `played_first_game`.
* The Tab list / bot end rows (10 bytes: kills, deaths, score) are the HUD rows at G+0xAD0 (= `player_kd_stat`); score from
  +0xA1C; kills / deaths from `kd_stats_rule` 0x20030ED50 (kills = enemy kills by a valid killer; deaths = every kill record, no
  team test). That rule feeds the HUD: LIKELY (offsets match, call not found).
* Core +0xEBD4 / +0xEBD6 (= S+0xEBDC / +0xEBDE) are the two teams' scores;
  every mode rule writes its own team value there (tdm tickets left, rewritten with the winner byte at every death and
  leave 0x200366AD1 via 0x2003669E7 / 0x200367009, artefact hunt deliveries
  0x20035E017, gvi / match-time-vi counters, capture-the-point points 0x2003AA5A5, search-and-destroy rounds 0x2003AE229).
  The world host reports them as `team_score`; that the original `97` showed exactly these is LIKELY.
* OPEN: the kd-rule -> HUD copy call; +0xA0A / +0xA0C / +0xA12 / +0xA1A / +0xA20 / +0xA21; the 21 callers of
  the generic event slot 11; the PvE result message id; the message that carries these blocks to clients.

## 9. Protocol audit

See `protocol-audit.md`: the 97 team scores, match level, quest order, signed scores and the 99 header order.

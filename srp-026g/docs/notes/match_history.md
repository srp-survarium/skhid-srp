# Match history and the stats API (`/api/history/...`)

What survarium.pro showed of players, matches and clans, kept by srp itself and served as JSON by
`servers/web-server` for the site's stats page. The history is written in
`crates/session/src/v026g/persist/history.rs`; the endpoints are `servers/web-server/src/history.rs`.

## Owner's rules

- The stats are part of our own site (skhid.link), not the old survarium.pro stack.
- Every finished match is kept in `srp.db`, private rooms and matches against bots too; only public
  matches are ever served, counted in totals, or shown in a list.
  - One list is the player's own (the owner's of 2026-11-10: "do how you think is appropriate and
    should be genre best practice"; the rule above was the site's, and the game's lists had taken
    it over, so a player could not find its own private match again once its results were closed):
    the game's Watch, Replays, Mine (`3B 00 <OWN_MATCHES_START>`, `MatchHistory::lobby().own_matches`)
    lists the asker's matches of every kind, a private room's marked "Private room" and one
    against bots "With bots" (`P2`'s last field, there only for those). Another player's list,
    Friends, Everyone, the site and every total stay public only.
- The server's bots (account 0) and test bots (`SRP_BOT_ACCOUNTS`) are never kept.
- Never served: login names, addresses, bans or their reasons, the hidden matchmaking rating
  (`g26_balance`). "Rating" is the visible one players see in the lobby.
- Only players with at least one public match are ever named: most in-game names are still the
  login (`v026g/names.rs`), so an account that never played one is answered as no account (`404`
  for its card by id or name, left out of its clan's members), and counting ids lists no logins.
- Banned accounts are left out of the top lists and the best scores.
- Every request is cheap (one vCPU runs the game too): indexed lookups, results of at most 50 rows (`player/days`: up to 90) (a clan card's members: up to 105),
  totals kept up to date when a match is recorded, never added up per request.

## What is kept

- `g26_matches`: one row a finished match: its own number (`id`, counting up; the match server's id
  is kept beside it, it is random and repeats across restarts), its `replay_id` (that id with the
  UTC second the match was made, `rec026g_20261014-023456_000000ab`: its recording's file stem
  whether it was recorded or not, so a match stays told apart across restarts), when it ended
  (Unix seconds), how long it lasted, map, mode, match level, the kind (`public`, `private`,
  `vs_bots`), the winner (`team0`, `team1`, `draw`, `unknown`, or `most_kills` for a free-for-all, with `top_kills`) and the two teams' points.
- A public match's eligibility, set when it is written (`history::Eligibility`): `counted`, or why
  not: `with_bots` (a bot played; it moved no rating either), `few_players` (fewer than 6 accounts
  in it, leavers and late joiners counted: `SRP_RECORD_MIN_PLAYERS`' default and a default room's
  smallest match), `short` (under 5 minutes of play, the warm-up left out). Every public match is
  served; only counted ones go into totals, favourites, days, top lists and best scores. A player
  whose public matches all did not count is still named, with totals of zeros.
- `g26_match_players`: one row a player who was in the match (finished or left): team, place in
  its team by score (from 1), its roster place (`seat`, which a replay is watched from; empty in
  rows written before it was kept), rating after the match, clan at the time, score, kills,
  deaths and the counters below.
- `g26_player_totals`, `g26_clan_totals`: the counted public matches added up, at record time. A public
  match counts once for a clan if at least one member played in it (its clan at the time), and is a
  win if at least one of them was on the winning team (members on both teams: a win). Score,
  kills, deaths and the counters are its members' rows added up, so a clan's `score_avg` is its
  members' score per match together, and its counted matches are those of `matches?clan=` with `eligibility: counted`.
- Counters (game events, `docs/notes/results.md` §9 and `snapshot_rules.md` §3.1): assists (1),
  headshots (15), grenade kills (25), melee kills (13), batteries put into a container (20),
  artefacts (5: every artefact taken from an anomaly, the results' explorer count and Vostok's old
  `use_artefact`; not exe patch 80's empty-slot bonus, event 16; `artefact-score.md`), and in
  Research the captures (6, a share of a capture in hundredths, served as a number with two
  decimals), shots (22, bullets fired) and hits (23, bullets that hit). Each is summed over the
  kits. Only a match with a trusted snapshot has counters; score, kills and deaths are the
  scoreboard's.
- `g26_match_kits`: one row a kit a player used in a match: its place among the account's three
  kits (0..2, the counters' kit) and that kit's own kills (0), headshots (15), shots and hits.
  `g26_match_kit_items`: what each held, by profile slot: the weapons (slots 7 and 10) and the
  armour (0..6), as items_dict ids; ammo and the pockets are not kept. Names come from the game
  data only when served.
- A kit is used when one of its counters is above 0 in the trusted snapshot (the rule the match's
  wear uses to call a kit worn); a player with none (no trusted snapshot, or nothing done) used
  the kit its account plays, the one the match took. What a kit held is the account's kit as the
  match ended, read before the match is settled. The snapshot's kit in use (`Supplies::kit`) is
  not read: its place is LIKELY only, and a kit that fired, killed or died shows in the counters.
- `g26_player_items`: the counted public matches added up by player and item, at record time: `matches`
  (counted public matches in which a kit the player used held the item; once a match however many kits
  held it) and the kills, headshots, shots and hits of every used kit that held it, with its kind
  (`weapon` or `armour`, by its slot). These are a kit's counts given to every item of the kit:
  a kit holding two firearms gives both all its kills and shots, and its armour too. They are
  not per-weapon numbers and are never shown as such.
- Not kept, as survarium.pro had them: artefact kills (0.26g has none), clan rating (clans have
  none), skills.

## Events

`g26_match_events` (`persist/match_events.rs`): what happened in a match, keyed by its `replay_id`
and written in the same transaction as the match. The match server makes the list
(`match-server/src/v026g/events.rs`) from the trusted worlds it reads (one asked for every 5 s,
`Timing::refresh`; more apart where one was not trusted). History and events start with the first
match after they shipped: older matches are not added from their recordings. No message names a
kill, a battery or a capture (`kill_weapon.md` 1), so each event is what changed between two
worlds' shared statistics and rules.

| kind | from | actor | victim | other |
|---|---|---|---|---|
| `kill` | the killer's kills (event 0) | killer | the death paired with it, if any | |
| `death` | a death (event 2) no kill was paired with: suicide, fall, anomaly, teammate, or open | | the dead | |
| `battery_found` | event 19 (first pick-up) | finder | | |
| `battery_delivered` | event 20 (put into a container) | carrier | | |
| `capture` | Research: a unit's owner became a team | | | `team`, `unit` |
| `score` | modes 2 and 3: the batteries in the containers changed (taken out too) | | | `points0`, `points1` |
| `end` | the match's end | | | the final points |

- **Time and precision.** `at_ms` is match ms (the replay's time). `after_ms` empty: `at_ms` is the
  game's own tick (50 ms) of it. That holds for a kill whose killer made no other kill in the
  window (the killer's `last_killed_enemy_ms`) and for the end. Else the event was after `after_ms`
  and at `at_ms` or before: the window between the two worlds, about 5 s (a killer's several kills:
  up to its last kill's tick). Deaths, batteries, captures and score changes are always windows.
- **Victims.** A death is paired with a kill when only one enemy of the dead killed in the window
  and no more of its enemies died than it killed, or only one of the killers left hurt the dead in
  the life it lost (its `received_damage`). Else the kill has no victim and the death stays a
  `death`.
- **Not known:** the weapon (the game never sends it, `kill_weapon.md`), the position (a dead
  player's block has no body, and the worlds are seconds from the event), headshot or melee per
  kill, who took a battery out of a container, who captured a unit, Team Deathmatch's tickets
  (they follow the deaths) and Research's points (they grow all the time).
- Places are the roster's (0 to 19) with their accounts (0: the server's bot).
- **The lobby** reads them with `MatchHistory::lobby().events(<match server's id>)` (the newest match of
  that id, of any kind: the caller says who may see them), and the side channel's rows are
  `lobby-server/src/v026g/match_events.rs` (`match_events_page`): a head
  `E1;<ok|none|error>;<match id 8 hex>;<events>;<page>`, then 254 events a page, one a row,
  `<K|D|F|B|C|S|E>;<at>;<after>;...` (the module's table).

## The API

- `GET` only, JSON, no sign-in; on the 0.26g site only. `lang=en|uk` names maps and modes.
- `400 {"error":"bad request"}` for a bad parameter, `404 {"error":"not found"}` for an unknown or
  non-public one (a list of an unknown player is `200` with no matches), `503 {"error":"no history"}`
  when srp keeps no database (`SRP_DB=off`), `500 {"error":"failed"}` when its read fails (logged).
- No settings of its own: it reads the `srp.db` srp writes.
- Paging is by match number: `before=<id>` gives the matches older than `id`; `next` is the value
  for the next page after a full one, `null` after a shorter one (a full last page is followed by
  an empty one). `limit` is 1..50 (default 15).

Common objects:

- A player: `{"id":1042,"name":"Sheep","clan":{"id":7,"tag":"SKH"}}` (`clan` is `null` without
  one; the current clan, registered ones only).
- Totals: `{"matches":120,"wins":64,"losses":56,"kills":900,"deaths":700,"assists":210,
  "score":30500,"headshots":180,"grenade_kills":12,"melee_kills":3,"batteries":40,"captures":5.5,
  "artefacts":25,"shots":52000,"hits":15600,"accuracy":0.3,"kd":1.29,"win_rate":0.533,
  "score_avg":254.2}`. `kd` = kills / deaths (deaths 0 counts as 1), two decimals; `win_rate`
  0..1, three decimals; `score_avg` one decimal; `accuracy` = hits / shots (0 with no shot), 0..1,
  three decimals.
- A match: `{"id":5531,"replay_id":"rec026g_20261103-115320_5e1a09c3","ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
  "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12,
  "eligibility":"counted"}`. `winner` is 0 or 1, `"draw"`, or `null` when not known and for every free-for-all. `map` and `mode` are `null` for a map the game data no longer
  has.
- A player's line of a match: `"team":0,"place":1,"won":true,"left":false,"rating":1180,
  "score":310,"kills":14,"deaths":6,"kd":2.33,"assists":4,"headshots":3,"grenade_kills":1,
  "melee_kills":0,"batteries":2,"captures":0,"artefacts":1,"shots":420,"hits":130,
  "accuracy":0.31,"kits":[...]`.
- A kit of a line (`kits`, the kits the player used, by place): `{"kit":0,"kills":12,
  "headshots":3,"shots":380,"hits":120,"accuracy":0.316,"weapons":[{"slot":7,"id":155,
  "name":"AK-74M"},{"slot":10,"id":60,"name":"TT-33"}],"armour":[{"slot":0,"id":301,
  "name":"Helmet"}]}`. `kit` is its place among the player's three kits (0..2); the counters are
  the kit's, not a weapon's; `weapons` and `armour` are by slot, `id` the items_dict id, `name`
  the game data's in `lang` (`null` for an item it no longer has). A match with no trusted
  snapshot has its kit with counters 0.
- An item's totals (`player/equipment`): `{"id":155,"name":"AK-74M","matches":40,"kills":380,
  "headshots":70,"shots":12000,"hits":3500,"accuracy":0.292}`: the counted public matches in which a kit
  the player used held it, and those kits' counters added up (the kit's kills with whatever
  else it held, not the item's own).

### `GET /api/history/player?name=<name>` or `?id=<id>`

A player's card. `name` is the in-game name, exact (Latin letters in any case). `404` for an
account without a public match, as for no account.

```json
{"player":{"id":1042,"name":"Sheep","clan":{"id":7,"tag":"SKH"}},
 "level":34,"experience":1234567,"rating":1180,
 "totals":{"matches":120,"wins":64,"losses":56,"kills":900,"deaths":700,"assists":210,"score":30500,
           "headshots":180,"grenade_kills":12,"melee_kills":3,"batteries":40,"captures":5.5,
           "artefacts":25,"shots":52000,"hits":15600,"accuracy":0.3,"kd":1.29,"win_rate":0.533,
           "score_avg":254.2}}
```

### `GET /api/history/player/matches?id=<id>&before=<match>&limit=<n>`

A player's public matches, newest first: the match, the player's line and its kits.

```json
{"matches":[{"id":5531,"replay_id":"rec026g_20261103-115320_5e1a09c3","ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
             "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12,
             "eligibility":"counted","team":0,"place":1,"won":true,"left":false,"rating":1180,"score":310,"kills":14,
             "deaths":6,"kd":2.33,"assists":4,"headshots":3,"grenade_kills":1,"melee_kills":0,
             "batteries":2,"captures":0,"artefacts":1,"shots":420,"hits":130,"accuracy":0.31,
             "kits":[{"kit":0,"kills":12,"headshots":3,"shots":380,"hits":120,"accuracy":0.316,
                      "weapons":[{"slot":7,"id":155,"name":"AK-74M"}],
                      "armour":[{"slot":0,"id":301,"name":"Helmet"}]},
                     {"kit":2,"kills":2,"headshots":0,"shots":40,"hits":10,"accuracy":0.25,
                      "weapons":[{"slot":7,"id":170,"name":"SVD"}],"armour":[]}]}],
 "next":5498}
```

### `GET /api/history/player/equipment?id=<id>&limit=<n>`

A player's favourite equipment: its most used weapons and armour of its counted public matches, each list
by matches, most first (ties: the higher id first), at most `limit` (1..50, default 10). Kept up
to date when a match is recorded (`g26_player_items`). An account without a public match has
both lists empty, as an unknown one. The counters are kit-level (see "What is kept"): with two
firearms in a kit, both carry all its kills and shots.

```json
{"weapons":[{"id":155,"name":"AK-74M","matches":40,"kills":380,"headshots":70,"shots":12000,
             "hits":3500,"accuracy":0.292}],
 "armour":[{"id":301,"name":"Helmet","matches":52,"kills":410,"headshots":75,"shots":13100,
            "hits":3800,"accuracy":0.29}]}
```

### `GET /api/history/player/days?id=<id>&days=<n>`

A player's counted public matches added up by day (UTC), oldest first, days without a match left out.
`days` is 1..90 (default 30).

```json
{"days":[{"day":"2026-10-31","matches":6,"wins":4,"losses":2,"kills":50,"deaths":31,"assists":9,
          "score":1700,"headshots":11,"grenade_kills":1,"melee_kills":0,"batteries":3,"captures":0,
          "artefacts":2,"shots":2400,"hits":700,"accuracy":0.292,"kd":1.61,"win_rate":0.667,
          "score_avg":283.3}]}
```

### `GET /api/history/matches?before=<match>&limit=<n>&clan=<clan id>`

The latest public matches, newest first; with `clan`, those a member of that clan played in.

```json
{"matches":[{"id":5531,"replay_id":"rec026g_20261103-115320_5e1a09c3","ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
             "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12,
             "eligibility":"counted"}],
 "next":5530}
```

### `GET /api/history/match?id=<match>`

A public match and every player's line with its kits, by team, then place.

```json
{"match":{"id":5531,"replay_id":"rec026g_20261103-115320_5e1a09c3","ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
          "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12,
          "eligibility":"counted"},
 "players":[{"player":{"id":1042,"name":"Sheep","clan":null},"level":34,
             "team":0,"place":1,"won":true,"left":false,"rating":1180,"score":310,"kills":14,
             "deaths":6,"kd":2.33,"assists":4,"headshots":3,"grenade_kills":1,"melee_kills":0,
             "batteries":2,"captures":0,"artefacts":1,"shots":420,"hits":130,"accuracy":0.31,
             "kits":[{"kit":0,"kills":14,"headshots":3,"shots":420,"hits":130,"accuracy":0.31,
                      "weapons":[{"slot":7,"id":155,"name":"AK-74M"}],
                      "armour":[{"slot":0,"id":301,"name":"Helmet"}]}]}]}
```

### `GET /api/history/top?by=<key>&limit=<n>`

The top players by their totals. `by`: `rating`, `matches`, `wins`, `kills`, `score`, `headshots`,
`batteries`, `artefacts`, `accuracy` (default `rating`); `limit` 1..50 (default 50). Players with
a counted public match only. `accuracy` lists only the players with at least 1000 shots in their counted public
matches (`ACCURACY_MIN_SHOTS`; a few lucky shots do not top it), best first; ties by account.

```json
{"by":"kills",
 "players":[{"player":{"id":1042,"name":"Sheep","clan":{"id":7,"tag":"SKH"}},"level":34,
             "rating":1180,"totals":{"matches":120,"wins":64,"losses":56,"kills":900,"deaths":700,
             "assists":210,"score":30500,"headshots":180,"grenade_kills":12,"melee_kills":3,
             "batteries":40,"captures":5.5,"artefacts":25,"shots":52000,"hits":15600,
             "accuracy":0.3,"kd":1.29,"win_rate":0.533,"score_avg":254.2}}]}
```

### `GET /api/history/best?period=<hour|day>`

The 20 best single-match scores of counted public matches that ended in the period (default `day`), banned
accounts left out.

```json
{"period":"day",
 "rows":[{"player":{"id":1042,"name":"Sheep","clan":null},"match":5531,"ended_at":1793500000,
          "level":12,"map":"Vostok","mode":"Batteries","score":420,"kills":20,"deaths":3}]}
```

### `GET /api/history/clans?by=<key>&limit=<n>`

Registered clans by their totals. `by`: `matches`, `wins`, `kills`, `score`, `headshots`,
`batteries`, `artefacts` (default `score`); `limit` 1..50 (default 50).

```json
{"by":"score",
 "clans":[{"id":7,"name":"Skhid","tag":"SKH","level":5,"members":18,
           "totals":{"matches":400,"wins":230,"losses":170,"kills":3100,"deaths":2500,"assists":700,
           "score":98000,"headshots":610,"grenade_kills":40,"melee_kills":9,"batteries":150,
           "captures":12.25,"artefacts":90,"shots":180000,"hits":52000,"accuracy":0.289,
           "kd":1.24,"win_rate":0.575,"score_avg":245}}]}
```

### `GET /api/history/clan?id=<clan>` or `?tag=<tag>`

A registered clan: its totals and its members with a public match, each with their own totals (all
their counted public matches), commander first, then by score. `members` counts every member, listed or
not; `leader` is `null` when the commander has no public match.

```json
{"clan":{"id":7,"name":"Skhid","tag":"SKH","level":5,"members":18,
         "leader":{"id":1042,"name":"Sheep","clan":{"id":7,"tag":"SKH"}}},
 "totals":{"matches":400,"wins":230,"losses":170,"kills":3100,"deaths":2500,"assists":700,
           "score":98000,"headshots":610,"grenade_kills":40,"melee_kills":9,"batteries":150,
           "captures":12.25,"artefacts":90,"shots":180000,"hits":52000,"accuracy":0.289,
           "kd":1.24,"win_rate":0.575,"score_avg":245},
 "members":[{"player":{"id":1042,"name":"Sheep","clan":{"id":7,"tag":"SKH"}},"commander":true,
             "level":34,"rating":1180,"totals":{"matches":120,"wins":64,"losses":56,"kills":900,
             "deaths":700,"assists":210,"score":30500,"headshots":180,"grenade_kills":12,
             "melee_kills":3,"batteries":40,"captures":5.5,"artefacts":25,"shots":52000,
             "hits":15600,"accuracy":0.3,"kd":1.29,"win_rate":0.533,"score_avg":254.2}}]}
```

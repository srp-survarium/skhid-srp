# Match history and the stats API (`/api/history/...`)

What survarium.pro showed of players, matches and clans, kept by srp itself and served as JSON by
`servers/web-server` for the site's stats page. The history is written in
`crates/session/src/v026g/persist/history.rs`; the endpoints are `servers/web-server/src/history.rs`.

## Owner's rules

- The stats are part of our own site (skhid.link), not the old survarium.pro stack.
- Every finished match is kept in `srp.db`, private rooms and matches against bots too; only public
  matches are ever served, counted in totals, or shown in a list.
- The server's bots (account 0) and test bots (`SRP_BOT_ACCOUNTS`) are never kept.
- Never served: login names, addresses, bans or their reasons, the hidden matchmaking rating
  (`g26_balance`). "Rating" is the visible one players see in the lobby.
- Banned accounts are left out of the top lists.
- Every request is cheap (one vCPU runs the game too): indexed lookups, results of at most 50 rows,
  totals kept up to date when a match is recorded, never added up per request.

## What is kept

- `g26_matches`: one row a finished match: its own number (`id`, counting up; the match server's id
  is kept beside it, it is random and repeats across restarts), when it ended (Unix seconds), how
  long it lasted, map, mode, match level, the kind (`public`, `private`, `vs_bots`), the winner
  (`team0`, `team1`, `draw`, `unknown`) and the two teams' points.
- `g26_match_players`: one row a player who was in the match (finished or left): team, place in
  its team by score (from 1), rating after the match, clan at the time, score, kills, deaths and
  the counters below.
- `g26_player_totals`, `g26_clan_totals`: the public matches added up, at record time. A clan's
  totals are its members' rows while they were in it (a match with three members counts three).
- Counters (game events, `docs/notes/results.md` §9 and `snapshot_rules.md` §3.1): assists (1),
  headshots (15), grenade kills (25), melee kills (13), batteries put into a container (20), and in
  Research the captures (6, a share of a capture in hundredths, served as a number with two
  decimals). Only a match with a trusted snapshot has counters; score, kills and deaths are the
  scoreboard's.
- Not kept, as survarium.pro had them: artefact kills and uses (no event says which), clan
  rating (clans have none), skills and kits.

## The API

- `GET` only, JSON, no sign-in; on the 0.26g site only. `lang=en|uk` names maps and modes.
- `400 {"error":"bad request"}` for a bad parameter, `404 {"error":"not found"}` for an unknown or
  non-public one, `503 {"error":"no history"}` when srp keeps no database (`SRP_DB=off`).
- No settings of its own: it reads the `srp.db` srp writes.
- Paging is by match number: `before=<id>` gives the matches older than `id`; `next` is the value
  for the next page after a full one, `null` after a shorter one (a full last page is followed by
  an empty one). `limit` is 1..50 (default 15).

Common objects:

- A player: `{"id":1042,"name":"Sheep","clan":{"id":7,"tag":"SKH"}}` (`clan` is `null` without
  one; the current clan, registered ones only).
- Totals: `{"matches":120,"wins":64,"losses":56,"kills":900,"deaths":700,"assists":210,
  "score":30500,"headshots":180,"grenade_kills":12,"melee_kills":3,"batteries":40,"captures":5.5,
  "kd":1.29,"win_rate":0.533,"score_avg":254.2}`. `kd` = kills / deaths (deaths 0 counts as 1),
  two decimals; `win_rate` 0..1, three decimals; `score_avg` one decimal.
- A match: `{"id":5531,"ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
  "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12}`. `winner` is 0 or 1,
  `"draw"`, or `null` when not known.
- A player's line of a match: `"team":0,"place":1,"won":true,"left":false,"rating":1180,
  "score":310,"kills":14,"deaths":6,"kd":2.33,"assists":4,"headshots":3,"grenade_kills":1,
  "melee_kills":0,"batteries":2,"captures":0`.

### `GET /api/history/player?name=<name>` or `?id=<id>`

A player's card. `name` is the in-game name, exact (Latin letters in any case).

```json
{"player":{"id":1042,"name":"Sheep","clan":{"id":7,"tag":"SKH"}},
 "level":34,"experience":1234567,"rating":1180,
 "totals":{"matches":120,"wins":64,"losses":56,"kills":900,"deaths":700,"assists":210,"score":30500,
           "headshots":180,"grenade_kills":12,"melee_kills":3,"batteries":40,"captures":5.5,
           "kd":1.29,"win_rate":0.533,"score_avg":254.2}}
```

### `GET /api/history/player/matches?id=<id>&before=<match>&limit=<n>`

A player's public matches, newest first: the match and the player's line.

```json
{"matches":[{"id":5531,"ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
             "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12,
             "team":0,"place":1,"won":true,"left":false,"rating":1180,"score":310,"kills":14,
             "deaths":6,"kd":2.33,"assists":4,"headshots":3,"grenade_kills":1,"melee_kills":0,
             "batteries":2,"captures":0}],
 "next":5498}
```

### `GET /api/history/player/days?id=<id>&days=<n>`

A player's public matches added up by day (UTC), oldest first, days without a match left out.
`days` is 1..90 (default 30).

```json
{"days":[{"day":"2026-10-31","matches":6,"wins":4,"losses":2,"kills":50,"deaths":31,"assists":9,
          "score":1700,"headshots":11,"grenade_kills":1,"melee_kills":0,"batteries":3,"captures":0,
          "kd":1.61,"win_rate":0.667,"score_avg":283.3}]}
```

### `GET /api/history/matches?before=<match>&limit=<n>&clan=<clan id>`

The latest public matches, newest first; with `clan`, those a member of that clan played in.

```json
{"matches":[{"id":5531,"ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
             "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12}],
 "next":5530}
```

### `GET /api/history/match?id=<match>`

A public match and every player's line, by team, then place.

```json
{"match":{"id":5531,"ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
          "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12},
 "players":[{"player":{"id":1042,"name":"Sheep","clan":null},"level":34,
             "team":0,"place":1,"won":true,"left":false,"rating":1180,"score":310,"kills":14,
             "deaths":6,"kd":2.33,"assists":4,"headshots":3,"grenade_kills":1,"melee_kills":0,
             "batteries":2,"captures":0}]}
```

### `GET /api/history/top?by=<key>&limit=<n>`

The top players by their totals. `by`: `rating`, `matches`, `wins`, `kills`, `score`, `headshots`,
`batteries` (default `rating`); `limit` 1..50 (default 50). Players with a public match only.

```json
{"by":"kills",
 "players":[{"player":{"id":1042,"name":"Sheep","clan":{"id":7,"tag":"SKH"}},"level":34,
             "rating":1180,"totals":{"matches":120,"wins":64,"losses":56,"kills":900,"deaths":700,
             "assists":210,"score":30500,"headshots":180,"grenade_kills":12,"melee_kills":3,
             "batteries":40,"captures":5.5,"kd":1.29,"win_rate":0.533,"score_avg":254.2}}]}
```

### `GET /api/history/best?period=<hour|day>`

The 20 best single-match scores of public matches that ended in the period (default `day`).

```json
{"period":"day",
 "rows":[{"player":{"id":1042,"name":"Sheep","clan":null},"match":5531,"ended_at":1793500000,
          "level":12,"map":"Vostok","mode":"Batteries","score":420,"kills":20,"deaths":3}]}
```

### `GET /api/history/clans?by=<key>&limit=<n>`

Registered clans by their totals. `by`: `matches`, `wins`, `kills`, `score`, `headshots`,
`batteries` (default `score`); `limit` 1..50 (default 50).

```json
{"by":"score",
 "clans":[{"id":7,"name":"Skhid","tag":"SKH","level":5,"members":18,
           "totals":{"matches":400,"wins":230,"losses":170,"kills":3100,"deaths":2500,"assists":700,
           "score":98000,"headshots":610,"grenade_kills":40,"melee_kills":9,"batteries":150,
           "captures":12.25,"kd":1.24,"win_rate":0.575,"score_avg":245}}]}
```

### `GET /api/history/clan?id=<clan>` or `?tag=<tag>`

A registered clan: its totals and its members, each with their own totals (all their public
matches), commander first, then by score.

```json
{"clan":{"id":7,"name":"Skhid","tag":"SKH","level":5,"members":18,
         "leader":{"id":1042,"name":"Sheep","clan":{"id":7,"tag":"SKH"}}},
 "totals":{"matches":400,"wins":230,"losses":170,"kills":3100,"deaths":2500,"assists":700,
           "score":98000,"headshots":610,"grenade_kills":40,"melee_kills":9,"batteries":150,
           "captures":12.25,"kd":1.24,"win_rate":0.575,"score_avg":245},
 "members":[{"player":{"id":1042,"name":"Sheep","clan":{"id":7,"tag":"SKH"}},"commander":true,
             "level":34,"rating":1180,"totals":{"matches":120,"wins":64,"losses":56,"kills":900,
             "deaths":700,"assists":210,"score":30500,"headshots":180,"grenade_kills":12,
             "melee_kills":3,"batteries":40,"captures":5.5,"kd":1.29,"win_rate":0.533,
             "score_avg":254.2}}]}
```

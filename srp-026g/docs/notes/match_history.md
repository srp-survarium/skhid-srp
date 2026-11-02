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
- Only players with at least one public match are ever named: most in-game names are still the
  login (`v026g/names.rs`), so an account that never played one is answered as no account (`404`
  for its card by id or name, left out of its clan's members), and counting ids lists no logins.
- Banned accounts are left out of the top lists and the best scores.
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
- `g26_player_totals`, `g26_clan_totals`: the public matches added up, at record time. A public
  match counts once for a clan if at least one member played in it (its clan at the time), and is a
  win if at least one of them was on the winning team (members on both teams: a win). Score,
  kills, deaths and the counters are its members' rows added up, so a clan's `score_avg` is its
  members' score per match together, and its matches are exactly those `matches?clan=` lists.
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
- `g26_player_items`: the public matches added up by player and item, at record time: `matches`
  (public matches in which a kit the player used held the item; once a match however many kits
  held it) and the kills, headshots, shots and hits of every used kit that held it, with its kind
  (`weapon` or `armour`, by its slot). These are a kit's counts given to every item of the kit:
  a kit holding two firearms gives both all its kills and shots, and its armour too. They are
  not per-weapon numbers and are never shown as such.
- Not kept, as survarium.pro had them: artefact kills (0.26g has none), clan rating (clans have
  none), skills.

## The API

- `GET` only, JSON, no sign-in; on the 0.26g site only. `lang=en|uk` names maps and modes.
- `400 {"error":"bad request"}` for a bad parameter, `404 {"error":"not found"}` for an unknown or
  non-public one (a list of an unknown player is `200` with no matches), `503 {"error":"no history"}`
  when srp keeps no database (`SRP_DB=off`).
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
- A match: `{"id":5531,"ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
  "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12}`. `winner` is 0 or 1,
  `"draw"`, or `null` when not known. `map` and `mode` are `null` for a map the game data no longer
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
  "headshots":70,"shots":12000,"hits":3500,"accuracy":0.292}`: the public matches in which a kit
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
{"matches":[{"id":5531,"ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
             "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12,
             "team":0,"place":1,"won":true,"left":false,"rating":1180,"score":310,"kills":14,
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

A player's favourite equipment: its most used weapons and armour of its public matches, each list
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

A player's public matches added up by day (UTC), oldest first, days without a match left out.
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
{"matches":[{"id":5531,"ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
             "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12}],
 "next":5530}
```

### `GET /api/history/match?id=<match>`

A public match and every player's line with its kits, by team, then place.

```json
{"match":{"id":5531,"ended_at":1793500000,"duration_s":845,"map_id":3,"map":"Vostok",
          "mode":"Batteries","level":12,"winner":0,"team_points":[5,3],"players":12},
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
a public match only. `accuracy` lists only the players with at least 1000 shots in their public
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

The 20 best single-match scores of public matches that ended in the period (default `day`), banned
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
their public matches), commander first, then by score. `members` counts every member, listed or
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

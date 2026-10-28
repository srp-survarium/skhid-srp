# Client tests: the runner that runs them one after another

The bots' fixes are checked on the bench, and what the bench cannot show (a human's play, the real client's bodies) is
checked by the owner in a local Player vs Bots match. The runner (`servers/match-server/src/v026g/bots/client_tests/`)
turns that list of checks into steps the server runs by itself, one after another, and measures from its own view.
The step API is documented in that module's docs (`client_tests/mod.rs`); this note is for the person who runs a plan and
records what the client allows.

TEST ONLY. It is compiled only with the cargo feature `client-test` and is inert unless `SRP_CLIENT_TEST` is set. A build
without the feature stops at start when the variable is set; a build with it stops when the configuration looks live
(`SRP_ADMIN_TOKEN` set, or `SRP_PUBLIC_HOST` / `SRP_LOBBY_PUBLIC_HOST` not a loopback, private or link-local address).

## Running a plan

```bash
cd srp-026g
SRP_CLIENT_TEST=all SRP_PVB=on <the usual local settings> cargo run -p srp --features client-test
```

- `SRP_CLIENT_TEST`: `all`, a list of step ids and package names (`a,b`), or `@file` (one per line).
- `SRP_CLIENT_TEST_DIR` (default `logs/client-test`): `client-test-results-<plan>-<time>.tsv` (one row per try) and
  `client-test-card-<plan>-<time>.md` (the printed card: the matches in order and every step's instruction).
- The log has `CLIENT_TEST plan ...`, `CLIENT_TEST match ...`, `CLIENT_TEST step <id> ARMED|START|END <result>` and
  `CLIENT_TEST tell: ...` for every line said to the owner.

## What the owner does

1. Starts Player vs Bots on the first match of the card (map, mode, size).
2. Reads each instruction in the game's chat (or the card) and does it. The runner arms the next step 4 s after the last
   ended, tells the instruction again every 6 s until the step starts, and starts it when its condition holds. Nothing is
   clicked between steps of one match.
3. When the match has no step left, the runner says which match is next. The owner leaves from the in-game menu and starts
   Player vs Bots with that map, mode and size.

## Answers the runner is built on

| question | answer | tag |
|---|---|---|
| Can a match change its map between steps? | No. The map and the mode are one byte given when the lobby makes the match and kept in `Formed`; the bots are built once from them. Each (map, mode, size) group is a new match. Steps are sorted by group so the number of matches is the number of groups. | PROVEN (`lobby-server/play.rs:931`, `match-server/registry.rs:186-207`, `game/driven.rs:173`) |
| Who picks the next match? | The owner, in the lobby's Player vs Bots panel (size 6, 8 or 10; batteries or team deathmatch; the map). No environment variable or test hook forces them. | PROVEN (`lobby-server/pvb.rs:90-94`) |
| Mode 3 (timed batteries)? | Not offered in Player vs Bots, so a step in it can never run. | PROVEN (`pvb.rs`; the panel folds it into batteries) |
| Who is on which team? | The owner is team 0 with N-1 bots; team 1 is N bots. A step's size is N. | PROVEN (`play.rs:908`) |
| How long is a match? | 900 s after a 20 s countdown; batteries end at 10 batteries, team deathmatch at 100 tickets (150 on Tarakanovsky Fort and Cologne). No setting shortens it. | PROVEN (`modes_maps.md`) |
| Can the owner leave early? | Yes, from the in-game menu; the match ends at once. | LIKELY that the menu is the caller |
| Can the server show the owner a message in the game? | Yes, as a chat line: `Inbox::push` of an `A2` line (sender type 5, id 0, empty name and tag, channel 2 "system") to the owner's account. The lobby link stays up in a match and the client's `A2` handler does not check the match state; the HUD chat log draws channel 2, and "hide chat in match" does not hide it. A line fades after about 7 s, so the runner tells again every 6 s. No match message carries text. | code path PROVEN, on-screen display LIKELY (never seen) |
| If the line does not show? | The card has every instruction; retry on channel 5 with a sender name, with "Hide chat in match" off. | |

The one live check before the first plan: start the server with a plan, enter a match, and look for the first line in the
chat log of the game.

## What the runner can measure

The server sees what the world uploads show (positions, health, kills, deaths) and what the bots log (their notes). It
does not see bullets or damage per hit: hits are read from health and the statistics. Scripted bots walk to their places
(the server cannot put a bot anywhere), so a step that needs a bot in place says `Cond::BotsInPlace`.

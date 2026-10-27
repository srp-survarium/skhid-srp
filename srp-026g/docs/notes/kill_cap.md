# Kill cap

Owner's rule (2026-10-27): killing the same person pays at most 10 times a match.

- Per player per match, kills of one enemy past 10 pay nothing (no silver, experience or
  reputation of the kills' row). Every mode, bots' matches too.
- The headshot, melee, silent and grenade kills (scored events) are cut in the same proportion.
- The scoreboard, the results screen's kill count, deaths, quests' kills and the rating are not
  touched: only the statistics the pay is counted from (as `battery_credit.md`'s cap).
- Who killed whom: the server sees running totals only. A trusted snapshot in which exactly one
  player's kills went up, and as many deaths of his enemies (no teammate of his dead), pairs
  them. Kills that come in the same snapshot as another player's kill are not paired, so they
  pay: the cap only misses kills, it never cuts one it cannot pair.
- `servers/match-server/src/v026g/kill_cap.rs` (`MOST_PAID`), applied in `Game::build_result`
  after the battery and Research caps; each cut is logged ("kills counted N, paid M").

# Battery events: at most once a battery of the map

The owner's rule against farming battery events and how srp keeps it. Code:
`servers/match-server/src/v026g/battery_cap.rs`, applied in `Game::build_result`.

- **The request (the owner, 2026-10-20):** "we have a problem with people farming and getting
  batteries in and out, every single person should only get a single bonus for rescuing a
  battery".
- **Seen live (2026-10-20, two 1 v 1 matches):**
  - one player takes a battery out of its own container and puts it back, again and again: 233
    put-ins, score ~1300 against 30 from kills;
  - two players steal two batteries from each other's base again and again (match 0x517f92b3):
    18 and 16 steals, scores ~830 and ~670, almost no kills.
- **A first try was too complex** (PR #239's first version: each battery followed world by
  world, ~1500 lines). The owner: "239 seems too complex? I expected us to just store whether
  each player saved a battery or not, and if they did, don't give them points again".
- **Why not per battery:** the server sees each player's running totals of each event in the
  worlds (one every 5 s), not single events, and not which battery an event was.
- **The rule (option B, the owner chose it):** "Per player per match, each battery event kind
  (put in, stolen, rescued) pays at most as many times as the map has batteries (10). Above
  that, nothing."
  - N is the map's `victory_items_count` (10 on every battery map); Team Deathmatch: no cap.
  - 20 put in: at most N. 6 found, 17 stolen, 18 rescued (hundredths of a delivery, shared
    among the carriers by distance): at most N × 100 each. 24 solo delivery and 26 own steal
    (weight 0, but quests 6 and 7 count them): at most N each.
  - Each cut is logged once per player and kind ("counted T, paid K").
- **The catch (the owner: "sure, this is fine"):** a very active honest carrier who delivers
  more than N batteries in one match stops earning from the (N+1)th.
- **What it changes:** the result's statistics, so the score on the results screen, the
  activity row's pay and the quests (all srp computes from the result). Not the in-match
  scoreboard (the clients count it) and not the winner (the batteries in each container).
- **On the live worlds** (`battery_cap_of_saved_worlds`, an ignored test, last world):
  - in and out: put-ins 233 → 10, score 1313 → 198 (its 7 found and 1 stolen kept);
  - ping-pong: steals 1800 → 1000 and 1600 → 1000 hundredths, put-ins 23 → 10 and 18 → 10,
    quest events 24/26 19/18 → 10 and 17/16 → 10; scores 830 → 485 and 672 → 422.
- **Release:** the match server only (srp). No client patch, no data.
- **Not touched:** what accounts earned before: the owner's decision.

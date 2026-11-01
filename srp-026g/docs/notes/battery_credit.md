# Battery events: at most N per kind, N the map's batteries

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
  that, nothing." So one battery can pay up to N times per kind: the owner knew it.
  - N is the map's `victory_items_count` (10 on every battery map); Team Deathmatch and Research
    (mode 5, whose captures count in events 6 and 18): no cap.
  - Research captures (6) and neutralisations (18) have a combat-adjusted payment allowance:
    200 score + 40 per paid kill + 20 per assist. Kill invader (10) stays whole. See
    `research_mode.md`; these payment-only limits do not change the battery cap's quest policy.
  - 20 put in and 19 found (`VICTORY_ITEM_FOUND`, weight 5, the results' "items" title): at
    most N each. 6 delivery share and progress (hundredths: the delivery share, shared among
    the carriers by distance, plus 40 per new 10 m of a battery's progress): at most N × 900.
    17 stolen delivery share, 18 rescued delivery share: at most N × 100 each.
    24 solo delivery and 26 own steal (weight 0, but quests 6 and 7 count them): at most N
    each.
  - A cut is shared among the kits by what each counted, and every kit that counted keeps at
    least 1: it is still a kit used in the match (its wear, its part of the pay).
  - Each cut is logged once per player and kind ("counted T, paid K").
- **The catch (the owner: "sure, this is fine"):** a very active honest carrier who delivers
  more than N batteries in one match stops earning from the (N+1)th.
- **What it changes:** the result's statistics, so the score on the results screen, the
  activity row's pay and the quests (all srp computes from the result). Not the in-match
  scoreboard (the clients count it) and not the winner (the batteries in each container).
- **On the live worlds** (`battery_cap_of_saved_worlds`, an ignored test, last world):
  - in and out: put-ins 233 → 10, score 1313 → 198 (its 7 deliveries' share and 1 stolen kept);
  - ping-pong: steals 1800 → 1000 and 1600 → 1000 hundredths, put-ins 23 → 10 and 18 → 10,
    quest events 24/26 19/18 → 10 and 17/16 → 10; scores 830 → 485 and 672 → 422.
- **Release:** the match server only (srp). No client patch, no data.
- **Points while carrying (the owner, 2026-11-01; exe patch 92):** "if it grew down, we can add a
  few points too", and "if someone brought the battery further, we should make sure it is not
  farmable".
  - Each battery keeps, per team's container, the nearest it has come in its life (from the
    match's start or its last delivery; whoever carried it, dropping or handing it over keeps it),
    in 10 m steps from the carrier's position. The first carry of a life only sets it.
  - Each step its own team's nearest comes nearer is held on the battery for the carrier (its
    place and kit), 2 points a step ("let's double the number of points for getting the battery
    closer"): 40 hundredths of event 6.
- **Paid at the delivery (the owner, 2026-11-01):** "can the point be given to everyone once the
  battery is delivered?", because "otherwise it is possible to see that someone is bringing in
  the battery" (the score ticking up mid-carry shows the enemy, on the scoreboard, who carries a
  battery toward the container).
  - Nothing of the progress shows during the carry.
  - Put into a container: every carrier of that container's team is paid its held steps at once,
    with the delivery's own share and put-in.
  - The battery's life ends otherwise (the other team delivers it, the match starts again or
    ends): the held steps are dropped unpaid. A delivery starts its life again.
- **Found and put in (the owner, 2026-11-01):** "spotting a battery and moving it shouldn't
  count", then "also spotting should give 5pts", paid at once (the owner's last word): found
  (event 19) 2.5 → 5, counted when it happens as before. "For putting it in, we can remove a bit
  of points": the delivery share 10 → 5 (event 6's weight 10 → 5; the progress steps are 40
  hundredths so that a step is still 2 points); put in (20) stays 5.
- **What a battery pays now:** found 5 (its finder, at once), put in 5, the delivery share 5
  (shared among the carriers by distance), and 2 a 10 m of new ground to each carrier, at the
  delivery. A typical delivery (about 70 m, one carrier who also found it) is 5 + 5 + 5 + 14 = 29;
  a steal home (about 100 m) is the stolen share 35 + put in 5 + 20 = 60.
- **The cap with it:** event 6 is at most N × 900 hundredths (a battery's share, 100, and 200 m of
  progress, 20 × 40). Found stays at most N, put in at most N.
- **Not touched:** what accounts earned before: the owner's decision.

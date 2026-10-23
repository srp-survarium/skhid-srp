# 0.26g: how long progression takes (measured 2026-10-13)

Version scope: the 0.26g server (`srp-026g`) and its players' server. §1 to §6 are the
measurement, made at 120 % silver and the original's reputation; §7 is what the owner chose
from it. It measures what a match pays there and sets it against the game's own tables
(`match_levels_dict`, `factions_dict`, the items' `base_cost`) and srp's level table.

Tags: **MEASURED** = counted from the players' database and logs; **DATA** = the client's files;
**MODEL** = arithmetic on the two, with the assumptions of §3.

## 1. What was read

A read-only copy of the players' database of 2026-10-13 21:51 UTC and the server's logs from
2026-10-12 09:23 UTC to then (36.5 hours of play since the server opened).

- 142 settled matches (118 to the time limit, 6 by the mode's rule, 18 deserted), 699 results,
  652 of them paid. Match levels 0 to 5 only: 17 / 31 / 39 / 38 / 16 / 1 matches.
- 105 accounts, 76 with experience. Account levels: 25 at 0, 18 at 1, 6 at 2, 14 at 3, 8 at 4,
  3 at 5, 2 at 8. The two most active accounts played 41 and 52 matches (14 and 17 hours).
- The last 64 match reports (`g26_results`) carry every reward in full: 363 finished results,
  278 at the 80 % silver rate and 85 at 120 % (from 2026-10-13 18:38 UTC).

## 2. What a match pays (MEASURED)

**Time.** A match plays 15.0 minutes (median; mean 13.7). Inside a play session the next match
starts 18.2 minutes after the one before (median of 487 pairs): **3.3 matches an hour**.

**A finished result of the 120 % time** (85 results, match levels 2 to 4; the 80 % time is the
same but for silver):

| | mean | median | middle half |
|---|---|---|---|
| Experience | 219 | 185 | 111 to 338 |
| Reputation of the match | 23 | 22 | 12 to 30 |
| Reputation of quests | 140 | 86 | 0 to 220 |
| Silver of the match | 1,958 | 1,735 | 1,118 to 2,671 |
| Silver of quests | 254 | 157 | 0 to 471 |
| Silver of levels reached | 94 | 0 | |
| Autobuy and repair | −78 | | 53 % of results had any |

The same from the accounts' side: all experience ever given / all paid results = 220, all
reputation = 140. Quests give 86 % of the reputation. The means hold the first-three-wins
doubling (a quarter of the results) and premium (13 to 18 % of them).

**By match level**, silver of the match at 80 % (564 paid results): 429 / 674 / 930 / 1,284 /
1,574 at levels 0 to 4. Divided by the rate and the level's `money_k` that is 536 to 627: a
match is worth about **600 silver before the multiplier and the rate** at every level seen, and
205 experience, 21 + 118 reputation and 85 quest silver before theirs.

## 3. The model

A match at level L pays an average player: experience 205 × `experience_k`, reputation
139 × `reputation_k`, silver (600 × rate + 85) × `money_k`, and the silver of the levels reached.

Assumptions, each of which the real server breaks a little:

- The player's matches are of the level of the gear it wears. In fact a match's level is the mean
  of its players', so today's newcomers are paid at level 2 to 4 and a player far ahead of the
  others will be paid below its gear.
- A tier's kit is the faction's dearest weapon of that item level and one of each armour piece
  it sells at that level. Bought in the order of the table, each as soon as the rank and the
  silver are there; nothing sold back (a quarter), nothing found (four results in ten find an
  item), no ammo cost.
- The reputation goes to one faction at a time (the active accounts gave 19 % to others).
- Quest awards are flat over the ranks (DATA: 50 to 150, about 100 a quest in every band).

Check against the two most active accounts (41 and 52 matches): level 8 both; the model says
level 8 at match 47. Their gear is of tiers 4 to 6; the model has tier 4 bought at match 52.

## 4. How long it takes (MODEL, at 3.3 matches an hour)

**Account level** (500 experience for the first, 250 more each next; a skill point a level):

| Level | Experience | Matches | Hours |
|---|---|---|---|
| 3 | 2,250 | 10 | 3 |
| 5 | 5,000 | 22 | 7 |
| 10 | 16,250 | 67 | 20 |
| 20 | 57,500 | 200 | 61 |
| 30 | 123,750 | 390 | 118 |
| 50 | 331,250 | 985 | 300 |
| 100 | 1,287,500 | 3,729 | 1,130 |

The skill tree has 81 points in 26 skills; its deepest skills need 50 points spent before them
(DATA). So the last skills open after about 300 hours and the whole tree after about 750.

**Reputation alone** (what a rank costs and when it comes):

| Rank | Scavengers, Black Market | Renaissance, Edge |
|---|---|---|
| 1 | 100: match 1 | 1,500: match 9 |
| 3 | 990: match 7 | 5,640: match 33 (10 h) |
| 5 | 3,000: match 20 (6 h) | 11,500: match 63 (19 h) |
| 7 | | 19,320: match 102 (31 h) |
| 10 | | 35,250: match 173 (52 h) |

**A whole kit of each tier in turn**, Scavengers to rank 5 and then Renaissance, silver at 120 %:

| Tier | Rank | Kit, silver | Bought at match | Hours | Matches of silver for it alone |
|---|---|---|---|---|---|
| 1 | Scavengers 1 | 6,860 | 7 | 2 | 8.5 |
| 3 | 3 | 31,070 | 32 | 10 | 26 |
| 4 | 4 | 43,350 | 52 | 16 | 21 |
| 5 | 5 | 49,690 | 72 | 22 | 20 |
| 6 | Renaissance 1 | 72,880 | 96 | 29 | 24 |
| 8 | 3 | 129,040 | 132 | 40 | 37 |
| 9 | 4 | 165,750 | 168 | 51 | 37 |
| 12 | 7 | 269,510 | 221 | 67 | 54 |
| 14 | 9 | 475,790 | 293 | 89 | 72 |
| 15 | 10 | 402,500 | 345 | 105 | 53 |

At every tier the rank is there before the silver for the whole kit. Armour is 70 to 75 % of a
kit's price; a weapon alone is 7 to 17 matches of silver, so a player who buys only the weapon
waits for the rank instead. At the 80 % rate the last row was match 487; at 150 % it is 283.

## 5. What it says

- The low end is the quick end. In the first evening (6 to 7 hours) an average player reaches
  account level 5 and the last Scavengers rank, and has bought a tier 1 kit and most of a tier 3.
- A tier takes 20 to 26 matches of silver up to tier 6, 37 at tiers 8 and 9, and 53 to 72 from
  tier 12: the prices grow faster than `money_k` does.
- The account level is the longest road by far: level 20 is 61 hours, and it is the only source
  of skill points.
- Nobody is past match level 5 yet, so everything above is the tables' arithmetic, not seen.

## 6. Levers, and what each would move (MODEL)

All are the server's own; the results screen shows what the server sends.

| Change | Tier 3 | Tier 5 | Tier 9 | Tier 15 |
|---|---|---|---|---|
| Now: 120 % | 32 | 72 | 168 | 345 |
| Silver 200 % at tiers 0 to 2, 160 % at 3 and 4, 120 % after | 20 | 51 | 148 | 325 |
| Silver 150 % everywhere | 26 | 59 | 138 | 283 |
| Silver 120 % to tier 5, rising to 200 % at tier 12 and up | 32 | 72 | 158 | 276 |

(the match the kit is bought at.) Experience at twice the rate to account level 5 and one and a
half to level 10: level 5 at match 12 (was 22), level 10 at match 43 (was 67), level 20 at
match 182 (was 200).

A boost of the low end only buys about 20 matches, all of them before tier 5, and nothing later.

Past about 220 % at the top the silver stops being what is waited for: with 150 % to tier 5
rising to 250 % the kits of tiers 12 to 15 wait for the Renaissance rank, tier 15 comes at match
238 as it does with 220 %, and 156,000 silver is left over.

## 7. What the owner chose (2026-10-14)

"150 to tier 5, steps of 10 to 220. increase reputation a bit for all.", and then "x2 exp".
Built as (`results.md` §13):

- match silver by the match's level: 150 % at levels 0 to 5, then 160, 170, 180, 190, 200, 210,
  and 220 % from level 12 (`pay::MONEY_RATES`);
- the reputation of a match's rows and of its quests at 120 % (`SRP_REPUTATION_RATE`; the 120 is
  srp's reading of "a bit");
- a match's experience twice (`SRP_EXPERIENCE_RATE`, which no longer multiplies the reputation).

The same model with them (the match a whole kit is bought at, and the hours):

| | Tier 3 | Tier 5 | Tier 6 | Tier 9 | Tier 12 | Tier 15 |
|---|---|---|---|---|---|---|
| Before: 120 %, reputation 100 % | 32 (10 h) | 72 (22 h) | 96 (29 h) | 168 (51 h) | 221 (67 h) | 345 (105 h) |
| Now | 26 (8 h) | 59 (18 h) | 78 (24 h) | 132 (40 h) | 167 (51 h) | 238 (72 h) |

Silver is still what a whole kit waits for at every tier. The ranks alone: the last of the
Scavengers at match 17 (was 20), Renaissance rank 5 at 53 (was 63) and rank 10 at 144 (was 173).
A player who buys weapons only has the tier 15 one at match 168 (was 202), waiting for the rank
from tier 6 on. At today's match levels (2 to 4) a finished result's match silver goes from
about 1,960 to about 2,450 and its reputation from about 163 to about 196.

The account level, with gear rising to tier 15 over those 238 matches (hours of play):

| Experience | Level 5 | Level 10 | Level 20 | Level 30 | Level 50 | The tree's 81 points | Level when the top kit is bought |
|---|---|---|---|---|---|---|---|
| Before: the original's | 8 | 23 | 67 | 125 | 305 | 757 | 21 |
| Now: twice | 4 | 12 | 38 | 71 | 161 | 387 | 30 |

(§4's table has the gear rising a little faster, which is why its level 20 is 61 hours.) The
levels' own rewards come twice as fast with it: about 700 gold by the top kit instead of 500.

The silver's percent follows the match's level, so nothing above 150 % is paid until matches are above
level 5. To measure again: the per-match numbers of §2 from a fresh copy, then the tables of §4.

## 8. Reputation at rev 30 (measured 2026-10-23)

Players said a match's reputation is small ("never more than 150"). Read: the server's reward lines
of six hours of rev 30 (62 matches: 14 batteries, 8 timed batteries, 27 Team Deathmatch, 13
Research; 212 paid results, every one with a faction). The lines carry the rows' reputation (with
the daily bonus, premium and the items' bonus) and the quests' silver; the quests' reputation is
counted from that silver, since every 0.26g quest awards as much silver as reputation, at the
match level the silver gives (levels 13 to 15 for most matches). A result is doubled when it is
one of its account's first three victories in those hours (27 % of the results). Research is left
out: its rows were inflated by the statistics bug fixed in #352.

**A paid result (MEASURED)**, reputation:

| Mode | Results | Rows: median / p90 | Quests completed a result | Quests: mean / p90 | Total: median / p90 / mean |
|---|---|---|---|---|---|
| Batteries | 61 | 32 / 76 | 0.57 | 146 / 459 | 77 / 504 / 183 |
| Timed batteries | 46 | 34 / 70 | 0.96 | 265 / 643 | 230 / 700 / 301 |
| Team Deathmatch | 44 | 24 / 115 | 0.11 | 39 / 0 | 25 / 171 / 76 |
| All three | 151 | 28 / 78 | 0.56 | 151 / 495 | 62 / 565 / 188 |

- The rows are a fifth of it; the quests are the rest, as the original meant ("Основной вклад в
  прокачку репутации вносит выполнение заданий", official news of 2015-01-13).
- Half the results complete no quest. Against §2 (levels 2 to 4: 118 a result before the rates)
  the quests are done half as often: a higher band asks more (15 kills, a streak of 7, 8 batteries)
  for the same award, 50 to 150 by kind, times `reputation_k` (1.6 to 1.7 at levels 13 to 15).
- Team Deathmatch completes almost none: 130 of the 369 quests (bring, steal, win with batteries,
  defend) are battery goals the client counts in no other mode, and they run 24 hours.
- `win_x_games` pays 50 whether it asks 1, 2 or 3 victories, the least of any kind.

**What a rank takes (MODEL)**: the mean of the three modes, 37 of rows and 151 of quests a result,
each scaled by its rate; Renaissance and Edge ranks (1,500 for the first, 5,910 for the tenth,
35,250 in all), 3.3 matches an hour. The original's rates (100 and 100) give 156 a result: 225
matches (68 hours) to rank 10.

| Rows' rate | Quests' rate | A result | Rank 1 | Rank 10 alone | Rank 10 from 0 | Team Deathmatch alone: a result, rank 10 alone |
|---|---|---|---|---|---|---|
| 120 (now) | 120 (now) | 188 | 8 | 31 | 188 (57 h) | 76, 77 |
| 120 | 240 | 339 | 4 | 17 | 104 (32 h) | 115, 51 |
| 120 | 360 | 490 | 3 | 12 | 72 (22 h) | 154, 38 |
| 120 | 480 | 641 | 2 | 9 | 55 (17 h) | 193, 31 |
| 150 | 240 | 348 | 4 | 17 | 101 (31 h) | 125, 47 |
| 150 | 360 | 499 | 3 | 12 | 71 (21 h) | 164, 36 |
| 200 | 240 | 363 | 4 | 16 | 97 (29 h) | 140, 42 |
| 200 | 360 | 514 | 3 | 12 | 69 (21 h) | 179, 33 |

(matches.) The Scavengers' and Black Market's five ranks (3,000) take 16 matches now, 9 at 240.
A quest rate does little for a player of Team Deathmatch alone; the rows' rate does more there.
Settings: `SRP_QUEST_REPUTATION_RATE`, `SRP_QUEST_MONEY_RATE`, `SRP_QUEST_KIND_RATES`
(`results.md` §14).

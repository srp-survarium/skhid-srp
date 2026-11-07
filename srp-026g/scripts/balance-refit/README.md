# Balance refit

Fits the team-balance weights (`docs/notes/matchmaking.md` section 8) to what
the server logged for each match: its "sides" line (what every player counted
for), its "late join" lines, its per-minute lines (who was in it) and its
"settled" line (the winner). Python 3 only, no packages; prints no player
names.

```
# copy the logs off the host (the srp data volume's logs/: lobby.log*, srp.log*, and the day files srp-YYYY-MM-DD.log of before 2026-11-05)
python scripts/balance-refit/refit.py LOGDIR --since 2026-10-13T21:31
python scripts/balance-refit/refit.py LOGDIR --weights 10,1,2,4 --weights 8,1,2.5,4 --strict
python scripts/balance-refit/refit.py LOGDIR --start-roster
python scripts/balance-refit/refit.py LOGDIR --min-players 0
```

- Free fit: the weights that would have predicted the winners best, in the
  server's units (gear = 1) with a bootstrap range.
- Scored weights: the ones in force, the ones before, any `--weights P,G,L,R`,
  and each part alone, by log loss on the same matches. Weights marked
  "wrong way" score as a coin: the further they put a side ahead, the less it
  won.
- Use it after about 60 matches on one setting. Under about 40 the free fit
  is noise and the script says so.
- Which roster: a match that has per-minute lines is fitted on who was in it,
  each side being the mean of its rosters over the minutes (a player there
  for half of the match is half a player, as the balance rating counts it,
  `matchmaking.md` section 9.4). The roster of a minute is the starting one,
  with the late joiners so far, without the players the line names as out. A
  match without per-minute lines (a log from before them) is fitted on its
  starting roster. The script says how many matches it read each way.
- `--start-roster` fits every match on the sides the lobby made. That is the
  question "were the sides made fair"; the rosters by minute answer "who
  wins". The second counts a player for more than it is worth when the sides
  are made, because players leave a match they are losing. Run both.
- `--strict` leaves out the matches whose roster the log does not give in
  full: a per-minute line that could not be read, or no such lines and a sign
  that the roster changed (a late join, or as many settled as were not
  seated at the start).
  With `--start-roster` it leaves out the matches whose roster changed.
- Left out, and counted in the first line of the output:
  - a match with no winner;
  - a match a whole side left (`Deserted`, `NoShow`): its winner is who
    stayed, and the server moves no balance rating for it either;
  - a match with fewer than `--min-players` (4) in it for most of its time.
    The size of a match is the number of players that were in it for at
    least half of its per-minute lines, so a match that began one against
    one and grew counts as what it grew to; without such lines it is the
    starting roster. One against one and two against one are left out
    because two players can only be split one way, so the match says nothing
    about how sides should be made, and because so small a match is the
    easiest to play by agreement. `--min-players 0` keeps every size: run
    both and compare. The server moves no balance rating for a match of
    fewer than four either (`SRP_BALANCE_RATED_PLAYERS`, `matchmaking.md`
    section 8.3); it counts the players who were in for half of the
    longest stay, which is not always the same number.
- A per-minute line is taken only when it agrees with itself: the players it
  leaves in are as many as it says and count for what it says, less the lead.
  One that does not is left out and counted in a NOTE. Lines written before
  2026-10-14 do not name who is out, and their late-join lines give a joiner
  no parts: of such a match only the minutes with the whole starting roster
  in are read.
- The logged rating is the one at match start; the server updates it after.
- Needs the numbers in force as `IN_FORCE` at the top of the script: edit it
  when `SRP_BALANCE_*` changes.
- The script checks each match's logged per-player values against the formula
  and prints a NOTE when a match was made with other weights or another trust
  constant (the fit still uses the logged parts).
- Not read: the score, the lead and the server's own "expected at" of the
  per-minute lines. The numbers of the late-join rule (what a battery or a
  ticket of lead counts for) are not fitted here.
- **The strength rule** (`docs/notes/kd-balance.md`, since 2026-11-05): `--db` with copies of
  `srp.db` (files or directories of them; the backups on the jump host, the live one copied off)
  tests it instead of fitting the weights. The databases' match reports (`g26_results`, each
  kept about a day) give each player's kills, deaths and time in each match; a player is counted
  by its other matches (never by the match predicted). It prints the server's kills a minute and
  how far players' rates are apart (the prior), whether K/D x kpm predicts the winners better
  than the K/D alone (log loss with a bootstrap range, on the reports and on the logged matches
  since the K/D rule by who was in each minute), how many players are strong at `STRONG` and
  other cuts, the starts the rivals rule would have changed, the squads too strong to play, and
  the late joins the weaker-side rule would have kept waiting, and for how long (`--room-size`,
  8 a side). `--show <regex>` and `--clan <tag>` print those players by name (their K/D, kpm,
  strength at each level, rank); nothing else prints names. The rule's numbers are at the top of
  the script's back-test part: edit them with `level_stats.rs`.

  ```
  python scripts/balance-refit/refit.py LOGDIR --db DBDIR --clan ARTS --show '^(Dr1t|Alex Stan)$'
  ```
- Tests: `python -I scripts/balance-refit/test_refit.py` (synthetic logs, and
  the lines of one real match: parsing, the since filter, the rosters by
  minute, the matches left out, the fit recovering known weights, the
  scoring).

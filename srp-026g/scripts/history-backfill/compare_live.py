#!/usr/bin/env python3
"""compare_live.py: the backfill's check against srp itself (docs/notes/match_history.md, "Matches of before
the history").

usage: python3 compare_live.py <srp.db> <bundle.db>

The bundle is one `srp-history-backfill bundle` made from recordings of matches srp wrote live (the
history's own time), into a copy of that srp.db whose history was emptied first, so that every
recording is made again. Each of its matches is set beside srp's line by line: the match's columns,
each line's (team, place, seat, presence, outcome, score, kills, deaths, the counters, its time, its
clan), each kit's counters and items. It prints how many agree and every difference found.

What is known to differ, and why (each said in the totals):
- a line's rating and experience: not in a recording (left out of the comparison);
- a kit srp kept with no counters (the kit an account played when it did nothing): a recording does
  not say which kit an account played, the backfill keeps none;
- a free-for-all's mode, from a twist on (Racket, Saloon): not in a recording (the backfill leaves
  those matches out);
- a kit's items where the account changed it during the match (srp keeps the kit as the match ended,
  the recording the one the match was given), and a line's clan where the options did not read.
Reads both files only.
"""
import collections
import sqlite3
import sys

MATCH = ('game_match_id map_id game_mode match_level kind eligibility winner team0_points team1_points '
         'duration_ms').split()
LINE = ('clan_id team place seat presence outcome score kills deaths assists headshots grenade_kills melee_kills '
        'batteries captures artefacts shots hits played_ms').split()


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    live = sqlite3.connect(f'file:{sys.argv[1]}?mode=ro', uri=True)
    made = sqlite3.connect(f'file:{sys.argv[2]}?mode=ro', uri=True)
    counts = collections.Counter()
    columns = collections.Counter()
    differences = []
    for replay_id, made_id, made_end in made.execute('SELECT replay_id, id, ended_at FROM matches ORDER BY ended_at'):
        row = live.execute(f'SELECT id, ended_at, {", ".join(MATCH)} FROM g26_matches WHERE replay_id = ?',
                           (replay_id,)).fetchone()
        if row is None:
            counts['matches srp does not have'] += 1
            continue
        live_id, live_end, *live_match = row
        made_match = made.execute(f'SELECT {", ".join(MATCH)} FROM matches WHERE id = ?', (made_id,)).fetchone()
        counts['matches'] += 1
        counts['match ends within 2 s' if abs(live_end - made_end) <= 2 else 'match ends apart'] += 1
        for name, a, b in zip(MATCH, live_match, made_match):
            if a != b:
                columns[f'match {name}'] += 1
                differences.append((replay_id, 'match', name, a, b))
        live_lines = {r[0]: r[1:] for r in live.execute(
            f'SELECT account_id, {", ".join(LINE)} FROM g26_match_players WHERE match_id = ?', (live_id,))}
        made_lines = {r[0]: r[1:] for r in made.execute(
            f'SELECT account_id, {", ".join(LINE)} FROM match_players WHERE match_id = ?', (made_id,))}
        for account in sorted(set(live_lines) | set(made_lines)):
            if account not in live_lines or account not in made_lines:
                counts['lines on one side only'] += 1
                differences.append((replay_id, 'line', account, account in live_lines, account in made_lines))
                continue
            counts['lines'] += 1
            same = True
            for name, a, b in zip(LINE, live_lines[account], made_lines[account]):
                if a != b:
                    same = False
                    columns[f'line {name}'] += 1
                    differences.append((replay_id, 'line', account, name, a, b))
            counts['lines equal' if same else 'lines that differ'] += 1
        live_kits = {(r[0], r[1]): r[2:] for r in live.execute(
            'SELECT account_id, kit, kills, headshots, shots, hits FROM g26_match_kits WHERE match_id = ?', (live_id,))}
        made_kits = {(r[0], r[1]): r[2:] for r in made.execute(
            'SELECT account_id, kit, kills, headshots, shots, hits FROM match_kits WHERE match_id = ?', (made_id,))}
        for key, numbers in live_kits.items():
            if key not in made_kits:
                counts['kits srp kept with no counters' if not any(numbers) else 'kits with counters not made'] += 1
        for key, numbers in made_kits.items():
            if key not in live_kits:
                counts['kits srp did not keep'] += 1
                continue
            counts['kits with equal counters' if numbers == live_kits[key] else 'kits whose counters differ'] += 1
            items = [set(c.execute(f'SELECT slot, dict_id FROM {t} WHERE match_id = ? AND account_id = ? AND kit = ?',
                                   (i, *key)))
                     for c, t, i in ((live, 'g26_match_kit_items', live_id), (made, 'match_kit_items', made_id))]
            if not items[1]:
                counts['kits with no items made (options not read)'] += 1
            else:
                counts['kits with equal items' if items[0] == items[1] else 'kits whose items differ'] += 1
    for name, count in sorted(counts.items()):
        print(f'{count}\t{name}')
    for name, count in sorted(columns.items()):
        print(f'{count}\tdiffer: {name}')
    for difference in differences[:40]:
        print('\t'.join(map(str, difference)))


if __name__ == '__main__':
    main()

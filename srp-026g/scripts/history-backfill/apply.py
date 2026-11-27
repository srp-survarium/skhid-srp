#!/usr/bin/env python3
"""apply.py: add the matches of before the history, made again from their recordings, to srp.db once
(docs/notes/match_history.md, "Matches of before the history").

usage: python3 apply.py [--apply | --undo] <bundle.db> <srp.db>

The bundle is made by `srp-history-backfill bundle` on a copy of srp.db: the matches of before the
history (2026-11-20) that a kept recording gives, each made as srp's settlement makes a match
(`source` 'recording'), with their lines, kits and items, and what they add to the totals by player,
clan and item (`delta_<table>`). This script adds those to srp.db; it computes nothing of its own.

Once, at the release that ships migration 27 (the matches' `source`, a line's unknown rating, the
order by the matches' end), after srp has started once with it, with srp stopped (the release's
one-off scripts: .claude/skills/skhid-release/SKILL.md, step 4). It never touches the accounts, their
ratings, the balance, level_stats, map_plays, rewards or quests: those took the matches when they
were played.

Without --apply or --undo, a dry run: srp.db is only read (any copy, srp running or not). It is copied
to a temporary file, the bundle applied to the copy, and what changed printed: the matches by day and
by mode, the lines, the accounts, the largest changes of players' totals, the all-time totals and the
top lists (by rating, matches, wins, kills) before and after.

With --apply: srp.db is first copied to backups/srp-<unix>-before-history-backfill.db beside it (with
the file's owner and mode); then in one transaction each match goes in under a number of its own (the
matches of srp keep theirs: the history lists them by their end), its children with it, the totals
get their deltas, and the mark history_backfill (the matches added) is set in g26_meta. A file with
the mark gets nothing more; a match srp.db has already (by its replay id) is left out, with its deltas
too: the script then stops, since the deltas were made for all of them.

With --undo: in one transaction, the bundle's matches come out (by replay id, source 'recording'),
their deltas are taken back from the totals, a total row left all zero by them goes when the history
holds no line of its key any more, and the mark goes.

It reads the 0.26g tables only: nothing that reads or rebuilds the 0.21d nickname index (its
collation is srp's own). Needs Python 3's sqlite3 module only (the host has no sqlite3 command).
"""
import argparse
import contextlib
import os
from pathlib import Path
import shutil
import sqlite3
import sys
import tempfile
import time

MARK = 'history_backfill'
SCHEMA = 27                  # history::BACKFILL_MIGRATION
CHILDREN = ('match_players', 'match_kits', 'match_kit_items', 'match_weapons')
TOTALS = {                   # table: its key
    'g26_player_totals': ('account_id',),
    'g26_clan_totals': ('clan_id',),
    'g26_player_items': ('account_id', 'dict_id'),
    'g26_player_weapons': ('account_id', 'dict_id'),
}
# A total that is not a sum (the longest kill): kept as the larger.
LARGEST = {'longest_kill_dm'}
# What a total's key is told by when it has no line left (undo).
LINES_OF = {
    'g26_player_totals': "SELECT 1 FROM g26_match_players p JOIN g26_matches m ON m.id = p.match_id "
                         "WHERE m.kind = 'public' AND p.account_id = ?",
    'g26_clan_totals': "SELECT 1 FROM g26_match_players p JOIN g26_matches m ON m.id = p.match_id "
                       "WHERE m.eligibility = 'counted' AND p.clan_id = ?",
    'g26_player_items': None,
    'g26_player_weapons': None,
}


def columns(connection, table, schema='main'):
    return [row[1] for row in connection.execute(f'PRAGMA {schema}.table_info({table})')]


def schema_of(connection):
    row = connection.execute("SELECT value FROM g26_meta WHERE key = 'schema'").fetchone()
    return row[0] if row else 0


def marked(connection):
    return connection.execute('SELECT EXISTS (SELECT 1 FROM g26_meta WHERE key = ?)', (MARK,)).fetchone()[0] == 1


def check(connection):
    if schema_of(connection) < SCHEMA:
        sys.exit(f'srp.db is at schema {schema_of(connection)}: start srp of migration {SCHEMA} once first')


def apply(connection):
    """The bundle (attached as b) into main, in the caller's transaction. Returns the matches added."""
    present = connection.execute(
        'SELECT COUNT(*) FROM b.matches bm JOIN main.g26_matches m ON m.replay_id = bm.replay_id').fetchone()[0]
    if present:
        raise SystemExit(f'{present} of the bundle\'s matches are in srp.db already: its deltas were made for '
                         'all of them; nothing added')
    match_columns = [c for c in columns(connection, 'g26_matches') if c != 'id']
    numbers = {}
    for row in connection.execute(f'SELECT id, {", ".join(match_columns)} FROM b.matches ORDER BY ended_at, id').fetchall():
        cursor = connection.execute(
            f'INSERT INTO main.g26_matches ({", ".join(match_columns)}) VALUES ({", ".join("?" * len(match_columns))})',
            row[1:])
        numbers[row[0]] = cursor.lastrowid
    for child in CHILDREN:
        table = f'g26_{child}'
        names = columns(connection, table)
        for row in connection.execute(f'SELECT {", ".join(names)} FROM b.{child}').fetchall():
            row = list(row)
            row[names.index('match_id')] = numbers[row[names.index('match_id')]]
            connection.execute(f'INSERT INTO main.{table} ({", ".join(names)}) VALUES ({", ".join("?" * len(names))})', row)
    for table, keys in TOTALS.items():
        names = [c for c in columns(connection, table)]
        values = [c for c in names if c not in keys and c != 'gear']
        sets = ', '.join(f'{c} = MAX({c}, excluded.{c})' if c in LARGEST else f'{c} = {c} + excluded.{c}' for c in values)
        for row in connection.execute(f'SELECT {", ".join(names)} FROM b.delta_{table}').fetchall():
            connection.execute(
                f'INSERT INTO main.{table} ({", ".join(names)}) VALUES ({", ".join("?" * len(names))}) '
                f'ON CONFLICT({", ".join(keys)}) DO UPDATE SET {sets}', row)
    connection.execute('INSERT INTO main.g26_meta (key, value) VALUES (?, ?)', (MARK, len(numbers)))
    return len(numbers)


def undo(connection):
    """The bundle's matches and deltas out of main, in the caller's transaction. Returns the matches taken."""
    ids = [row[0] for row in connection.execute(
        "SELECT m.id FROM main.g26_matches m JOIN b.matches bm ON bm.replay_id = m.replay_id "
        "WHERE m.source = 'recording'")]
    for match_id in ids:
        for child in CHILDREN:
            connection.execute(f'DELETE FROM main.g26_{child} WHERE match_id = ?', (match_id,))
        connection.execute('DELETE FROM main.g26_matches WHERE id = ?', (match_id,))
    for table, keys in TOTALS.items():
        names = columns(connection, table)
        values = [c for c in names if c not in keys and c != 'gear']
        sums = [c for c in values if c not in LARGEST]
        where = ' AND '.join(f'{k} = ?' for k in keys)
        for row in connection.execute(f'SELECT {", ".join(keys)}, {", ".join(sums)} FROM b.delta_{table}').fetchall():
            key, deltas = row[:len(keys)], row[len(keys):]
            connection.execute(f'UPDATE main.{table} SET {", ".join(f"{c} = {c} - ?" for c in sums)} WHERE {where}',
                               (*deltas, *key))
            zero = connection.execute(f'SELECT {" AND ".join(f"{c} = 0" for c in sums)} FROM main.{table} WHERE {where}',
                                      key).fetchone()
            if zero and zero[0]:
                lines = LINES_OF[table]
                if lines is None or connection.execute(lines, key).fetchone() is None:
                    connection.execute(f'DELETE FROM main.{table} WHERE {where}', key)
    connection.execute('DELETE FROM main.g26_meta WHERE key = ?', (MARK,))
    return len(ids)


def summary(connection):
    """What the dry run compares: the all-time totals and the top lists (account ids by place)."""
    totals = connection.execute(
        "SELECT COUNT(*), SUM(matches), SUM(kills), SUM(assists), SUM(shots), SUM(hits), SUM(played_ms) "
        "FROM g26_player_totals WHERE matches > 0").fetchone()
    counted = connection.execute("SELECT COUNT(*) FROM g26_matches WHERE eligibility = 'counted'").fetchone()[0]
    tops = {}
    for by in ('matches', 'wins', 'kills'):
        tops[by] = [row[0] for row in connection.execute(
            f'SELECT account_id FROM g26_player_totals WHERE matches > 0 ORDER BY {by} DESC, account_id LIMIT 20')]
    return {'players': totals[0], 'counted matches': counted, 'kills': totals[2] or 0, 'assists': totals[3] or 0,
            'shots': totals[4] or 0, 'hits': totals[5] or 0, 'hours': round((totals[6] or 0) / 3_600_000, 1)}, tops


def report(connection, before, after, names):
    print('matches added, by day (UTC) and kind:')
    for day, kind, eligibility, count in connection.execute(
            "SELECT date(ended_at, 'unixepoch'), kind, IFNULL(eligibility, ''), COUNT(*) FROM b.matches "
            "GROUP BY 1, 2, 3 ORDER BY 1, 2, 3"):
        print(f'  {day}\t{kind}\t{eligibility}\t{count}')
    print('matches added, by mode (lobby number) of the counted ones:')
    for mode, count in connection.execute(
            "SELECT game_mode, COUNT(*) FROM b.matches WHERE eligibility = 'counted' GROUP BY 1 ORDER BY 2 DESC"):
        print(f'  {mode}\t{count}')
    lines, accounts, unrated = connection.execute(
        'SELECT COUNT(*), COUNT(DISTINCT account_id), SUM(rating IS NULL) FROM b.match_players').fetchone()
    print(f'lines {lines}, accounts {accounts}, lines with no rating known {unrated}')
    print('the largest changes of a player\'s totals (account, name, matches, wins, kills, deaths, hours):')
    for row in connection.execute(
            'SELECT d.account_id, a.name, d.matches, d.wins, d.kills, d.deaths, d.played_ms '
            'FROM b.delta_g26_player_totals d LEFT JOIN main.g26_accounts a ON a.account_id = d.account_id '
            'ORDER BY d.matches DESC, d.kills DESC LIMIT 15'):
        print(f'  {row[0]}\t{row[1]}\t+{row[2]}\t+{row[3]}\t+{row[4]}\t+{row[5]}\t+{row[6] / 3_600_000:.1f}')
    (totals_before, tops_before), (totals_after, tops_after) = before, after
    print('all-time totals, before -> after:')
    for key in totals_before:
        print(f'  {key}\t{totals_before[key]} -> {totals_after[key]}')
    for by in tops_before:
        print(f'top 20 by {by} (place: account, name; * new in the 20, (was n) moved):')
        for place, account in enumerate(tops_after[by], 1):
            was = tops_before[by].index(account) + 1 if account in tops_before[by] else None
            mark = '*' if was is None else ('' if was == place else f'(was {was})')
            print(f'  {place}\t{account}\t{names.get(account, "")}\t{mark}')


def backup(db):
    folder = db.parent / 'backups'
    folder.mkdir(exist_ok=True)
    stamp = int(time.time())
    copy = folder / f'srp-{stamp}-before-history-backfill.db'
    n = 1
    while copy.exists():    # a second run in the same second keeps the first copy
        n += 1
        copy = folder / f'srp-{stamp}-{n}-before-history-backfill.db'
    source = sqlite3.connect(f'file:{db}?mode=ro', uri=True)
    target = sqlite3.connect(copy)
    with target:
        source.backup(target)
    source.close()
    target.close()
    stat = db.stat()
    try:
        os.chown(copy, stat.st_uid, stat.st_gid)
    except PermissionError:
        pass
    shutil.copymode(db, copy)
    return copy


def attach(connection, bundle):
    """The bundle as b, read only (the connection takes URIs)."""
    connection.execute('ATTACH DATABASE ? AS b', (f'file:{bundle.resolve()}?mode=ro',))
    if connection.execute("SELECT COUNT(*) FROM b.sqlite_master WHERE name = 'matches'").fetchone()[0] != 1:
        sys.exit(f'{bundle} is no bundle of srp-history-backfill')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    action = parser.add_mutually_exclusive_group()
    action.add_argument('--apply', action='store_true')
    action.add_argument('--undo', action='store_true')
    parser.add_argument('bundle', type=Path)
    parser.add_argument('db', type=Path)
    args = parser.parse_args()
    for path in (args.bundle, args.db):
        if not path.is_file():
            sys.exit(f'no file {path}')
    if not (args.apply or args.undo):
        with tempfile.TemporaryDirectory() as folder:
            copy = Path(folder) / 'srp.db'
            source = sqlite3.connect(f'file:{args.db}?mode=ro', uri=True)
            target = sqlite3.connect(copy)
            with target:
                source.backup(target)
            source.close()
            target.close()
            with contextlib.closing(sqlite3.connect(copy, isolation_level=None, uri=True)) as connection:
                connection.execute('PRAGMA foreign_keys = ON')
                check(connection)
                attach(connection, args.bundle)
                if marked(connection):
                    print(f'applied before (g26_meta {MARK}): --apply adds nothing')
                    return
                names = dict(connection.execute('SELECT account_id, name FROM g26_accounts'))
                before = summary(connection)
                connection.execute('BEGIN')
                added = apply(connection)
                connection.execute('COMMIT')
                print(f'dry run (--apply changes the file): {added} matches would be added')
                report(connection, before, summary(connection), names)
        return
    copy = backup(args.db)
    print(f'copied {args.db} to {copy}')
    with contextlib.closing(sqlite3.connect(args.db, isolation_level=None, uri=True)) as connection:
        connection.execute('PRAGMA foreign_keys = ON')
        check(connection)
        attach(connection, args.bundle)
        connection.execute('BEGIN IMMEDIATE')
        try:
            if args.apply and marked(connection):
                connection.execute('ROLLBACK')
                print(f'applied before (g26_meta {MARK}): nothing added')
                return
            if args.undo and not marked(connection):
                connection.execute('ROLLBACK')
                print(f'not applied (no g26_meta {MARK}): nothing taken')
                return
            count = apply(connection) if args.apply else undo(connection)
            connection.execute('COMMIT')
        except BaseException:
            connection.execute('ROLLBACK')
            raise
    print(f'{"added" if args.apply else "taken out"}: {count} matches')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""grant.py: give each account of id 1 to 1000 its number decal, once (docs/notes/number-decals.md).

usage: python3 grant.py [--apply] <srp.db>

Once, at the release that adds the pack number-decals (the owner, 2026-11-26: "we will add them
to the game and I will give a number based on the id for every single person (in first 1k)"):
the account of id N, 1 to 1000, owns "Number N" (the item decals_num_<NNNN>, dictionary id
5000 + N). Ids that no account has give nothing; bans do not matter; the test account (4) and
System (590) get none. An account of such an id that has no 0.26g account yet gets its number
from srp when that account is made (session v026g::number_decals); this script lists it.

Without --apply, a dry run: reads the file only (any copy, srp running or not) and prints, one
row each, what --apply would give (number, account id, 0.26g name, what is done), then the
totals. With --apply, on the live srp.db with srp stopped (as the release's other one-off
scripts: .claude/skills/skhid-release/SKILL.md, step 4): it first copies the file to
backups/srp-<unix>-before-number-decals.db beside it (with the file's owner and mode), then, in
one transaction, puts each decal into its account's inventory (the next item id, after its
items) and sets the mark number_decals_given in g26_meta. A file with the mark gets nothing more
(a dry run says so), and an account that owns its decal already is left as it is.

It reads the accounts' ids (accounts.id) and the 0.26g tables only: nothing that reads or
rebuilds the 0.21d nickname index (its collation is srp's own). Needs Python 3's sqlite3 module
only (the host has no sqlite3 command).
"""
import argparse
import contextlib
import os
from pathlib import Path
import shutil
import sqlite3
import sys
import time

NUMBERS = range(1, 1001)
FIRST_DICT_ID = 5000            # names.py: N's dictionary id is 5000 + N
LEFT_OUT = {4: 'a test account', 590: 'System'}   # session v026g::number_decals::LEFT_OUT
FULL_CONDITION = 1_000_000      # a new item's condition (lobby::FULL_CONDITION)
MARK = 'number_decals_given'


def dict_id(number):
    return FIRST_DICT_ID + number


def plan(connection):
    """[(number, account id, 0.26g name or None, what)] for every account of a numbered id, in
    id order. what: 'give', 'owns' (it has it already), 'at sign-up' (no 0.26g account yet:
    srp gives it then), 'left out'."""
    rows = connection.execute(
        """SELECT a.id, g.name,
                  EXISTS (SELECT 1 FROM g26_items i WHERE i.account_id = a.id AND i.dict_id = ? + a.id)
             FROM accounts a LEFT JOIN g26_accounts g ON g.account_id = a.id
            WHERE a.id BETWEEN ? AND ? ORDER BY a.id""",
        (FIRST_DICT_ID, NUMBERS.start, NUMBERS.stop - 1)).fetchall()
    out = []
    for account_id, name, owns in rows:
        if account_id in LEFT_OUT:
            what = 'left out'
        elif name is None:
            what = 'at sign-up'
        elif owns:
            what = 'owns'
        else:
            what = 'give'
        out.append((account_id, account_id, name, what))
    return out


def given_before(connection):
    return connection.execute('SELECT EXISTS (SELECT 1 FROM g26_meta WHERE key = ?)', (MARK,)).fetchone()[0] == 1


def report(rows):
    for number, account_id, name, what in rows:
        note = LEFT_OUT.get(account_id, '')
        print(f'{number}\t{account_id}\t{name or "-"}\t{what}{" (" + note + ")" if note else ""}')
    counts = {what: sum(1 for row in rows if row[3] == what) for what in ('give', 'owns', 'at sign-up', 'left out')}
    taken = {row[1] for row in rows}
    print(f'{counts["give"]} decals to give, {counts["owns"]} owned already, {counts["at sign-up"]} at their '
          f'0.26g sign-up, {counts["left out"]} left out; {len(NUMBERS) - len(taken)} numbers no account has')


def give(connection, rows):
    """Each 'give' row's decal into its account: after its items (seq), at the next item id."""
    for number, account_id, _, what in rows:
        if what != 'give':
            continue
        next_id, last_id, last_seq = connection.execute(
            """SELECT g.next_item_id,
                      (SELECT COALESCE(MAX(item_id), 0) FROM g26_items WHERE account_id = g.account_id),
                      (SELECT COALESCE(MAX(seq), -1) FROM g26_items WHERE account_id = g.account_id)
                 FROM g26_accounts g WHERE g.account_id = ?""", (account_id,)).fetchone()
        item_id = max(next_id, last_id + 1)
        connection.execute(
            """INSERT INTO g26_items (account_id, item_id, seq, dict_id, condition_or_stack, repair_time,
                                      reroll_time, modifications, upgrades)
               VALUES (?, ?, ?, ?, ?, 0, 0, '', '')""",
            (account_id, item_id, last_seq + 1, dict_id(number), FULL_CONDITION))
        connection.execute('UPDATE g26_accounts SET next_item_id = ? WHERE account_id = ?', (item_id + 1, account_id))
    connection.execute('INSERT INTO g26_meta (key, value) VALUES (?, ?)', (MARK, int(time.time())))


def backup(db):
    folder = db.parent / 'backups'
    folder.mkdir(exist_ok=True)
    copy = folder / f'srp-{int(time.time())}-before-number-decals.db'
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


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('db', type=Path)
    args = parser.parse_args()
    if not args.db.is_file():
        sys.exit(f'no file {args.db}')
    if not args.apply:
        with contextlib.closing(sqlite3.connect(f'file:{args.db}?mode=ro', uri=True)) as connection:
            if given_before(connection):
                print('given before (g26_meta number_decals_given): --apply gives nothing')
                return
            print('dry run (--apply changes the file):')
            report(plan(connection))
        return
    copy = backup(args.db)
    print(f'copied {args.db} to {copy}')
    with contextlib.closing(sqlite3.connect(args.db, isolation_level=None)) as connection:
        connection.execute('BEGIN IMMEDIATE')
        try:
            if given_before(connection):
                connection.execute('ROLLBACK')
                print('given before (g26_meta number_decals_given): nothing given')
                return
            rows = plan(connection)
            give(connection, rows)
            connection.execute('COMMIT')
        except BaseException:
            connection.execute('ROLLBACK')
            raise
    report(rows)
    print('applied')


if __name__ == '__main__':
    main()

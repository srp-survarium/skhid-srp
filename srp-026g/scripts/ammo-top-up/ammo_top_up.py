#!/usr/bin/env python3
"""Once, at the release after revision 58: 1000 rounds of every ammo for every account.

The owner, 2026-11-13: "we added new type of ammo", "and didn't add 1000 bullets
for everyone". A new account is given 1000 rounds of every ammo
(`tutorial::signup_grant`, scripts/content-edit/new-account.tsv `rounds`); an
account made before an ammo was added never got it: on the pre-rev58 copy, the
five newest (23mm, 7.62x51 of 20, the two belts, the SCAR-H's AP) were missing
on about 590 of 799 accounts.

The rule, per account and ammo: fewer than 1000 rounds owned -> topped up to
1000, onto the stack it owns, else as a new stack in its storage (the next item
id); never any taken. Kits, presets and the rest are left as they are. Run
again, it adds nothing: every account has its 1000 by then.

    python3 ammo_top_up.py <srp.db>           # dry run: reads the file only
    python3 ammo_top_up.py --apply <srp.db>   # srp stopped

The dry run works on any copy, srp running or not, and prints what --apply
would give, per ammo and in total. --apply is for the live srp.db with srp
stopped (the release skill, "one-off change of the accounts": right after the
swap and srp's start copy of srp.db, `docker compose stop srp26`, run as root
on the data volume's srp.db, `docker compose start srp26`). It first copies the
file to backups/srp-<unix>-before-ammo-top-up.db beside it (SQLite's backup,
with the file's owner and mode), then changes everything in one transaction.
It touches only g26_items and g26_accounts.next_item_id: nothing rebuilds or
reads the 0.21d nickname index, whose collation is srp's own
(session::nickname_key) and unknown here.

The ammo: every stack under gameplay/items/weapons/ammo/ of the released data
of revision 58, 39 of them (the list srp-026g/crates/session/tests/
starter_kit.rs checks on that data). Ammo added after it needs its row here
before the script runs again.
"""

import argparse
import os
import pathlib
import sqlite3
import stat
import sys
import time

ROUNDS = 1000

AMMO = [
    (7, "ammo_5.45x39_7n6m"), (20, "ammo_.357m"), (22, "ammo_12mm_buck"),
    (50, "ammo_7.62x25"), (51, "ammo_7.62x51"), (52, "ammo_7.62x51_ap"),
    (53, "ammo_9x19p_para"), (70, "ammo_9x18_makarov"), (71, "ammo_9x19_7n21"),
    (72, "ammo_12mm_slug"), (81, "ammo_7.62x39"), (91, "ammo_.30-30"),
    (103, "ammo_9x18_pbm"), (104, "ammo_7.62x54r_old"), (147, "ammo_7.62x39_m"),
    (148, "ammo_7.62x54_7n1"), (217, "ammo_5.45x39_7n10"), (218, "ammo_7.62x39_7n23"),
    (300, "ammo_pab9"), (301, "ammo_sp5"), (302, "ammo_sp6"), (322, "ammo_9x21"),
    (323, "ammo_45acp"), (331, "ammo_5.45x39_7n22"), (368, "ammo_9x19p_7n31"),
    (369, "ammo_6.5x39"), (370, "ammo_12.7x55"), (371, "ammo_12.7x54_pt"),
    (372, "ammo_12.7x54_vps"), (2201, "ammo_5.56x45"), (2202, "ammo_4.6x30"),
    (2203, "ammo_5.7x28"), (2204, "ammo_7.62x54_7n13"), (2205, "ammo_12mm_buck2"),
    (2253, "ammo_23mm_r"), (2254, "ammo_7.62x51_20"), (2265, "ammo_5.56x45_belt"),
    (2266, "ammo_7.62x51_scar_ap"), (2275, "ammo_7.62x51_belt"),
]

# What each account is short of each ammo: temporary tables only.
PLAN = [
    "CREATE TEMP TABLE ammo (dict_id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
    f"""CREATE TEMP TABLE short AS
      SELECT account_id, dict_id, name, {ROUNDS} - owned AS rounds, stack_seq FROM (
        SELECT a.account_id, m.dict_id, m.name,
               COALESCE((SELECT SUM(i.condition_or_stack) FROM g26_items i
                         WHERE i.account_id = a.account_id AND i.dict_id = m.dict_id), 0) AS owned,
               (SELECT MIN(i.seq) FROM g26_items i
                 WHERE i.account_id = a.account_id AND i.dict_id = m.dict_id) AS stack_seq
          FROM g26_accounts a CROSS JOIN temp.ammo m)
      WHERE owned < {ROUNDS}""",
]

# The change: a stack owned grows, an ammo not owned is a new stack after the
# account's items, with the next item ids.
CHANGE = [
    """UPDATE g26_items SET condition_or_stack = condition_or_stack + (
        SELECT s.rounds FROM temp.short s
         WHERE s.account_id = g26_items.account_id AND s.dict_id = g26_items.dict_id)
     WHERE EXISTS (SELECT 1 FROM temp.short s
                    WHERE s.account_id = g26_items.account_id AND s.dict_id = g26_items.dict_id
                      AND s.stack_seq = g26_items.seq)""",
    """CREATE TEMP TABLE new_stack AS
      SELECT s.account_id, s.dict_id, s.rounds,
             MAX(a.next_item_id, (SELECT COALESCE(MAX(i.item_id), 0) + 1 FROM g26_items i
                                   WHERE i.account_id = s.account_id))
               + ROW_NUMBER() OVER (PARTITION BY s.account_id ORDER BY s.dict_id) - 1 AS item_id,
             (SELECT COALESCE(MAX(i.seq), -1) + 1 FROM g26_items i
               WHERE i.account_id = s.account_id)
               + ROW_NUMBER() OVER (PARTITION BY s.account_id ORDER BY s.dict_id) - 1 AS seq
        FROM temp.short s JOIN g26_accounts a ON a.account_id = s.account_id
       WHERE s.stack_seq IS NULL""",
    """INSERT INTO g26_items (account_id, item_id, seq, dict_id, condition_or_stack, repair_time,
                             reroll_time, modifications, upgrades)
      SELECT account_id, item_id, seq, dict_id, rounds, 0, 0, '', '' FROM temp.new_stack""",
    """UPDATE g26_accounts SET next_item_id = (
        SELECT MAX(n.item_id) + 1 FROM temp.new_stack n
         WHERE n.account_id = g26_accounts.account_id)
     WHERE account_id IN (SELECT account_id FROM temp.new_stack)""",
]


def plan(db: sqlite3.Connection) -> None:
    """The temporary tables of what is short, and the report."""
    for statement in PLAN[:1]:
        db.execute(statement)
    db.executemany("INSERT INTO temp.ammo VALUES (?, ?)", AMMO)
    for statement in PLAN[1:]:
        db.execute(statement)
    rows = db.execute(
        "SELECT dict_id, name, COUNT(*), SUM(rounds) FROM temp.short "
        "GROUP BY dict_id ORDER BY dict_id"
    )
    for dict_id, name, accounts, rounds in rows:
        print(f"{dict_id}\t{name}\t{accounts} accounts\t{rounds} rounds")
    accounts, rounds = db.execute(
        "SELECT COUNT(DISTINCT account_id), COALESCE(SUM(rounds), 0) FROM temp.short"
    ).fetchone()
    (total,) = db.execute("SELECT COUNT(*) FROM g26_accounts").fetchone()
    (new,) = db.execute("SELECT COUNT(*) FROM temp.short WHERE stack_seq IS NULL").fetchone()
    print(f"{accounts} of {total} accounts topped up, {rounds} rounds given, {new} new stacks")


def copy_first(db: sqlite3.Connection, path: pathlib.Path) -> None:
    """backups/srp-<unix>-before-ammo-top-up.db beside the file, its owner and mode."""
    backups = path.parent / "backups"
    backups.mkdir(exist_ok=True)
    copy = backups / f"srp-{int(time.time())}-before-ammo-top-up.db"
    if copy.exists():
        sys.exit(f"{copy} is there already")
    target = sqlite3.connect(copy)
    with target:
        db.backup(target)
    target.close()
    st = path.stat()
    os.chmod(copy, stat.S_IMODE(st.st_mode))
    try:
        os.chown(copy, st.st_uid, st.st_gid)
    except PermissionError:
        pass  # not root: the copy is the runner's, as the file is
    print(f"copied {path} to {copy}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--apply", action="store_true", help="change the file (srp stopped)")
    parser.add_argument("db", type=pathlib.Path, help="srp.db")
    args = parser.parse_args()
    path = args.db.resolve()
    if not path.is_file():
        sys.exit(f"no file {path}")
    if not args.apply:
        print("dry run (--apply changes the file):")
        db = sqlite3.connect(f"{path.as_uri()}?mode=ro", uri=True, isolation_level=None)
        plan(db)
        db.close()
        return 0
    db = sqlite3.connect(path, isolation_level=None)
    copy_first(db, path)
    db.execute("BEGIN IMMEDIATE")
    try:
        plan(db)
        for statement in CHANGE:
            db.execute(statement)
        db.execute("COMMIT")
    except BaseException:
        db.execute("ROLLBACK")
        raise
    db.close()
    print("applied")
    return 0


if __name__ == "__main__":
    sys.exit(main())

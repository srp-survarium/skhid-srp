#!/usr/bin/env python3
"""Empty every RGD-5 pocket of every kit and preset, once (revision 49).

Revision 49 ships #1276: an RGD-5 takes two pocket units, so a pocket holds
at most 2 of them, 1 in a pocket of 3 or fewer. The server does not know a
kit's pocket size (the exe sets it), so kits saved before the rule are not
fixed at load; the release empties their RGD-5 pockets once instead
(docs/notes/one-pocket-a-type.md, "Kits saved before the rule"). The
grenades stay in the storage (g26_items is not touched): players put them
back under the rule.

  python3 -I rgd5-pocket-reset.py <srp.db> [--dry-run]

Run it with srp stopped and after a backup of srp.db. In one transaction:

- every kit that has an RGD-5 (dict_id 5) in a pocket (slots 13..18) gets
  its revision raised by one (u32, wrapping, as the server does), so the
  lobby takes the kit anew;
- those kit slots are deleted (an empty slot has no row);
- every preset slot of dict_id 5 in a pocket is deleted (a preset slot keeps
  the dict id: a stack used up is found again by it);
- the key `rgd5_pockets_emptied` is written to g26_meta.

The counts are printed before and after. With the key already there it
changes nothing (a second run would empty pockets players filled since);
--dry-run does it all and rolls back. Python's standard library only (the
live host has python3, no sqlite3 command).
"""

import sqlite3
import sys

RGD5 = 5  # rgd-5's dict_id: items_dict of the 0.26g game data (revision 49's)
POCKETS = (13, 18)  # profile slots 13..18, loadouts::POCKET_SLOTS
MARK = "rgd5_pockets_emptied"

# What the reset reads and writes, as srp's schema has it.
COLUMNS = {
    "g26_kits": {"account_id", "id", "revision"},
    "g26_kit_slots": {"account_id", "kit_id", "slot", "item_id", "count"},
    "g26_items": {"account_id", "item_id", "dict_id", "condition_or_stack"},
    "g26_preset_slots": {"account_id", "preset", "kit", "slot", "dict_id", "count"},
    "g26_meta": {"key", "value"},
}

KIT_POCKETS = """
    FROM g26_kit_slots s JOIN g26_items i
      ON i.account_id = s.account_id AND i.item_id = s.item_id
    WHERE i.dict_id = :rgd5 AND s.slot BETWEEN :first AND :last"""

PRESET_POCKETS = """
    FROM g26_preset_slots
    WHERE dict_id = :rgd5 AND slot BETWEEN :first AND :last"""

ARGS = {"rgd5": RGD5, "first": POCKETS[0], "last": POCKETS[1]}


def counts(db):
    def one(sql):
        return db.execute(sql, ARGS).fetchone()

    kit = one(
        "SELECT COUNT(*), COALESCE(SUM(s.count), 0),"
        " COUNT(DISTINCT s.account_id || '/' || s.kit_id), COUNT(DISTINCT s.account_id)"
        + KIT_POCKETS
    )
    preset = one(
        "SELECT COUNT(*), COALESCE(SUM(count), 0),"
        " COUNT(DISTINCT account_id || '/' || preset), COUNT(DISTINCT account_id)"
        + PRESET_POCKETS
    )
    accounts = one(
        "SELECT COUNT(*) FROM (SELECT s.account_id" + KIT_POCKETS
        + " UNION SELECT account_id" + PRESET_POCKETS + ")"
    )[0]
    owned = one(
        "SELECT COUNT(*), COALESCE(SUM(condition_or_stack), 0) FROM g26_items"
        " WHERE dict_id = :rgd5"
    )
    revisions = one("SELECT COALESCE(SUM(revision), 0) FROM g26_kits")[0]
    return {
        "kit pockets": kit[0], "kit pocket units": kit[1], "kits": kit[2],
        "kit accounts": kit[3],
        "preset pockets": preset[0], "preset pocket units": preset[1],
        "presets": preset[2], "preset accounts": preset[3],
        "accounts touched": accounts,
        "RGD-5 stacks owned": owned[0], "RGD-5 units owned": owned[1],
        "sum of kit revisions": revisions,
    }


def show(title, values):
    print(title)
    for key, value in values.items():
        print(f"  {key:22} {value}")


def main(argv):
    dry = "--dry-run" in argv
    paths = [a for a in argv if a != "--dry-run"]
    if len(paths) != 1 or paths[0].startswith("-"):
        print(__doc__.split("\n\n")[2], file=sys.stderr)
        return 2
    db = sqlite3.connect(f"file:{paths[0]}?mode=rw", uri=True, isolation_level=None)
    for table, wanted in COLUMNS.items():
        have = {row[1] for row in db.execute(f"PRAGMA table_info({table})")}
        if not wanted <= have:
            lacks = sorted(wanted - have)
            print(f"{paths[0]}: {table} lacks {lacks}: not srp's", file=sys.stderr)
            return 1
    # The write lock first: a running srp holding it makes this fail (in 5 s).
    db.execute("BEGIN IMMEDIATE")
    try:
        before = counts(db)
        show(f"{paths[0]}, before:", before)
        if db.execute("SELECT 1 FROM g26_meta WHERE key = ?", (MARK,)).fetchone():
            print(f"g26_meta has {MARK}: the reset was done already, nothing changed")
            db.execute("ROLLBACK")
            return 0
        kits = db.execute(
            "UPDATE g26_kits SET revision = (revision + 1) % 4294967296"
            " WHERE (account_id, id) IN"
            " (SELECT s.account_id, s.kit_id" + KIT_POCKETS + ")",
            ARGS,
        ).rowcount
        kit_slots = db.execute(
            "DELETE FROM g26_kit_slots WHERE slot BETWEEN :first AND :last AND EXISTS ("
            " SELECT 1 FROM g26_items i WHERE i.account_id = g26_kit_slots.account_id"
            " AND i.item_id = g26_kit_slots.item_id AND i.dict_id = :rgd5)",
            ARGS,
        ).rowcount
        preset_slots = db.execute("DELETE" + PRESET_POCKETS, ARGS).rowcount
        db.execute("INSERT INTO g26_meta (key, value) VALUES (?, 1)", (MARK,))
        after = counts(db)
        show("after:", after)
        print(f"changed: {kits} kit revision(s) raised, {kit_slots} kit pocket(s) and"
              f" {preset_slots} preset pocket(s) emptied, {MARK} written")
        problems = []
        if (kits, kit_slots, preset_slots) != (
            before["kits"], before["kit pockets"], before["preset pockets"]
        ):
            problems.append("the rows changed are not the rows counted")
        if after["kit pockets"] or after["preset pockets"]:
            problems.append("RGD-5 pockets are left")
        for key in ("RGD-5 stacks owned", "RGD-5 units owned"):
            if after[key] != before[key]:
                problems.append(f"{key} changed")
        if problems:
            print("NOT DONE, rolled back: " + "; ".join(problems), file=sys.stderr)
            db.execute("ROLLBACK")
            return 1
        if dry:
            db.execute("ROLLBACK")
            print("--dry-run: rolled back, nothing changed")
            return 0
        db.execute("COMMIT")
        print("committed")
        return 0
    except BaseException:
        if db.in_transaction:
            db.execute("ROLLBACK")
        raise
    finally:
        db.close()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

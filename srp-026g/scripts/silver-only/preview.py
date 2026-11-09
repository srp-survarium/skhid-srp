#!/usr/bin/env python3
"""What the release's one-off pay-back will do to an srp.db; changes nothing.

Gold and the vouchers but the drop card are gone (the owner, 2026-11-08). At
the first start of the release that removed them, srp pays them back once
(`SqliteGameRepository::pay_back_once`, crates/session/src/v026g/persist.rs),
after its own copy of srp.db and before the accounts load:

- a gold balance is silver at 100 a gold, the gold column 0;
- each repair kit, rename voucher, one-day premium voucher is its shop price
  in silver (5,000, 25,000, 5,000) and is taken away.

This prints the same for a copy of the file, per account, and the totals.

    python3 preview.py <srp.db>
"""

import sqlite3
import sys

# dict id -> (name, silver each): the shop price, gold price x 100 (items.tsv).
VOUCHERS = {151: ("repair kit", 5_000), 152: ("rename voucher", 25_000), 213: ("premium day", 5_000)}
GOLD_IN_SILVER = 100
MOST = 4_294_967_295  # what a balance holds (a u32)


def main(path: str) -> int:
    db = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    if db.execute("SELECT 1 FROM g26_meta WHERE key = 'silver_only_paid_back'").fetchone():
        print(f"{path}: already paid back; the release changes nothing")
        return 0
    held = ", ".join(
        f"COALESCE((SELECT SUM(condition_or_stack) FROM g26_items i "
        f"WHERE i.account_id = a.account_id AND i.dict_id = {d}), 0)"
        for d in VOUCHERS
    )
    rows = db.execute(
        "SELECT a.account_id, a.name, a.generic_money, a.premium_money, "
        f"{held} FROM g26_accounts a ORDER BY a.account_id"
    ).fetchall()
    touched = gold_total = silver_total = 0
    counts = dict.fromkeys(VOUCHERS, 0)
    for account_id, name, silver, gold, *held_counts in rows:
        if not (gold or any(held_counts)):
            continue
        given = gold * GOLD_IN_SILVER + sum(VOUCHERS[d][1] * n for d, n in zip(VOUCHERS, held_counts))
        touched += 1
        gold_total += gold
        silver_total += given
        for d, n in zip(VOUCHERS, held_counts):
            counts[d] += n
        what = [f"{gold} gold"] if gold else []
        what += [f"{n} {VOUCHERS[d][0]}" for d, n in zip(VOUCHERS, held_counts) if n]
        print(f"{account_id}\t{name}\t{', '.join(what)}\t+{given} silver\t{silver} -> {min(silver + given, MOST)}")
    vouchers = ", ".join(f"{counts[d]} {VOUCHERS[d][0]}" for d in VOUCHERS)
    print(
        f"{touched} accounts, {gold_total} gold, {vouchers}: {silver_total} silver given"
    )
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1]))

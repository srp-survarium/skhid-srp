#!/usr/bin/env python3
"""What the release's one-off change of the golden weapons will do to an srp.db; changes nothing.

There are no golden weapons (the owner, 2026-11-14: "those all should be normal weapons", "and follow
standard rules"; docs/notes/golden_weapons.md). At the first start of the release that removed them,
srp makes each one owned its plain weapon, once
(`SqliteGameRepository::make_golden_weapons_plain_once`, crates/session/src/v026g/persist.rs), after
its own copy of srp.db and before the accounts load: the item's dict id (and a preset slot's) is the
plain weapon's; its item id, modifications (a silver bonus 1230 among them), upgrades and kit slots
stay.

This prints the same for a copy of the file: each weapon, per account, and the totals.

    python3 preview.py <srp.db>
"""

import sqlite3
import sys

# golden dict id: plain dict id (persist::GOLDEN_WEAPONS; a test keeps them the same).
GOLDEN_WEAPONS = {
    332: 88,  # kiparis_premium -> kiparis
    333: 55,  # uzi_premium -> uzi
    334: 19,  # vityaz_premium -> vityaz
    335: 308,  # vss_premium -> vss
    336: 309,  # sr3m_premium -> sr3m
    337: 108,  # izh_27_lancaster_premium -> izh_27_lancaster
    338: 306,  # ak_74n_premium -> ak_74n
    2222: 2221,  # wpn_mp7_silencer_special -> mp7_silencer
    2226: 2225,  # vepr_12_premium -> vepr_12
    2228: 2227,  # saiga_12_premium -> saiga_12
    2231: 2230,  # neostead -> neostead_2
    2241: 80,  # akmn_premium -> akmn
    2242: 80,  # akmn_premium_2 -> akmn
    2243: 13,  # ak_74u_premium -> ak_74u
    2244: 216,  # vepr_premium -> vepr
    2245: 363,  # ak_12_premium -> ak_12
    2246: 109,  # sks_premium -> sks
    2247: 109,  # sks_premium_2 -> sks
    2248: 305,  # vsk_94_premium -> vsk_94
    2249: 14,  # rem_700_premium -> rem_700
    2251: 108,  # izh_27_lancaster_premium_2 -> izh_27_lancaster
    2252: 55,  # uzi_premium_2 -> uzi
}


def main(path: str) -> int:
    db = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    if db.execute("SELECT 1 FROM g26_meta WHERE key = 'golden_weapons_made_plain'").fetchone():
        print(f"{path}: the golden weapons were made plain already; the release changes nothing")
        return 0
    ids = ", ".join(str(golden) for golden in GOLDEN_WEAPONS)
    rows = db.execute(
        "SELECT a.account_id, a.name, i.item_id, i.dict_id, i.modifications, i.upgrades "
        "FROM g26_items i JOIN g26_accounts a ON a.account_id = i.account_id "
        f"WHERE i.dict_id IN ({ids}) ORDER BY a.account_id, i.item_id"
    ).fetchall()
    accounts = set()
    for account_id, name, item_id, dict_id, modifications, upgrades in rows:
        accounts.add(account_id)
        upgraded = f"\tupgrades {upgrades}" if upgrades else ""
        print(
            f"{account_id}\t{name}\titem {item_id}\t{dict_id} -> {GOLDEN_WEAPONS[dict_id]}"
            f"\tmodifications {modifications or '-'}{upgraded}"
        )
    presets = db.execute(f"SELECT COUNT(*) FROM g26_preset_slots WHERE dict_id IN ({ids})").fetchone()[0]
    print(f"{len(rows)} golden weapons of {len(accounts)} accounts made plain; {presets} preset slots")
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1]))

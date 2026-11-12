#!/usr/bin/env python3
"""Plan duplicate-nickname repairs; apply a reviewed plan only with srp stopped.

Plans contain display names and ids, never credentials. Login names, account ids,
progress, friends and inventories are preserved. Unicode case and the explicit Cyrillic/Latin lookalikes use the same
rules as the server (nickname-lookalikes.tsv).
"""
import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import sqlite3

RULES = dict(line.split("\t") for line in Path(__file__).with_name("nickname-lookalikes.tsv").read_text().splitlines() if line and not line.startswith("#"))
LOOKALIKES = str.maketrans(RULES)


def key(name):
    return name.lower().translate(LOOKALIKES)


def compare_names(a, b):
    return (key(a) > key(b)) - (key(a) < key(b))


def connect(path, mode="ro"):
    con = sqlite3.connect(Path(path).resolve().as_uri() + f"?mode={mode}", uri=True)
    con.create_collation("skhid_nickname", compare_names)
    return closing(con)


def rows(con):
    has_game = con.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='g26_accounts'").fetchone()
    if has_game:
        return con.execute("SELECT a.id, a.login, a.name, a.game_name, g.name FROM accounts a LEFT JOIN g26_accounts g ON g.account_id=a.id ORDER BY a.id").fetchall()
    return con.execute("SELECT id, login, name, game_name, NULL FROM accounts ORDER BY id").fetchall()


def fingerprint(accounts):
    # Include all names and logins so a new account or rename invalidates the
    # reviewed plan, without including login strings in the output.
    return hashlib.sha256(json.dumps(accounts, ensure_ascii=False).encode()).hexdigest()


def plan(con, keep=(), only=()):
    accounts = rows(con)
    groups = {}
    occupied = set()
    for account_id, login, name, _, _ in accounts:
        groups.setdefault(key(name), []).append((account_id, name))
        occupied.update((key(login), key(name)))
    keep = set(keep)
    used_overrides = set()
    changes = []
    for group in groups.values():
        if len(group) < 2:
            continue
        chosen = [account_id for account_id, _ in group if account_id in keep]
        if len(chosen) > 1:
            raise ValueError("only one keeper is allowed per duplicate group")
        winner = chosen[0] if chosen else group[0][0]
        used_overrides.update(chosen)
        for account_id, name in group:
            if account_id == winner:
                continue
            attempt = 0
            while True:
                suffix = f"_{account_id}" + (f"_{attempt}" if attempt else "")
                prefix = name.encode()[:63-len(suffix)].decode("utf-8", errors="ignore")
                replacement = prefix + suffix
                if key(replacement) not in occupied:
                    break
                attempt += 1
            occupied.add(key(replacement))
            changes.append({"id": account_id, "from": name, "to": replacement, "kept_id": winner})
    if keep != used_overrides:
        raise ValueError("keeper override does not identify an account in a duplicate group")
    only = set(only)
    if only and not only.issubset({change["id"] for change in changes}):
        raise ValueError("selected account is not a duplicate requiring a rename")
    if only:
        changes = [change for change in changes if change["id"] in only]
    return {"format": 1, "namespace_sha256": fingerprint(accounts), "keep": sorted(keep), "only": sorted(only), "changes": changes}


def apply(path, reviewed, backup):
    with connect(path, "rw") as con:
        con.execute("BEGIN IMMEDIATE")
        expected = plan(con, reviewed["keep"], reviewed["only"])
        if reviewed != expected:
            raise ValueError("nickname plan is stale or edited; audit and review again")
        if not reviewed["changes"]:
            return
        # A second read connection can back up the committed database while
        # this connection's write lock excludes other writers. Never overwrite.
        fd = os.open(backup, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        os.close(fd)
        with connect(path) as source, closing(sqlite3.connect(backup)) as target:
            source.backup(target)
            target.create_collation("skhid_nickname", compare_names)
            if target.execute("PRAGMA integrity_check").fetchone() != ("ok",):
                raise ValueError("backup integrity check failed")
        has_game = con.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='g26_accounts'").fetchone()
        for change in reviewed["changes"]:
            changed = con.execute("UPDATE accounts SET name=?, game_name=? WHERE id=? AND name=?",
                                  (change["to"], change["to"], change["id"], change["from"]))
            if changed.rowcount != 1:
                raise ValueError("account changed while applying the plan")
            if has_game:
                con.execute("UPDATE g26_accounts SET name=? WHERE account_id=?", (change["to"], change["id"]))
        remaining = plan(con)["changes"]
        if remaining and not reviewed["only"]:
            raise ValueError("duplicate nicknames remain")
        # The new server's migration installs its collation-backed index.
        # Do not install it for an older server that lacks that collation.
        if con.execute("PRAGMA foreign_key_check").fetchall():
            raise ValueError("foreign key check failed")
        con.commit()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("database", type=Path)
    parser.add_argument("--keep", type=int, action="append", default=[], help="account id that keeps a duplicate name (default: oldest id)")
    parser.add_argument("--only", type=int, action="append", default=[], help="repair only these losing account ids")
    parser.add_argument("--apply-plan", type=Path)
    parser.add_argument("--backup", type=Path)
    parser.add_argument("--server-stopped", action="store_true", help="operator has verified srp is stopped; never edit its database while running")
    args = parser.parse_args()
    if args.apply_plan:
        if not args.server_stopped or not args.backup:
            parser.error("applying requires --server-stopped and a new --backup path")
        if args.keep or args.only:
            parser.error("keeper choices must come from the reviewed plan")
        reviewed = json.loads(args.apply_plan.read_text())
        apply(args.database, reviewed, args.backup)
        print(json.dumps({"applied": reviewed["changes"], "backup": str(args.backup)}, ensure_ascii=False, indent=2))
    else:
        with connect(args.database) as con:
            print(json.dumps(plan(con, args.keep, args.only), ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()

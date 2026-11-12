import copy
from contextlib import closing, contextmanager
import importlib.util
from pathlib import Path
import sqlite3
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("repair", Path(__file__).with_name("repair-nicknames.py"))
repair = importlib.util.module_from_spec(spec)
spec.loader.exec_module(repair)


@contextmanager
def writable(path):
    with closing(sqlite3.connect(path)) as con, con:
        con.create_collation("skhid_nickname", repair.compare_names)
        yield con


class RepairTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.db = Path(self.temp.name) / "srp.db"
        self.backup = Path(self.temp.name) / "before.db"
        with writable(self.db) as con:
            con.executescript("""
                CREATE TABLE accounts(id INTEGER PRIMARY KEY, login TEXT UNIQUE, name TEXT, game_name TEXT, password_hash TEXT, money INTEGER);
                CREATE TABLE g26_accounts(account_id INTEGER PRIMARY KEY, name TEXT, progress TEXT);
                CREATE TABLE friends(account_id INTEGER, friend_id INTEGER);
                INSERT INTO accounts VALUES(777,'old-login','Mamoru','Old','secret-1',123),(778,'mamoru','Mamoru','Mamoru','secret-2',456),
                    (1,'another-login','Alice','Alice','secret-3',789),(2,'alice','ALICE','ALICE','secret-4',10);
                INSERT INTO g26_accounts VALUES(777,'Mamoru','progress1'),(778,'Mamoru','progress2'),(1,'Alice','progress3'),(2,'ALICE','progress4');
                INSERT INTO friends VALUES(778,760),(760,778);
            """)

    def current_plan(self, **kwargs):
        with repair.connect(self.db) as con:
            return repair.plan(con, **kwargs)

    def test_targeted_repair_preserves_newest_mamoru_and_all_other_data(self):
        reviewed = self.current_plan(keep=[778], only=[777])
        self.assertEqual(reviewed['changes'], [{'id': 777, 'from': 'Mamoru', 'to': 'Mamoru_777', 'kept_id': 778}])
        repair.apply(self.db, reviewed, self.backup)
        with repair.connect(self.db) as con, repair.connect(self.backup) as before:
            self.assertEqual(con.execute('SELECT id,login,password_hash,money FROM accounts ORDER BY id').fetchall(), before.execute('SELECT id,login,password_hash,money FROM accounts ORDER BY id').fetchall())
            self.assertEqual(con.execute('SELECT * FROM friends').fetchall(), before.execute('SELECT * FROM friends').fetchall())
            self.assertEqual(con.execute('SELECT account_id,progress FROM g26_accounts ORDER BY account_id').fetchall(), before.execute('SELECT account_id,progress FROM g26_accounts ORDER BY account_id').fetchall())
            self.assertEqual(con.execute('SELECT name,game_name FROM accounts WHERE id=777').fetchone(), ('Mamoru_777','Mamoru_777'))
            self.assertEqual(con.execute('SELECT name FROM g26_accounts WHERE account_id=777').fetchone(), ('Mamoru_777',))
            self.assertEqual(con.execute('SELECT name FROM accounts WHERE id=778').fetchone(), ('Mamoru',))
            self.assertEqual(before.execute('SELECT name FROM accounts WHERE id=777').fetchone(), ('Mamoru',))
            self.assertEqual(len(repair.plan(con)['changes']), 1)

    def test_full_repair_enforces_unique_names(self):
        reviewed = self.current_plan(keep=[778])
        repair.apply(self.db, reviewed, self.backup)
        self.assertEqual(self.current_plan()['changes'], [])
        with writable(self.db) as con:
            con.execute("CREATE UNIQUE INDEX accounts_unique_nickname ON accounts(name COLLATE skhid_nickname)")
            with self.assertRaises(sqlite3.IntegrityError):
                con.execute("UPDATE accounts SET name='mAMОRU' WHERE id=777")

    def test_stale_or_edited_plan_never_writes(self):
        reviewed = self.current_plan(keep=[778])
        edited = copy.deepcopy(reviewed)
        edited['changes'][0]['to'] = 'other'
        with self.assertRaises(ValueError):
            repair.apply(self.db, edited, self.backup)
        with writable(self.db) as con:
            con.execute("UPDATE accounts SET name='NewName' WHERE id=2")
        with self.assertRaises(ValueError):
            repair.apply(self.db, reviewed, self.backup)
        self.assertFalse(self.backup.exists())

    def test_failure_rolls_back_every_name_and_preserves_backup(self):
        reviewed = self.current_plan(keep=[778])
        with writable(self.db) as con:
            con.executescript("CREATE TRIGGER stop_repair BEFORE UPDATE ON g26_accounts BEGIN SELECT RAISE(ABORT, 'fail'); END;")
        with self.assertRaises(sqlite3.IntegrityError):
            repair.apply(self.db, reviewed, self.backup)
        self.assertEqual(self.current_plan(keep=[778]), reviewed)
        self.assertTrue(self.backup.exists())

    def test_suffix_avoids_existing_logins_and_fits_utf8_wire_limit(self):
        long_name = 'Ж' * 31
        with writable(self.db) as con:
            con.execute("UPDATE accounts SET name=? WHERE id IN (1,2)", (long_name,))
            con.execute("UPDATE accounts SET login='mamoru_777' WHERE id=1")
        reviewed = self.current_plan(keep=[778])
        changes = {row['id']: row['to'] for row in reviewed['changes']}
        self.assertEqual(changes[777], 'Mamoru_777_1')
        self.assertLessEqual(len(changes[2].encode()), 63)
        self.assertTrue(changes[2].endswith('_2'))

    def test_cyrillic_lookalikes_use_the_same_explicit_rules_as_the_server(self):
        for source, target in repair.RULES.items():
            self.assertEqual(repair.key(source), repair.key(target))
            self.assertEqual(repair.key(source.upper()), repair.key(target))
        with writable(self.db) as con:
            con.execute("UPDATE accounts SET name='sovspiranha' WHERE id=1")
            con.execute("UPDATE accounts SET name='sоvspiranha' WHERE id=2")
        reviewed = self.current_plan(keep=[778])
        self.assertIn({'id': 2, 'from': 'sоvspiranha', 'to': 'sоvspiranha_2', 'kept_id': 1}, reviewed['changes'])
        repair.apply(self.db, reviewed, self.backup)
        self.assertEqual(self.current_plan()['changes'], [])


if __name__ == '__main__':
    unittest.main()

"""apply.py over a small srp.db and bundle: the dry run changes nothing, --apply adds the matches under
numbers of their own with their deltas once, --undo gives back the file as it was."""
import contextlib
import hashlib
import io
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).parent))
import apply  # noqa: E402

# The tables apply.py reads, with the columns of srp's (their keys and a few counters: it takes every
# column it finds).
SCHEMA = """
CREATE TABLE g26_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
CREATE TABLE g26_accounts (account_id INTEGER PRIMARY KEY, name TEXT NOT NULL, rating INTEGER NOT NULL);
CREATE TABLE g26_matches (id INTEGER PRIMARY KEY, replay_id TEXT NOT NULL UNIQUE, ended_at INTEGER NOT NULL,
    game_mode INTEGER NOT NULL, kind TEXT NOT NULL, eligibility TEXT, source TEXT NOT NULL DEFAULT 'live');
CREATE TABLE g26_match_players (match_id INTEGER NOT NULL REFERENCES g26_matches(id), account_id INTEGER NOT NULL,
    clan_id INTEGER, rating INTEGER, kills INTEGER NOT NULL, ended_at INTEGER NOT NULL,
    PRIMARY KEY (match_id, account_id));
CREATE TABLE g26_match_kits (match_id INTEGER NOT NULL, account_id INTEGER NOT NULL, kit INTEGER NOT NULL,
    kills INTEGER NOT NULL, PRIMARY KEY (match_id, account_id, kit));
CREATE TABLE g26_match_kit_items (match_id INTEGER NOT NULL, account_id INTEGER NOT NULL, kit INTEGER NOT NULL,
    slot INTEGER NOT NULL, dict_id INTEGER NOT NULL, PRIMARY KEY (match_id, account_id, kit, slot));
CREATE TABLE g26_match_weapons (match_id INTEGER NOT NULL, account_id INTEGER NOT NULL, dict_id INTEGER NOT NULL,
    kills INTEGER NOT NULL, longest_kill_dm INTEGER NOT NULL, PRIMARY KEY (match_id, account_id, dict_id));
CREATE TABLE g26_player_totals (account_id INTEGER PRIMARY KEY, matches INTEGER NOT NULL, wins INTEGER NOT NULL,
    kills INTEGER NOT NULL, deaths INTEGER NOT NULL, assists INTEGER NOT NULL, shots INTEGER NOT NULL,
    hits INTEGER NOT NULL, played_ms INTEGER NOT NULL);
CREATE TABLE g26_clan_totals (clan_id INTEGER PRIMARY KEY, matches INTEGER NOT NULL, kills INTEGER NOT NULL);
CREATE TABLE g26_player_items (account_id INTEGER NOT NULL, dict_id INTEGER NOT NULL, gear TEXT NOT NULL,
    matches INTEGER NOT NULL, PRIMARY KEY (account_id, dict_id));
CREATE TABLE g26_player_weapons (account_id INTEGER NOT NULL, dict_id INTEGER NOT NULL, matches INTEGER NOT NULL,
    kills INTEGER NOT NULL, longest_kill_dm INTEGER NOT NULL, PRIMARY KEY (account_id, dict_id));
"""


def make_db(path):
    with contextlib.closing(sqlite3.connect(path)) as c:
        c.executescript(SCHEMA)
        c.execute("INSERT INTO g26_meta VALUES ('schema', 27)")
        c.executemany('INSERT INTO g26_accounts VALUES (?, ?, ?)', [(10, 'Alice', 1100), (11, 'Bob', 900)])
        # srp's own match: number 1, ended at 5000.
        c.execute("INSERT INTO g26_matches VALUES (1, 'rec_live', 5000, 2, 'public', 'counted', 'live')")
        c.execute('INSERT INTO g26_match_players VALUES (1, 10, 7, 1100, 4, 5000)')
        c.execute('INSERT INTO g26_player_totals VALUES (10, 1, 1, 4, 2, 1, 50, 20, 600000)')
        c.execute('INSERT INTO g26_clan_totals VALUES (7, 1, 4)')
        c.execute("INSERT INTO g26_player_items VALUES (10, 155, 'weapon', 1)")
        c.commit()


def make_bundle(path):
    with contextlib.closing(sqlite3.connect(path)) as c:
        c.executescript("""
        CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE matches (id INTEGER, replay_id TEXT, ended_at INTEGER, game_mode INTEGER, kind TEXT,
            eligibility TEXT, source TEXT);
        CREATE TABLE match_players (match_id, account_id, clan_id, rating, kills, ended_at);
        CREATE TABLE match_kits (match_id, account_id, kit, kills);
        CREATE TABLE match_kit_items (match_id, account_id, kit, slot, dict_id);
        CREATE TABLE match_weapons (match_id, account_id, dict_id, kills, longest_kill_dm);
        CREATE TABLE delta_g26_player_totals (account_id, matches, wins, kills, deaths, assists, shots, hits,
            played_ms, new);
        CREATE TABLE delta_g26_clan_totals (clan_id, matches, kills, new);
        CREATE TABLE delta_g26_player_items (account_id, dict_id, gear, matches, new);
        CREATE TABLE delta_g26_player_weapons (account_id, dict_id, matches, kills, longest_kill_dm, new);
        """)
        # Two matches of before (numbers of the copy they were made in), Alice in both, Bob in one.
        c.executemany('INSERT INTO matches VALUES (?, ?, ?, 2, ?, ?, ?)', [
            (40, 'rec_old_b', 2000, 'public', 'counted', 'recording'),
            (39, 'rec_old_a', 1000, 'public', 'short', 'recording')])
        c.executemany('INSERT INTO match_players VALUES (?, ?, ?, ?, ?, ?)', [
            (40, 10, 7, None, 6, 2000), (40, 11, None, 905, 3, 2000), (39, 10, None, None, 1, 1000)])
        c.executemany('INSERT INTO match_kits VALUES (?, ?, ?, ?)', [(40, 10, 0, 6), (40, 11, 1, 3)])
        c.executemany('INSERT INTO match_kit_items VALUES (?, ?, ?, ?, ?)', [(40, 10, 0, 7, 155), (40, 11, 1, 7, 160)])
        c.executemany('INSERT INTO delta_g26_player_totals VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)', [
            (10, 1, 1, 6, 1, 0, 70, 30, 700000, 0), (11, 1, 0, 3, 6, 2, 40, 10, 700000, 1)])
        c.execute('INSERT INTO delta_g26_clan_totals VALUES (7, 1, 6, 0)')
        c.executemany('INSERT INTO delta_g26_player_items VALUES (?, ?, ?, ?, ?)', [
            (10, 155, 'weapon', 1, 0), (11, 160, 'weapon', 1, 1)])
        c.commit()


def fingerprint(path):
    with contextlib.closing(sqlite3.connect(f'file:{path}?mode=ro', uri=True)) as c:
        tables = [row[0] for row in c.execute("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")]
        return {t: hashlib.sha256(repr(sorted(c.execute(f'SELECT * FROM {t}'))).encode()).hexdigest() for t in tables}


def run(*args):
    out = io.StringIO()
    with mock.patch.object(sys, 'argv', ['apply.py', *map(str, args)]), contextlib.redirect_stdout(out):
        apply.main()
    return out.getvalue()


class ApplyTest(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.dir = Path(self.folder.name)
        self.db, self.bundle = self.dir / 'srp.db', self.dir / 'bundle.db'
        make_db(self.db)
        make_bundle(self.bundle)

    def tearDown(self):
        self.folder.cleanup()

    def test_the_dry_run_changes_nothing_and_says_what_would_change(self):
        before = fingerprint(self.db)
        out = run(self.bundle, self.db)
        self.assertEqual(fingerprint(self.db), before)
        self.assertIn('2 matches would be added', out)
        self.assertIn('counted matches\t1 -> 2', out)
        self.assertIn('players\t1 -> 2', out)

    def test_apply_once_then_undo_gives_the_file_back(self):
        before = fingerprint(self.db)
        self.assertIn('added: 2 matches', run('--apply', self.bundle, self.db))
        with contextlib.closing(sqlite3.connect(self.db)) as c:
            # Under numbers of their own, in the order they ended; srp's keeps its number.
            self.assertEqual(c.execute('SELECT id, replay_id FROM g26_matches ORDER BY id').fetchall(),
                             [(1, 'rec_live'), (2, 'rec_old_a'), (3, 'rec_old_b')])
            self.assertEqual(c.execute('SELECT match_id, account_id, rating FROM g26_match_players '
                                       'ORDER BY match_id, account_id').fetchall(),
                             [(1, 10, 1100), (2, 10, None), (3, 10, None), (3, 11, 905)])
            self.assertEqual(c.execute('SELECT * FROM g26_player_totals ORDER BY account_id').fetchall(),
                             [(10, 2, 2, 10, 3, 1, 120, 50, 1300000), (11, 1, 0, 3, 6, 2, 40, 10, 700000)])
            self.assertEqual(c.execute('SELECT * FROM g26_clan_totals').fetchall(), [(7, 2, 10)])
            self.assertEqual(c.execute('SELECT account_id, dict_id, matches FROM g26_player_items '
                                       'ORDER BY account_id').fetchall(), [(10, 155, 2), (11, 160, 1)])
            # The accounts and their ratings are not touched.
            self.assertEqual(c.execute('SELECT * FROM g26_accounts').fetchall(), [(10, 'Alice', 1100), (11, 'Bob', 900)])
        applied = fingerprint(self.db)
        self.assertIn('nothing added', run('--apply', self.bundle, self.db))
        self.assertEqual(fingerprint(self.db), applied)
        self.assertIn('taken out: 2 matches', run('--undo', self.bundle, self.db))
        self.assertEqual(fingerprint(self.db), before)
        self.assertEqual(len(list((self.dir / 'backups').iterdir())), 3)

    def test_a_file_before_the_step_is_refused(self):
        with contextlib.closing(sqlite3.connect(self.db)) as c:
            c.execute("UPDATE g26_meta SET value = 25 WHERE key = 'schema'")
            c.commit()
        with self.assertRaises(SystemExit):
            run(self.bundle, self.db)

    def test_a_bundle_whose_match_is_there_adds_nothing(self):
        with contextlib.closing(sqlite3.connect(self.db)) as c:
            c.execute("INSERT INTO g26_matches VALUES (2, 'rec_old_a', 1000, 2, 'public', 'short', 'live')")
            c.commit()
        before = fingerprint(self.db)
        with self.assertRaises(SystemExit):
            run('--apply', self.bundle, self.db)
        after = fingerprint(self.db)
        self.assertEqual({t: h for t, h in after.items()}, before)


if __name__ == '__main__':
    unittest.main()

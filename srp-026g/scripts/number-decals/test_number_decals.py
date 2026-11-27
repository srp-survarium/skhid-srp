"""The number decals' checks (docs/notes/number-decals.md): the rows names.py makes are in the
files, the ids and names, the grant's mapping, its dry run and that it gives once.

python3 -m unittest test_number_decals            (from this folder; the standard library only)
NUMBER_DECALS_PACK=<packs>/number-decals-1 python3 -m unittest test_number_decals
                                                  (also the built pack: every number, give-only)
"""
import contextlib
import io
import json
import os
from pathlib import Path
import re
import sqlite3
import struct
import sys
import tempfile
import unittest
import unittest.mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as ce  # noqa: E402
import grant  # noqa: E402
import names  # noqa: E402

LANGUAGES = sorted(names.FORMS)
RUST = HERE.parents[1] / 'crates' / 'session' / 'src' / 'v026g' / 'number_decals.rs'


class Rows(unittest.TestCase):
    def test_the_files_hold_the_rows_names_py_makes(self):
        for path, lines in names.planned().items():
            with self.subTest(path=path.name):
                self.assertEqual(path.read_text(encoding='utf-8'), '\n'.join(lines) + '\n',
                                 'run python3 -I names.py --write')

    def test_every_language_names_every_number(self):
        files = sorted(p.stem for p in names.LOCALIZATION.glob('*.tsv'))
        self.assertEqual(files, LANGUAGES)
        for language in LANGUAGES:
            table = dict(line.split('\t', 1) for line in
                         (names.LOCALIZATION / f'{language}.tsv').read_text(encoding='utf-8').splitlines()[1:])
            ours = {key: text for key, text in table.items() if key.startswith('st_character_decals_num_')}
            self.assertEqual(sorted(ours), [names.name_key(n) for n in names.NUMBERS], language)
            for n in (1, 42, 1000):
                self.assertIn(str(n), ours[names.name_key(n)])
        self.assertEqual(names.name('english', 42), 'Number 42')
        self.assertEqual(names.name('russian', 42), 'Номер 42')

    def test_hungarian_endings(self):
        want = {1: '1-es', 2: '2-es', 3: '3-as', 5: '5-ös', 6: '6-os', 8: '8-as', 10: '10-es', 20: '20-as',
                30: '30-as', 40: '40-es', 60: '60-as', 80: '80-as', 100: '100-as', 101: '101-es',
                500: '500-as', 1000: '1000-es'}
        for n, form in want.items():
            self.assertEqual(names.name('hungarian', n), f'{form} szám')

    def test_one_contiguous_block_of_ids(self):
        ids = [names.dict_id(n) for n in names.NUMBERS]
        self.assertEqual(ids, list(range(5001, 6001)))
        items = (names.ITEMS_TSV).read_text(encoding='utf-8').splitlines()
        taken = [int(line.split('\t', 1)[0]) for line in items if line[:1].isdigit()]
        self.assertEqual(len(taken), len(set(taken)), 'a dictionary id twice')
        others = [i for i in taken if i not in set(ids)]
        self.assertLess(max(others), 5001)
        self.assertEqual([names.rc(n) for n in (1, 42, 1000)], ['num_0001', 'num_0042', 'num_1000'])
        with self.assertRaises(ValueError):
            names.rc(1001)

    def test_given_never_sold(self):
        """items.tsv: no price, premium (the shops' premium list leaves out what shop.tsv says is
        not sold, as Snufkin's); shop.tsv: not sold, each."""
        rows = {line.split('\t')[1]: line.split('\t') for line in
                names.ITEMS_TSV.read_text(encoding='utf-8').splitlines() if line[:1].isdigit()}
        shop = {line.split('\t')[0] for line in names.SHOP_TSV.read_text(encoding='utf-8').splitlines()}
        for n in names.NUMBERS:
            row = rows[names.item_name(n)]
            self.assertEqual(row[2:4], ['0', 'true'])
            self.assertIn(names.item_name(n) + '/not_sold', shop)

    def test_the_script_and_srp_agree_with_names_py(self):
        self.assertEqual(grant.FIRST_DICT_ID, names.FIRST_DICT_ID)
        self.assertEqual(grant.NUMBERS, names.NUMBERS)
        rust = RUST.read_text(encoding='utf-8')
        left_out = re.search(r'pub const LEFT_OUT: \[u32; 2\] = \[(\d+), (\d+)\];', rust)
        self.assertEqual({int(left_out[1]), int(left_out[2])}, set(grant.LEFT_OUT))
        self.assertIn('format!("decals_num_{:04}", self.0)', rust)
        self.assertIn('pub const LAST: u16 = 1000;', rust)


SCHEMA = """
CREATE TABLE accounts (id INTEGER PRIMARY KEY, login TEXT NOT NULL, name TEXT NOT NULL);
CREATE TABLE g26_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
CREATE TABLE g26_accounts (account_id INTEGER PRIMARY KEY, name TEXT NOT NULL, next_item_id INTEGER NOT NULL);
CREATE TABLE g26_items (
    account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
    item_id INTEGER NOT NULL, seq INTEGER NOT NULL, dict_id INTEGER NOT NULL,
    condition_or_stack INTEGER NOT NULL, repair_time INTEGER NOT NULL, reroll_time INTEGER NOT NULL,
    modifications TEXT NOT NULL, upgrades TEXT NOT NULL, reroll TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (account_id, item_id));
"""


class Grant(unittest.TestCase):
    """grant.py on a small srp.db: ids 3 to 7, 42, 590, 1000, 1001; 6 has no 0.26g account, 7
    owns its decal already, 5 is banned (bans do not matter)."""

    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.db = Path(self.folder.name) / 'srp.db'
        c = sqlite3.connect(self.db)
        c.executescript(SCHEMA)
        for account_id in (3, 4, 5, 6, 7, 42, 590, 1000, 1001):
            c.execute('INSERT INTO accounts VALUES (?, ?, ?)', (account_id, f'l{account_id}', f'n{account_id}'))
            if account_id != 6:
                c.execute('INSERT INTO g26_accounts VALUES (?, ?, ?)', (account_id, f'p{account_id}', 3))
                c.execute("INSERT INTO g26_items VALUES (?, 1, 0, 3, 1000000, 0, 0, '', '', '')", (account_id,))
        # 42's next item id is behind its items (it must not be reused); 7 owns its decal.
        c.execute("INSERT INTO g26_items VALUES (42, 9, 4, 7, 30, 0, 0, '', '', '')")
        c.execute("INSERT INTO g26_items VALUES (7, 2, 1, 5007, 1000000, 0, 0, '', '', '')")
        c.commit()
        c.close()

    def tearDown(self):
        self.folder.cleanup()

    def run_grant(self, *args):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), unittest.mock.patch.object(sys, 'argv', ['grant.py', *args]):
            grant.main()
        return out.getvalue().splitlines()

    def decals(self):
        c = sqlite3.connect(self.db)
        rows = c.execute('SELECT account_id, item_id, seq, dict_id, condition_or_stack FROM g26_items '
                         'WHERE dict_id > 5000 ORDER BY account_id').fetchall()
        next_ids = dict(c.execute('SELECT account_id, next_item_id FROM g26_accounts').fetchall())
        c.close()
        return rows, next_ids

    def test_the_dry_run_lists_and_changes_nothing(self):
        before = self.db.read_bytes()
        lines = self.run_grant(str(self.db))
        self.assertEqual(lines[0], 'dry run (--apply changes the file):')
        self.assertEqual(lines[1:-1], [
            '3\t3\tp3\tgive', '4\t4\tp4\tleft out (a test account)', '5\t5\tp5\tgive',
            '6\t6\t-\tat sign-up', '7\t7\tp7\towns', '42\t42\tp42\tgive',
            '590\t590\tp590\tleft out (System)', '1000\t1000\tp1000\tgive'])
        self.assertTrue(lines[-1].startswith('4 decals to give, 1 owned already, 1 at their 0.26g sign-up, '
                                             '2 left out; 992 numbers no account has'))
        self.assertEqual(self.db.read_bytes(), before)
        self.assertFalse(list((self.db.parent / 'backups').glob('*')) if (self.db.parent / 'backups').exists() else [])

    def test_apply_gives_each_number_once(self):
        lines = self.run_grant('--apply', str(self.db))
        self.assertEqual(lines[-1], 'applied')
        self.assertEqual(len(list((self.db.parent / 'backups').glob('srp-*-before-number-decals.db'))), 1)
        rows, next_ids = self.decals()
        self.assertEqual(rows, [
            (3, 3, 1, 5003, 1_000_000), (5, 3, 1, 5005, 1_000_000), (7, 2, 1, 5007, 1_000_000),
            (42, 10, 5, 5042, 1_000_000), (1000, 3, 1, 6000, 1_000_000)])
        self.assertEqual((next_ids[3], next_ids[42], next_ids[4], next_ids[590], next_ids[1001]), (4, 11, 3, 3, 3))
        # Run again: the mark says it was done, nothing more is given (nor in a dry run).
        self.assertEqual(self.run_grant('--apply', str(self.db))[-1],
                         'given before (g26_meta number_decals_given): nothing given')
        self.assertEqual(self.run_grant(str(self.db)),
                         ['given before (g26_meta number_decals_given): --apply gives nothing'])
        self.assertEqual(self.decals(), (rows, next_ids))

    def test_the_copy_holds_the_file_as_it_was(self):
        self.run_grant('--apply', str(self.db))
        copy, = (self.db.parent / 'backups').glob('srp-*-before-number-decals.db')
        c = sqlite3.connect(copy)
        self.assertEqual(c.execute('SELECT COUNT(*) FROM g26_items WHERE dict_id > 5000').fetchone()[0], 1)
        self.assertEqual(c.execute('SELECT COUNT(*) FROM g26_meta').fetchone()[0], 0)
        c.close()


@unittest.skipUnless(os.environ.get('NUMBER_DECALS_PACK'), 'NUMBER_DECALS_PACK names no built pack')
class Pack(unittest.TestCase):
    """The built pack: every number's item, config, textures and names; given only."""

    @classmethod
    def setUpClass(cls):
        cls.root = Path(os.environ['NUMBER_DECALS_PACK'])
        cls.pack = json.loads((cls.root / 'pack.json').read_text(encoding='utf-8'))

    def ops(self, kind):
        return [op for op in self.pack['operations'] if op['op'] == kind]

    def test_every_number_and_nothing_else(self):
        entries, = (op['entries'] for op in self.ops('dictionary_add'))
        self.assertEqual([e['dict_id'] for e in entries], list(range(5001, 6001)))
        self.assertEqual([e['name'] for e in entries], [names.item_name(n) for n in names.NUMBERS])
        for e in entries:
            self.assertEqual((e['base_cost'], e['is_premium'], e['is_stack'], e['item_category']), (0, True, False, 26))
        self.assertEqual([i['dict_id'] for i in self.pack['server']['items']], list(range(5001, 6001)))
        files = {op['path'] for op in self.ops('add_file')}
        want = set()
        for n in names.NUMBERS:
            rc = names.rc(n)
            want.add(f'gameplay/items/upgrades/decals/char_decal_{rc}.options')
            for role in ('', '_fmap', '_nmap', '_smap'):
                want |= {f'textures/character/human/decal/{rc}{role}.dds', f'textures/character/human/decal/{rc}{role}.options'}
        self.assertEqual(files, want)
        for path in files:
            self.assertTrue((self.root / 'files' / path).is_file(), path)

    def test_never_dropped_nor_sold_and_its_own_modification(self):
        modifications = set()
        for n in names.NUMBERS:
            data = bytearray((self.root / 'files' / f'gameplay/items/upgrades/decals/char_decal_{names.rc(n)}.options').read_bytes())
            values = {p: v for p, _, v in ce.walk(data)}
            self.assertEqual(values['/parameters/drop_weight'], 0)
            self.assertEqual(values['/parameters/type'], b'prm_decl')
            self.assertEqual(values['/parameters/modification'], 210000 + n)
            self.assertEqual(values['/modifications[0]/modifiers[0]/value[1][0]'], f'character/human/decal/{names.rc(n)}'.encode())
            self.assertEqual(values['/ui_desc/text_descriptions/name'], names.name_key(n).encode())
            self.assertEqual((values['/ui_desc/icon[0]'], values['/ui_desc/icon[1]']), (1, 25))
            modifications.add(values['/modifications[0]/id'])
        self.assertEqual(len(modifications), 1000)

    def test_names_in_the_packs_languages(self):
        ops = self.ops('localization_add')
        self.assertEqual(sorted(op['languages'][0] for op in ops),
                         ['english', 'german', 'original', 'polish', 'russian', 'ukrainian'])
        for op in ops:
            language = 'russian' if op['languages'][0] == 'original' else op['languages'][0]
            self.assertEqual(set(op['strings']), {names.name_key(n) for n in names.NUMBERS})
            text = op['strings'][names.name_key(42)]
            raw = bytes(0x98 if ch == '\x98' else ch.encode('cp1251')[0] for ch in text)
            self.assertEqual(raw.decode('utf-8'), names.name(language, 42))

    def test_the_textures(self):
        """The diffuse DXT5 256x256 with 7 mips, the normal map DXT5 and specular DXT1 16x16 with 3."""
        def header(path):
            data = (self.root / 'files' / path).read_bytes()
            height, width = struct.unpack_from('<II', data, 12)
            return data[84:88], width, height, struct.unpack_from('<I', data, 28)[0], len(data)
        for n in (1, 500, 1000):
            base = f'textures/character/human/decal/{names.rc(n)}'
            self.assertEqual(header(base + '.dds'), (b'DXT5', 256, 256, 7, 128 + 87376 + 1))
            self.assertEqual(header(base + '_nmap.dds'), (b'DXT5', 16, 16, 3, 128 + 336 + 1))
            self.assertEqual(header(base + '_smap.dds'), (b'DXT1', 16, 16, 3, 128 + 168 + 1))
            self.assertEqual(header(base + '_fmap.dds'), (b'DXT1', 16, 16, 3, 297))

    def test_one_shared_icon(self):
        atlas, = self.ops('atlas_icons_add')
        self.assertEqual(atlas['symbol'], 'LobbyMenu_DecalsIcons')
        self.assertEqual([(i['row'], i['col']) for i in atlas['icons']], [(1, 25)])
        self.assertTrue((self.root / atlas['icons'][0]['source']).is_file())


if __name__ == '__main__':
    unittest.main()

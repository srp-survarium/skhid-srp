"""Map-editor edits preserve neighboring entries and reject stale input.

Run with: python3 -m unittest discover -s srp-026g/scripts/content-edit
"""
import io
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
import unittest.mock
import zlib

import content_edit as ce


def config(value):
    """Encode a small independent binary-config fixture (tuples are vectors)."""
    data = bytearray(24)

    def put(raw):
        offset = len(data)
        data.extend(raw)
        return offset

    def record(name, value):
        name_offset = put(name.encode() + b'\0')
        crc = zlib.crc32(name.encode())
        count = 1
        if isinstance(value, dict):
            records = [record(k, v) for k, v in value.items()]
            records.sort(key=lambda row: struct.unpack_from('<I', row, 16)[0])
            offset, kind, count = put(b''.join(records)), 3, len(records)
        elif isinstance(value, list):
            records = [record('', v) for v in value]
            offset, kind, count = put(b''.join(records)), 4, len(records)
        elif isinstance(value, tuple):
            offset, kind = put(struct.pack('<%df' % len(value), *value)), 4 + len(value)
        elif isinstance(value, str):
            offset, kind, count = put(value.encode() + b'\0'), 5, len(value.encode()) + 1
        elif isinstance(value, int):
            offset, kind = value & 0xffffffff, 1
        else:
            offset, kind = struct.unpack('<I', struct.pack('<f', value))[0], 2
        return struct.pack('<QQIHH', offset, name_offset, crc, kind, count)

    data[:24] = record('', value)
    return data


def fixture():
    return config({'objects': [
        {'full_name': 'first', 'position': (1.0, 2.0, 3.0),
         'spawned_item': {'position': (4.0, 5.0, 6.0), 'count': 2},
         'geometries': [{'name': 'original'}], 'containers': ['a', 'b']},
        {'full_name': 'second', 'position': (7.0, 8.0, 9.0)},
    ]})


def value(data, path):
    return ce.value_of(data, ce.find(data, path))


class MapEditorEdits(unittest.TestCase):
    def test_hud_string_rows_append_without_changing_neighbors(self):
        data = config({'hud_strings': [['existing', 'st_existing']], 'other': ['unchanged']})
        self.assertEqual(ce.append_rows(data, 'hud_strings', [['carry', 'st_carry']]), 1)
        self.assertEqual(value(data, 'hud_strings[0][0]'), 'existing')
        self.assertEqual(value(data, 'hud_strings[1][0]'), 'carry')
        self.assertEqual(value(data, 'hud_strings[1][1]'), 'st_carry')
        self.assertEqual(value(data, 'other[0]'), 'unchanged')

    def test_vectors_and_nested_keys_keep_other_values(self):
        data = fixture()
        before = dict((p, (t, v)) for p, t, v in ce.walk(data))
        changes = {'position': [3.5, 2.0, -1.0],
                   'spawned_item/position': [6.0, 5.0, 4.0],
                   'spawned_item/count': 3, 'geometries[0]/name': 'changed'}
        ce.set_entry(data, 'objects', {'full_name': 'first'},
                     {'position': [1.0001, 2, 3], 'spawned_item/count': 2}, changes)
        after = dict((p, (t, v)) for p, t, v in ce.walk(data))
        self.assertEqual({p for p in before if before[p] != after[p]},
                         {'/objects[0]/' + k for k in changes})
        for key, expected in changes.items():
            self.assertTrue(ce.same(value(data, 'objects[0]/' + key), expected))

    def test_stale_vector_and_wrong_dimensions_are_refused(self):
        for frm, to in [({'position': [9, 2, 3]}, {'position': [0, 0, 0]}),
                        ({}, {'position': [0, 0]})]:
            data = fixture()
            before = bytes(data)
            with self.assertRaises(ValueError):
                ce.set_entry(data, 'objects', {'full_name': 'first'}, frm, to)
            self.assertEqual(data, before)

    def test_set_vector_changes_one_float3_and_refuses_a_stale_one(self):
        data = config({'bounding_box': {'min': (-1.0, -1.0, -1.0), 'max': (1.0, 1.0, 1.0)}})
        ce.set_vector(data, 'bounding_box/min', [-1, -1, -1], [-0.5, -0.25, -0.125])
        self.assertTrue(ce.same(value(data, 'bounding_box/min'), [-0.5, -0.25, -0.125]))
        self.assertTrue(ce.same(value(data, 'bounding_box/max'), [1.0, 1.0, 1.0]))
        before = bytes(data)
        with self.assertRaises(ValueError):
            ce.set_vector(data, 'bounding_box/max', [2, 1, 1], [0, 0, 0])
        self.assertEqual(data, before)

    def test_nested_copy_does_not_change_the_model(self):
        data = fixture()
        ce.add_entry(data, 'objects', {'full_name': 'first'}, {'full_name': 'first'},
                     {'full_name': 'copy', 'spawned_item/position': [0, 1, 2],
                      'geometries[0]/name': 'copy-geometry'})
        self.assertEqual(value(data, 'objects[0]/spawned_item/position'), [4, 5, 6])
        self.assertEqual(value(data, 'objects[0]/geometries[0]/name'), 'original')
        self.assertEqual(value(data, 'objects[1]/spawned_item/position'), [0, 1, 2])
        self.assertEqual(value(data, 'objects[1]/geometries[0]/name'), 'copy-geometry')
        self.assertEqual(value(data, 'objects[2]/full_name'), 'second')
        ce.walk(data)  # Names, CRC ordering and every reachable value still parse.

    def test_add_value_takes_a_table_of_a_vector_and_a_float(self):
        """A lobby's srp_lobby (lobby-scenes-1): a named table at the root holding a float3 and a float."""
        data = fixture()
        ce.add_value(data, '', 'srp_lobby', {'weapon': [4.25, 1.3, 3], 'weapon_view': -1.5707964})
        rows = {p: (t, v) for p, t, v in ce.walk(data)}
        self.assertEqual(rows['/srp_lobby'], (3, 2))
        self.assertEqual(rows['/srp_lobby/weapon'], (7, struct.pack('<3f', 4.25, 1.3, 3.0)))
        self.assertEqual(ce.rec(data, ce.find(data, 'srp_lobby/weapon'))[4], 12)
        self.assertTrue(ce.same(value(data, 'srp_lobby/weapon_view'), -1.5707964))
        self.assertEqual(value(data, 'objects[1]/full_name'), 'second')
        for bad in ([1.0], [1, 2, 3, 4, 5], [1, True], ['a', 'b']):
            with self.assertRaises(ValueError):
                ce.add_value(fixture(), '', 'v', bad)

    def test_modifier_retarget_by_its_path(self):
        """A modification's modifiers differ only in their `path`: `where` takes a path in the entry."""
        def modifier(key):
            return {'path': ['recoil', key], 'value': -0.02, 'type': 2}
        data = config({'modifications': [{'id': 2080, 'modifiers': [modifier('a'), modifier('b'), modifier('c')]}]})
        ce.set_entry(data, 'modifications', {'id': 2080}, {'modifiers[0]/path[1]': 'a'},
                     {'modifiers[0]/path[1]': 'max_top_recoil'})
        with self.assertRaises(ValueError):  # every modifier has this value and type
            ce.remove_entry(bytearray(data), 'modifications[0]/modifiers', {'type': 2})
        ce.remove_entry(data, 'modifications[0]/modifiers', {'path[1]': 'b'})
        with self.assertRaises(ValueError):  # gone: a second removal is refused
            ce.remove_entry(data, 'modifications[0]/modifiers', {'path[1]': 'b'})
        self.assertEqual([value(data, 'modifications[0]/modifiers[%d]/path[1]' % i) for i in range(2)],
                         ['max_top_recoil', 'c'])
        self.assertTrue(ce.same(value(data, 'modifications[0]/modifiers[1]/value'), -0.02))
        ce.walk(data)

    def test_append_item_to_a_root_list_of_numbers(self):
        """An item's default_modifications gains a value; a value it holds already is refused."""
        data = config({'default_modifications': [1030, 1050], 'other': 7})
        ce.append_item(data, 'default_modifications', 1103)
        self.assertEqual(ce.items_of(data, 'default_modifications'), [1030, 1050, 1103])
        with self.assertRaises(ValueError):
            ce.append_item(data, 'default_modifications', 1050)
        with self.assertRaises(ValueError):
            ce.append_item(data, 'default_modifications', 'text')
        self.assertEqual(value(data, 'other'), 7)
        ce.walk(data)

    def test_cli_checks_set_and_append_item_of_a_root_list(self):
        """The whole tool on a suit-like config: a default replaced by `set`, two appended; the check
        afterwards expects exactly those changes."""
        edits = {'set': [{'file': 'suit', 'path': 'default_modifications[1]', 'from': 1112, 'to': 1113}],
                 'append_item': [{'file': 'suit', 'path': 'default_modifications', 'value': 1103},
                                 {'file': 'suit', 'path': 'default_modifications', 'value': 1094}]}
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'live').mkdir()
            (root / 'live/suit').write_bytes(bytes(config({'default_modifications': [1030, 1112], 'weight': 3.5})))
            (root / 'edits.json').write_text(json.dumps(edits))
            result = subprocess.run([sys.executable, str(Path(ce.__file__).resolve()),
                                     str(root / 'edits.json'), str(root / 'live'), str(root / 'out')],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn('0 unexpected differences', result.stdout)
            data = bytearray((root / 'out/suit').read_bytes())
            self.assertEqual(ce.items_of(data, 'default_modifications'), [1030, 1113, 1103, 1094])

    def test_cli_removes_keys_and_refuses_one_no_file_has(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'live/ammo').mkdir(parents=True)
            (root / 'live/ammo/a').write_bytes(bytes(config(
                {'parameters': {'arp': 1.2, 'damage': 1.1, 'weight': 0.3, 'item_level': 4}, 'data': {'k_arp': 1.2}})))
            (root / 'live/ammo/b').write_bytes(bytes(config({'parameters': {'weight': 0.2}})))

            def run(keys, out):
                (root / 'edits.json').write_text(json.dumps({'remove_key': [{'files': 'ammo/*', 'keys': keys}]}))
                return subprocess.run([sys.executable, str(Path(ce.__file__).resolve()),
                                       str(root / 'edits.json'), str(root / 'live'), str(root / out)],
                                      capture_output=True, text=True)
            result = run(['parameters/arp', 'parameters/damage'], 'out')
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn('1 files edited, 0 unexpected differences', result.stdout)
            data = bytearray((root / 'out/ammo/a').read_bytes())
            self.assertEqual({p for p, _, _ in ce.walk(data)},
                             {'', '/data', '/data/k_arp', '/parameters', '/parameters/weight', '/parameters/item_level'})
            self.assertEqual(value(data, 'parameters/item_level'), 4)
            self.assertNotEqual(run(['parameters/buckshot'], 'out2').returncode, 0)

    def test_cli_promotes_rolled_defaults_of_equipment_only(self):
        """Built-in bonuses of a kind that rolls are raised to its second-strongest row, on equipment
        only; a stronger one, a kind that never rolls and a wear-only one stay; the check afterwards
        expects exactly that."""
        def mask(kind):
            return '0' * kind + '1' + '0' * (40 - kind)
        def row(mid, kind, weight, path):
            return {'id': mid, 'type_mask': mask(kind), 'drop_weight': weight,
                    'modifiers': [{'path': path, 'value': 0.01, 'type': 2}]}
        speed = [row(1110 + i, 23, w, ['parameters', 'movement_speed_modifier'])
                 for i, w in enumerate([10.0, 8.0, 6.4, 5.12, 4.096])]
        wear = [row(1200 + i, 32, w, ['parameters', 'degrade_per_match']) for i, w in enumerate([10.0, 8.0])]
        slot = [row(1030, 15, 0.0, ['parameters', 'pocket_slots_modifier'])]
        live = {
            'gameplay/items/modifications/modifications.options': config({'modifications': speed + wear + slot}),
            'gameplay/db_static_dictionaries': config({'items_dict': [
                {'cfg_name': 'gameplay/items/armour/torso/suit.options', 'item_category': 3},
                {'cfg_name': 'gameplay/items/armour/legs/fast.options', 'item_category': 5},
                {'cfg_name': 'gameplay/items/weapons/gun.options', 'item_category': 8}]}),
            'gameplay/items/armour/torso/suit.options': config({'default_modifications': [1030, 1110, 1200]}),
            'gameplay/items/armour/legs/fast.options': config({'default_modifications': [1114]}),
            'gameplay/items/weapons/gun.options': config({'default_modifications': [1112]}),
        }
        edits = {'promote_rolled_defaults': [{'categories': [1, 2, 3, 4, 5, 6, 7], 'from_top': 2}]}
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for path, data in live.items():
                (root / 'live' / path).parent.mkdir(parents=True, exist_ok=True)
                (root / 'live' / path).write_bytes(bytes(data))
            (root / 'edits.json').write_text(json.dumps(edits))
            result = subprocess.run([sys.executable, str(Path(ce.__file__).resolve()),
                                     str(root / 'edits.json'), str(root / 'live'), str(root / 'out')],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn('0 unexpected differences', result.stdout)
            suit = bytearray((root / 'out/gameplay/items/armour/torso/suit.options').read_bytes())
            # +1 % -> the second strongest (1113); the item slot and the wear-only bonus stay.
            self.assertEqual(ce.items_of(suit, 'default_modifications'), [1030, 1113, 1200])
            # It only raises: the top strength (+5 %, 1114) stays.
            fast = bytearray((root / 'out/gameplay/items/armour/legs/fast.options').read_bytes())
            self.assertEqual(ce.items_of(fast, 'default_modifications'), [1114])
            self.assertNotIn('legs/fast.options default_modifications', result.stdout)
            # A weapon is left alone (not written: unchanged).
            self.assertFalse((root / 'out/gameplay/items/weapons/gun.options').exists())

    def test_cli_checks_add_remove_and_entry_index_shifts(self):
        edits = {
            'add_entry': [{'files': 'project', 'list': 'objects',
                           'after': {'full_name': 'first'}, 'like': {'full_name': 'first'},
                           'values': {'full_name': 'copy', 'spawned_item/position': [0, 1, 2]}}],
            'add_item': [{'file': 'project', 'list': 'objects', 'where': {'full_name': 'copy'},
                          'key': 'containers', 'value': 'c'}],
            'remove_item': [{'file': 'project', 'path': 'objects[1]/containers', 'value': 'a'}],
            'remove_entry': [{'file': 'project', 'list': 'objects', 'where': {'full_name': 'first'}}],
        }
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'live').mkdir()
            (root / 'live/project').write_bytes(fixture())
            (root / 'edits.json').write_text(json.dumps(edits))
            result = subprocess.run([sys.executable, str(Path(ce.__file__).resolve()),
                                     str(root / 'edits.json'), str(root / 'live'), str(root / 'out')],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            data = bytearray((root / 'out/project').read_bytes())
            self.assertEqual(ce.items_of(data, 'objects[0]/containers'), ['b', 'c'])
            self.assertEqual(value(data, 'objects[1]/full_name'), 'second')



def weapon(damage, card_damage, rpm=600, chamber=None):
    """A weapon config with a card of three lines: damage x100, rate of fire, weight."""
    parameters = {'bullet_damage': damage, 'rounds_per_minute': rpm, 'weight': 3.0}
    if chamber is not None:
        parameters['chamber_a_round_time'] = chamber
    return config({'parameters': parameters, 'melee': {'data': {'k_damage': 2}},
                   'ui_desc': {'props_list': [{'prop_id': 1, 'prop_value': card_damage},
                                              {'prop_id': 4, 'prop_value': str(rpm)},
                                              {'prop_id': 15, 'prop_value': '3.0'}]}})


WEAPON = ce.STATS['weapon-stats']


def table(rows, columns=None):
    """weapon-stats.tsv of {weapon: {column: cell}}; other cells empty."""
    columns = columns or list(WEAPON['columns'])
    return '\t'.join(['weapon'] + columns) + '\n' + ''.join(
        '\t'.join([name] + [cells.get(c, '') for c in columns]) + '\n' for name, cells in rows.items())


def row_of(text, name):
    with tempfile.TemporaryDirectory() as folder:
        (Path(folder) / 'weapon-stats.tsv').write_text(text)
        return ce.read_stats(str(Path(folder) / 'weapon-stats.tsv'))[2][name]


def cells_of(damage, rpm=600, k_damage=2):
    return {'damage': damage, 'rpm': str(rpm), 'weight': '3', 'melee': str(k_damage)}


class WeaponTable(unittest.TestCase):
    def run_table(self, text, weapons):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        root = Path(folder.name)
        for name, data in weapons.items():
            path = root / 'live' / ce.stats_config(WEAPON, name)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(bytes(data))
        (root / 'weapon-stats.tsv').write_text(text)
        result = subprocess.run([sys.executable, str(Path(ce.__file__).resolve()), str(root / 'weapon-stats.tsv'),
                                 str(root / 'live'), str(root / 'out')], capture_output=True, text=True)
        return result, root / 'out'

    def out(self, out, name):
        return bytearray((out / ce.stats_config(WEAPON, name)).read_bytes())

    def test_each_cell_is_its_configs_value_and_every_card_follows(self):
        text = table({'wpn_a': cells_of('0.5', k_damage=1.5), 'wpn_b': cells_of('0.45'), 'wpn_c': cells_of('0.3')})
        result, out = self.run_table(text, {'wpn_a': weapon(0.55, '55'), 'wpn_b': weapon(0.45, '40'),
                                            'wpn_c': weapon(0.3, '30')})
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('0 unexpected differences', result.stdout)
        a = self.out(out, 'wpn_a')
        self.assertTrue(ce.same(value(a, 'parameters/bullet_damage'), 0.5))
        self.assertEqual(ce.rec(a, ce.find(a, 'melee/data/k_damage'))[3], ce.T_FLOAT)
        self.assertEqual(value(a, 'melee/data/k_damage'), 1.5)
        self.assertEqual(ce.rec(a, ce.find(a, 'parameters/rounds_per_minute'))[3], ce.T_INT)
        self.assertEqual(value(a, 'ui_desc/props_list[0]/prop_value'), '50')
        self.assertEqual(value(self.out(out, 'wpn_b'), 'ui_desc/props_list[0]/prop_value'), '45')
        self.assertFalse((out / ce.stats_config(WEAPON, 'wpn_c')).exists())
        for name in ('wpn_a', 'wpn_b'):
            data = self.out(out, name)
            self.assertEqual(ce.card_differences(data, ce.weapon_card), [])
            self.assertEqual(ce.stats_differences(data, 0, WEAPON['columns'], row_of(text, name), name), [])

    def test_a_table_the_configs_cannot_take_is_refused(self):
        good = {'wpn_a': cells_of('0.55')}
        for text, weapons in (
                (table(good, list(WEAPON['columns']) + ['range']), {'wpn_a': weapon(0.55, '55')}),  # unknown
                (table(good, list(WEAPON['columns'])[1:]), {'wpn_a': weapon(0.55, '55')}),          # missing
                (table(good), {'wpn_a': weapon(0.55, '55'), 'wpn_b': weapon(0.4, '40')}),           # no row
                (table({'wpn_a': {**good['wpn_a'], 'melee': ''}}), {'wpn_a': weapon(0.55, '55')}),  # key kept
                (table({'wpn_a': {**good['wpn_a'], 'pierce': '0.3'}}), {'wpn_a': weapon(0.55, '55')})):  # none
            result, out = self.run_table(text, weapons)
            self.assertNotEqual(result.returncode, 0, text)
            self.assertFalse(out.exists())

    def test_the_dump_reads_back_as_its_configs(self):
        weapons = {'wpn_a': weapon(0.55, '55'), 'wpn_b': weapon(0.1 + 0.2, '30')}
        with tempfile.TemporaryDirectory() as folder:
            for name, data in weapons.items():
                path = Path(folder) / ce.stats_config(WEAPON, name)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(bytes(data))
            text = subprocess.run([sys.executable, str(Path(ce.__file__).resolve()), '--dump-stats', 'weapon-stats',
                                   folder], capture_output=True, text=True, check=True).stdout
        for name, data in weapons.items():
            self.assertEqual(ce.stats_differences(data, 0, WEAPON['columns'], row_of(text, name), name), [])

    def test_a_bolt_guns_rate_of_fire_counts_its_chambering(self):
        self.assertEqual(ce.weapon_card(weapon(1.1, '110', rpm=160, chamber=1.8))[4], '27')
        self.assertEqual(ce.weapon_card(weapon(1.2, '120', rpm=200, chamber=1.2))[4], '40')
        self.assertEqual(ce.card_differences(weapon(0.5, '50', rpm=160, chamber=1.8), ce.weapon_card),
                         [(4, '160', '27')])


LEVELS = {'configs': 'projects/*/*/client_project', 'match': {'protect'},
          'columns': {'protect': 'respawn_points[*]/time', 'zone': 'zones[kind=effect]/ms'}}
MODS = {'configs': 'mods.options#modifications[id]', 'match': {'value'},
        'columns': {'value': 'modifiers[0]/value', 'name': 'ui/name'}}
ROOMS = {'configs': 'srp/rooms.options', 'match': {'quick/wait'}}


def read_table(category, text):
    with tempfile.TemporaryDirectory() as folder, unittest.mock.patch.dict(ce.STATS, {'t': category}):
        (Path(folder) / 't.tsv').write_text(text)
        return ce.read_stats(str(Path(folder) / 't.tsv'))[1:]


def level(*times):
    return config({'respawn_points': [{'time': t} for t in times],
                   'zones': [{'kind': 'effect', 'ms': 1000}, {'kind': 'resupply', 'ms': 1000}]})


class OtherTables(unittest.TestCase):
    def test_a_row_is_every_file_of_its_level_and_every_entry_its_path_selects(self):
        files = {'projects/l2/day/client_project': level(6, 2), 'projects/l2/rain/client_project': level(6, 6),
                 'projects/l3/day/client_project': level(5)}
        players = {p: bytearray(b) for p, b in files.items()}
        self.assertEqual({row: [p for p, _ in places] for row, places in ce.stats_targets(LEVELS, files).items()},
                         {'l2': ['projects/l2/day/client_project', 'projects/l2/rain/client_project'],
                          'l3': ['projects/l3/day/client_project']})
        self.assertEqual(ce.stats_cells(files['projects/l2/day/client_project'], 0, LEVELS['columns'], 'l2'),
                         {'protect': '2|6', 'zone': '1000'})
        columns, rows = read_table(LEVELS, 'level\tprotect\tzone\nl2\t6\t2000\nl3\t5\t1000\n')
        written, made = ce.stats_write(LEVELS, columns, rows, files, 't')
        self.assertEqual(written, {'projects/l2/day/client_project': ['/respawn_points[1]/time', '/zones[0]/ms'],
                                   'projects/l2/rain/client_project': ['/zones[0]/ms']})
        self.assertEqual(value(files['projects/l2/day/client_project'], 'zones[1]/ms'), 1000)
        self.assertEqual(ce.stats_problems(LEVELS, columns, rows, ce.stats_targets(LEVELS, files), files, 't'), [])
        self.assertEqual(ce.stats_moved(LEVELS, columns, players, files),
                         ['projects/l2/day/client_project respawn_points[*]/time'])

    def test_rows_of_list_entries_named_by_their_key(self):
        mods = config({'modifications': [{'id': 1030, 'modifiers': [{'value': 0.1}], 'ui': {'name': 'a'}},
                                         {'id': 1031, 'modifiers': [{'value': 0.2}], 'ui': {'name': 'b'}}]})
        files = {'mods.options': mods}
        players = {'mods.options': bytearray(mods)}
        columns, rows = read_table(MODS, 'id\tvalue\tname\n1030\t0.1\ta\n1031\t0.25\tc\n')
        ce.stats_write(MODS, columns, rows, files, 't')
        self.assertEqual((value(mods, 'modifications[1]/ui/name'), ce.cell_text(mods, ce.find(mods, 'modifications[1]/modifiers[0]/value'))),
                         ('c', '0.25'))
        self.assertEqual(ce.stats_moved(MODS, columns, players, files), ['mods.options#modifications[id=1031] modifiers[0]/value'])
        columns, rows = read_table(MODS, 'id\tvalue\tname\n1030\t0.1\ta\n')
        self.assertEqual(ce.stats_unmatched(MODS, rows, ce.stats_targets(MODS, files), 't'),
                         ['mods.options#modifications[id=1031]: no row in t'])

    def test_a_key_value_table_writes_its_config_or_makes_it(self):
        old = config({'quick': {'wait': 10.0, 'size': 6}, 'on': 0, 'name': 'x'})
        columns, rows = read_table(ROOMS, 'key\tvalue\nquick/wait\t12\nquick/size\t8\nname\ty\n')
        files = {'srp/rooms.options': old}
        written, made = ce.stats_write(ROOMS, columns, rows, files, 't')
        self.assertEqual((made, ce.rec(old, ce.find(old, 'quick/wait'))[3], value(old, 'quick/wait'), value(old, 'name')),
                         (set(), ce.T_FLOAT, 12.0, 'y'))
        with self.assertRaises(ValueError):
            ce.stats_write(ROOMS, *read_table(ROOMS, 'key\tvalue\nquick/size\tsix\n'), files, 't')
        columns, rows = read_table(ROOMS, 'key\tvalue\nwaits[0]\t10\nwaits[1]\t20\nquick/wait\t0.5\non\ttrue\nname\tx\n')
        files = {}
        written, made = ce.stats_write(ROOMS, columns, rows, files, 't')
        new = files['srp/rooms.options']
        self.assertEqual(made, {'srp/rooms.options'})
        self.assertEqual({q: t for q, t, _ in ce.walk(new)},
                         {'': ce.T_NAMED, '/waits': ce.T_INDEXED, '/waits[0]': ce.T_INT, '/waits[1]': ce.T_INT,
                          '/quick': ce.T_NAMED, '/quick/wait': ce.T_FLOAT, '/on': ce.T_BOOL, '/name': ce.T_STR})
        self.assertEqual(ce.stats_cells(new, 0, columns, ''), rows[''])

    def test_the_gate_sees_a_reordered_list_and_a_dropped_duplicate(self):
        columns = LEVELS['columns']
        for was, now in ((level(2, 6), level(6, 2)), (level(6, 6), level(6))):
            self.assertEqual(ce.stats_moved(LEVELS, columns, {'projects/l2/day/client_project': was},
                                            {'projects/l2/day/client_project': now}),
                             ['projects/l2/day/client_project respawn_points[*]/time'])

    def test_a_new_config_moves_each_of_its_match_values(self):
        columns, rows = read_table(ROOMS, 'key\tvalue\nquick/wait\t5\nname\tx\n')
        files = {}
        ce.stats_write(ROOMS, columns, rows, files, 't')
        self.assertEqual(ce.stats_moved(ROOMS, columns, {}, files), ['srp/rooms.options quick/wait'])

    def test_a_selector_that_selects_nothing_is_refused(self):
        files = {'projects/l2/day/client_project': config({'respawn_points': [],
                                                           'zones': [{'kind': 'resupply', 'ms': 1000}]})}
        columns, rows = read_table(LEVELS, 'level\tprotect\tzone\nl2\t\t\n')
        with self.assertRaisesRegex(ValueError, 'row l2: respawn_points\\[\\*\\]/time matches nothing'):
            ce.stats_write(LEVELS, columns, rows, files, 't')
        self.assertEqual(ce.stats_problems(LEVELS, columns, rows, ce.stats_targets(LEVELS, files), files, 't'),
                         ['projects/l2/day/client_project: row l2: respawn_points[*]/time matches nothing'])

    def test_the_dump_of_a_row_over_several_files_is_its_one_value_or_refused(self):
        with tempfile.TemporaryDirectory() as folder, unittest.mock.patch.dict(ce.STATS, {'t': LEVELS}):
            for weather, times in (('day', (6, 6)), ('rain', (6,))):
                path = Path(folder) / 'projects/l2' / weather / 'client_project'
                path.parent.mkdir(parents=True)
                path.write_bytes(bytes(level(*times)))
            with unittest.mock.patch('sys.stdout', new=io.StringIO()) as out:
                ce.dump_stats('t', folder)
            self.assertEqual(out.getvalue(), 't\tprotect\tzone\nl2\t6\t1000\n')
            (Path(folder) / 'projects/l2/rain/client_project').write_bytes(bytes(level(2)))
            with self.assertRaisesRegex(SystemExit, 'row l2 protect: 2 \\| 6'):
                ce.dump_stats('t', folder)


    def test_a_list_of_files_names_a_row_by_each_and_an_absent_list_is_no_such_key(self):
        category = {'configs': ['w/rgd-5.options', 'd/trap.options'], 'match': set(),
                    'columns': {'radius': 'data/splash_radius', 'damage': 'data/hits[*]/amount'}}
        files = {'w/rgd-5.options': config({'data': {'splash_radius': 10}}),
                 'd/trap.options': config({'data': {'hits': [{'amount': 0.6}, {'amount': 0.6}]}})}
        columns, rows = read_table(category, 'name\tradius\tdamage\nrgd-5\t10\t\ntrap\t\t0.6\n')
        self.assertEqual(ce.stats_problems(category, columns, rows, ce.stats_targets(category, files), files, 't'), [])
        self.assertEqual(ce.stats_label(category, 'gone'), 'gone')

    def test_an_ammo_card_counts_every_pellet(self):
        ammo = config({'data': {'k_damage': 0.2, 'buck_shot': 10, 'k_arp': 0.2},
                       'parameters': {'clip_size': 15, 'clip_weight': 0.45}})
        self.assertEqual(ce.ammo_card(ammo), {21: '200', 22: '20', 16: '15', 15: '0.4'})

    def test_a_grenade_card_counts_every_body_and_a_trap_card_its_foot(self):
        grenade = config({'data': {'splash_max_damage': 1.3, 'splash_radius': 10, 'splash_affected_bodies': [6, 0, 1]},
                          'parameters': {'weight': 0.35}})
        trap = config({'data': {'damage_parameters': [{'amount': 0.6}, {'amount': 0.6}]}, 'parameters': {'weight': 0.8}})
        self.assertEqual(ce.throwable_card(grenade), {1: '390', 23: '10', 15: '0.3'})
        self.assertEqual(ce.throwable_card(trap), {1: '60', 15: '0.8'})


if __name__ == '__main__':
    unittest.main()

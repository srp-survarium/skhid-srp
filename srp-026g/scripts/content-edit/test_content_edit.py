"""Map-editor edits preserve neighboring entries and reject stale input.

Run with: python3 -m unittest discover -s srp-026g/scripts/content-edit
"""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
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


if __name__ == '__main__':
    unittest.main()


def weapon(damage, card_damage, rpm=600, chamber=None):
    """A weapon config with a card of three lines: damage x100, rate of fire, weight."""
    parameters = {'bullet_damage': damage, 'rounds_per_minute': rpm, 'weight': 3.0}
    if chamber is not None:
        parameters['chamber_a_round_time'] = chamber
    return config({'parameters': parameters, 'melee': {'data': {'k_damage': 2}},
                   'ui_desc': {'props_list': [{'prop_id': 1, 'prop_value': card_damage},
                                              {'prop_id': 4, 'prop_value': str(rpm)},
                                              {'prop_id': 15, 'prop_value': '3.0'}]}})


WEAPON = ce.STATS['weapon']


def table(rows, columns=None):
    """A weapon-stats.tsv: {weapon: {column: cell}}, the other cells empty."""
    columns = columns or list(WEAPON['columns'])
    return '# a comment\n' + '\t'.join(['weapon'] + columns + ['notes']) + '\n' + ''.join(
        '\t'.join([name] + [cells.get(c, '') for c in columns] + ['why']) + '\n' for name, cells in rows.items())


def row_of(text, name):
    """The cells of a weapon's row of a table's text, as read_stats reads them."""
    with tempfile.TemporaryDirectory() as folder:
        (Path(folder) / 'weapon-stats.tsv').write_text(text)
        return ce.read_stats(str(Path(folder) / 'weapon-stats.tsv'))[1][ce.stats_config(WEAPON, name)][0]


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
        """A changed cell is written (an int key with a fraction becomes a float), every card is written
        from its config, a weapon whose row and card agree is not written."""
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
            self.assertEqual(ce.stats_differences(data, WEAPON, row_of(text, name)), [])

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
            text = subprocess.run([sys.executable, str(Path(ce.__file__).resolve()), '--dump-stats', 'weapon',
                                   folder], capture_output=True, text=True, check=True).stdout
        for name, data in weapons.items():
            self.assertEqual(ce.stats_differences(data, WEAPON, row_of(text, name)), [])

    def test_a_bolt_guns_rate_of_fire_counts_its_chambering(self):
        """The stock cards' rule: rounds a minute with chambering each, rounded down (the Mosin: 27)."""
        self.assertEqual(ce.weapon_card(weapon(1.1, '110', rpm=160, chamber=1.8))[4], '27')
        self.assertEqual(ce.weapon_card(weapon(1.2, '120', rpm=200, chamber=1.2))[4], '40')
        self.assertEqual(ce.card_differences(weapon(0.5, '50', rpm=160, chamber=1.8), ce.weapon_card),
                         [(4, '160', '27')])

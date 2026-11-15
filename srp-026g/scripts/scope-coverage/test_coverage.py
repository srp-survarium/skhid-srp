"""Attachment graph regressions: compatibility links alone are insufficient."""
import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from audit import mounts


def module(group, parent, child=None):
    return dict(group=group, slots_to_attach_to=[dict(slot=parent, attach_params='attach')],
                attach=dict(slots_to_add=[dict(id=child, locator=child + '_point')] if child else []))


class AttachmentGraph(unittest.TestCase):
    def setUp(self):
        self.game = dict(weapons={'gun': dict(default_modules=[], slots=[dict(id='receiver', locator='receiver_point')])},
                         weapon_modules={}, module_to_item_links=[])

    def add(self, name, value):
        self.game['weapon_modules'][name] = value
        self.game['module_to_item_links'].append(dict(item='gun', module=name))

    def test_linked_piggyback_does_not_become_standalone(self):
        self.add('magnifier', module('scope_optical', 'receiver', 'piggyback'))
        self.add('dot', module('scope_collimator', 'piggyback'))
        slots, _ = mounts(self.game, 'gun')
        self.assertNotIn('piggyback', slots)

    def test_handguard_and_rail_chain_is_required(self):
        self.add('handguard', module('handguard_rail', 'receiver', 'rail'))
        self.add('medium_picatinnyrail', module('picatinny rail', 'rail', 'optic'))
        slots, _ = mounts(self.game, 'gun')
        path = slots['optic']
        self.assertEqual(path.root, 'receiver_point')
        self.assertEqual([s.module for s in path.steps], ['handguard', 'medium_picatinnyrail'])
        self.assertEqual([s.locator for s in path.steps], ['rail_point', 'optic_point'])

    def test_weapon_exception_can_make_rail_tactical_only(self):
        rail = module('picatinny rail', 'receiver', 'optic')
        rail['exceptions'] = {'gun': {'slots_to_attach_to': [dict(slot='grip', attach_params='attach')]}}
        self.add('rail', rail)
        slots, _ = mounts(self.game, 'gun')
        self.assertNotIn('optic', slots)

    def test_default_module_need_not_be_sold_separately(self):
        self.game['weapon_modules']['cover'] = module('receiver_cover', 'receiver', 'optic')
        self.game['weapons']['gun']['default_modules'] = [dict(module='cover', slot='receiver')]
        slots, _ = mounts(self.game, 'gun')
        self.assertEqual(slots['optic'].steps[0].module, 'cover')

    def test_prefers_direct_socket_to_handguard_replacement(self):
        self.game['weapons']['gun']['slots'].append(dict(id='optic', locator='integral_rail'))
        self.add('handguard', module('handguard_rail', 'receiver', 'optic'))
        slots, _ = mounts(self.game, 'gun')
        self.assertEqual(slots['optic'].root, 'integral_rail')
        self.assertEqual(slots['optic'].steps, ())

    def test_disabled_slots_are_not_paths(self):
        self.game['weapons']['gun']['slots'][0]['accept_module'] = False
        self.add('rail', module('picatinny rail', 'receiver', 'optic'))
        self.assertEqual(mounts(self.game, 'gun')[0], {})

    def test_audit_does_not_mutate_donor(self):
        self.add('rail', module('picatinny rail', 'receiver', 'optic'))
        before = copy.deepcopy(self.game)
        mounts(self.game, 'gun')
        self.assertEqual(self.game, before)


if __name__ == '__main__':
    unittest.main()

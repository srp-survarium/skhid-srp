#!/usr/bin/env python3
"""check_content.py <before> <after>: extracted gameplay paths from the manifest builds.
Check displayed medical values against their actual influences, and ensure this
presentation change leaves every other item configuration field unchanged.
"""
import math
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'content-edit'))
import content_edit as config

def value(data, at):
    kind = config.rec(data, at)[3]
    if kind in (config.T_NAMED, config.T_INDEXED):
        children = list(config.children(data, at))
        if kind == config.T_INDEXED:
            return [value(data, child) for child in children]
        return {config.name_at(data, config.rec(data, child)[1]).decode():value(data, child) for child in children}
    return config.value_of(data, at)

def read(root, name):
    return value((root / name).read_bytes(), 0)

def main():
    before, after = map(Path, sys.argv[1:])
    metadata = read(after, 'gameplay/static_game_parameters.options')
    props = {p['prop_id']:p for p in metadata['ui_properties']}
    assert props[19]['direction'] == props[15]['direction'] == 0
    for prop in (135, 136, 137):
        assert props[prop]['direction'] == 1 and props[prop]['postfix'] == 'st_item_props_percent'
    for name in ('bandages', 'medkit'):
        path = f'gameplay/items/drugs/{name}.options'
        old, new = read(before, path), read(after, path)
        rows = {p['prop_id']:float(p['prop_value']) for p in new['ui_desc']['props_list']}
        assert len(rows) == 6 and 12 not in rows
        prior = {p['prop_id']:float(p['prop_value']) for p in old['ui_desc']['props_list']}
        assert all(rows[p] == prior[p] for p in (15,18,19)), 'weight or times changed'
        maximum = {'left_hand':.5,'right_hand':.5,'left_leg':.5,'right_leg':.5,'body':1,'head':.4,'pain':1}
        shown = {'left_hand':135,'right_hand':135,'left_leg':135,'right_leg':135,'body':136,'head':136,'pain':137}
        duration = new['data']['activity_time_sec']
        for influence in new['data']['influences']:
            part = influence['body_part']
            if part in shown:
                healed = min(100, 100 * influence['amount'] * duration / maximum[part])
                assert math.isclose(rows[shown[part]], healed, abs_tol=.001), (name,part,healed)
        old['ui_desc'].pop('props_list');new['ui_desc'].pop('props_list')
        assert old == new, 'non-presentation item configuration changed'
        print(name, rows, 'matches native influences; weight/times and gameplay unchanged')

if __name__ == '__main__': main()

#!/usr/bin/env python3
"""Check the authored edits over the pinned pack and stock border templates.

Registration collision with every other map is checked by build-content.
This fixture uses the pack's own dictionary rows to check edit preservation.
"""
import argparse
import json
from pathlib import Path
import tempfile

import make_ffa_edits as m
import make_truck_pack as truck
import check_ffa_edits
import check_training_arena

MANIFEST = m.HERE.parents[2] / 'release/content.tsv'


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--pack', type=Path, required=True)
    ap.add_argument('--native', type=Path, required=True)
    a = ap.parse_args()
    pack = json.loads((a.pack / 'pack.json').read_text())
    rows = next(op['entries'] for op in pack['operations']
                if op['op'] == 'dictionary_add' and op['table'] == 'maps_dict')
    with tempfile.TemporaryDirectory(prefix='stalker-sources-') as temp:
        dictionary = Path(temp) / 'dictionary'
        data = bytearray(24)
        data[:24] = m.c.value_record(data, 0, '', {'maps_dict': dict(enumerate(rows))})
        dictionary.write_bytes(data)
        check_ffa_edits.check(a.pack, dictionary)
        check_training_arena.check(a.pack, dictionary, a.native, m.EDIT)
    authored = m.points()[m.project('cs_training_camp')]
    fence = check_training_arena.arena.outline()
    # Every point is inside the base's fence, and stands on what its row says: a raised floor of
    # the tables (an island, a tower's stairs, a cabin) with a way down, or, GROUND of them, the
    # ground. 8 and 19 of the first plan are where they were.
    assert all(check_training_arena.inside_of(fence, p['x'], p['z']) for p in authored)
    assert authored[8]['y'] > 20 and authored[19]['y'] > 15
    raised = []
    for p in authored:
        on = m.stands_on(p)
        if on == m.ON_GROUND:
            note = on
        else:
            assert p['y'] >= 6, (p['point'], 'is not raised')
            note = f'{on}, way down on foot'
            raised.append(on)
        assert p['reason'].endswith(f'; spacing >=8m in 3D; {note}'), (p['point'], on, p['reason'])
    assert len(authored) - len(raised) == m.GROUND
    kinds = {kind: sum(on.startswith(kind) for on in raised) for kind in ('raised island', 'stairs of tower', 'cabin')}
    # The truck's pack as the manifest pins it: made again from stalker-maps@6, the same pack.json.
    made, report = truck.build(truck.sources(a.pack))
    pinned = [line.split('\t')[2] for line in MANIFEST.read_text().splitlines()
              if line.startswith(f'pack\t{truck.PACK_ID}@{truck.VERSION}\t')]
    assert pinned == [truck.sha256(truck.manifest(made).encode())], 'release/content.tsv does not pin the truck this makes'
    print(f'PASS: {len(authored)} Training Camp points inside the fence; {len(raised)} on raised floors '
          f'({kinds["raised island"]} on islands, {kinds["stairs of tower"]} on the towers\' stairs, {kinds["cabin"]} in cabins), '
          f'{m.GROUND} on the ground; raised spawns 8 and 19 are preserved; the truck pack is the manifest\'s ({report})')

if __name__ == '__main__':
    main()

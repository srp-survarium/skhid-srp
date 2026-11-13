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
import check_ffa_edits
import check_training_arena


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
    # The raised-island spawns are intentional, not scenery candidates: 8 and
    # 19 of the distributed 24, then those added on the islands (24 and up).
    assert authored[8]['y'] > 20 and authored[19]['y'] > 15
    added = authored[24:]
    assert added and all(m.on_island(p) and '; raised island ' in p['reason'] for p in added)
    # Every other perimeter point moved inside the base's fence, keeping its id.
    moved = [p for p in authored[:24] if '; moved from the perimeter' in p['reason']]
    fence = [(float(x), float(z)) for x, z in
             (line.split('\t') for line in (m.HERE / 'training-fence.tsv').read_text().splitlines()[1:])]
    assert len(moved) == 7 and all(check_training_arena.inside_of(fence, p['x'], p['z']) for p in moved)
    print(f'PASS: raised-island spawns 8 and 19 are preserved; {len(added)} added on the islands; '
          f'{len(moved)} perimeter points moved inside the fence')


if __name__ == '__main__':
    main()

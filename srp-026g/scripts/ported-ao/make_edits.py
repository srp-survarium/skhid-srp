#!/usr/bin/env python3
"""The ported maps' ambient occlusion at native 0.26g's strength.

usage: python3 -I make_edits.py <live folder> <out.json> [<level prefix>...]

<live folder> holds every project of the ported maps at its game path, as the content build has them
where the list's row is (every mode's copy made). <level prefix>: the levels the list covers (default
v011e_ v021d_); London (v0.32) and (v0.42) get the rule from resource-porter's near_build.

`ao_saturation` is the exponent of the occlusion (docs/notes/stalker-maps.md): native levels 1 to 8 have
1 to 1.29; a ported map above that (0.21's rain 1.5, Vostok v0.11 1.5) draws a dark halo round every
object (the owner, 2026-11-22, a shelf on London (v0.42)'s rain: "a strange shadow which we never
fixed"). Such a map takes 1, the STALKER maps' rule (stalker-maps-ao-1).
"""
import json
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'content-edit'))
import content_edit as ce  # noqa: E402

NATIVE_MAX = 1.29
PORTED = 1.0


def main():
    live, out, *levels = sys.argv[1:]
    levels = levels or ['v011e_', 'v021d_']
    edits = []
    root = os.path.join(live, 'projects')
    for level in sorted(os.listdir(root)):
        if not level.startswith(tuple(levels)):
            continue
        for project in sorted(os.listdir(os.path.join(root, level))):
            f = 'projects/%s/%s/client_project' % (level, project)
            if not os.path.isfile(os.path.join(live, f)):
                continue
            b = bytearray(open(os.path.join(live, f), 'rb').read())
            for i, o in enumerate(ce.children(b, ce.find(b, 'client_objects'))):
                try:
                    kind = ce.value_of(b, ce.find(b, 'game_object_type', o))
                    value = ce.value_of(b, ce.find(b, 'properties/ao_saturation', o))
                except KeyError:
                    continue
                if kind == 'environment' and value > NATIVE_MAX + 1e-6:
                    edits.append({'file': f, 'path': 'client_objects[%d]/properties/ao_saturation' % i,
                                  'from': round(value, 6), 'to': PORTED})
    about = ' '.join(__doc__.split('\n\n')[2].split())
    json.dump({'_about': about, 'set': edits}, open(out, 'w'), indent=1)
    print('%d projects' % len(edits))


if __name__ == '__main__':
    main()

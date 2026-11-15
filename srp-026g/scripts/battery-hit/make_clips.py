"""Write the battery swing's clips (swing.py's keys) where the edit list
battery-hit-1.json takes them from: the carrier's (its arms) battery_hit and
battery_hit_recover, the battery's own battery_hit_electrobox and
battery_hit_recover_electrobox (.b-spline and .bones_names each). `--check` instead
compares the committed files with what the keys make.

    python make_clips.py <assets folder> [--check]

(the assets: extract.sh's folder; numpy needed)."""
import sys
from pathlib import Path

import clips
import swing
from rig import Rig, HANDS, ITEM

HERE = Path(__file__).resolve().parent


def made(assets):
    rig = Rig(assets)
    idle = rig.clip(HANDS + 'idle')
    names = (Path(assets) / (HANDS + 'idle.bones_names')).read_bytes()
    item_names = (Path(assets) / (ITEM + 'idle_electrobox.bones_names')).read_bytes()
    swing_clip, recover_clip, item_swing, item_recover = swing.clips_of(rig, idle, swing.author(rig, idle))
    return {
        'battery_hit.b-spline': clips.write_clip(swing_clip),
        'battery_hit.bones_names': names,
        'battery_hit_recover.b-spline': clips.write_clip(recover_clip),
        'battery_hit_recover.bones_names': names,
        'battery_hit_electrobox.b-spline': clips.write_clip(item_swing),
        'battery_hit_electrobox.bones_names': item_names,
        'battery_hit_recover_electrobox.b-spline': clips.write_clip(item_recover),
        'battery_hit_recover_electrobox.bones_names': item_names,
    }


def main():
    out = HERE / 'clips'
    files = made(sys.argv[1])
    if '--check' in sys.argv:
        stale = [n for n, data in files.items() if not (out / n).exists() or (out / n).read_bytes() != data]
        sys.exit('stale: ' + ', '.join(stale) if stale else 0)
    out.mkdir(exist_ok=True)
    for name, data in files.items():
        (out / name).write_bytes(data)
        print(name, len(data))


if __name__ == '__main__':
    main()

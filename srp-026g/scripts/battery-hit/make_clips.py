"""Write the battery strikes' clips (swing.py's keys) where the edit list
battery-hit-1.json takes them from: for each kind of strike (swing.STRIKES:
battery_hit the ram, battery_hit_shove the shove, battery_hit_slam the slam)
the carrier's (his shoulders and arms) <name> and <name>_recover and the
battery's own <name>_electrobox and <name>_recover_electrobox (.b-spline and
.bones_names each). `--check` instead compares the committed files with what
the keys make.

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
    files = {}
    for name, strike in swing.STRIKES.items():
        swing_clip, recover_clip, item_swing, item_recover = swing.clips_of(rig, idle, swing.author(rig, idle, strike))
        files.update({
            name + '.b-spline': clips.write_clip(swing_clip),
            name + '.bones_names': names,
            name + '_recover.b-spline': clips.write_clip(recover_clip),
            name + '_recover.bones_names': names,
            name + '_electrobox.b-spline': clips.write_clip(item_swing),
            name + '_electrobox.bones_names': item_names,
            name + '_recover_electrobox.b-spline': clips.write_clip(item_recover),
            name + '_recover_electrobox.bones_names': item_names,
        })
    return files


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

"""Write the battery swing's two clips (swing.py's keys) where the edit list
battery-hit-1.json takes them from: clips/battery_hit.{b-spline,bones_names}
and clips/battery_hit_recover.{b-spline,bones_names}. `--check` instead
compares the committed files with what the keys make.

    python make_clips.py <assets folder> [--check]

(the assets: extract.sh's folder; numpy needed)."""
import sys
from pathlib import Path

import clips
import swing
from rig import Rig, HANDS

HERE = Path(__file__).resolve().parent


def made(assets):
    rig = Rig(assets)
    idle = rig.clip(HANDS + 'idle')
    names = (Path(assets) / (HANDS + 'idle.bones_names')).read_bytes()
    frames = swing.author(rig, idle)
    return {
        'battery_hit.b-spline': clips.write_clip(swing.write(rig, idle, frames, 0, swing.IMPACT)),
        'battery_hit.bones_names': names,
        'battery_hit_recover.b-spline': clips.write_clip(swing.write(rig, idle, frames, swing.IMPACT, swing.END)),
        'battery_hit_recover.bones_names': names,
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

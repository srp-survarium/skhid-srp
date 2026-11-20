"""The clips through the game's own code: each track baked by the game's
baker (bake_time_channel 0x3B3250: EAX the track, ESI the channel header,
the data's place on the stack) and evaluated by its evaluate_frame
(0x3B3180) at 41 times, under Unicorn, against clips.py's evaluation (what
make_clips.py and preview.py use). Prints the worst difference.

    python engine_bake.py <original survarium.exe> <clip.b-spline>...

Needs Python `unicorn` and `numpy` (srp-026g's dev shell has neither: see
check_battery_hit_026g.py's way of adding unicorn's site-packages)."""
import os
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent.parent / 'client-patches'))
os.environ.setdefault('SRP_026G_EXE', sys.argv[1])
import check_animation_math_026g as engine  # noqa: E402
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESI  # noqa: E402

import clips  # noqa: E402

BAKE, EVALUATE = 0x3B3250, 0x3B3180


def main():
    engine.LIMIT = 2_000_000
    run = engine.Run(bytearray(Path(sys.argv[1]).read_bytes()))
    track, head, baked = engine.DATA, engine.DATA + 0x10000, engine.DATA + 0x10010
    out, current = engine.DATA + 0x30000, engine.DATA + 0x30100
    worst, samples = 0.0, 0
    for path in sys.argv[2:]:
        channels, _ = clips.parse_clip(Path(path).read_bytes())
        for bone in channels:
            for c in bone:
                run.emu.mem_write(track, struct.pack('<I', len(c.knots))
                                  + b''.join(struct.pack('<ff', k, p) for k, p in zip(c.knots, c.points)))
                run.emu.mem_write(head, bytes(0x20000))
                done, _, _ = run.call(BAKE, {UC_X86_REG_EAX: track, UC_X86_REG_ESI: head}, [baked])
                assert done is True, done
                run.word(current, 0)
                for i in range(41):
                    t = c.knots[-1] * i / 40
                    done, _, _ = run.call(EVALUATE, {UC_X86_REG_EAX: out, UC_X86_REG_ECX: head},
                                          [engine.bits(t), current, 1])
                    assert done is True, done
                    worst = max(worst, abs(struct.unpack('<f', run.read(out))[0] - c.evaluate(t)))
                    samples += 1
        print(path, 'tracks', sum(len(b) for b in channels))
    print('samples', samples, 'worst |game - clips.py|', worst)
    sys.exit(0 if worst < 1e-5 else 'the game evaluates the clips otherwise')


if __name__ == '__main__':
    main()

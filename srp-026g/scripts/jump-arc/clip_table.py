"""clip_table.py <anim dir>: the jump clips' root (bone 0) per frame, 0..end, for the A318 jump arc: rise (y), forward (z) and side (x),
from the stock clips (clamped cubic B-splines, 30 frames a second). The B-spline code is sprint-jump-re/work/jumpcurve.py's.
Prints a summary per clip and a Rust table (`--rust`). Reads only the clip files."""
import os
import struct
import sys

CLIPS = ['on_site', 'on_site_fwd', 'on_site_fwd_right', 'on_site_right', 'on_site_back_right', 'on_site_back', 'on_site_back_left',
         'on_site_left', 'on_site_fwd_left', 'sprint__sprint_from_left_leg', 'sprint__sprint_from_right_leg',
         'sprint__sprint_right_from_left_leg', 'sprint__sprint_right_from_right_leg', 'sprint__sprint_left_from_left_leg',
         'sprint__sprint_left_from_right_leg']


def tracks(path):
    b = open(path, 'rb').read()
    bones, events, kind = struct.unpack_from('<HBB', b, 0)
    off = 4 + bones * 0x48 + events * 0x10
    out = []
    for _ in range(9):
        n = struct.unpack_from('<I', b, off)[0]
        kv = struct.unpack_from('<%df' % (2 * n), b, off + 4)
        out.append(list(zip(kv[0::2], kv[1::2])))
        off += 4 + 8 * n
    return out


def curve(tr):
    end = tr[-1][0]
    pts = [kv for kv in tr if kv[1] > -1e8]
    c = [v for _, v in pts]
    n = len(c)
    T = [k for k, _ in pts] + [end] * 4

    def f(t):
        t = max(0.0, min(end - 1e-6, t))
        k = max(i for i in range(3, n) if T[i] <= t)
        d = [c[j + k - 3] for j in range(4)]
        for r in range(1, 4):
            for j in range(3, r - 1, -1):
                i = j + k - 3
                den = T[i + 4 - r] - T[i]
                a = 0 if den == 0 else (t - T[i]) / den
                d[j] = (1 - a) * d[j - 1] + a * d[j]
        return d[3]
    return f, end


def main():
    d = sys.argv[1]
    rust = '--rust' in sys.argv
    for name in CLIPS:
        p = os.path.join(d, 'animations__single__weapons__ak__player__stand__3rd_view__jump__%s.b-spline' % name)
        if not os.path.exists(p):
            print('# missing', name)
            continue
        t = tracks(p)
        X, end = curve(t[0])
        Y, _ = curve(t[1])
        Z, _ = curve(t[2])
        frames = range(0, int(end) + 1)
        if rust:
            rows = ', '.join('[%.4f, %.4f, %.4f]' % (Y(f), Z(f), X(f)) for f in frames)
            print('    // %s: %d frames' % (name, int(end)))
            print('    &[%s],' % rows)
        else:
            top = max(frames, key=Y)
            print('%-40s end %2d  apex %.3f @%2d  fwd %.2f side %.2f (end)' % (name, end, Y(top), top, Z(end), X(end)))


if __name__ == '__main__':
    main()

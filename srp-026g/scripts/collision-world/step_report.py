"""What a step trace holds (`step_trace.py`'s file): the frame of the game's tick and the chain of its steps.

    python step_report.py steps.bdt [--show 60 --from 20000]

- the atomic ticks by where they start and end on the 50 ms grid;
- per controller, the chain of whole steps: does a whole step start with the bytes the whole step before left;
- a look-ahead part against the whole step made from the same bytes;
- who calls updateAction and set_transform;
- with --show, that many stops from --from on, readable.
"""
import argparse
import collections
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import step_file  # noqa: E402

STATE = ("origin", "velocity", "jumping", "flags", "surface normal", "ceiling normal", "ceiling slide", "dims")


def vec(x):
    return "(" + ",".join(f"{c:.5f}" for c in x) + ")"


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def tick_classes(trace):
    """Counts of the atomic ticks by their place on the grid, and of the parts between two whole steps."""
    classes = collections.Counter()
    between = collections.Counter()
    run = 0
    for time, length, _ in trace.ticks:
        start = time - length
        if trace.is_whole(time, length):
            classes["a whole grid step"] += 1
            between[run] += 1
            run = 0
            continue
        run += 1
        if start % 50 == 0 and time % 50:
            classes["a part from the grid that ends inside the step (a look ahead, or a split step's first part)"] += 1
        elif start % 50 == 0:
            classes[f"from the grid to a later grid time ({length} ms)"] += 1
        elif time % 50 == 0:
            classes["a part that starts inside a step and ends on the grid (a split step)"] += 1
        else:
            classes["a part that starts and ends inside a step (a split step)"] += 1
    return classes, between


def chain(whole):
    """How each whole step starts against what the whole step one grid time before left."""
    out = collections.Counter()
    holes = collections.Counter()
    for steps in whole.values():
        times = sorted(steps)
        for a, b in zip(times, times[1:]):
            if b - a != 50:
                out["no whole step at the grid time before (a hole)"] += 1
                if b - a == 100:
                    holes[a + 50] += 1
                continue
            left, starts = steps[a][1].state(), steps[b][0].state()
            differ = [name for name, x, y in zip(STATE, left, starts) if x != y]
            out["starts with the bytes the whole step before left" if not differ else "differs in " + " + ".join(differ)] += 1
    return out, holes


def parts_against_whole(trace):
    """A part's step and the next whole step of the same controller, where both start from the same bytes."""
    out = collections.Counter()
    last_part = {}
    for time, length, steps in trace.ticks:
        whole = trace.is_whole(time, length)
        for controller, (pre, post) in steps.items():
            if not whole:
                last_part[controller] = (pre, post)
                continue
            before = last_part.pop(controller, None)
            if before is None:
                continue
            ppre, ppost = before
            if (ppre.state(), ppre.last_pos) != (pre.state(), pre.last_pos):
                out["the whole step starts from other bytes"] += 1
                continue
            f = ppre.dt_arg / ppre.own_dt
            if max(abs(w * f - p) for w, p in zip(pre.walk, ppre.walk)) >= 1e-6:
                out["same bytes, another walk vector (a newer input)"] += 1
                continue
            out["same bytes, the part's walk vector is the whole step's times dt / own dt"] += 1
            out["  the part's end pos (+40h) is the whole step's" if ppost.pos == post.pos else
                "  the part's end pos (+40h) is another"] += 1
            on_line = max(abs(a + (b - a) * f - c) for a, b, c in zip(ppre.origin, post.origin, ppost.origin)) < 1e-4
            out["  the part's origin is at dt / own dt of the whole step's way" if on_line else
                "  the part's origin is elsewhere"] += 1
    return out


def show(path, first, count):
    for name, seq, seconds, tid, regs, payload in step_file.records(path):
        if seq < first:
            continue
        if seq >= first + count:
            break
        if name == "atomic_tick":
            length = struct.unpack_from("<3I", payload, 0)[2]
            line = f"atomic_tick to {regs['edi']} ms, {length} ms long"
        elif name == "set_transform":
            caller, controller = struct.unpack_from("<II", payload, 0)
            feet = struct.unpack_from("<3f", payload, 32 + 0x30) if len(payload) >= 32 + 0x40 else ()
            line = f"set_transform {controller:#x} feet{vec(feet)} caller {caller:#x}"
        else:
            s = step_file.Step(payload)
            kind = "whole" if s.whole else "part"
            if name == "pre_step":
                line = (f"pre  {regs['esi']:#x} dt {s.dt_arg:.9g} {kind} origin{vec(s.origin)} last_pos{vec(s.last_pos)} "
                        f"walk{vec(s.walk)} velocity{vec(s.velocity)} jumping {s.jumping} flags {s.flags.hex()}")
            else:
                line = (f"post {regs['esi']:#x} dt {s.dt_arg:.9g} {kind} origin{vec(s.origin)} pos{vec(s.pos)} "
                        f"velocity{vec(s.velocity)} surface{vec(s.surface_normal)} jumping {s.jumping} flags {s.flags.hex()}")
        print(f"{seq:7d} {seconds:8.3f} t{tid:<6d} {line}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("path")
    ap.add_argument("--show", type=int, default=0)
    ap.add_argument("--from", dest="first", type=int, default=0)
    args = ap.parse_args()
    trace = step_file.Trace(args.path)
    steps = sum(len(s) for _, _, s in trace.ticks)
    print(f"{len(trace.ticks)} atomic ticks, match time {trace.ticks[0][0]} .. {trace.ticks[-1][0]} ms, {steps} controller steps, "
          f"{len(trace.moves)} set_transform calls, {len(trace.threads)} thread(s)")
    classes, between = tick_classes(trace)
    for name, n in classes.most_common():
        print(f"  {n:6d}  {name}")
    print("  parts between two whole steps:", dict(sorted(between.items())))
    dts = collections.Counter()
    most = collections.Counter()
    for _, length, tick_steps in trace.ticks:
        most[len(tick_steps)] += 1
        for pre, _ in tick_steps.values():
            # bullet_physics_world::tick: the ms (an integer) times the f32 0.001, stored as an f32
            dts[(pre.dt_arg == f32(length * f32(0.001)), pre.whole)] += 1
    print("  steps whose dt is the tick's ms times 0.001 (f32):", sum(n for (same, _), n in dts.items() if same), "of", steps,
          "; whole (dt is the controller's own):", sum(n for (_, whole), n in dts.items() if whole))
    print("  controller steps in one atomic tick:", dict(sorted(most.items())))
    whole = trace.whole_steps()
    print("whole steps per controller:", sorted(len(s) for s in whole.values()))
    links, holes = chain(whole)
    print("the chain of whole steps:")
    for name, n in links.most_common():
        print(f"  {n:6d}  {name}")
    if holes:
        print("  grid times with no whole step (one-step holes -> controllers):", dict(sorted(holes.items())))
    print("a part against the whole step after it:")
    for name, n in parts_against_whole(trace).items():
        print(f"  {n:6d}  {name}")
    print("updateAction's callers (return addresses):", {(hex(k) if k is not None else "?"): n for k, n in trace.callers.items()})
    movers = collections.Counter(caller for _, _, _, caller in trace.moves)
    print("set_transform's callers (return addresses):", {hex(k): n for k, n in movers.most_common()})
    if args.show:
        show(args.path, args.first, args.show)
    return 0


if __name__ == "__main__":
    sys.exit(main())

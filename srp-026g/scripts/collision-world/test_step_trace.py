"""Tests of the step trace's file and its reports on a made-up trace (no game needed).

    python -m unittest discover -s srp-026g/scripts/collision-world -p 'test_*.py'
"""
import os
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import step_file  # noqa: E402
import step_report  # noqa: E402
import step_vs_recording  # noqa: E402

STOPS = [(0x7F6D0, "set_transform"), (0x7B7A9, "pre_step"), (0x7B7B7, "post_step"), (0x2D1E20, "atomic_tick")]
OWN_DT = struct.pack("<f", 0.05)


def controller(origin, last_pos, walk=(0.0, 0.0, 0.0), pos=None, velocity=(0.0, 0.0, 0.0), jumping=0, crouch=0):
    a = bytearray(step_file.A_HI - step_file.A_LO)
    b = bytearray(step_file.B_HI - step_file.B_LO)
    struct.pack_into("<3f", a, 0x20, *walk)
    struct.pack_into("<3f", a, 0x40, *(pos or origin))
    struct.pack_into("<3f", a, 0x50, *last_pos)
    struct.pack_into("<3f", a, 0xB0, *origin)
    struct.pack_into("<2f", a, 0x1D0, 0.38, 0.52)
    a[0x200], a[0x202] = crouch, 1
    b[4:8] = OWN_DT
    b[0x0C] = jumping
    struct.pack_into("<3f", b, 0x10, 0.0, 1.0, 0.0)
    struct.pack_into("<3f", b, 0x70, *velocity)
    return bytes(a), bytes(b)


class Made:
    """Writes a trace the way the tracer does."""

    def __init__(self, path):
        self.out = open(path, "wb")
        self.writer = step_file.Writer(self.out, STOPS, pid=0)
        self.t = 0.0

    def regs(self, **named):
        return tuple(named.get(r, 0) for r in step_file.REGS)

    def tick(self, time, length):
        self.writer.record(3, self.t, 1, self.regs(edi=time), struct.pack("<8I", 0x2D22E7, 0x1000, length, 0, 0, 0, 0, 0))

    def step(self, ctl, dt, before, after):
        bits = struct.pack("<f", dt)
        frame = struct.pack("<4I", 0, 0x9B848, 0x2000, struct.unpack("<I", bits)[0])
        self.writer.record(1, self.t, 1, self.regs(esi=ctl), step_file.step_payload(bits, *before, frame))
        self.writer.record(2, self.t, 1, self.regs(esi=ctl), step_file.step_payload(bits, *after, frame))

    def move(self, ctl, feet, caller):
        transform = bytearray(0x40)
        struct.pack_into("<3f", transform, 0x30, *feet)
        self.writer.record(0, self.t, 1, self.regs(), struct.pack("<8I", caller, ctl, 0, 0, 0, 0, 0, 0) + bytes(transform))

    def close(self):
        self.writer.close()
        self.out.close()


def walk_line(made, ctl, first_time, count, x0=10.0, step=0.25, look_ahead_at=()):
    """A body walking +x by `step` a whole step; before the whole steps named, a look ahead of 30 ms."""
    x = x0
    last = x0
    for k in range(count):
        time = first_time + 50 * k
        if time in look_ahead_at:
            made.tick(time - 20, 30)
            f = 0.03 / 0.05
            made.step(ctl, 0.03, controller((x, 5.0, 2.0), (last, 5.0, 2.0), walk=(step * f, 0.0, 0.0)),
                      controller((x + step * f, 5.0, 2.0), (x, 5.0, 2.0), walk=(step * f, 0.0, 0.0), pos=(x + step, 5.0, 2.0)))
            made.move(ctl, (x, 4.1, 2.0), 0x2DB939)
        made.tick(time, 50)
        made.step(ctl, 0.05, controller((x, 5.0, 2.0), (last, 5.0, 2.0), walk=(step, 0.0, 0.0)),
                  controller((x + step, 5.0, 2.0), (x, 5.0, 2.0), walk=(step, 0.0, 0.0)))
        last, x = x, x + step


class StepTrace(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = os.path.join(self.dir.name, "steps.bdt")

    def tearDown(self):
        self.dir.cleanup()

    def test_the_file_reads_back_what_was_written(self):
        made = Made(self.path)
        made.tick(1050, 50)
        made.step(0x5000, 0.05, controller((1.0, 2.0, 3.0), (0.5, 2.0, 3.0), walk=(0.5, 0.0, 0.0), jumping=1),
                  controller((1.5, 2.0, 3.0), (1.0, 2.0, 3.0), velocity=(0.0, -0.49, 0.0)))
        made.move(0x5000, (1.5, 1.1, 3.0), 0x2D94C7)
        made.close()
        names = [name for name, *_ in step_file.records(self.path)]
        self.assertEqual(names, ["atomic_tick", "pre_step", "post_step", "set_transform"])
        trace = step_file.Trace(self.path)
        self.assertEqual([(t, n) for t, n, _ in trace.ticks], [(1050, 50)])
        pre, post = trace.ticks[0][2][0x5000]
        self.assertTrue(pre.whole)
        self.assertEqual(pre.walk, (0.5, 0.0, 0.0))
        self.assertEqual(pre.jumping, 1)
        self.assertEqual(pre.ret, 0x9B848)
        self.assertEqual(post.origin, (1.5, 2.0, 3.0))
        self.assertEqual(post.last_pos, (1.0, 2.0, 3.0))
        self.assertAlmostEqual(post.velocity[1], -0.49, places=6)
        self.assertEqual(trace.moves, [(0, 0x5000, (1.5, struct.unpack("<f", struct.pack("<f", 1.1))[0], 3.0), 0x2D94C7)])
        self.assertEqual(trace.callers, {0x9B848: 1})

    def test_the_ticks_are_sorted_by_their_place_on_the_grid(self):
        made = Made(self.path)
        walk_line(made, 0x5000, 1050, 4, look_ahead_at=(1100, 1200))
        made.tick(1220, 20)   # a split step's first part
        made.tick(1250, 30)   # and its second
        made.close()
        trace = step_file.Trace(self.path)
        classes, between = step_report.tick_classes(trace)
        self.assertEqual(classes["a whole grid step"], 4)
        self.assertEqual(classes["a part from the grid that ends inside the step (a look ahead, or a split step's first part)"], 3)
        self.assertEqual(classes["a part that starts inside a step and ends on the grid (a split step)"], 1)
        self.assertEqual(between, {0: 2, 1: 2})

    def test_a_chain_of_whole_steps_and_what_breaks_it(self):
        made = Made(self.path)
        walk_line(made, 0x5000, 1050, 3)
        # a whole step that starts crouched and elsewhere, then a hole of one grid time
        made.tick(1200, 50)
        made.step(0x5000, 0.05, controller((30.0, 5.0, 2.0), (10.5, 5.0, 2.0), crouch=1), controller((30.0, 5.0, 2.0), (30.0, 5.0, 2.0), crouch=1))
        made.tick(1300, 50)
        made.step(0x5000, 0.05, controller((30.0, 5.0, 2.0), (30.0, 5.0, 2.0), crouch=1), controller((30.0, 5.0, 2.0), (30.0, 5.0, 2.0), crouch=1))
        made.close()
        links, holes = step_report.chain(step_file.Trace(self.path).whole_steps())
        self.assertEqual(links["starts with the bytes the whole step before left"], 2)
        self.assertEqual(links["differs in origin + flags"], 1)
        self.assertEqual(links["no whole step at the grid time before (a hole)"], 1)
        self.assertEqual(holes, {1250: 1})

    def test_a_look_ahead_sits_on_the_whole_steps_line(self):
        made = Made(self.path)
        walk_line(made, 0x5000, 1050, 3, look_ahead_at=(1100, 1150))
        made.close()
        parts = step_report.parts_against_whole(step_file.Trace(self.path))
        self.assertEqual(parts["same bytes, the part's walk vector is the whole step's times dt / own dt"], 2)
        self.assertEqual(parts["  the part's end pos (+40h) is the whole step's"], 2)
        self.assertEqual(parts["  the part's origin is at dt / own dt of the whole step's way"], 2)

    def test_an_upload_is_the_origin_one_step_before_the_worlds_time(self):
        made = Made(self.path)
        walk_line(made, 0x5000, 1050, 12)        # origin at grid time t: 10 + 0.25 * (t - 1000) / 50
        made.close()
        whole = step_file.Trace(self.path).whole_steps()
        times = sorted(whole[0x5000])
        # place 3 uploads m_last_pos: the origin one step before each world's time
        rows = [(t, 3, (10.0 + 0.25 * (t - 50 - 1000) / 50, 5.0, 2.0)) for t in range(1100, 1600, 100)]
        place_of, _ = step_vs_recording.controllers_of_places(whole, rows, -50, times[0], times[-1])
        self.assertEqual(place_of, {3: 0x5000})
        laid = step_vs_recording.lay(whole, rows, place_of, times[0], times[-1])
        self.assertTrue(laid)
        for _time, _place, _controller, _centre, found in laid:
            self.assertLess(found["one grid step before the world's time"], 1e-5)
            self.assertGreater(found["at the world's time"], 0.2)


if __name__ == "__main__":
    unittest.main()

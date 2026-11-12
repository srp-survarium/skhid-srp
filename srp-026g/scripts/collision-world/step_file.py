"""A step trace's file (`step_trace.py` writes it): the format, a writer and a reader.

    SRPBDT2\\n, u32 length, that many bytes of JSON ({"stops": [[address, name], ...], ...}),
    then one zlib stream of records.

A record: u8 stop (index into the stops), u32 seq, f64 seconds since the trace began, u32 thread id, the registers
(eax ecx edx ebx esp ebp esi edi eip), u32 payload length, the payload:

- `pre_step`, `post_step`: the dt argument (f32), the controller's bytes +0h..+210h and +2C700h..+2C790h, then
  16 bytes of updateAction's frame (the saved ebp, the return address, the world argument, the dt argument);
- `set_transform`: 32 bytes of the stack (the caller's return address, the controller) and the new transform
  (0x40 bytes: a 3x4 basis and the feet's position at +30h);
- `atomic_tick`: 32 bytes of the stack (the return address, the world core, the step's length in ms); edi is
  the match time the step ends at.

Controller fields are named as the exe names them in `bullet_character_controller::serialize` 0x7F7F0
(docs/notes/body_trace.md).
"""
import json
import struct
import zlib

MAGIC = b"SRPBDT2\n"
A_LO, A_HI = 0x0, 0x210
B_LO, B_HI = 0x2C700, 0x2C790
HEAD = struct.Struct("<BIdI9II")
REGS = ("eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi", "eip")


class Step:
    """The controller's fields at one stop of a step."""

    def __init__(self, payload):
        self.dt_bits = payload[0:4]
        self.dt_arg = struct.unpack_from("<f", payload, 0)[0]
        self.a = payload[4:4 + A_HI - A_LO]
        self.b = payload[4 + A_HI - A_LO:4 + A_HI - A_LO + B_HI - B_LO]
        tail = payload[4 + A_HI - A_LO + B_HI - B_LO:]
        self.ret = struct.unpack_from("<I", tail, 4)[0] if len(tail) >= 8 else None

    def f(self, off, n=3):
        if off >= B_LO:
            return struct.unpack_from(f"<{n}f", self.b, off - B_LO)
        return struct.unpack_from(f"<{n}f", self.a, off - A_LO)

    def raw(self, off, n):
        if off >= B_LO:
            return self.b[off - B_LO:off - B_LO + n]
        return self.a[off - A_LO:off - A_LO + n]

    walk = property(lambda s: s.f(0x20))                 # the walk vector handed to the step (metres over dt)
    air_control = property(lambda s: s.f(0x30))
    pos = property(lambda s: s.f(0x40))                  # the step's end, rounded to a millimetre at its end
    last_pos = property(lambda s: s.f(0x50))             # m_last_pos: where the step began
    origin = property(lambda s: s.f(0xB0))               # the capsule's centre (the ghost's transform)
    dims = property(lambda s: s.f(0x1D0, 2))             # radius, half height
    flags = property(lambda s: s.raw(0x200, 6))          # input, logic crouch, can stand, capsule crouch, sprinting, colliding
    own_dt = property(lambda s: s.f(0x2C704, 1)[0])
    gravity = property(lambda s: s.f(0x2C708, 1)[0])
    jumping = property(lambda s: s.raw(0x2C70C, 1)[0])
    surface_normal = property(lambda s: s.f(0x2C710))    # m_supporting_surface_normal
    ceiling_normal = property(lambda s: s.f(0x2C720))    # m_blocking_ceiling_normal
    ceiling_slide = property(lambda s: s.f(0x2C730))     # m_blocking_ceiling_slide_velocity
    velocity = property(lambda s: s.f(0x2C770))          # m_fall_and_slide_velocity
    whole = property(lambda s: s.dt_bits == s.raw(0x2C704, 4))

    def state(self):
        """What a step starts from and leaves: the bytes the chain of whole steps is checked on."""
        return (self.origin, self.velocity, self.jumping, self.flags, self.surface_normal, self.ceiling_normal,
                self.ceiling_slide, self.dims)


class Writer:
    def __init__(self, out, stops, **meta):
        self.out = out
        self.z = zlib.compressobj(1)
        self.seq = 0
        head = json.dumps({"stops": [[addr, name] for addr, name in stops], **meta}).encode()
        out.write(MAGIC + struct.pack("<I", len(head)) + head)

    def record(self, stop, seconds, tid, regs, payload):
        """regs: the nine registers in `REGS` order."""
        self.out.write(self.z.compress(HEAD.pack(stop, self.seq, seconds, tid, *regs, len(payload)) + payload))
        self.seq += 1

    def close(self):
        self.out.write(self.z.flush())


def step_payload(dt_bits, a, b, frame=b""):
    return dt_bits + a + b + frame


def records(path):
    """Yields (stop name, seq, seconds, thread id, registers, payload)."""
    with open(path, "rb") as f:
        if f.read(8) != MAGIC:
            raise ValueError("not a step trace")
        meta = json.loads(f.read(struct.unpack("<I", f.read(4))[0]))
        data = zlib.decompress(f.read())
    names = [name for _, name in meta["stops"]]
    off = 0
    while off < len(data):
        stop, seq, seconds, tid, *regs, size = HEAD.unpack_from(data, off)
        off += HEAD.size
        yield names[stop], seq, seconds, tid, dict(zip(REGS, regs)), data[off:off + size]
        off += size


class Trace:
    """A trace sorted into what the game did: its atomic ticks and, in each, the controllers' steps."""

    def __init__(self, path):
        self.ticks = []          # [time the tick ends at (ms), its length (ms), {controller: (pre Step, post Step)}]
        self.moves = []          # (index of the tick it came after, controller, feet (x, y, z), caller)
        self.threads = set()
        self.callers = {}
        pre = {}
        for name, _seq, _seconds, tid, regs, payload in records(path):
            self.threads.add(tid)
            if name == "atomic_tick":
                self.ticks.append([regs["edi"], struct.unpack_from("<3I", payload, 0)[2], {}])
            elif name == "set_transform":
                caller, controller = struct.unpack_from("<II", payload, 0)
                feet = struct.unpack_from("<3f", payload, 32 + 0x30) if len(payload) >= 32 + 0x40 else None
                self.moves.append((len(self.ticks) - 1, controller, feet, caller))
            elif name in ("pre_step", "post_step") and len(payload) >= 4 + A_HI - A_LO + B_HI - B_LO and self.ticks:
                step = Step(payload)
                controller = regs["esi"]
                if name == "pre_step":
                    pre[controller] = step
                    self.callers[step.ret] = self.callers.get(step.ret, 0) + 1
                elif controller in pre:
                    self.ticks[-1][2][controller] = (pre.pop(controller), step)

    @staticmethod
    def is_whole(time, length):
        """A whole step of the 50 ms grid (the rest are parts: a look ahead to the frame's time, or a split step)."""
        return length == 50 and time % 50 == 0

    def whole_steps(self):
        """controller -> {the grid time the step ended at: (pre, post)}."""
        out = {}
        for time, length, steps in self.ticks:
            if self.is_whole(time, length):
                for controller, pair in steps.items():
                    out.setdefault(controller, {})[time] = pair
        return out

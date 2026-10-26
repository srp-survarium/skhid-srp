"""The equivalence spec of patch 31's warmup camera and patch 46's death
camera (equiv_026g.py): warmup_look_026g.asm at 0xD40 of `.srp`,
warmup_limits_026g.asm at 0xA98, death_camera_026g.asm in `.srpc` (their
states at 0xE20 and 0xE60 of the late join's state page) before the Rust
port; rust/patches/src/warmup_camera.rs and death_camera.rs after.

- move, 0x25D0E0: the game world's on_mouse_move at its start (ECX the game
  world's input handler, [esp+4] the input world). The original's: the ten
  bytes and on at 0x25D0EA (EAX and EDX are written there before any read:
  `mov eax,[ecx]` / `mov edx,[eax]`; live ECX, EBX, ESI, EDI, EBP); or the
  answer, `ret 10h` (live AL, EBX, ESI, EDI, EBP). Calls: the game world's
  set_pointer and set_cursor_position (its vtable+30h, +3Ch), the window's
  size (the game's +5348h, vtable+1Ch).
- pitch, 0x24225E: process_warmup_mode's `movss [esi+350h],xmm0`; the
  pitch and the distance written, on at 0x242266 (XMM0 is zeroed there,
  `xorps`; EAX and EDX written before read, the asm's own word; live ECX,
  EBX, ESI, EDI, EBP).
- view, 0x1F364B: update_camera's `call 2422B0h` (EBX the player, [esp] its
  input handler, [esp+4] the eyes). The original's: the view's update (the
  asm's tail jump to it, the port's replay of the call: one External), on at
  0x1F3650 with its two arguments popped (live EBX, ESI, EDI, EBP: EAX and
  EDX written there first). The turn: the update in its third-person step
  on the identity, the asm by its own copy of the update's start and a jump
  to 0x2422DE (its `.third`, out of the ranges, a call there the External),
  the port by the update itself with the mode borrowed (the update reads
  the mode once, at 0x2422C6, before 0x2422DE): the same External, its
  model the update's view of the identity by the warmup fields; then the
  product and `ret 8` / the exit popping 8.

The handler's game world (+334h) is never 0 (the handler's constructor
sets it, 0x2410F8): the asm reads through it, the port takes none as the
kits not shown; no case gives none.

The state compares by place: the drag (12 bytes) and the turn (8). The view
cases' floats are finite: with two not-a-number operands the product's
payload may be either one's (the asm multiplies the row's value by the
eyes', the port the eyes' by the row's); no game state has them.
"""
import random
import struct

import equiv_026g as E

HANDLER = 0x20000000                    # the game world's input handler (the world at -1F8h)
WORLD = HANDLER - 0x1F8
WORLD_VT, RENDER_VT = 0x20100000, 0x20100100
INPUT_WORLD, MOUSE = 0x20200000, 0x20200100
PLAYER_INPUT, GAME, CLIENT, LOCAL, RENDER = 0x20300000, 0x20400000, 0x20500000, 0x20600000, 0x20700000
VIEW_WORLD, PLAYER, EYES = 0x20800000, 0x20900000, 0x20A00000
STUBS = 0x30000000
SET_POINTER, SET_CURSOR, WINDOW_SIZE = STUBS, STUBS + 0x10, STUBS + 0x20
VIEW_UPDATE = 0x2422B0
SENSITIVITY, INVERT = 0xA74AEC, 0x1F1D9D3
OLD_LOOK_STATE, OLD_TURN = 0xA428E20, 0xA428E60
OLD_DEATH = 0xA429000                   # .srpc, as the old build has it (death_camera_va)
OLD_THIRD = OLD_DEATH + 0x33F           # its `.third`: the update's own start, then jmp 2422DEh


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def window_size(machine, args, rng):
    w, h = machine.case_window
    machine.uc.mem_write(args[0], struct.pack("<ff", w, h))
    machine.set_reg("eax", args[0])
    clobber(machine)


def clobber(machine):
    machine.set_reg("ecx", machine.rng.getrandbits(32))
    machine.set_reg("edx", machine.rng.getrandbits(32))


def view_update(machine, args, rng):
    """The update's view: of the transform by the warmup yaw and pitch (any
    function of them alike in both builds; finite)."""
    handler = args[0]
    yaw, pitch = struct.unpack("<ff", machine.uc.mem_read(handler + 0x34C, 8))
    t = struct.unpack("<16f", machine.uc.mem_read(machine.read32(machine.reg("esp") + 8), 64))
    view = [f32(f32(v + f32(yaw * 0.5)) + f32(pitch * 0.25)) for v in t]
    machine.uc.mem_write(handler + 0x48, struct.pack("<16f", *view))
    machine.uc.mem_write(handler + 0x35C, b"\0")
    clobber(machine)


UPDATE = E.External("view_update", ["arg0", "arg1:mem:64"], pops=8, model=view_update)
EXTERNALS = {
    SET_POINTER: E.External("set_pointer", ["ecx", "arg0", "arg1", "arg2"], pops=12, model=lambda m, a, r: clobber(m)),
    SET_CURSOR: E.External("set_cursor_position", ["ecx", "arg0", "arg1"], pops=8, model=lambda m, a, r: clobber(m)),
    WINDOW_SIZE: E.External("window_size", ["ecx"], pops=4, model=window_size),
    VIEW_UPDATE: UPDATE,
    OLD_THIRD: UPDATE,                  # the asm's own start of the update: a call of it
}
MOVE = E.Hook("move", 0x25D0E0, ("ecx", "ebx", "esi", "edi", "ebp"))
PITCH = E.Hook("pitch", 0x24225E, ("ecx", "ebx", "esi", "edi", "ebp"), aliases={0x242263: 0x242266})
VIEW = E.Hook("view", 0x1F364B, ("ebx", "esi", "edi", "ebp"), tails=(VIEW_UPDATE,))


class Returns(dict):
    """At a return of on_mouse_move (any exit but 0x25D0EA): the answer, AL."""

    def get(self, at, default=None):
        return default if at == 0x25D0EA else ("al", "ebx", "esi", "edi", "ebp")


MOVE.live_at = Returns()
HOOKS = [MOVE, PITCH, VIEW]
THIRD = bytes.fromhex("81ecd4000000538b9c24e0000000558bac24e00000005657e9")


def old(P):
    return "warmup_look_bin" in dir(P)


def old_pieces(P):
    late_join = open(P.late_join_bin(), "rb").read()
    death = open(P.death_camera_bin(), "rb").read()
    death_va = P.death_camera_va(late_join)
    third = death_va + death.index(THIRD)
    look = open(P.warmup_look_bin(), "rb").read()
    limits = open(P.warmup_limits_bin(), "rb").read()
    return death_va, death, third, look, limits


def ranges(P, linked):
    if old(P):
        death_va, death, third, look, limits = old_pieces(P)
        assert third == OLD_THIRD, hex(third)
        return [(P.WARMUP_LOOK_VA, P.WARMUP_LOOK_VA + len(look)), (P.WARMUP_LIMITS_VA, P.WARMUP_LIMITS_VA + len(limits)),
                (death_va, third), (third + len(THIRD) + 4, death_va + len(death))]
    blocks = [b for b in linked.blocks.values() if b.name in ("patches.code", "engine.code")
              or b.kind == "code" and b.va <= linked.symbols.get("memcpy", 0) < b.va + b.size]
    return [(b.va, b.va + b.size) for b in blocks]


def entries(P, linked):
    if old(P):
        death_va, death, third, look, limits = old_pieces(P)
        return [death_va, death_va + P.DEATH_VIEW_OFFSET, P.WARMUP_LIMITS_VA + P.WARMUP_LIMITS_ENTRY_OFFSET]
    return [linked.symbols[s] for s in ("death_camera_on_mouse_move", "death_camera_on_view_update",
                                        "warmup_camera_on_pitch_store")]


def places(P, linked):
    if old(P):
        return {"drag": (OLD_LOOK_STATE, 12), "turn": (OLD_TURN, 8)}
    named = lambda patch, word: next(va for n, va, _, _ in linked.every if patch in n and word in n)
    return {"drag": (named("warmup_camera", "DRAG"), 12), "turn": (named("death_camera", "TURN"), 8)}


def install():
    if getattr(E.Machine, "_cameras", False):
        return
    run = E.Machine.run

    def wrapped(self, hook, case, seed, old, **more):
        self.case_window = case.window
        return run(self, hook, case, seed, old, **more)
    E.Machine.run, E.Machine._cameras = wrapped, True


install()


class Case(E.Case):
    def __init__(self, what, regs, stack, memory, window=(1920.0, 1080.0)):
        super().__init__(what, regs, stack, memory)
        self.window = window


def regs(rng, **fixed):
    out = {r: rng.getrandbits(32) for r in ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")}
    out["eflags"] = rng.choice([0x202, 0x246, 0x283])
    out.update(fixed)
    return out


def dword(value):
    return struct.pack("<I", value & 0xFFFFFFFF)


def floats(*values):
    return struct.pack(f"<{len(values)}f", *values)


def move_case(rng, what, kits=1, input_=True, mode=3, game=True, client=True, local=True, alive=0,
              buttons=0, x=0, y=0, wheel=0, travel=0, pointer=(300, 200), turn=(0.0, 0.0), sensitivity=1.0,
              invert=0, window=(1920.0, 1080.0), mouse=True):
    memory = {
        HANDLER + 0x2E3: bytes([kits]), HANDLER + 0x4B60: dword(PLAYER_INPUT if input_ else 0),
        HANDLER - 0x15C: dword(GAME if game else 0), WORLD: dword(WORLD_VT),
        HANDLER - 0x16C: dword(pointer[0]) + dword(pointer[1]),
        WORLD_VT + 0x30: dword(SET_POINTER), WORLD_VT + 0x3C: dword(SET_CURSOR),
        GAME + 0x53AC: dword(CLIENT if client else 0), GAME + 0x5348: dword(RENDER),
        RENDER: dword(RENDER_VT), RENDER_VT + 0x1C: dword(WINDOW_SIZE),
        CLIENT + 4: dword(LOCAL if local else 0), LOCAL + 0x33C: bytes([alive]),
        PLAYER_INPUT + 0x358: dword(mode),
        INPUT_WORLD + 0x28: dword(MOUSE if mouse else 0),
        MOUSE + 4: dword(x) + dword(y) + dword(wheel) + bytes([buttons]),
        SENSITIVITY: floats(sensitivity), INVERT: bytes([invert]),
        "drag+0x0": dword(travel) + dword(rng.getrandbits(32)) + dword(rng.getrandbits(32)),
        "turn+0x0": floats(*turn),
    }
    stack = [rng.getrandbits(32), INPUT_WORLD, x & 0xFFFFFFFF, y & 0xFFFFFFFF, wheel & 0xFFFFFFFF] + \
        [rng.getrandbits(32) for _ in range(11)]
    return MOVE, Case(what, regs(rng, ecx=HANDLER), stack, memory, window)


def pitch_case(rng, what, pitch_bits, distance_bits):
    memory = {PLAYER_INPUT + 0x34C: dword(rng.getrandbits(32)) + dword(rng.getrandbits(32)) + dword(distance_bits)}
    r = regs(rng, esi=PLAYER_INPUT)
    r["xmm0"] = rng.getrandbits(96) << 32 | pitch_bits
    return PITCH, Case(what, r, [rng.getrandbits(32) for _ in range(16)], memory)


def eyes(rng):
    return [rng.uniform(-2, 2) for _ in range(12)] + [rng.uniform(-50, 50) for _ in range(3)] + [1.0]


def view_case(rng, what, alive=0, mode=0, kits=1, world=True, turn=(0.5, 0.2)):
    memory = {
        PLAYER + 0x33C: bytes([alive]), PLAYER_INPUT + 0x358: dword(mode),
        PLAYER_INPUT + 0x334: dword(VIEW_WORLD if world else 0), VIEW_WORLD + 0x4DB: bytes([kits]),
        PLAYER_INPUT + 0x34C: floats(rng.uniform(-3, 3), rng.uniform(-1, 1), rng.uniform(0, 5)),
        PLAYER_INPUT + 0x48: bytes(rng.getrandbits(8) for _ in range(64)),
        EYES: floats(*eyes(rng)), "turn+0x0": floats(*turn),
    }
    stack = [PLAYER_INPUT, EYES] + [rng.getrandbits(32) for _ in range(14)]
    return VIEW, Case(what, regs(rng, ebx=PLAYER), stack, memory)


def cases(seed, count):
    rng = random.Random(seed)
    out = []
    LEFT, RIGHT, LOBBY = 0x01, 0x02, 0x52
    for mode in (3, 0):
        dead = dict(mode=mode, alive=0)
        for what, kw in (("no button", dict()), ("the wheel", dict(wheel=120)),
                         ("a left press, a click", dict(buttons=LEFT, x=2, y=-3)),
                         ("a left press past the click", dict(buttons=LEFT, x=7, y=-3)),
                         ("a left press at 8", dict(buttons=LEFT, x=4, y=-4)),
                         ("a left press, the click before", dict(buttons=LEFT, x=1, travel=6)),
                         ("in a drag", dict(buttons=LEFT, x=-40, y=25, travel=0xFFFFFFFF)),
                         ("in a drag, the wheel", dict(buttons=RIGHT, x=3, wheel=120, travel=9)),
                         ("the lobby's buttons at once", dict(buttons=RIGHT, x=3)),
                         ("the 5th button", dict(buttons=0x10, x=-3, y=1)),
                         ("the 7th button", dict(buttons=0x40, x=0, y=0)),
                         ("an other button", dict(buttons=0x04 | 0x08 | 0x20 | 0x80, x=5)),
                         ("a drag's end", dict(travel=0xFFFFFFFF)), ("a drag's end at 9", dict(travel=9)),
                         ("no drag at 8", dict(travel=8)), ("a drag's end, the wheel", dict(travel=12, wheel=-120)),
                         ("abs of the least int", dict(buttons=LEFT, x=-0x80000000, y=0)),
                         ("travel wraps", dict(buttons=LEFT, x=-7, y=3, travel=0xFFFFFFFE & 7)),
                         ("the pitch's limit", dict(buttons=RIGHT, y=-30000, turn=(0.0, 0.9))),
                         ("the yaw's limit", dict(buttons=RIGHT, x=90000, turn=(1.0, 0.0))),
                         ("a not-a-number turn", dict(buttons=RIGHT, x=5, y=5, turn=(float("nan"), float("nan")))),
                         ("inverted", dict(buttons=RIGHT, x=-7, y=-9, invert=1)),
                         ("a slow mouse", dict(buttons=RIGHT, x=12, y=6, sensitivity=0.25)),
                         ("a zero window", dict(buttons=RIGHT, x=1, y=1, window=(0.0, 0.0))),
                         ("y 0", dict(buttons=RIGHT, x=11, y=0))):
            for _ in range(3):
                out.append(move_case(rng, f"mode {mode}, {what}", **dead, **kw))
    for what, kw in (("the kits hidden", dict(kits=0, buttons=RIGHT, x=3)),
                     ("no player input", dict(input_=False, buttons=RIGHT, x=3)),
                     ("a spectator", dict(mode=4, buttons=RIGHT, x=3)), ("third person", dict(mode=2, buttons=RIGHT)),
                     ("alive, mode 0", dict(mode=0, alive=1, buttons=RIGHT, x=3)),
                     ("no game", dict(mode=0, game=False, buttons=RIGHT, x=3)),
                     ("no client", dict(mode=0, client=False, buttons=RIGHT, x=3)),
                     ("no local player", dict(mode=0, local=False, buttons=RIGHT, x=3)),
                     ("alive in the warmup", dict(mode=3, alive=1, buttons=LEFT, x=30))):
        for _ in range(3):
            out.append(move_case(rng, what, **kw))
    for pitch in (0, 0x3F860A92, 0x3F860A93, 0xBF860A93, 0x7F800000, 0x7FC00001, 0xFFC00000, 0x80000000, 0x3F000000,
                  0xBF800000):
        for distance in (0x3F800000, 0x3FC00000, 0x3FBFFFFF, 0x40933333, 0x40933334, 0xBF800000, 0x7FC00000, 0,
                         0x40000000, 0x80000001):
            out.append(pitch_case(rng, f"pitch {pitch:#x} distance {distance:#x}", pitch, distance))
    for what, kw in (("dead, a turn", dict()), ("dead, yaw alone", dict(turn=(0.5, 0.0))),
                     ("dead, pitch alone", dict(turn=(0.0, -0.6))), ("dead, no turn", dict(turn=(0.0, 0.0))),
                     ("dead, a turn of -0.0", dict(turn=(-0.0, -0.0))), ("dead, -0.0 and a pitch", dict(turn=(-0.0, 0.3))),
                     ("alive: cleared", dict(alive=1)), ("alive, no turn", dict(alive=1, turn=(0.0, 0.0))),
                     ("a spectator", dict(mode=4)), ("third person", dict(mode=2)),
                     ("the kits hidden", dict(kits=0))):
        for _ in range(8):
            out.append(view_case(rng, what, **kw))
    yield from out
    for i in range(max(0, count - len(out))):
        kind = i % 3
        if kind == 0:
            yield move_case(rng, "random move", kits=rng.choice([0, 1, 1]), mode=rng.choice([0, 3, 3, 4]),
                            alive=rng.choice([0, 0, 1]), buttons=rng.choice([0, LEFT, RIGHT, LOBBY, rng.getrandbits(8)]),
                            x=rng.randint(-50, 50), y=rng.choice([0, rng.randint(-50, 50)]),
                            wheel=rng.choice([0, 0, 120, -120]), travel=rng.choice([0, 3, 8, 9, 0xFFFFFFFF]),
                            turn=(rng.uniform(-1.3, 1.3), rng.uniform(-0.8, 1.1)),
                            sensitivity=rng.uniform(0.1, 3), invert=rng.choice([0, 1]),
                            window=(rng.choice([800.0, 1920.0, 2560.0]), rng.choice([600.0, 1080.0, 1440.0])))
        elif kind == 1:
            yield pitch_case(rng, "random pitch", rng.getrandbits(32), rng.getrandbits(32))
        else:
            yield view_case(rng, "random view", alive=rng.choice([0, 0, 1]), mode=rng.choice([0, 0, 2, 4]),
                            kits=rng.choice([0, 1, 1]), turn=(rng.choice([0.0, -0.0, rng.uniform(-1.2, 1.2)]),
                                                              rng.choice([0.0, rng.uniform(-0.7, 1.0)])))

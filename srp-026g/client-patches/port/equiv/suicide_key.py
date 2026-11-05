"""The equivalence spec of patch 34, the commit_suicide key (equiv_026g.py;
the asm at 0x980 of `.srp` before the Rust port, rust/suicide_key after).

The hook: the input builder's last action call, 0x242196 `call 0x242460`
(ebx the handler; pushed: the handler, action 22h, &state). It calls the
game's action_state 0x242460 (stdcall: handler, action, &state; al, the
state written) and leaves by `jmp 0x242460` with the builder's arguments;
action_state reads only its stack (0x242460: eax, ecx and edx are loaded
before they are read), so EBX, ESI, EDI, EBP and ESP are live there.

Inputs:
- "stubbed": action_state answered by the stub: absent (al 0), or present
  with a state (0 down, 1 repeat, 2 up, or any dword), al's upper bits and
  the mask (+33Ch) random, edge masks (0, all ones, bit 24 already set);
- "the game's": action_state itself on made-up event buffers (+98h: write
  and read index, 65 pairs {action, state}) and held actions (+2A8h), the
  actions among 1Fh, 22h and others, as check_suicide_key_026g.py's cases.
"""
import random

import equiv_026g as E

HANDLER = 0x20000000
HELD = HANDLER + 0x800
ESP = E.STACK_TOP - 0x8000                 # the run's ESP at the site
STATE = ESP + 0x40                         # the builder's state, in its frame
ACTION_STATE = 0x242460


def stub(machine, args, rng):
    """action_state answered: al and, when present, the state."""
    present = rng.random() < 0.6
    machine.set_reg("eax", rng.getrandbits(32) & ~0xFF | int(present))
    if present:
        state = rng.choice([0, 0, 1, 2, rng.getrandbits(32)])
        machine.write32(args[2], state)


STUBBED = {ACTION_STATE: E.External("action_state", ["arg0", "arg1", "arg2"], pops=12, model=stub)}
REAL = {ACTION_STATE: E.External("action_state", ["arg0", "arg1", "arg2"], pops=12, real=True)}


LIVE = ("ebx", "esi", "edi", "ebp")
HOOKS = [E.Hook("stubbed", 0x242196, LIVE, STUBBED), E.Hook("the game's", 0x242196, LIVE, REAL)]
EXTERNALS = STUBBED


def ranges(P, linked):
    """Where the patch's code is in a build: the asm port's block, or (since
    the Rust framework) the patches crate's block."""
    for name in ("suicide_key.code", "patches.code"):
        if name in linked.blocks:
            block = linked.blocks[name]
            return [(block.va, block.va + block.size)]
    raise SystemExit("no suicide_key.code or patches.code block in this build")


def entries(P, linked):
    return [lo for lo, _ in ranges(P, linked)]


def frame(handler=HANDLER, state=0x5A5A5A5A):
    stack = [0] * 24
    stack[0:3] = [handler, 0x22, STATE]
    stack[(STATE - ESP) // 4] = state
    return stack


def regs(rng, handler=HANDLER):
    return {"ebx": handler, "esi": rng.getrandbits(32), "edi": rng.getrandbits(32), "ebp": rng.getrandbits(32),
            "eax": rng.getrandbits(32), "ecx": rng.getrandbits(32), "edx": rng.getrandbits(32),
            "eflags": rng.choice([0x202, 0x246, 0x283])}


def mask_bytes(mask):
    return {HANDLER + 0x33C: mask.to_bytes(4, "little")}


def buffer(events, held, mask):
    """A handler's event buffer, held actions and mask (check_suicide_key's
    layout: +98h write index, +9Ch read index, then {action, state} pairs)."""
    memory = bytearray(0x1000)
    count = len(events) % 65
    memory[0x98:0x9C] = (count).to_bytes(4, "little")
    memory[0x9C:0xA0] = (0).to_bytes(4, "little")
    for i, (action, state) in enumerate(events[:count]):
        at = 0xA0 + 8 * i
        memory[at:at + 4] = action.to_bytes(4, "little")
        memory[at + 4:at + 8] = state.to_bytes(4, "little")
    memory[0x2A8:0x2AC] = HELD.to_bytes(4, "little")
    memory[0x2AC:0x2B0] = (HELD + 4 * len(held)).to_bytes(4, "little")
    for i, action in enumerate(held):
        memory[0x800 + 4 * i:0x804 + 4 * i] = action.to_bytes(4, "little")
    memory[0x33C:0x340] = mask.to_bytes(4, "little")
    return {HANDLER: bytes(memory)}


def cases(seed, count):
    rng = random.Random(seed)
    stubbed, real = HOOKS
    # (c) edge values: masks, the states the stub gives come from its generator
    for mask in (0, 0xFFFFFFFF, 1 << 24, 0xFEFFFFFF, 0xA1):
        for _ in range(8):
            yield stubbed, E.Case(f"edge mask {mask:#x}", regs(rng), frame(), mask_bytes(mask))
    # (a) check_suicide_key_026g.py's cases, with the game's action_state
    DOWN, REPEAT, UP, SUICIDE, SPOT = 0, 1, 2, 0x1F, 0x22
    checks = [([], [], 0), ([(SUICIDE, DOWN)], [], 0), ([(SUICIDE, REPEAT)], [], 0), ([(SUICIDE, UP)], [], 0),
              ([], [SUICIDE], 0), ([(SPOT, DOWN)], [], 0), ([(SUICIDE, DOWN), (SPOT, DOWN)], [], 0),
              ([(SPOT, UP), (SUICIDE, DOWN)], [], 0), ([(0x0B, DOWN), (SUICIDE, DOWN), (0x26, DOWN)], [0x28], 0xA1),
              ([(0x1E, DOWN)], [], 0x2000)]
    for events, held, mask in checks:
        yield real, E.Case(f"check: {events} held {held}", regs(rng), frame(), buffer(events, held, mask))
    # (b) seeded random states
    actions = [0x1F, 0x1F, 0x22, 0x0B, 0x1E, 0x26, 0x28]
    for i in range(max(0, count - 50)):
        if i % 2:
            events = [(rng.choice(actions), rng.choice([0, 1, 2, 0, rng.getrandbits(32)]))
                      for _ in range(rng.randint(0, 64))]
            held = [rng.choice(actions) for _ in range(rng.randint(0, 3))]
            yield real, E.Case("random buffer", regs(rng), frame(state=rng.getrandbits(32)),
                               buffer(events, held, rng.getrandbits(32)))
        else:
            yield stubbed, E.Case("random stub", regs(rng), frame(state=rng.getrandbits(32)),
                                  mask_bytes(rng.getrandbits(32)))

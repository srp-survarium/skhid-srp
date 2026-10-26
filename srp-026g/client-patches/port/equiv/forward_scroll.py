"""The equivalence spec of patch 28, the transparent effects' UV scrolling
(equiv_026g.py; forward_scroll_026g.asm at 0xB00 of `.srp` before the Rust
port, rust/patches/src/forward_scroll.rs after).

The hooks (sites_026g.tsv):
- forward_compile.switch, 0x1D90DC `mov dl,[esp+56h]; mov esi,[ebp+10h]`
  (EBX the material's config node, the shader configuration at [esp+50h],
  EBP compile's frame): unless [esp+54h] bit 4 (use_sequence), the
  material's use_uv_scrolling (value_exists 0xF6C00, then operator[]
  0xF6B10 for the key and its "value") ors 40h into [esp+5Ch] when its
  dword is not 0; then the two instructions, on at 0x1D90E3;
- forward_compile.constants, 0x1D94CB `mov cl,[esp+57h]; and cl,3`: with
  [esp+5Ch] bit 6, the four numbers (a float as it is, an integer made a
  float, 0 when missing) sent with set_constant<float4> 0x1A8900 (ECX the
  four floats, the compiler [ebp+8], the name; `ret 8`) as
  uv_scrolling_parameters0/1; on at 0x1D94D2;
- renderer_context.scene_time_bound, 0x14990F `mov [esp+2Ch],edi; lea
  esi,[ebx+123Ch]`: the shared string of "scene_slomo" (0xE0B50, stdcall),
  held while register_constant_binding 0x3BCCC0 (EAX the binding, the
  resource manager [226047Ch] pushed, `ret 4`) is given {&1.0, 4, name, 0,
  4}, then let go (0xE0AE0 at a count of 0); on at 0x149919.

Live after each: every register (the asm's pushad/popad, then the replayed
instructions; the Rust lohook's entry restores the machine, then replays).
The flags are not compared: the game writes them again before it reads
them (0x1D90E3's `shl al,6`, 0x1D94D2's `cmp cl,2`, 0x149919's loop's
`cmp eax,edi`).

What is compared beside the gate's own: the texts named to the config's
helpers and to the shared string (our texts are at other addresses in the
two builds), the four floats' bits, and the binding's fields with its
value's bits (the 1.0 is in `.srp` in one, in patches.code in the other).

Inputs: made-up materials (none to all of the keys, a value node of 0, a
switch of 0 or not, floats of any bits, NaNs among them, integers), the
configuration's bits random, seeded random registers and stack; the
shared string pooled or not or none, the binding's name taken by
register_constant_binding or left.
"""
import random
import struct

import equiv_026g as E

MAT = 0x20000000                  # the material's config node (EBX)
KEYS = 0x20001000                 # its keys: 64-byte records, the name then the value node at +30h
FRAME = 0x20003000                # compile's frame (EBP); +8 the compiler
COMPILER = 0x20004000
VALUES = 0x20005000               # value nodes, 20h each
SHARED = 0x20006000               # the shared string 0xE0B50 gives
MANAGER = 0x20007000
RESOURCE_MANAGER = 0x226047C
SLOTS = 10
KEY_EXISTS, CHILD, SET_FLOAT4 = 0xF6C00, 0xF6B10, 0x1A8900
SHARED_STRING, STRING_DESTROY, REGISTER_BINDING = 0xE0B50, 0xE0AE0, 0x3BCCC0
NAMES = (b"use_uv_scrolling", b"transparency_map_scroll_u_direction", b"transparency_map_scroll_v_direction",
         b"emissive_map_scroll_u_direction", b"emissive_map_scroll_v_direction", b"use_sequence", b"other")


def text(machine, at):
    out = bytearray()
    while len(out) < 200:
        byte = machine.uc.mem_read(at + len(out), 1)[0]
        if byte == 0:
            break
        out.append(byte)
    return bytes(out)


def record(machine, name):
    for i in range(SLOTS):
        at = KEYS + 64 * i
        if text(machine, at) == name and machine.read32(at + 0x3C) == 0x4B455921:   # "!YEK": a record
            return at
    return None


def key_exists(machine, args, rng):
    name = text(machine, machine.reg("esi"))
    machine.calls.append(("key_exists: the name", name))
    found = args[0] == MAT and record(machine, name) is not None
    machine.set_reg("eax", rng.getrandbits(32) & ~0xFF | int(found))


def child(machine, args, rng):
    name = text(machine, machine.reg("edi"))
    machine.calls.append(("child: the name", name))
    node = args[0]
    if node == MAT:
        at = record(machine, name)
        if at is None:
            machine.error = f"operator[] of {name!r}, which the material does not have"
            machine.uc.emu_stop()
            return
        machine.set_reg("eax", at)
    elif name == b"value" and KEYS <= node < KEYS + 64 * SLOTS:
        machine.set_reg("eax", machine.read32(node + 0x30))
    else:
        machine.error = f"operator[] of {name!r} on {node:#x}"
        machine.uc.emu_stop()


def set_float4(machine, args, rng):
    machine.calls.append(("float4: the value", bytes(machine.uc.mem_read(machine.reg("ecx"), 16)).hex()))
    machine.set_reg("eax", args[0])


def shared_string(machine, args, rng):
    machine.calls.append(("shared_string: the text", text(machine, machine.read32(machine.reg("esp") + 4))))
    machine.set_reg("eax", SHARED if rng.random() < 0.8 else 0)


def register_binding(machine, args, rng):
    binding = machine.at_call["eax"]           # the binding, on the caller's stack: compared by its fields
    value, size, name, zero, size_again = struct.unpack("<5I", machine.uc.mem_read(binding, 20))
    bits = machine.read32(value)
    machine.calls.append(("register_constant_binding: the binding", (f"{bits:#x}", size, name, zero, size_again),
                          machine.read32(SHARED)))
    if rng.random() < 0.25:                     # the name taken by the callee: read again after it
        machine.write32(binding + 8, 0)


EXTERNALS = {
    KEY_EXISTS: E.External("value_exists", ["eax"], model=key_exists),
    CHILD: E.External("operator[]", ["eax"], model=child),
    SET_FLOAT4: E.External("set_constant<float4>", ["arg0", "arg1"], pops=8, model=set_float4),
    SHARED_STRING: E.External("shared_string", [], pops=4, model=shared_string),
    STRING_DESTROY: E.External("shared string destroy", ["arg0"], pops=4),
    REGISTER_BINDING: E.External("register_constant_binding", ["arg0"], pops=4, model=register_binding),
}
LIVE = ("eax", "ecx", "edx", "ebx", "ebp", "esi", "edi")
SWITCH = E.Hook("forward_compile.switch", 0x1D90DC, LIVE)
CONSTANTS = E.Hook("forward_compile.constants", 0x1D94CB, LIVE)
SLOMO = E.Hook("renderer_context.scene_time_bound", 0x14990F, LIVE)
HOOKS = [SWITCH, CONSTANTS, SLOMO]


def ranges(P, linked):
    """Where the patch's code is in a build: forward_scroll_026g.asm at 0xB00
    of `.srp`, or (the port) the patches crate's block."""
    if "forward_scroll_bin" in dir(P):
        return [(P.SECTION_VA + 0xB00, P.SECTION_VA + 0xB00 + len(open(P.forward_scroll_bin(), "rb").read()))]
    block = linked.blocks["patches.code"]
    return [(block.va, block.va + block.size)]


def entries(P, linked):
    if "forward_scroll_bin" in dir(P):
        return [P.SECTION_VA + 0xB00 + offset for offset in (0, 0x40, 0x100)]
    return [linked.symbols[f"forward_scroll_{h}"] for h in ("on_switch", "on_constants", "on_scene_time_bound")]


def dword(value):
    return struct.pack("<I", value & 0xFFFFFFFF)


def number_bits(rng):
    return rng.choice([0, 0x3F800000, 0xBE000000, 0x7FC00000, 0x7F800001, 0xFFFFFFFF, 0x80000000,
                       rng.getrandbits(32), rng.getrandbits(32)])


def material(rng, keys):
    """{address: bytes} of a material with `keys` {name: (value, kind) or
    None for a value node of 0}."""
    memory = {MAT: bytes(0x18), KEYS: bytes(64 * SLOTS)}
    for i, (name, value) in enumerate(keys.items()):
        at = KEYS + 64 * i
        node = 0 if value is None else VALUES + 0x20 * i
        memory[at] = name.ljust(0x30, b"\0") + dword(node) + bytes(8) + dword(0x4B455921)
        if value is not None:
            bits, kind = value
            memory[node] = dword(bits) + rng.randbytes(0x10) + struct.pack("<H", kind) + bytes(10)
    return memory


def random_keys(rng, names):
    keys = {}
    for name in names:
        if rng.random() < 0.7:
            roll = rng.random()
            if roll < 0.1:
                keys[name] = None
            else:
                kind = 2 if rng.random() < 0.6 else rng.choice([0, 1, 3, 4, 0x102, 0xFFFF])
                bits = number_bits(rng) if kind == 2 else rng.choice([0, 1, -2 & 0xFFFFFFFF, 0x7FFFFFFF,
                                                                       0x80000000, 16777217, rng.getrandbits(32)])
                keys[name] = (bits, kind)
    return keys


def regs(rng, ebx, ebp):
    return {"ebx": ebx, "ebp": ebp, "eax": rng.getrandbits(32), "ecx": rng.getrandbits(32),
            "edx": rng.getrandbits(32), "esi": rng.getrandbits(32), "edi": rng.getrandbits(32),
            "eflags": rng.choice([0x202, 0x246, 0x283, 0xA97])}


def compile_stack(rng, sequence, scrolling):
    stack = [rng.getrandbits(32) for _ in range(0x70 // 4)]
    raw = bytearray(b"".join(dword(v) for v in stack))
    raw[0x54] = raw[0x54] & ~0x10 | (0x10 if sequence else 0)
    raw[0x5C] = raw[0x5C] & ~0x40 | (0x40 if scrolling else 0)
    return list(struct.unpack(f"<{len(raw) // 4}I", raw))


def cases(seed, count):
    rng = random.Random(seed)
    names = list(NAMES)
    fixed = [
        (SWITCH, "no key", False, {}),
        (SWITCH, "use_uv_scrolling 1", False, {b"use_uv_scrolling": (1, 1)}),
        (SWITCH, "use_uv_scrolling 0", False, {b"use_uv_scrolling": (0, 1)}),
        (SWITCH, "use_uv_scrolling of a value node 0", False, {b"use_uv_scrolling": None}),
        (SWITCH, "use_uv_scrolling 1 with use_sequence", True, {b"use_uv_scrolling": (1, 1)}),
        (CONSTANTS, "no number", True, {}),
        (CONSTANTS, "four floats", True, {n: (0x3F800000 + i, 2) for i, n in enumerate(NAMES[1:5])}),
        (CONSTANTS, "four integers", True, {n: (i - 2 & 0xFFFFFFFF, 1) for i, n in enumerate(NAMES[1:5])}),
        (CONSTANTS, "value nodes of 0", True, {n: None for n in NAMES[1:5]}),
        (CONSTANTS, "the bit clear", False, {n: (0x3F800000, 2) for n in NAMES[1:5]}),
    ]
    for hook, what, bit, keys in fixed:
        for _ in range(8):
            memory = material(rng, keys)
            memory[FRAME + 8] = dword(COMPILER)
            stack = compile_stack(rng, bit, rng.random() < 0.5) if hook is SWITCH \
                else compile_stack(rng, rng.random() < 0.5, bit)
            yield hook, E.Case(what, regs(rng, MAT, FRAME), stack, memory)
    for count_ in (0, 1, 2, 5):
        for _ in range(6):
            memory = {RESOURCE_MANAGER: dword(MANAGER), SHARED: dword(count_) + rng.randbytes(12)}
            yield SLOMO, E.Case(f"scene_slomo, a count of {count_}", regs(rng, rng.getrandbits(32), rng.getrandbits(32)),
                                [rng.getrandbits(32) for _ in range(16)], memory)
    for i in range(max(0, count - 104)):
        hook = rng.choice([SWITCH, CONSTANTS, CONSTANTS, SLOMO])
        if hook is SLOMO:
            memory = {RESOURCE_MANAGER: dword(rng.choice([MANAGER, rng.getrandbits(32)])),
                      SHARED: dword(rng.choice([0, 1, 2, rng.getrandbits(32)])) + rng.randbytes(12)}
            yield SLOMO, E.Case("random scene_slomo", regs(rng, rng.getrandbits(32), rng.getrandbits(32)),
                                [rng.getrandbits(32) for _ in range(16)], memory)
            continue
        rng.shuffle(names)
        memory = material(rng, random_keys(rng, names[:rng.randrange(len(names) + 1)]))
        memory[FRAME + 8] = dword(rng.choice([COMPILER, COMPILER, rng.getrandbits(32) | 1]))
        stack = compile_stack(rng, rng.random() < 0.3, rng.random() < 0.6)
        yield hook, E.Case(f"random {hook.name}", regs(rng, MAT, FRAME), stack, memory)

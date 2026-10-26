"""The equivalence spec of the replay viewer's camera after a kit change
(README 24 to 26, its 24; equiv_026g.py): replay_network_client's vtable
slot +54h named a 7-byte thunk patch_026g.py wrote into int3 padding at
0x242A69 (`mov eax,ecx` / `jmp 242A40h`) before the Rust port, the Rust
patch replay_viewer's pointer hook replay_client.place_mine after.

The run: the client respawn rule's kit change from 0x2678F0 (ESI the
replay client; `mov edx,[esi]` / `mov eax,[edx+54h]` / `push edi` / `mov
edi,[esp+10h]` (the place) / `push edi` / `mov ecx,esi` / `call eax`) to its
return 0x2678FF. Both call the game's is_player_current 0x242A40 (EAX the
client, [esp+4] the place; the thunk's a tail jump to it) and answer its 0
or 1 in EAX. Live there: EAX, EBX, ESI, EDI, EBP; ECX and EDX are not
(0x267903 loads EDX, 0x26790E ECX before any read; `test al,al / jz` reads
AL alone).
"""
import random
import struct

import equiv_026g as E

CLIENT, PLAYER = 0x20000000, 0x20001000
IS_PLAYER_CURRENT = 0x242A40


def is_player_current(machine, args, rng):
    """The game's answer (check_replay_reattach_026g.py runs the real one): the
    client's +4 player in the place (its +114h byte against the low byte)."""
    player = machine.read32(args[0] + 4)
    yes = player != 0 and machine.uc.mem_read(player + 0x114, 1)[0] == args[1] & 0xFF
    machine.set_reg("eax", int(yes))


EXTERNALS = {IS_PLAYER_CURRENT: E.External("is_player_current", ["eax", "arg0"], pops=4, model=is_player_current)}
KIT_CHANGE = E.Hook("kit change", 0x2678F0, ("eax", "ebx", "esi", "edi", "ebp"), tails=(IS_PLAYER_CURRENT,))
HOOKS = [KIT_CHANGE]


def ranges(P, linked):
    if "REPLAY_REATTACH_THUNK" in dir(P):
        return [(0x242A69, 0x242A70)]
    blocks = [b for b in linked.blocks.values() if b.name == "patches.code" or b.name == "engine.code"]
    return [(b.va, b.va + b.size) for b in blocks]


def entries(P, linked):
    if "REPLAY_REATTACH_THUNK" in dir(P):
        return [0x242A69]
    return [linked.symbols["replay_viewer_on_place_mine"]]


def case(rng, what, player_place, place):
    memory = {CLIENT: struct.pack("<II", 0x9FF344, 0 if player_place is None else PLAYER)}
    if player_place is not None:
        memory[PLAYER + 0x114] = bytes([player_place])
    regs = {r: rng.getrandbits(32) for r in ("eax", "ecx", "edx", "ebx", "edi", "ebp")}
    regs["esi"] = CLIENT
    regs["eflags"] = rng.choice([0x202, 0x246, 0x283])
    stack = [rng.getrandbits(32) for _ in range(16)]
    stack[3] = place
    return KIT_CHANGE, E.Case(what, regs, stack, memory)


def cases(seed, count):
    rng = random.Random(seed)
    for player_place, place in ((3, 3), (3, 4), (0, 0), (None, 0), (0xFF, 0xFF), (5, 0x105), (5, 0x205)):
        for _ in range(5):
            yield case(rng, f"player {player_place} place {place:#x}", player_place, place)
    for _ in range(max(0, count - 35)):
        player_place = rng.choice([None, rng.randrange(256)])
        place = rng.choice([player_place or 0, rng.getrandbits(32), rng.randrange(256)])
        yield case(rng, "random client", player_place, place)

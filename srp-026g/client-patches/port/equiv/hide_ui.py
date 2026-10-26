"""The equivalence spec of patch 62, one key hides the interface
(equiv_026g.py; hide_ui_026g.asm, the arena's blocks hide_ui.code and
hide_ui.data, before the Rust port; rust/patches/src/hide_ui.rs after).

- match, 0x25CED1: the game world's case of action 19h (`mov esi,[esp+10h]`
  / `cmp dword [esi+18h],0` / `jz 25CF0Dh`). With the replay's movie: on at
  0x25CEDB with ESI the handler (the asm's own `mov esi`, the port's
  replay); live ESI, EBX, EDI, EBP (EAX, ECX, EDX are written there first).
  Without: the hud movie asked root.interface_hidden, the hud and icons
  movies root.hide_interface(not that); on at 0x25CF0D, which pops EDI, ESI,
  EBX: live EBP alone.
- lobby, 0x26CCD2: the call of get_binded_action (EAX the binder, EDI 2,
  ESI the key, [esp] the toggle's place; the callee pops it). The asm is
  called there and returns `ret 4`; the port's exits pop it. Back at
  0x26CCD7 with EAX the lobby's answer (live EAX, EBX, ESI, EDI, EBP), or at
  0x26CCAE (taken) or 0x26CDD2 (to the movie) (live EBX, ESI, EDI, EBP).
- reset, 0x234AE8: initialize_match's Invoke of root.reset (ECX the movie,
  four arguments the callee pops; EBP the hud): the Invoke, the hud's and
  icons' root.hide_interface(false); back at 0x234AED (`mov eax,[ebp+10h]`
  there: live EBX, ESI, EDI, EBP).

The game's get_binded_action and Invoke are stubs; an Invoke's result and
arguments compare by whether there are some and what they hold.
"""
import random
import struct

import equiv_026g as E

HANDLER, LOBBY, HUD, BINDER = 0x20000000, 0x20100000, 0x20200000, 0x20300000
HOLDERS = 0x20400000                    # a movie holder 0x100 apart: +108h its slot, the slot's +4 the movie
MOVIES = 0x20500000
NAME, RESULT, ARGS = 0x20600000, 0x20600100, 0x20600200
TOGGLE = 0x20700000
INVOKE, BINDED = 0x511600, 0x207D50
NO_ACTION, TOGGLE_UI = 0x49, 0x19


def clobber(machine):
    machine.set_reg("ecx", machine.rng.getrandbits(32))
    machine.set_reg("edx", machine.rng.getrandbits(32))


def invoke(machine, args, rng):
    """Movie::Invoke: root.interface_hidden answers the case's bool for that
    movie; any other method nothing."""
    method, result = args[1], machine.read32(machine.reg("esp") + 8)
    if method == "root.interface_hidden" and result:
        hidden = machine.case_hidden.get(args[0], 0)
        machine.uc.mem_write(result, struct.pack("<6I", 0, 2, hidden | rng.getrandbits(24) << 8, 0, 0, 0))
    machine.set_reg("eax", rng.getrandbits(32))
    clobber(machine)


def result_input(machine):
    at = machine.read32(machine.reg("esp") + 8)
    return at if at == RESULT else bool(at)


def args_input(machine):
    esp = machine.reg("esp")
    at, count = machine.read32(esp + 12), machine.read32(esp + 16)
    if at == ARGS or not at or not count:
        return at if at == ARGS else bool(at)
    return bytes(machine.uc.mem_read(at, 24 * min(count, 4))).hex()


def binded(machine, args, rng):
    """get_binded_action: the case's answers by (kinds, key); its toggle
    written."""
    machine.set_reg("eax", machine.case_actions.get((args[1], args[2]), NO_ACTION))
    toggle = machine.read32(machine.reg("esp") + 4)
    if toggle:
        machine.write32(toggle, rng.getrandbits(1))
    clobber(machine)


def toggle_input(machine):
    at = machine.read32(machine.reg("esp") + 4)
    return at if at == TOGGLE else "a place of the patch's"


EXTERNALS = {
    INVOKE: E.External("invoke", ["ecx", "arg0:str", result_input, args_input, "arg3"], pops=16, model=invoke),
    BINDED: E.External("get_binded_action", ["eax", "edi", "esi", toggle_input], pops=4, model=binded),
}
MATCH = E.Hook("match", 0x25CED1, ("esi", "ebx", "edi", "ebp"), live_at={0x25CF0D: ("ebp",)})
LOBBY_HOOK = E.Hook("lobby", 0x26CCD2, ("ebx", "esi", "edi", "ebp"), live_at={0x26CCD7: ("eax", "ebx", "esi", "edi", "ebp")})
RESET = E.Hook("reset", 0x234AE8, ("ebx", "esi", "edi", "ebp"))
HOOKS = [MATCH, LOBBY_HOOK, RESET]


def old(P):
    return "HIDE_UI_HOOKS" in dir(P)


def ranges(P, linked):
    names = ("hide_ui.code", "engine.code") if old(P) else ("patches.code", "engine.code")
    return [(b.va, b.va + b.size) for b in linked.blocks.values() if b.name in names]


def entries(P, linked):
    if old(P):
        return [linked.symbols[s] for s in ("hide_ui_match", "hide_ui_lobby", "hide_ui_reset")]
    return [linked.symbols[s] for s in ("hide_ui_on_match_toggle", "hide_ui_on_lobby_action", "hide_ui_on_match_reset")]


def install():
    if getattr(E.Machine, "_hide_ui", False):
        return
    run = E.Machine.run

    def wrapped(self, hook, case, seed, old):
        self.case_hidden, self.case_actions = case.hidden, case.actions
        return run(self, hook, case, seed, old)
    E.Machine.run, E.Machine._hide_ui = wrapped, True


install()


class Case(E.Case):
    def __init__(self, what, regs, stack, memory, hidden=None, actions=None):
        super().__init__(what, regs, stack, memory)
        self.hidden, self.actions = dict(hidden or {}), dict(actions or {})


def regs(rng, **fixed):
    out = {r: rng.getrandbits(32) for r in ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")}
    out["eflags"] = rng.choice([0x202, 0x246, 0x283])
    out.update(fixed)
    return out


def dword(value):
    return struct.pack("<I", value & 0xFFFFFFFF)


def movie(memory, rng, index, present=True, slot=True):
    """A movie holder (its slot, the slot's movie); its address, the movie."""
    holder = HOLDERS + 0x100 * index
    the_movie = MOVIES + 0x100 * index
    slot_at = holder + 0x80
    memory[holder + 0x108] = dword(slot_at if slot else 0)
    memory[slot_at + 4] = dword(the_movie if present else 0)
    return holder, the_movie


def match_case(rng, what, replay=False, hud=True, icons=True, hud_slot=True, hidden=0):
    memory = {HANDLER + 0x18: dword(rng.getrandbits(32) | 1 if replay else 0)}
    hidden_map = {}
    if hud:
        holder, m = movie(memory, rng, 0, slot=hud_slot)
        memory[HANDLER + 4 + 8] = dword(holder)
        hidden_map[m] = hidden
    else:
        memory[HANDLER + 4 + 8] = dword(0)
    if icons:
        holder, m = movie(memory, rng, 1)
        memory[HANDLER + 4 + 0x10] = dword(holder)
    else:
        memory[HANDLER + 4 + 0x10] = dword(0)
    stack = [rng.getrandbits(32) for _ in range(4)] + [HANDLER] + [rng.getrandbits(32) for _ in range(11)]
    return MATCH, Case(what, regs(rng), stack, memory, hidden=hidden_map)


def lobby_case(rng, what, key, lobby_action, match_action, movie_there=True, hidden=0):
    memory = {TOGGLE: dword(rng.getrandbits(32))}
    hidden_map = {}
    if movie_there:
        holder, m = movie(memory, rng, 2)
        memory[LOBBY + 0x690] = dword(holder)
        hidden_map[m] = hidden
    else:
        memory[LOBBY + 0x690] = dword(0)
    actions = {(2, key): lobby_action, (1, key): match_action}
    stack = [TOGGLE] + [rng.getrandbits(32) for _ in range(15)]
    return LOBBY_HOOK, Case(what, regs(rng, eax=BINDER, edi=2, esi=key, ebx=LOBBY), stack, memory, hidden=hidden_map,
                            actions=actions)


def reset_case(rng, what, hud=True, icons=True):
    memory = {NAME: b"root.reset\0", RESULT: bytes(24), ARGS: bytes(rng.getrandbits(8) for _ in range(48))}
    holder, m = movie(memory, rng, 3, present=hud)
    memory[HUD + 8] = dword(holder)
    holder, m = movie(memory, rng, 4, present=icons)
    memory[HUD + 0x10] = dword(holder)
    stack = [NAME, RESULT, ARGS, rng.randrange(0, 3)] + [rng.getrandbits(32) for _ in range(12)]
    return RESET, Case(what, regs(rng, ecx=MOVIES + 0x700, ebp=HUD), stack, memory)


def cases(seed, count):
    rng = random.Random(seed)
    out = []
    for what, kw in (("a replay", dict(replay=True)), ("shown", dict(hidden=0)), ("hidden", dict(hidden=1)),
                     ("hidden, another byte", dict(hidden=0x80)), ("no hud movie", dict(hud=False)),
                     ("no hud slot", dict(hud_slot=False)), ("no icons", dict(icons=False)),
                     ("no movie at all", dict(hud=False, icons=False))):
        for _ in range(4):
            out.append(match_case(rng, f"match: {what}", **kw))
    for key, la, ma in ((0x57, NO_ACTION, TOGGLE_UI), (0x57, NO_ACTION, 0x20), (0x41, 0x40, TOGGLE_UI),
                        (0x40, 0x44, 0x07), (0x1C, NO_ACTION, NO_ACTION)):
        for there in (True, False):
            for hidden in (0, 1):
                for _ in range(2):
                    out.append(lobby_case(rng, f"lobby: key {key:#x}, {la:#x}/{ma:#x}, movie {there}, hidden {hidden}",
                                          key, la, ma, there, hidden))
    for hud in (True, False):
        for icons in (True, False):
            for _ in range(3):
                out.append(reset_case(rng, f"reset: hud {hud}, icons {icons}", hud, icons))
    yield from out
    for i in range(max(0, count - len(out))):
        kind = i % 3
        if kind == 0:
            yield match_case(rng, "random match", replay=rng.random() < 0.2, hud=rng.random() < 0.9,
                             icons=rng.random() < 0.9, hidden=rng.choice([0, 1, rng.getrandbits(8)]))
        elif kind == 1:
            yield lobby_case(rng, "random lobby", rng.getrandbits(8), rng.choice([NO_ACTION, rng.randrange(0x60)]),
                             rng.choice([TOGGLE_UI, NO_ACTION, rng.randrange(0x60)]), rng.random() < 0.8,
                             rng.choice([0, 1]))
        else:
            yield reset_case(rng, "random reset", rng.random() < 0.8, rng.random() < 0.8)

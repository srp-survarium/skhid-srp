"""The equivalence spec of patch 58, the kit keys and the match chat option
(equiv_026g.py; match_keys_026g.asm in the arena's blocks match_keys.code and
match_keys.state before the Rust port, rust/patches/src/match_keys.rs after).

The hooks:
- the keys, on_keyboard_action's kind test 0x25CD41 (`cmp dword
  [ebp+10h],1 / jnz near 0x25CF0D`; EDI the input handler, EBP the frame):
  the asm (jumped to) redid the test and left by 0x25CF0D or 0x25CD4B; the
  port's lohook replays it (the jnz made again) to the same two. Every
  register is live at both (the asm's pushad/popad kept them all, so does
  the lohook entry). Invoke 0x511600 (through engine_movie_invoke, ECX the
  movie) compared by the movie, the method's name, the result, the
  argument's six dwords and the count;
- the hud, the game world's constructor's `call 0x233810` (0x1F6845, the
  hud's constructor: EAX the world, the hud pushed, `ret 4`): the asm was
  called in its place, called it itself and registered; the port leaves
  the call to the game and registers at 0x1F684A (a lohook replaying `mov
  [ebp+1D88h],ebx`). The constructor is the same stub in both (called from
  the patch, or from the game: from_game). Both runs end at 0x1F6850, every
  register live. cc_value<bool>'s constructor 0x1DDB0 (through
  engine_cc_bool) compared by its object and byte as places of the patch's
  state, the name's text, saved and kind; the state compared by place (the
  asm's chat_variable, hide_chat, registered; the crate's HIDE_CHAT, the
  same layout);
- the chat, process_incoming_text_message's `call 0x208AD0` (add_message,
  stdcall, 5 arguments, `ret 14h`): the asm dropped the line (`ret 14h`) or
  tail-jumped to it; the port is a call hook that calls it or not. Both end
  at 0x26B91B. Live there: EBX, ESI, EDI, EBP; EAX, ECX and EDX are
  add_message's to clobber (a stdcall function's, the original's call), and
  0x26B91B loads EDX (`mov edx,[ebx+8]`) and reads neither EAX nor ECX
  before the next call (0x209330).
"""
import random

import equiv_026g as E

HANDLER, HUD_OWNER, SLOT, MOVIE = 0x20000000, 0x20001000, 0x20002000, 0x20003000
CHAT = 0x20004000
FRAME = E.STACK_TOP - 0x8000 + 0x40          # on_keyboard_action's ebp, above the site's esp
INVOKE, HUD_CONSTRUCTOR, CC_BOOL, ADD_MESSAGE = 0x511600, 0x233810, 0x1DDB0, 0x208AD0
PLACE = "match_keys.chat"                     # the variable: the command, +48h its byte, +49h made


def text_at(slot):
    return lambda machine: machine.text(machine.read32(machine.reg("esp") + 4 + 4 * slot))


def place_of(read):
    return lambda machine: machine.normal(read(machine))


def argument(machine):
    at = machine.read32(machine.reg("esp") + 12)
    return [machine.read32(at + 4 * i) for i in range(6)] if at else None


EXTERNALS = {
    INVOKE: E.External("Movie::Invoke", ["ecx", text_at(0), "arg1", argument, "arg3"], pops=16),
    HUD_CONSTRUCTOR: E.External("the hud's constructor", ["eax", "arg0"], pops=4, from_game=True),
    CC_BOOL: E.External("cc_value<bool>", [place_of(lambda m: m.reg("eax")), text_at(0),
                                           place_of(lambda m: m.read32(m.reg("esp") + 8)), "arg2", "arg3"],
                        pops=16),
    ADD_MESSAGE: E.External("add_message", ["arg0", "arg1", "arg2", "arg3", "arg4"], pops=20, from_game=True),
}
ALL = ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")
KEYS = E.Hook("the keys", 0x25CD41, ALL)
HUD = E.Hook("the hud", 0x1F6845, ALL, until=0x1F6850)
CHAT_HOOK = E.Hook("the chat", 0x26B916, ("ebx", "esi", "edi", "ebp"), until=0x26B91B)
HOOKS = [KEYS, HUD, CHAT_HOOK]


def ranges(P, linked):
    """Where the patch's code is: match_keys_026g.asm's block, or (the port)
    the patches crate's block."""
    for name in ("match_keys.code", "patches.code"):
        if name in linked.blocks:
            block = linked.blocks[name]
            return [(block.va, block.va + block.size)]


def entries(P, linked):
    if "match_keys.code" in linked.blocks:
        return [linked.symbols[s] for s in ("match_keys_keyboard", "match_keys_hud", "match_keys_chat")]
    return [linked.symbols[s] for s in ("match_keys_on_kind_test", "match_keys_on_hud_made",
                                        "match_keys_on_add_message")]


def places(P, linked):
    """The option's variable: the asm's chat_variable (48h), hide_chat and
    registered after it; the crate's HIDE_CHAT (vostok::console::ConsoleBool,
    the same layout)."""
    if "chat_variable" in linked.symbols:
        assert linked.symbols["hide_chat"] == linked.symbols["chat_variable"] + 0x48
        return {PLACE: (linked.symbols["chat_variable"], 0x4A)}
    va = next(va for name, va, _, _ in linked.every if name.endswith("10match_keys9HIDE_CHAT"))
    return {PLACE: (va, 0x4A)}


def dword(value):
    return (value & 0xFFFFFFFF).to_bytes(4, "little")


def regs(rng, **fixed):
    out = {r: rng.getrandbits(32) for r in E.REG_NAMES}
    out["eflags"] = rng.choice([0x202, 0x246, 0x283])
    out.update(fixed)
    return out


def key_case(rng, what, key, kind, cursor, owner, slot, movie):
    memory = {HANDLER + 0x2E0: bytes([cursor]), HANDLER + 0xC: dword(HUD_OWNER if owner else 0),
              HUD_OWNER + 0x108: dword(SLOT if slot else 0), SLOT + 4: dword(MOVIE if movie else 0)}
    stack = [rng.getrandbits(32) for _ in range(24)]      # the frame (ebp) is 40h above the site's esp
    stack[(0x40 + 0xC) // 4], stack[(0x40 + 0x10) // 4] = key & 0xFFFFFFFF, kind & 0xFFFFFFFF
    return KEYS, E.Case(what, regs(rng, edi=HANDLER, ebp=FRAME), stack, memory)


def hud_case(rng, what, made):
    stack = [0x20005000] + [rng.getrandbits(32) for _ in range(15)]     # the hud pushed
    memory = {PLACE + "+0x0": bytes(rng.getrandbits(8) for _ in range(0x48)) if made else bytes(0x48),
              PLACE + "+0x48": bytes([rng.getrandbits(8)]), PLACE + "+0x49": bytes([made])}
    return HUD, E.Case(what, regs(rng, eax=0x20006000), stack, memory)


def chat_case(rng, what, hide, in_match, channel):
    stack = [CHAT, channel] + [rng.getrandbits(32) for _ in range(14)]
    memory = {CHAT + 0xD: bytes([in_match]), PLACE + "+0x48": bytes([hide])}
    return CHAT_HOOK, E.Case(what, regs(rng), stack, memory)


def cases(seed, count):
    rng = random.Random(seed)
    # check_match_keys_026g.py's cases
    for key in (1, 2, 3, 4, 5, 0x1C):
        for kind in (1, 2, 3):
            for cursor in (0, 1):
                for movie in (True, False):
                    yield key_case(rng, f"key {key} kind {kind} cursor {cursor} movie {movie}", key, kind, cursor,
                                   True, True, movie)
    for made in (0, 1, 0, 1):
        yield hud_case(rng, f"the hud, made {made}", made)
    for hide in (0, 1):
        for in_match in (0, 1):
            for channel in range(10):
                yield chat_case(rng, f"chat {hide} {in_match} {channel}", hide, in_match, channel)
    # edges: no movie owner, no slot, odd kinds and bytes
    for owner, slot in ((False, True), (True, False)):
        yield key_case(rng, "no owner or slot", 3, 1, 1, owner, slot, True)
    for i in range(max(0, count - 118)):
        which = i % 3
        if which == 0:
            yield key_case(rng, "random key", rng.choice([1, 2, 3, 4, 5, 0x1C, rng.getrandbits(32)]),
                           rng.choice([1, 1, 2, 3, 0, rng.getrandbits(32)]), rng.choice([0, 1, 0x80]),
                           rng.random() < 0.9, rng.random() < 0.9, rng.random() < 0.9)
        elif which == 1:
            yield chat_case(rng, "random chat", rng.choice([0, 1, 0xFF]), rng.choice([0, 1, 0x40]),
                            rng.choice(list(range(11)) + [rng.getrandbits(32)]))
        else:
            yield hud_case(rng, "random hud", rng.choice([0, 1, 0x20]))

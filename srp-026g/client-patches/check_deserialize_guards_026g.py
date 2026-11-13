#!/usr/bin/env python3
"""Compare patch 115's actual native entries with stock, including absent ids.

    python check_deserialize_guards_026g.py ORIGINAL_EXE

The generated hooks run at four stack alignments. Valid reads must preserve all
registers, flags, XMM state and stack results. Invalid ids never dereference past
an anomaly/artefact vector or call a null FSM state.
"""
import argparse
import re
import struct
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
from check_world_check_026g import Run, WORLD, STACK, STUBS, FRAME

REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
        UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP,
        UC_X86_REG_EFLAGS, UC_X86_REG_XMM0, UC_X86_REG_XMM7)
FSM, ANOMALY, ARTEFACT = 0x38CCF0, 0x31B0FF, 0x2FE65E
FSM_END, ANOMALY_END, ARTEFACT_END = 0x38CD37, 0x31B108, 0x2FE663
# get_death_animations(set, out, index): both death lists indexed by index.
DEATH, DEATH_RETURN = 0x338280, STUBS + 0x200
# A slot past a list: a pointer to nothing, which the stock takes a reference
# through (lock xadd [slot+0DCh]) and faults on.
PAST = 0x7F000000
# Each in-list slot names a resource of its own, its reference count at +0DCh.
REFS = 100
# The fire deserialize's store of m_weapon_animation_index (mov [esi+16Ch],cl)
# and the instruction after it.
FIRE, FIRE_END = 0x310224, 0x31022A
# is_ready_for_transition (ECX the throw state) and the throw state's throw
# (ESI); grenade_set_core::throw_grenade, where the throw hands the index on;
# the two calls before it, stubbed (each ret 4: the fixture has no player).
READY, THROW, SET_THROW = 0x33A650, 0x33A770, 0x30A250
# throw_grenade past its first reference: the slot's grenade taken
# (0x2E9040: *esi = *edi, lock xadd [grenade+0F0h]).
SET_THROW_TAKEN = 0x30A27C
THROW_STUBS = (0x309F00, 0x727B0)
GRENADE_RETURN = STUBS + 0x210
# The two grenade set vtables (grenade_set, grenade_set_core) and another
# item's (a weapon's), which a world can name in the grenade slot.
GRENADE_SETS, NOT_A_GRENADE_SET = (0xA0B56C, 0xA14824), 0xA04174
# The animation-end callbacks that compare [[state+130h]+index*4]: every
# animation-end-aware state's, the throw substate's, the melee state's.
WAIT_END, WAIT_THROW, WAIT_MELEE = 0x30DB60, 0x310680, 0x310CF0
# Where each goes on a match: the state's on_animation_end_impl (vtable
# +24h, a stub in the fixture's copy of the vtable), the throw 0x33A770,
# the melee's 0x3921A0.
MATCHED = {WAIT_END: STUBS + 0x220, WAIT_THROW: 0x33A770, WAIT_MELEE: 0x3921A0}
WAIT_RETURN = STUBS + 0x230
# Each register_animations (vtable +1Ch): the state's handles, from
# [state+130h], contiguous (read off the registrators 0x24B700..0x24B958).
LAYOUTS = {0x24B700: (0x13C, 5), 0x24B760: (0x144, 5), 0x24B7C0: (0x144, 9), 0x24B820: (0x144, 10),
           0x24B880: (0x140, 4), 0x24B8C0: (0x138, 5), 0x24B920: (0x144, 4)}
# A page of its own: the state's table ends at its end, so a read past the
# handles faults.
EDGE = 0x45000000

class GuardRun(Run):
    def __init__(self, data):
        super().__init__(data)
        for at in (FSM_END, ANOMALY_END, ARTEFACT_END, DEATH_RETURN, FIRE_END, GRENADE_RETURN, SET_THROW_TAKEN):
            self.emu.hook_add(UC_HOOK_CODE, self.stop, begin=at, end=at)
        for at in THROW_STUBS:
            self.emu.hook_add(UC_HOOK_CODE, self.ret4, begin=at, end=at)
        self.emu.hook_add(UC_HOOK_CODE, self.state_reader, begin=STUBS + 0x90, end=STUBS + 0x90)
        self.at_set_throw = False
        self.emu.hook_add(UC_HOOK_CODE, self.set_throw, begin=SET_THROW, end=SET_THROW)
        self.matched = None
        for at in set(MATCHED.values()) | {WAIT_RETURN}:
            self.emu.hook_add(UC_HOOK_CODE, self.wait_stop, begin=at, end=at)
        self.emu.mem_map(EDGE, 0x1000, UC_PROT_ALL)

    def wait_stop(self, emu, address, size, _):
        """A callback's match (when that is the one awaited) or its return."""
        if address in (self.matched, WAIT_RETURN):
            self.stop(emu, address, size, _)

    def ret4(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_ESP, sp + 8)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def set_throw(self, emu, address, size, _):
        """grenade_set_core::throw_grenade's entry: the throw stops there,
        a run of throw_grenade itself goes on."""
        if self.at_set_throw:
            self.stop(emu, address, size, _)

    def state_reader(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.fsm_calls.append((emu.reg_read(UC_X86_REG_ECX),
                               tuple(self.read(sp + 4*i) for i in (1,2,3))))
        emu.reg_write(UC_X86_REG_EAX, 0x1234)
        emu.reg_write(UC_X86_REG_ESP, sp + 16)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def guard(self, kind, index, align, count=3):
        self.emu.mem_write(WORLD, bytes(0x20000))
        self.emu.mem_write(STACK, bytes(0x10000))
        self.fsm_calls = []
        obj, table, reader = WORLD, WORLD + 0x1a000, WORLD + 0x1b000
        regs = {r: 0x11111111*(i+1) for i,r in enumerate(REGS[:7])}
        sp = FRAME - 0x100 + align
        self.emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        self.emu.reg_write(UC_X86_REG_XMM0, 0x123456789abcdef)
        self.emu.reg_write(UC_X86_REG_XMM7, 0xfedcba987654321)
        for n in range(count):
            node = table + 0x100 + n*0x20
            self.word(table + n*4, node)
            self.word(node, table + 0x500)
            self.word(node+4, node+0x20 if n+1<count else 0)
        self.word(table+0x500+0x14, STUBS+0x90)
        if kind == FSM:
            self.word(obj+8, table+0x100 if count else 0)
            self.word(reader+4, reader+0x20)
            self.emu.mem_write(reader+0x20, bytes([index]))
            self.word(sp+4, 0x11)
            self.word(sp+8, 0x22)
            regs.update({UC_X86_REG_ESI: obj, UC_X86_REG_EDI: reader})
        elif kind == ANOMALY:
            self.word(obj+0x68, table)
            self.word(obj+0x6c, table+count*4)
            regs.update({UC_X86_REG_EDI: obj, UC_X86_REG_ECX: index})
        else:
            self.word(obj+0x18940, table)
            self.word(obj+0x18944, table+count*4)
            regs.update({UC_X86_REG_ECX: obj, UC_X86_REG_EAX: index, UC_X86_REG_EDX: table})
        self.start(kind, regs, sp)
        return self.stopped, tuple(self.emu.reg_read(r) for r in REGS), self.fsm_calls

    def death(self, index, align, count=2):
        """get_death_animations from a call with `index`, the set's two lists
        `count` long, each slot a resource of its own and every slot past them
        PAST: the result is the out pair and every resource's count, so which
        slot was read shows."""
        self.emu.mem_write(WORLD, bytes(0x20000))
        self.emu.mem_write(STACK, bytes(0x10000))
        self.fsm_calls = []
        sets, death, hud, out = WORLD, WORLD + 0x1a000, WORLD + 0x1b000, WORLD + 0x1c000
        resources = [WORLD + 0x1d000 + 0x100*(t*4 + n) for t in range(2) for n in range(count)]
        for t, table in enumerate((death, hud)):
            for n in range(count):
                self.word(resources[t*count + n] + 0xDC, REFS)
                self.word(table + n*4, resources[t*count + n])
            for n in range(count, 256):
                self.word(table + n*4, PAST)
        self.word(sets + 0x590, death)
        self.word(sets + 0x594, count)
        self.word(sets + 0xA20, hud)
        self.word(sets + 0xA24, count)
        regs = {r: 0x11111111*(i+1) for i,r in enumerate(REGS[:7])}
        sp = FRAME - 0x100 + align
        for n, value in enumerate((DEATH_RETURN, sets, out, index)):
            self.word(sp + 4*n, value)
        self.emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        self.emu.reg_write(UC_X86_REG_XMM0, 0x123456789abcdef)
        self.emu.reg_write(UC_X86_REG_XMM7, 0xfedcba987654321)
        try:
            self.start(DEATH, regs, sp)
        except RuntimeError:
            return None
        counts = tuple(self.read(r + 0xDC) for r in resources)
        return self.stopped, tuple(self.emu.reg_read(r) for r in REGS), self.read(out), self.read(out+4), counts

    def fresh(self, align):
        self.emu.mem_write(WORLD, bytes(0x20000))
        self.emu.mem_write(STACK, bytes(0x10000))
        self.fsm_calls = []
        self.emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        self.emu.reg_write(UC_X86_REG_XMM0, 0x123456789abcdef)
        self.emu.reg_write(UC_X86_REG_XMM7, 0xfedcba987654321)
        return {r: 0x11111111*(i+1) for i,r in enumerate(REGS[:7])}, FRAME - 0x100 + align

    def fire(self, index, align):
        """The fire deserialize's store with `index` in CL: the registers and
        the byte stored at +16Ch."""
        regs, sp = self.fresh(align)
        state = WORLD
        self.emu.mem_write(state + 0x16C, b'\xEE')
        regs.update({UC_X86_REG_ESI: state, UC_X86_REG_ECX: 0x33333300 | index})
        self.start(FIRE, regs, sp)
        return self.stopped, tuple(self.emu.reg_read(r) for r in REGS), self.emu.mem_read(state + 0x16C, 1)[0]

    def throw_state(self, index, grenades, vtable=GRENADE_SETS[1]):
        """A throw grenade state at WORLD: its set (of `vtable`) holds
        `grenades` grenades (None: no set), each its own object, every slot
        past them PAST."""
        state, sub, set_, table = WORLD, WORLD + 0x1000, WORLD + 0x2000, WORLD + 0x3000
        self.word(state + 0x150, sub)            # the substate: +13Ch clear
        self.emu.mem_write(state + 0x180, bytes([index]))
        if grenades is not None:
            self.word(state + 0x17C, set_)
            self.word(set_, vtable)
            self.word(set_ + 0x268, table)
            self.word(set_ + 0x26C, table + grenades*4)
            for n in range(256):
                grenade = WORLD + 0x4000 + n*0x200
                self.word(table + n*4, grenade if n < grenades else PAST)
                if n < grenades:
                    self.emu.mem_write(grenade + 0x154, bytes([n & 1]))  # ready or not, by slot
        # Its weapon (the throw reads [[state+120h]+8] for the player).
        self.word(state + 0x120, WORLD + 0x7000)
        self.word(WORLD + 0x7008, WORLD + 0x8000)
        return state

    def ready(self, index, align, grenades, vtable=GRENADE_SETS[1]):
        """is_ready_for_transition of a throw state with `index`: its result."""
        regs, sp = self.fresh(align)
        regs[UC_X86_REG_ECX] = self.throw_state(index, grenades, vtable)
        self.word(sp, GRENADE_RETURN)
        try:
            self.start(READY, regs, sp)
        except RuntimeError:
            return None
        return self.stopped, tuple(self.emu.reg_read(r) for r in REGS)

    def throw(self, index, align, grenades, vtable=GRENADE_SETS[1]):
        """The throw state's throw with `index`: where it stops (the set's
        throw_grenade, or its return), the registers and, at the set's
        throw, the index and the arguments it hands on."""
        regs, sp = self.fresh(align)
        regs[UC_X86_REG_ESI] = self.throw_state(index, grenades, vtable)
        self.word(sp, GRENADE_RETURN)
        self.at_set_throw = True
        self.start(THROW, regs, sp)
        at = self.stopped
        handed = None
        if at == SET_THROW:
            esp = self.emu.reg_read(UC_X86_REG_ESP)
            handed = (self.emu.reg_read(UC_X86_REG_EAX) & 0xff, tuple(self.read(esp + 4*n) for n in range(1, 4)))
        return at, tuple(self.emu.reg_read(r) for r in REGS), handed

    def set_throw_runs(self, index, align, grenades):
        """grenade_set_core::throw_grenade itself with `index`, to its first
        reference: False when it faults (the live write through a slot past
        the set)."""
        regs, sp = self.fresh(align)
        self.throw_state(index, grenades)
        for n, value in enumerate((GRENADE_RETURN, WORLD + 0x2000, WORLD + 0x5000, WORLD + 0x6000)):
            self.word(sp + 4*n, value)
        regs[UC_X86_REG_EAX] = index
        self.at_set_throw = False
        try:
            self.start(SET_THROW, regs, sp)
        except RuntimeError:
            return False
        return self.stopped == SET_THROW_TAKEN

    def wait(self, callback, vtable, index, align, match):
        """`callback` (an animation-end callback) on a state of `vtable`
        whose handles end at the end of the EDGE page, waiting for `index`,
        the ending clip that slot's handle (`match`) or another: where it
        stops (the match's continuation or the return) and the registers,
        None when it faults; and the state's +12Dh."""
        regs, sp = self.fresh(align)
        self.emu.mem_write(EDGE, bytes(0x1000))
        offset, count = LAYOUTS[self.read(vtable + 0x1C)]
        state = EDGE + 0x1000 - (offset + count*4)
        methods, weapon, owner, params = WORLD + 0x9000, WORLD + 0xA000, WORLD + 0xB000, WORLD + 0xD000
        for n in range(16):
            self.word(methods + 4*n, self.read(vtable + 4*n))
        self.word(methods + 0x24, MATCHED[WAIT_END])   # on_animation_end_impl
        self.word(state, methods)
        self.word(state + 0x120, weapon)
        self.word(weapon + 8, owner)
        self.word(state + 0x130, state + offset)
        self.emu.mem_write(state + 0x134, bytes([index]))
        if callback == WAIT_THROW:
            self.word(state + 0x138, WORLD + 0xC000)  # the throw state
        for n in range(count):
            self.word(state + offset + 4*n, 0x50000000 + 0x10*n)
        self.word(params, owner if callback == WAIT_THROW else weapon)
        self.word(params + 4, 0x50000000 + 0x10*index if match else 0x12345678)
        self.word(sp, WAIT_RETURN)
        self.word(sp + 4, params)
        regs[UC_X86_REG_ECX] = state
        self.matched = MATCHED[callback]
        try:
            self.start(callback, regs, sp)
        except RuntimeError:
            return None
        finally:
            self.matched = None
        return self.stopped, tuple(self.emu.reg_read(r) for r in REGS), self.emu.mem_read(state + 0x12D, 1)[0]

    def weapon_states(self, data):
        """Every weapon state class's vtable (RTTI, offset 0) whose
        register_animations walks a table of handles."""
        found = []
        for name, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            if not name.startswith(b'.rdata'):
                continue
            lo, hi = base.IMAGE_BASE + rva, base.IMAGE_BASE + rva + rsize
            for p in range(lo, hi - 8, 4):
                col = self.read(p)
                if not (lo <= col < hi - 16) or self.read(col) or self.read(col + 4):
                    continue
                td = self.read(col + 12)
                if not (base.IMAGE_BASE < td < hi + 0x2000000):
                    continue
                label = bytes(self.emu.mem_read(td + 8, 160)).split(b'\0')[0]
                if re.search(rb'weapon_core_\w*(state|substate)', label) and self.read(p + 4 + 0x1C) in range(0x24B700, 0x24B960):
                    found.append((label.decode(), p + 4))
        return found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe')
    args = parser.parse_args()
    original = open(args.exe,'rb').read()
    built, _ = base.build(original, 'section', open(base.default_bin('section'),'rb').read(),
                          None, open(base.late_join_bin(),'rb').read())
    stock, ours = GuardRun(original), GuardRun(bytes(built))
    states = stock.weapon_states(original)
    layouts = {stock.read(vtable + 0x1C) for _, vtable in states}
    assert layouts == set(LAYOUTS), [hex(l) for l in layouts]
    print(f'{len(states)} weapon state classes with a table of handles (RTTI): {len(layouts)} layouts')
    checks = 0
    for align in (0,4,8,12):
        for kind, end in ((FSM,FSM_END),(ANOMALY,ANOMALY_END),(ARTEFACT,ARTEFACT_END)):
            for index in (0,1,2):
                expected = stock.guard(kind,index,align)
                actual = ours.guard(kind,index,align)
                assert actual == expected, (hex(kind), index, align, actual, expected)
                checks += 1
            for count,index in ((3,3),(3,254),(3,255),(0,0)):
                at, regs, calls = ours.guard(kind,index,align,count)
                assert at == end, (hex(kind),index,hex(at))
                if kind == FSM: assert not calls
                elif kind == ANOMALY: assert regs[REGS.index(UC_X86_REG_EBP)] == 0
                else: assert regs[REGS.index(UC_X86_REG_EAX)] == 0
                assert regs[REGS.index(UC_X86_REG_ESP)] == FRAME-0x100+align
                assert regs[REGS.index(UC_X86_REG_XMM0)] == 0x123456789abcdef
                assert regs[REGS.index(UC_X86_REG_XMM7)] == 0xfedcba987654321
                checks += 1
        for count in (1, 2):
            for index in range(count):
                expected = stock.death(index, align, count)
                assert expected is not None and expected[0] == DEATH_RETURN, (index, count, expected)
                assert expected[-1] != (REFS,)*(2*count), expected  # a reference was taken
                assert ours.death(index, align, count) == expected, (index, count, align)
                checks += 1
            for index in (count, count + 1, 255):
                assert stock.death(index, align, count) is None, (index, count)  # the live fault
                # Past the lists: what the game does with the first death animation.
                assert ours.death(index, align, count) == stock.death(0, align, count), (index, count, align)
                checks += 1
        # A fire state's two fire animations: 0 and 1 are stored as read,
        # any other byte as 0 (the stock stores it, and the presentation then
        # takes a reference through [state+164h+index*4]).
        for index in (0, 1):
            expected = stock.fire(index, align)
            assert expected[0] == FIRE_END and expected[2] == index, expected
            assert ours.fire(index, align) == expected, (index, align)
            checks += 1
        for index in (2, 100, 254, 255):
            assert stock.fire(index, align)[2] == index  # the live store
            assert ours.fire(index, align) == stock.fire(0, align), (index, align)
            checks += 1
        # A throw's grenade: one of the set's runs as the stock; none (0FFh)
        # is not ready and, at the throw, throws nothing; past the set (or
        # with no set) the stock reads (is_ready) or writes (throw_grenade's
        # reference) through the slot past it, ours is not ready and throws
        # nothing.
        for grenades in (1, 4):
            for index in range(grenades):
                expected = stock.ready(index, align, grenades)
                assert expected is not None and expected[0] == GRENADE_RETURN, (index, grenades)
                assert ours.ready(index, align, grenades) == expected, (index, grenades, align)
                expected = stock.throw(index, align, grenades)
                assert expected[0] == SET_THROW and expected[2][0] == index, expected
                assert ours.throw(index, align, grenades) == expected, (index, grenades, align)
                assert stock.set_throw_runs(index, align, grenades)
                checks += 1
            not_ready = stock.ready(0xFF, align, grenades)
            assert not_ready is not None and not_ready[1][0] == 0, not_ready
            assert ours.ready(0xFF, align, grenades) == not_ready
            for index in (grenades, grenades + 1, 254):
                assert stock.ready(index, align, grenades) is None, (index, grenades)  # the live read
                assert not stock.set_throw_runs(index, align, grenades), (index, grenades)  # the live write
                assert stock.throw(index, align, grenades)[0] == SET_THROW  # handed on unbounded
                assert ours.ready(index, align, grenades) == not_ready, (index, grenades, align)
                checks += 1
            for index in (grenades, 254, 0xFF):
                at, regs, handed = ours.throw(index, align, grenades)
                assert at == GRENADE_RETURN and handed is None, (index, grenades, hex(at))
                assert regs[REGS.index(UC_X86_REG_ESP)] == FRAME - 0x100 + align + 4
                for r, value in zip(REGS[1:7], (0x22222222, 0x33333333, 0x44444444, WORLD, 0x66666666, 0x77777777)):
                    assert regs[REGS.index(r)] == value, (index, r, hex(regs[REGS.index(r)]))
                checks += 1
        assert stock.ready(0, align, None) is None  # no set: the stock reads [0+268h]
        assert ours.ready(0, align, None) == stock.ready(0xFF, align, None)
        assert ours.throw(0, align, None)[0] == GRENADE_RETURN
        checks += 1
        # Either grenade set's vtable runs as the stock; another item's in
        # the grenade slot (a weapon's) is not a grenade set: the stock reads
        # its +268h as a vector, ours is not ready and throws nothing.
        for vtable in GRENADE_SETS:
            assert ours.ready(1, align, 4, vtable) == stock.ready(1, align, 4, vtable)
            assert ours.throw(1, align, 4, vtable) == stock.throw(1, align, 4, vtable)
        assert stock.throw(1, align, 4, NOT_A_GRENADE_SET)[0] == SET_THROW
        assert ours.ready(1, align, 4, NOT_A_GRENADE_SET) == stock.ready(0xFF, align, 4)
        assert ours.throw(1, align, 4, NOT_A_GRENADE_SET)[0] == GRENADE_RETURN
        checks += 1
        # An animation to wait for: every weapon state class (by its
        # register_animations, the layout of its handles), at each callback
        # that serves it. In its handles: the stock's match or no match;
        # past them (one past, the show state's 63, 0FFh) the stock reads
        # past the object (the page ends there: a fault), ours is no match.
        for label, vtable in states:
            layout = stock.read(vtable + 0x1C)
            offset, count = LAYOUTS[layout]
            callbacks = [WAIT_END] + ([WAIT_THROW] if layout == 0x24B880 else []) \
                + ([WAIT_MELEE] if layout == 0x24B920 else [])
            for callback in callbacks:
                for index in sorted({0, count - 1}):
                    for match in (True, False):
                        expected = stock.wait(callback, vtable, index, align, match)
                        assert expected is not None, (label, hex(callback), index)
                        assert expected[0] == (MATCHED[callback] if match else WAIT_RETURN), (label, index, match)
                        assert ours.wait(callback, vtable, index, align, match) == expected, (label, hex(callback), index, match, align)
                        checks += 1
                for index in sorted({count, 63, 0xFF} - set(range(count))):
                    assert stock.wait(callback, vtable, index, align, True) is None, (label, index)
                    at, regs, ended = ours.wait(callback, vtable, index, align, True)
                    assert at == WAIT_RETURN and ended == 0, (label, hex(callback), index)
                    assert regs[REGS.index(UC_X86_REG_EAX)] == 0
                    assert regs[REGS.index(UC_X86_REG_ESP)] == FRAME - 0x100 + align + 8
                    for r in (UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_EBP):
                        assert regs[REGS.index(r)] == {UC_X86_REG_EBX: 0x22222222, UC_X86_REG_EDI: 0x66666666,
                                                       UC_X86_REG_EBP: 0x77777777}[r], (label, r)
                    checks += 1
    print(f'{checks} native guard cases passed (stock equivalence and invalid indices)')

if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Reproduce the PIN crash in Scaleform OnCharEvent, then exercise its Rust guard.
The original and patched whole function run with raw/collectible AS3 pointers;
only the normal nonnull WillTrigger call is stubbed (no textInput listeners).
"""
import sys
from unicorn import UC_HOOK_CODE, UC_PROT_ALL, UcError
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_EBP, UC_X86_REG_EIP, UC_X86_REG_ESP
import patch_026g as base
import check_hide_ui_026g as hide

DATA, AVM, ROOT, BUILTINS, AS3 = 0x30000000, 0x30000100, 0x30000200, 0x30000400, 0x30000800

class Run(hide.Run):
    def __init__(self, data, raw, collectible):
        super().__init__(data)
        self.emu.mem_map(DATA, 0x10000, UC_PROT_ALL)
        self.word(AVM - 0x18, ROOT)
        self.word(ROOT + 0x10, BUILTINS)
        self.word(AVM - 0x1c, raw)
        self.word(AVM - 0x20, collectible)
        self.triggered = []
        self.emu.hook_add(UC_HOOK_CODE, self.trigger, begin=0x74A860, end=0x74A860)

    def trigger(self, emu, *_):
        target = emu.reg_read(UC_X86_REG_ECX)
        self.triggered.append(target)
        if target == 0:
            return  # reproduce the real original null read at 74A800
        assert target == AS3
        sp = emu.reg_read(UC_X86_REG_ESP)
        assert self.read(sp + 4) == BUILTINS + 0x1a4
        assert self.read(sp + 8) == 0
        emu.reg_write(UC_X86_REG_EAX, 0)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 12)

    def char(self, code):
        sp = hide.STACK + 0x9000
        for i, value in enumerate((hide.SENTINEL, code, 0)):
            self.word(sp + 4*i, value)
        regs = {**hide.GAME, UC_X86_REG_ECX: AVM}
        try:
            after, end = self.run(0x79D360, (hide.SENTINEL,), sp, regs)
        except UcError:
            return ('fault', self.emu.reg_read(UC_X86_REG_EIP))
        assert end == sp + 12
        assert all(after[r] == regs[r] for r in (UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_EBP))
        return ('return', after[UC_X86_REG_EAX] & 255, self.triggered)

def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    for label, raw, collectible in [('raw', AS3, 0), ('collectible', 0, AS3), ('tagged collectible', 0, AS3|1),
                                    ('raw preferred', AS3, 1), ('detached', 0, 0), ('detached GC tag', 0, 1)]:
        for char in (ord('1'), ord('w'), 0x444, 13):
            stock = Run(original, raw, collectible).char(char)
            fixed = Run(patched, raw, collectible).char(char)
            if raw or collectible > 1:
                assert stock == fixed == ('return', 1, [AS3]), (label, stock, fixed)
            else:
                assert stock == ('fault', 0x74A800), (label, stock)
                assert fixed == ('return', 0, []), (label, fixed)
        print('ok', label, ': original behavior preserved or detached character ignored; registers/stack preserved')
    print('all cases passed')

if __name__ == '__main__':
    main()

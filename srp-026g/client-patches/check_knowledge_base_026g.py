#!/usr/bin/env python3
"""Run the original Knowledge database callback and the wiki redirect in Unicorn.

Only ShellExecuteA is doubled; original URL construction, language branches,
stack handling and return execute natively. No browser or network is opened.
"""
import sys
from pathlib import Path

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EIP, UC_X86_REG_ESP, UC_X86_REG_ESI

import patch_026g as P
from localization_026g import LANGUAGES
from check_artefacts_026g import Game, STACK, STOP

HANDLER, LANGUAGE, SHELL_EXECUTE = 0x28CAB0, 0x21E75E0, 0xAEFE74


class BrowserGame(Game):
    def __init__(self, image, language):
        super().__init__(image)
        self.opened = []
        text = self.block(32)
        self.emu.mem_write(text, language.encode() + b"\0")
        self.word(LANGUAGE, text)
        self.word(SHELL_EXECUTE, STOP + 0x200)
        self.emu.hook_add(UC_HOOK_CODE, self.open_browser, begin=STOP + 0x200, end=STOP + 0x200)

    def string(self, address):
        out = bytearray()
        while len(out) < 128:
            byte = self.emu.mem_read(address + len(out), 1)
            if byte == b"\0":
                return out.decode()
            out.extend(byte)
        raise AssertionError("unterminated browser URL")

    def open_browser(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        assert self.read(sp + 4) == 0
        assert self.string(self.read(sp + 8)) == "open"
        assert self.read(sp + 16) == self.read(sp + 20) == 0
        assert self.read(sp + 24) == 1
        self.opened.append(self.string(self.read(sp + 12)))
        emu.reg_write(UC_X86_REG_EAX, 33)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 28)


def check(original):
    patched = bytearray(original)
    for site in P.KNOWLEDGE_BASE:
        P.patch_section_value(patched, *site)
    for image, redirected in [(original, False), (patched, True)]:
        for language, *_ in LANGUAGES:
            game = BrowserGame(image, language)
            game.call(HANDLER, (0x12345678,), esi=0x76543210)
            code = "ru" if language in ("ukrainian", "russian") else "en"
            expected = P.KNOWLEDGE_BASE_URL.decode() if redirected else \
                f"https://support.survarium.com/index.php?/{code}/Knowledgebase/List"
            assert game.opened == [expected], (language, game.opened)
            assert game.emu.reg_read(UC_X86_REG_EAX) & 0xFF == 1
            assert game.emu.reg_read(UC_X86_REG_ESI) == 0x76543210
            assert game.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
    print("PASS: Knowledge database opens Skhid wiki in every game language; native browser call, stack and return preserved")


if __name__ == "__main__":
    check(Path(sys.argv[1]).read_bytes())

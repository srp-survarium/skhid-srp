#!/usr/bin/env python3
"""Exercise the original synchronous resource-query/callback ABI.

Resource I/O submission and dispatch are fixtures. The original query-and-wait
wrapper, its boost::function copies, callback proxy and dispatch back into our
callback run natively. This proves the call/ownership envelope, not that a
weapon cook is safe from every game hook or that actual assets load.
"""
from pathlib import Path
import struct
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game, STACK, STOP
import engine_026g as engine
import hooks_026g as hooks
import link_026g as linker
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_ESP, UC_X86_REG_EIP,
                              UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)

SUBMIT, DISPATCH, WAIT = 0x101F30, 0xE5AB0, 0xE5C20


class QueryGame(Game):
    def __init__(self, data, callback):
        super().__init__(data)
        self.submissions = []
        self.pending = None
        self.dispatched = 0
        self.result = self.block(0x400)
        self.context = self.block(12)
        at, blob, labels = callback
        self.emu.mem_map(at, (len(blob) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
        self.emu.mem_write(at, blob)
        self.invoker = labels["callback_inventory_ready"]
        vtable = self.block(8)
        self.word(vtable, 0)  # trivial function: native copy/destroy skip manager
        self.word(vtable + 4, self.invoker)
        self.params = self.block(0x64)
        self.requests = self.block(8)
        name = self.block(16)
        self.emu.mem_write(name, b"inventory\0")
        self.word(self.requests, name)
        self.word(self.requests + 4, 0x41)
        self.word(self.params, self.requests)
        self.word(self.params + 8, 1)
        self.word(self.params + 0x10, vtable | 1)
        self.word(self.params + 0x18, self.context)
        self.allocator = self.block(4)
        self.user_data = self.block(4)
        self.word(self.params + 0x30, self.allocator)
        self.word(self.params + 0x3C, self.user_data)
        self.word(self.params + 0x60, 1)
        # Resource manager's already-initialized threaded mode: dispatch only.
        self.word(0xA8161C, 1)
        self.emu.hook_add(UC_HOOK_CODE, self.hook, begin=SUBMIT, end=SUBMIT)
        self.emu.hook_add(UC_HOOK_CODE, self.hook, begin=DISPATCH, end=DISPATCH)

    def return_from(self, cleanup):
        esp = self.emu.reg_read(UC_X86_REG_ESP)
        ret = self.read(esp)
        self.emu.reg_write(UC_X86_REG_ESP, esp + 4 + cleanup)
        self.emu.reg_write(UC_X86_REG_EIP, ret)

    def hook(self, emu, address, _size, _data):
        esp = emu.reg_read(UC_X86_REG_ESP)
        if address == SUBMIT:
            params = self.read(esp + 4)
            copied = self.block(0x64)
            self.emu.mem_write(copied, bytes(self.emu.mem_read(params, 0x64)))
            self.submissions.append(copied)
            self.pending = copied + 0x10
            emu.reg_write(UC_X86_REG_EAX, 123)
            self.return_from(4)
        else:
            assert self.pending is not None, "wait loop dispatched after completion"
            self.dispatched += 1
            # Dispatch through the original function invocation helper. Its
            # proxy calls our copied callback and sets the native done flag.
            ret = self.read(esp)
            self.word(esp - 4, ret)
            self.word(esp, self.result)
            emu.reg_write(UC_X86_REG_ESP, esp - 4)
            emu.reg_write(UC_X86_REG_EAX, self.pending)
            emu.reg_write(UC_X86_REG_EIP, 0x205E0)
            self.pending = None


def recipe_context(data):
    """Execute the original player's inventory-recipe builder.

    The match-options virtual getter is a fixture endpoint. The builder's
    pointer chain, byte-sized kit/flag and seven initialized fields are real.
    This does not establish that a saved context may outlive player cooking.
    """
    g = Game(data)
    profile = g.block(4)
    stage = g.block(0x27C)
    recipe = g.block(0x1C)
    frame = g.block(0x40) + 0x20
    world, game, interface, vtable = (g.block(n) for n in [0xA0, 0x53B0, 4, 0x38])
    options = g.block(0x577B4)
    getter = 0x2200F000
    # Return an options object, preserving the native thiscall convention.
    g.emu.mem_write(getter, b'\xB8' + struct.pack('<I', options) + b'\xC3')
    g.word(vtable + 0x34, getter)
    g.word(interface, vtable)
    g.word(game + 0x53AC, interface)
    g.word(world + 0x9C, game)
    g.word(stage + 0x1E4, world)
    g.word(stage + 0x9C, profile)
    g.word(stage + 0xA4, world)
    g.word(stage + 0xA8, 0x23456789)
    g.word(stage + 0x1E8, 0x3456789A)
    g.word(options + 0x577B0, 0x456789AB)
    g.word(frame - 0x18, stage + 0x9C)
    g.word(frame - 0x14, stage)
    for kit, flag in [(0, 0), (1, 1), (2, 0)]:
        g.emu.mem_write(stage + 0xB0, bytes([kit, flag]))
        g.emu.mem_write(recipe, b'\xA5' * 0x1C)
        g.emu.reg_write(UC_X86_REG_EBP, frame)
        g.emu.reg_write(UC_X86_REG_ESI, recipe)
        g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
        g.emu.emu_start(0x20284C, 0x2028A9, count=1000)
        assert g.emu.reg_read(UC_X86_REG_EIP) == 0x2028A9
        assert g.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
        assert [g.read(recipe + n) for n in [0, 8, 12, 16, 20]] == [
            profile, 0x3456789A, 0x23456789, world, 0x456789AB]
        assert bytes(g.emu.mem_read(recipe + 4, 4)) == bytes([kit, 0xA5, 0xA5, 0xA5])
        assert bytes(g.emu.mem_read(recipe + 24, 4)) == bytes([flag, 0xA5, 0xA5, 0xA5])
    print("native inventory recipe: original context pointer chain, seven fields, "
          "three kits and untouched byte-field padding passed")


def run(data):
    recipe_context(data)
    callbacks = [cb for cb in engine.callbacks() if cb["symbol"] == "callback_inventory_ready"]
    assert len(callbacks) == 1
    # The real generated alignment entry; only the Rust handler is replaced
    # by a recorder of (result, invocation count, Rust entry ESP modulo 16).
    source = hooks.asm_text([], callbacks)
    stub = """
bits 32
section .text
global hook_callback_inventory_ready
hook_callback_inventory_ready:
    mov eax, [esp+4]
    mov eax, [eax]
    mov edx, [esp+8]
    mov [eax], edx
    inc dword [eax+4]
    mov edx, esp
    and edx, 15
    mov [eax+8], edx
    ret
"""
    source = source.replace('%include "rust_hook_026g.inc"',
                            f'%include "{hooks.HERE / "rust_hook_026g.inc"}"')
    with tempfile.TemporaryDirectory(prefix="srp-pickup-callback-") as temporary:
        root = Path(temporary)
        (root / "entry.asm").write_text(source)
        (root / "stub.asm").write_text(stub)
        block = linker.ArenaBlock("callback.code", "code", 0x1000,
                                  (str(root / "entry.asm"), str(root / "stub.asm")))
        linked = linker.link([block], 0x10000000, 0x100000)
        callback = linked.va, linker.block_bytes(linked, "callback.code"), linked.symbols
    for resource, padding in [(r, p) for r in [0, 0x12345678] for p in [0, 4, 8, 12]]:
        g = QueryGame(data, callback)
        g.word(g.result + 0x12C, resource)
        original = bytes(g.emu.mem_read(g.params, 0x64))
        esp = STACK - 0x100 - padding
        g.word(esp, STOP)
        g.emu.reg_write(UC_X86_REG_ESP, esp)
        g.emu.reg_write(UC_X86_REG_EAX, g.params)
        saved = [UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP]
        for i, reg in enumerate(saved):
            g.emu.reg_write(reg, 0x12340000 + i)
        g.emu.emu_start(WAIT, STOP, count=500000)
        assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
        assert g.emu.reg_read(UC_X86_REG_ESP) == esp + 4
        assert all(g.emu.reg_read(reg) == 0x12340000 + i for i, reg in enumerate(saved))
        assert g.dispatched == 1 and len(g.submissions) == 1
        assert g.read(g.context) == g.result and g.read(g.context + 4) == 1
        assert g.read(g.context + 8) == 12, "Rust callback entered with misaligned stack"
        sent = g.submissions[0]
        for offset in [0, 4, 8, 0x30, 0x34, 0x38, 0x3C, 0x40, 0x60]:
            assert g.read(sent + offset) == g.read(g.params + offset)
        assert bytes(g.emu.mem_read(g.params, 0x64)) == original
        assert g.read(g.result + 0x12C) == resource
    print("native resource query: parameter copy, deferred dispatch, trivial Rust-compatible "
          "callback ABI, all four stack alignments, preserved registers, null/non-null result "
          "and synchronous completion passed")


if __name__ == "__main__":
    run(Path(sys.argv[1]).read_bytes())

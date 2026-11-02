#!/usr/bin/env python3
"""Run battery pickup and both render hooks in the built executable.

The initiating client's ray/box geometry has Rust tests. Here the actual
native can_use/use_initialize and generated Rust hooks run: replicas with
very different battery transforms accept the same id, first claimant wins,
invalid states refuse, and cosmetic flight changes only draw-stack matrices.
Engine resources and the item-switch callback are fixtures, not a real match.
"""
import struct
import sys
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_EBP, UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP
import patch_026g as base
import check_artefact_carry_026g as carry
import check_ragdoll_026g as rag


def packed_matrix(at):
    return struct.pack('<16f', *rag.matrix(at))


def setup_rule(g, core, allocate, put, battery_at=(0., 1., 0.)):
    stats, rule, slots, item = (allocate(n) for n in (0x4000, 0x200, 40, 0x400))
    put(core + 0x18FA8, stats)
    put(stats + 8 + 0x2760 + 0xA38, rule)
    put(rule + 0x110, slots)
    put(rule + 0x114, slots + 4)
    put(slots, item)
    put(item, 0xA042AC)
    put(item + 0x14, 0xA0437C)
    g.emu.mem_write(item + 0x2C8, b'\1')
    g.emu.mem_write(item + 0x204, packed_matrix(battery_at))
    return item


def pickup(built, at, refusal=None):
    g = carry.Run(built)
    item = setup_rule(g, g.world, g.block, g.word, at)
    adapter = g.block(0x20)
    state_ptr, state = g.block(4), g.block(0x400)
    get_state, set_active, ready = g.block(16), g.block(16), g.block(16)
    g.word(state_ptr, state)
    g.emu.mem_write(get_state, b'\xb8'+struct.pack('<I', state_ptr)+b'\xc3')
    g.emu.mem_write(set_active, b'\xc2\x04\x00')
    g.emu.mem_write(ready, b'\x31\xc0\xc3')
    g.word(adapter+8, get_state)
    g.word(adapter+4, set_active)
    for p in g.players:
        g.word(p+0x108, adapter)
        g.word(p+0x130, p)
        weapon = g.read(p+0x128)
        g.word(g.read(weapon)+0x50, ready)
        transform_adapter, transform_get = g.block(0x40), g.block(16)
        g.word(p+0x118,transform_adapter)
        g.word(transform_adapter+0x28,transform_get)
        g.emu.mem_write(transform_get,b'\xb8'+struct.pack('<I',p+0x40C)+b'\xc3')
    p = g.players[0]
    if refusal == 'dead': g.emu.mem_write(p+0x33C,b'\0')
    if refusal == 'carrying': g.word(g.inventories[0]+0x178, item)
    if refusal == 'state': g.emu.mem_write(state+0x389,b'\1')
    if refusal == 'taken': g.word(item+8,g.players[1])
    if refusal == 'container': g.word(item+0x1A8,1)
    patterns = [v for v in range(64) if v.bit_count() >= 2]
    command = (1 << 29) | (patterns[20] << 14)
    before = bytes(g.emu.mem_read(item,0x400))
    g.input(p,1000,command)
    count = g.read(item+0x14+0x28)
    assert count == (0 if refusal else 1), (refusal,count)
    if refusal:
        assert bytes(g.emu.mem_read(item,0x400)) == before
    else:
        assert g.read(p+0x134) == item+0x14
        assert g.read(item+0x14+0x30) == p+0x130
        # A replica does not store the cosmetic contact point. Its use point
        # is the same simulated eye, so native use_tick sees equal reach.
        point = bytes(g.emu.mem_read(p+0x148,12))
        assert point == bytes(g.emu.mem_read(p+0x47C,12))
        # The real native continuation ignores a replica's different
        # detected object, yet still refuses walking away from the use point.
        g.site(0x300830,[],(p+0x130,1001,0),ecx=item+0x14)
        assert g.emu.reg_read(UC_X86_REG_EAX) & 0xFF == 1
        g.emu.mem_write(p+0x43C,struct.pack('<f',3.0))
        g.site(0x300830,[],(p+0x130,1001,item+0x14),ecx=item+0x14)
        assert g.emu.reg_read(UC_X86_REG_EAX) & 0xFF == 0
        g.emu.mem_write(p+0x43C,struct.pack('<f',0.0))
        g.input(g.players[1],1000,command)
        assert g.read(item+0x14+0x28) == 1, 'second claimant stole the battery'
        assert g.read(g.players[1]+0x134) == 0
        g.input(p,1001,command)
        assert g.read(item+0x14+0x28) == 1, 'duplicate use added a second user'
        return point


class DrawGame(rag.Game):
    def __init__(self, built):
        super().__init__(built)
        self.add_player(0,0,0)
        self.set_local(self.players[0])
        self.u32(rag.WORLD+0x4D4C, rag.NC_CORE)
        self.next = rag.HEAP+0xC00000
        def allocate(n):
            at = self.next; self.next += (n+15)&~15
            return at
        self.item = setup_rule(self,self.core,allocate,self.u32)
        self.native_body = allocate(0x100)
        self.u32(self.item+0x2A8,self.native_body)
        self.objects.append(('native battery', (0xA18,0x418), 0.6,
                             lambda p: abs(p[0]) < 0.4 and abs(p[2]) < 0.4))

    def ray_test(self, emu, *args):
        sp = emu.reg_read(UC_X86_REG_ESP)
        result = self.u32(sp+4)
        super().ray_test(emu,*args)
        if self.u32(result+0x1C) and abs(self.floats(result,3)[1]-0.6) < 1e-5:
            self.u32(result+0x1C,self.native_body)

    def draw_battery(self, secondary=False):
        site, end = (0x24687B,0x246883) if secondary else (0x24681D,0x246822)
        stack = bytearray(0xA0)
        native = packed_matrix((20.,30.,40.))
        stack[0x1C:0x5C] = native
        stack[0x5C:0x9C] = native
        sp = rag.STACK-0x1000
        self.emu.mem_write(sp, bytes(stack))
        for reg,value in {UC_X86_REG_EDI:self.item, UC_X86_REG_ESI:self.item,
                          UC_X86_REG_EBP:rag.FRAME, UC_X86_REG_EAX:0x1234, UC_X86_REG_ESP:sp}.items():
            self.emu.reg_write(reg,value)
        self.emu.emu_start(site,end,count=2_000_000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == end
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp-(8 if secondary else 4), 'native pushes preserved'
        assert self.emu.reg_read(UC_X86_REG_EDI) == self.item
        assert self.emu.reg_read(UC_X86_REG_ESI) == self.item
        assert self.emu.reg_read(UC_X86_REG_EBP) == rag.FRAME
        first = bytes(self.emu.mem_read(rag.STACK-0x1000+0x1C,64))
        second = bytes(self.emu.mem_read(rag.STACK-0x1000+0x5C,64))
        assert first == second, 'both render matrices must receive the same pose'
        return first


def main():
    original = open(sys.argv[1],'rb').read()
    built,_ = base.build(original,'section',open(base.default_bin('section'),'rb').read(),None,
                         open(base.late_join_bin(),'rb').read())
    assert pickup(built,(0.,0.,1.)) == pickup(built,(100.,20.,-80.))
    print('ok: near and far replicas accept the same battery id; first claimant wins')
    for why in ('dead','carrying','state','taken','container'):
        pickup(built,(0.,0.,1.),why)
    print('ok: native state, carrying, ownership, life and container refusals stay enforced')
    g = DrawGame(built)
    native = bytes(g.emu.mem_read(g.item,0x400))
    for _ in range(120): g.step()
    first = g.draw_battery()
    assert first == g.draw_battery(True)
    pose = struct.unpack('<16f',first)
    assert -0.01 < pose[13] < 0.15, pose
    assert g.hits.get('native battery',0) > 0, 'native body exclusion was not exercised'
    assert g.hits.get('hittable floor',0) > 0, 'skipping native body lost the actual floor'
    assert bytes(g.emu.mem_read(g.item,0x400)) == native, 'cosmetic flight wrote simulation state'
    g.u32(g.item+8,g.players[0])
    assert g.draw_battery() == packed_matrix((20.,30.,40.)), 'carried draw was overridden'
    g.step()
    g.u32(g.item+8,0)
    g.emu.mem_write(g.item+0x204,packed_matrix((3.,2.,0.)))
    g.step()
    pose = struct.unpack('<16f',g.draw_battery())
    assert abs(pose[12]-3.) < 0.01 and pose[13] > 1.5, pose
    print('ok: both models fall locally; native battery bytes stay intact; carry and re-drop reset flight')
    # Seeking/resyncing backwards must discard a previous cosmetic fall.
    g.u32(g.core+0x18FC0,1000)
    for _ in range(120): g.step()
    g.emu.mem_write(g.item+0x204,packed_matrix((5.,3.,0.)))
    g.u32(g.core+0x18FC0,500)
    g.step()
    pose = struct.unpack('<16f',g.draw_battery())
    assert abs(pose[12]-5.) < 0.01 and pose[13] > 2.5, pose
    print('ok: native battery body is skipped while the floor remains; rewind resets local flight')


if __name__ == '__main__': main()

#!/usr/bin/env python3
"""Native medical hook checks over constructed player/item fixtures.

Runs the generated entries and Rust in Unicorn, including the native modifier
getter, pool lookup, healing/reset and core player lookup. Effect rendering and
pool notifications are stubs; this is not a played match or a full world load.
Usage: python check_medical_skills_026g.py ORIGINAL_EXE
"""
import struct
import sys
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
from check_artefacts_026g import Game, MEMORY, STACK, PLAYER, KITS, STOP, REGISTER

REGISTER['ebp']=UC_X86_REG_EBP

def bits(value): return struct.unpack('<I',struct.pack('<f',value))[0]
def close(a,b): assert abs(a-b)<0.0001,(a,b)

class MedicalGame(Game):
    def __init__(self,data,padding=0):
        super().__init__(data)
        self.emu.mem_map(MEMORY+0x100000,0x200000,UC_PROT_ALL)
        self.next += padding
        self.effects=[]
        self.effect_instances=[]
        self.emitted=[]
        self.clocks=[]
        self.emit_stub=STOP+0x2000
        self.emu.mem_write(self.emit_stub,b'\xc2\x0c\x00')
        self.emu.hook_add(UC_HOOK_CODE,self.emit,begin=self.emit_stub,end=self.emit_stub)
        self.emu.hook_add(UC_HOOK_CODE,self.effect,begin=0x2de090,end=0x2de090)
        self.core=self.block(0x19000)
        self.places=self.block(20*0x30)
        self.word(self.core+0x18458,self.places)
        self.word(self.core+0x1845c,self.places+20*0x30)
        self.pools={}
        self.player(0,0,(0,0,0),PLAYER,KITS)
    def emit(self,emu,address,size,_):
        # Native resource emit (0x329050) runs; only the renderer's emitter is
        # substituted. Each emission owns a new mutable effect with count 1.
        esp=emu.reg_read(UC_X86_REG_ESP)
        assert self.read(esp+8)==bits(0) and self.read(esp+12)==bits(1)
        effect=self.block(0x60)
        self.word(effect+0x1c,1)
        self.word(self.read(esp+4),effect)
        self.emitted.append(effect)
    def effect_resource(self):
        resource=self.block(0x120);emitter=self.block(4);vtable=self.block(0x30)
        self.word(emitter,vtable);self.word(vtable+0x20,self.emit_stub)
        self.word(vtable+0x28,self.emit_stub+0x20)
        self.emu.mem_write(self.emit_stub+0x20,b'\xc2\x04\x00')
        self.word(resource+0x108,emitter);self.word(resource+0x114,bits(1))
        return resource
    def effect(self,emu,address,size,_):
        self.effects.append(emu.reg_read(UC_X86_REG_EDI)-0x358)
        esp=emu.reg_read(UC_X86_REG_ESP)
        effect=self.read(self.read(esp+4))
        self.clocks.append(self.read(esp+20))
        # Reject the original bug: adding the source's live instance again.
        assert effect in self.emitted and effect not in self.effect_instances
        self.effect_instances.append(effect)
        self.word(effect+0x1c,self.read(effect+0x1c)+1)  # native add retains
        emu.reg_write(UC_X86_REG_EIP,self.read(esp))
        emu.reg_write(UC_X86_REG_ESP,esp+24)
    def player(self,place,team,position,at=None,profile=None):
        at=at or self.block(0xf500); profile=profile or self.block(0x4700)
        self.word(at+0xd0,10);self.word(at+0x6b8,profile)
        self.word(at+0x120,self.core);self.word(profile+0x45ec,team)
        self.emu.mem_write(at+0x114,bytes([place,0]));self.emu.mem_write(at+0x33c,b'\1\0\0\1')
        self.emu.mem_write(at+0x43c,struct.pack('<fff',*position))
        model=self.block(0x500);self.word(at+0x2e0,model);self.word(model+0x3b0,at)
        pools={}
        for name,maximum in [('body',1),('head',.4),('pain',1),('left_hand',.5),('right_hand',.5),('left_leg',.5),('right_leg',.5)]:
            pool=self.block(0x400);text=self.block(16);self.emu.mem_write(text,name.encode()+b'\0')
            self.word(pool+0x80,text);self.word(pool+0x7c,model)
            for off in [0x9c,0xa0,0xb4]:self.word(pool+off,bits(maximum))
            self.word(pool+0xac,8000)
            if pools:self.word(list(pools.values())[-1],pool)
            else:self.word(model+0x110,pool)
            pools[name]=pool
        self.pools[at]=pools
        self.word(self.places+place*0x30,at)
        self.word(self.core+0x18fd8,self.read(self.core+0x18fd8)|(1<<place))
        return at
    def ranks(self,encoded): self.word(KITS+0xf4+43*4,bits(encoded))
    def medkit(self,pain=True,heal=False):
        item=self.block(0x400);inventory=self.block(0x200)
        self.word(inventory+0x174,PLAYER+0x108);self.word(item+0x250,inventory)
        self.emu.mem_write(item+0x288,bytes([int(heal)]))
        if pain:
            protection=self.block(0x90);self.word(item+0x294,protection)
            self.emu.mem_write(item+0x298,b'\1');self.emu.mem_write(protection+0x70,b'pain\0')
            self.word(protection+0x84,bits(.9))
        return item
    def step(self,site,span,xmm=0,stack=(),**regs):
        self.emu.reg_write(UC_X86_REG_ESP,STACK-0x100)
        for i,v in enumerate(stack):self.word(STACK-0x100+4*i,v)
        self.emu.reg_write(UC_X86_REG_XMM0,bits(xmm))
        for name,value in regs.items():self.emu.reg_write(REGISTER[name],value)
        self.emu.emu_start(site,site+span,count=2_000_000)
        if self.emu.reg_read(UC_X86_REG_EIP)!=site+span:
            print('registers', {name:hex(self.emu.reg_read(reg)) for name,reg in REGISTER.items()})
            print('pools', [(name,hex(at),hex(self.read(at))) for name,at in self.pools[PLAYER].items()])
        assert self.emu.reg_read(UC_X86_REG_EIP)==site+span, (hex(site), hex(self.emu.reg_read(UC_X86_REG_EIP)))
        return struct.unpack('<f',(self.emu.reg_read(UC_X86_REG_XMM0)&0xffffffff).to_bytes(4,'little'))[0]

def check_particle_capacity(data):
    g=MedicalGame(data)
    query=g.block(0x120);fallback=g.block(64);resolved=g.block(64)
    g.word(query+0xf8,fallback)
    for requested,override,initial,expected in [
        ('artefact_lifebone',None,1,20),('artefact_lifebone',None,30,30),
        ('artefact_onyx',None,1,1),('artefact_lifebone','artefact_spring',1,1),
        ('unused','artefact_lifebone',1,20)]:
        g.emu.mem_write(fallback,requested.encode()+b'\0')
        g.word(query+0xfc,resolved if override else 0)
        if override:g.emu.mem_write(resolved,override.encode()+b'\0')
        g.step(0x2ec81e,5,ebx=query,esi=initial)
        assert g.emu.reg_read(UC_X86_REG_ESI)==expected,(requested,override,expected)
    # Exercise the actual particle emitter's free -> active -> free transitions,
    # including the pop that crashed. Only allocation of the outer effect and
    # renderer work are outside these native slices. All 20 instances coexist.
    emitter=g.block(0x150);active=g.block(80);free=g.block(80)
    for off,value in [(0x120,active),(0x124,active),(0x128,active+80),
                      (0x12c,free),(0x130,free+80),(0x134,free+80)]:g.word(emitter+off,value)
    particles=[g.block(0x100) for _ in range(20)]
    for i,particle in enumerate(particles):g.word(particle+0xd0,10);g.word(free+4*i,particle)
    out=g.block(4)
    for i in range(20):
        g.step(0x29a250,0x51,stack=(STOP,out,bits(0),bits(1)),edi=emitter)
        assert g.read(emitter+0x130)==free+4*(19-i)
        assert g.read(emitter+0x124)==active+4*(i+1)
    # Arbitrary destruction order, then a second activation: returned resources
    # must be reusable, including during rollback/re-simulation.
    effect=g.block(0x40)
    for particle in particles[::2]+particles[1::2]:
        g.word(effect+0x30,particle)
        g.step(0x29a33e,0x7e,eax=effect,edi=emitter)
    assert g.read(emitter+0x130)==free+80 and g.read(emitter+0x124)==active
    for i in range(20):g.step(0x29a250,0x51,stack=(STOP,out,bits(0),bits(1)),edi=emitter)
    print('Larkspur pool: targeted cook capacity; 20 simultaneous native particle claims, arbitrary returns, repeated claims: PASS')

def run(data,padding=0):
    # Execute the full native profile getter with real skill sums. Its equipment
    # slots are empty; the epilogue is also exercised for every modifier index.
    g=MedicalGame(data,padding)
    for slot in range(23): g.word(KITS+0x98+slot*4,0xffff)
    for index in [15,17]:
        for total in [0,1,4,5,6,7,20]:
            g.word(KITS+0xf4+index*4,bits(total-1))
            g.call(0x2f7660,(bits(1),KITS,0),eax=0,ecx=index)
            actual=struct.unpack('<f',(g.emu.reg_read(UC_X86_REG_XMM0)&0xffffffff).to_bytes(4,'little'))[0]
            close(actual,min(total,6 if index==15 else 5))
            assert g.emu.reg_read(UC_X86_REG_ESP)==STACK-0x100-12
    for index in [14,16,57]:
        g.word(KITS+0xf4+index*4,bits(9))
        g.call(0x2f7660,(bits(1),KITS,0),eax=0,ecx=index)
        actual=struct.unpack('<f',(g.emu.reg_read(UC_X86_REG_XMM0)&0xffffffff).to_bytes(4,'little'))[0]
        close(actual,10)
    # Native artifact allocation shares the same six inventory positions.
    inventory=g.block(0x200)
    for first in range(13,19):
        for requested in [0,1,3,9]:
            g.step(0x2fd214,0x97,xmm=requested,stack=(0,0,0,0,0,0,0,first),edi=inventory)
            assert g.read(inventory+0x16c)&255 == min(requested,19-first)
            assert g.read(inventory+0x168) == inventory+0x10c+first*4
    print('six pockets, five units per pocket, and six combined supply/artifact positions: PASS')
    g=MedicalGame(data,padding);item=g.medkit();healing=g.medkit(False,True);other=g.medkit(False,False)
    for rank in range(6):
        g.ranks(rank|(rank<<3))
        for site,regs in [(0x33d41b,dict(ebp=item,ebx=PLAYER)),(0x33d852,dict(edi=item,ebp=PLAYER)),(0x33dd65,dict(edi=item,ebx=PLAYER))]:
            close(g.step(site,6,xmm=10000,**regs),10000+rank*1000)
        close(g.step(0x33d41b,6,xmm=1000,ebp=healing,ebx=PLAYER),1000)
        close(g.step(0x33d486,6,xmm=1000,ebp=healing,ebx=PLAYER),1000*(1-.02*rank))
        close(g.step(0x33d486,6,xmm=1000,ebp=other,ebx=PLAYER),1000)
        hit=g.block(0x50);damage=g.block(4);pierce=g.block(4);name=g.block(16)
        g.emu.mem_write(name,b'pain\0');g.word(hit,name);g.word(damage,bits(.5));g.word(pierce,bits(.3))
        g.call(0x33db30,(hit,damage,pierce),ecx=item)
        close(g.float(damage),.5*(1-.1*(1+.05*rank)));close(g.float(pierce),.3)
    g.ranks((5<<6)|(5<<17)|(5<<12))
    g.step(0x2d706f,7,ebp=PLAYER)
    for name,pool in g.pools[PLAYER].items():
        assert g.read(pool+0xac)==5500
        if name.endswith(('hand','leg')):
            close(g.float(pool+0x9c),.75);close(g.float(pool+0xa0),.75)
    # Native respawn reset restores the derived maximum, without compounding.
    limb=g.pools[PLAYER]['left_hand']
    for _ in range(3):
        g.word(limb+0xa0,bits(.1))
        g.call(0x32ac90,eax=limb)
        close(g.float(limb+0xa0),.75);close(g.float(limb+0x9c),.75)
    body=g.pools[PLAYER]['body']
    for health,expected in [(1,4),(.2,4),(.19,4.6),(0,4)]:
        g.word(body+0xa0,bits(health));close(g.step(0x2dbc91,5,xmm=4,ebx=PLAYER,ebp=STACK+0x40),expected)
    g.emu.mem_write(PLAYER+0x33e,b'\0');g.step(0x32a460,8,xmm=.4,esi=body)
    assert g.emu.mem_read(PLAYER+0x33e,1)==b'\1'
    g.emu.mem_write(PLAYER+0x33e,b'\0');g.step(0x32a28b,8,xmm=.1,ebp=body)
    assert g.emu.mem_read(PLAYER+0x33e,1)==b'\1'
    ally=g.player(1,0,(3,4,0));far=g.player(2,0,(5.01,0,0));enemy=g.player(3,1,(1,0,0));dead=g.player(4,0,(1,0,0))
    g.emu.mem_write(dead+0x33c,b'\0')
    hit=g.block(0x50);owner=g.block(0x20);g.word(hit+0x20,owner)
    g.ranks(5<<21)
    for place,expected in [(0,.8),(1,.4),(2,.4),(3,.8),(19,.8)]:
        g.word(owner+4,place)
        g.step(0x2e50c7,5,stack=(0,0,bits(.8)),edi=g.read(PLAYER+0x2e0),esi=hit)
        close(g.float(STACK-0x100+8),expected)
    assert all(g.read(p+0xd0)==10 for p in [PLAYER,ally,far,enemy,dead])
    item=g.block(0x500);vt=g.block(0x100);g.word(item,vt);g.word(vt+0x80,0x312250)
    for at in [PLAYER,ally,far,enemy,dead]:
        for name in ['body','head','pain']:g.word(g.pools[at][name]+0xa0,bits(.1))
    resource=g.effect_resource();g.word(item+0x290,resource);g.word(item+0x494,3000)
    inventory=g.block(0x200);g.word(item+0x250,inventory);g.word(inventory+0x174,PLAYER+0x108)
    second=g.player(5,0,(1,0,0))
    source_effect=g.block(0x60);source_slot=g.block(4);g.word(source_slot,source_effect)
    g.word(source_effect+0x1c,1)
    for skill in [0,1<<20,1<<20]:
        g.ranks(skill);g.effects.clear()
        g.step(0x313333,6,esi=item,edi=1234)
        assert g.effects==([ally,second] if skill else []),g.effects
        assert g.read(source_effect+0x1c)==1
        assert all(g.read(e+0x1c)==1 for e in g.emitted)  # guard released its ref
        assert all(g.read(e+0x10)==resource for e in g.emitted)  # native metadata
    assert len(g.effect_instances)==4 and len(set(g.effect_instances))==4
    assert len(set(g.clocks))==1
    clock=g.clocks[0]
    assert clock not in [at+0x394 for at in [PLAYER,ally,second]]
    assert g.read(clock+4)==0xffffffff
    assert all(g.float(e+0x24)==3 for e in g.emitted)
    # Exercise the real callback entry/return and the native local calculator
    # codec. Playback freezes at interval/time zero, like the source artefact,
    # until the independently serialized start+length expire. No source access.
    node=g.block(0x50);out=g.block(12)
    for duration in [3000,5000,6500]:
        effect=g.emitted[0];g.word(effect+0x24,bits(duration/1000))
        for start in [1234,0xfffffff0]:
            g.word(node,effect);g.word(node+0x18,start)
            for elapsed in [0,999,1000,duration-1,duration,duration+50]:
                now=(start+elapsed)&0xffffffff
                g.call(g.read(g.read(clock)),(out,node,start,now),ecx=clock)
                assert g.read(out)==0 and g.float(out+4)==0
                assert g.emu.mem_read(out+8,1)==bytes([int(elapsed>=duration)])
                assert g.emu.reg_read(UC_X86_REG_EAX)==out
    # A restored native effect retains its saved length and its clock pointer.
    # Execute native pointer-decoding slice (local visual stream) and callback.
    writer=g.block(24);buffer=g.block(16);payload=g.block(64)
    g.word(writer+0x14,buffer);g.word(buffer+4,payload);g.word(buffer+8,64)
    g.step(0x2ddc63,0x2b,ebx=node,esi=writer)
    g.call(0x2efd00,(clock,),esi=writer)
    assert g.read(buffer+0xc)==9
    assert g.read(payload)==bits(6.5)
    assert g.emu.mem_read(payload+4,1)==b'\xff' and g.read(payload+5)==clock
    restored=g.block(0x60);g.word(restored+0x24,g.read(payload));g.word(node,restored)
    reader=g.block(16);payload+=5;g.word(reader+4,payload)
    g.step(0x2ddf1c,0x41,eax=payload,ecx=clock,esi=reader)
    assert g.read(STACK-0x100+0x18)==clock and g.read(reader+4)==payload+4
    g.word(node+0x18,100)
    for now,ended in [(6599,0),(6600,1)]:
        g.call(g.read(g.read(clock)),(out,node,100,now),ecx=clock)
        assert g.emu.mem_read(out+8,1)==bytes([ended])
    # Ordinary clocks must continue through the unchanged native calculation.
    normal=g.block(8);g.word(normal,g.read(clock));interval=g.block(8)
    g.word(interval+4,bits(1));g.word(node+0x10,1);g.word(node+0x14,interval)
    g.word(node+0x1c,0);g.word(node+0x20,bits(0))
    for elapsed in [500,1000]:
        g.call(g.read(g.read(normal)),(out,node,0,elapsed),ecx=normal)
        close(g.float(out+4),min(elapsed/1000,1))
        assert g.emu.mem_read(out+8,1)==bytes([int(elapsed>=1000)])
    print('Medic clock: full activation duration, wrapping time, local pointer restore, ordinary clocks unchanged: PASS')
    print('Medic: independent native resource emission per ally/activation; source untouched; retained references balanced: PASS')
    for at in [PLAYER,far,enemy,dead]:close(g.float(g.pools[at]['body']+0xa0),.1)
    for name in ['body','head','pain']:close(g.float(g.pools[ally][name]+0xa0),g.float(g.pools[ally][name]+0x9c))
    assert all(g.read(p+0xd0)==10 for p in [PLAYER,ally,far,enemy,dead])
    # The old visual-helper hook was reachable from native deserialize. Running
    # that exact point again must not heal or emit additional teammate effects.
    for at in [ally,second]:g.word(g.pools[at]['body']+0xa0,bits(.1))
    before=list(g.effects)
    g.step(0x313435,6,stack=(source_slot,1234,0,2,item+0x268),eax=PLAYER,edi=item+0x268)
    assert g.effects==before
    for at in [ally,second]:close(g.float(g.pools[at]['body']+0xa0),.1)
    # Missing visual resources must not disable the gameplay skill.
    g.word(item+0x290,0)
    g.step(0x313333,6,esi=item,edi=2234)
    for at in [ally,second]:close(g.float(g.pools[at]['body']+0xa0),g.float(g.pools[at]['body']+0x9c))
    assert g.effects==before
    print('Medic: activation-only healing; visual reconstruction never heals; missing visuals do not disable healing: PASS')
    print('medical skills: ranks 0..5; duration start/tick/restore; healing-only activation; protection; limb health; regeneration; Adrenalin damage/heal; allied damage; Medic radius/team/life filters and references: PASS')

    return (
        tuple(tuple((name,g.read(pool+0xa0)) for name,pool in g.pools[at].items())
              for at in [PLAYER,ally,far,enemy,dead,second]),
        tuple(g.read(at+0x114)&255 for at in g.effects),
        tuple((g.read(e+0x14),g.read(e+0x18),g.read(e+0x1c)) for e in g.emitted),
    )

def main():
    original=open(sys.argv[1],'rb').read()
    patched,_=base.build(original,'section',open(base.default_bin('section'),'rb').read(),None,open(base.late_join_bin(),'rb').read())
    check_particle_capacity(patched)
    assert run(patched)==run(patched,0x4000)
    print("Medical simulation outcome identical across independent client memory layouts: PASS")

if __name__=='__main__':main()

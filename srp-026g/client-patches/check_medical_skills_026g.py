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
from check_artefacts_026g import Game, MEMORY, STACK, PLAYER, KITS, STOP, REGISTER, HOLDER_VT, DM, \
    SPRING, PASSIVE_LINK, ACTIVE_LINK, STAGGER, MOVEMENT

MEDKIT_VTABLE=0xA1C79C

REGISTER['ebp']=UC_X86_REG_EBP

def bits(value): return struct.unpack('<I',struct.pack('<f',value))[0]
def close(a,b): assert abs(a-b)<0.0001,(a,b)

LOG_OPEN_SLOT = 0x870048                  # CreateFileA's import slot (the patches' log)
THREAD_ID_SLOT = 0x870118                 # GetCurrentThreadId's import slot (the log's one writer)

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
        # The patches' log (srp_patches.log): CreateFileA fails (mov eax,-1; ret 1Ch), nothing written.
        self.emu.mem_write(STOP+0x2100,b'\xb8\xff\xff\xff\xff\xc2\x1c\x00');self.word(LOG_OPEN_SLOT,STOP+0x2100)
        # The log's writer asks whose line it is: GetCurrentThreadId (mov eax,1; ret), one thread.
        self.emu.mem_write(STOP+0x2110,b'\xb8\x01\x00\x00\x00\xc3');self.word(THREAD_ID_SLOT,STOP+0x2110)
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
    def ranks(self,encoded,slot=43):
        kit=self.emu.mem_read(PLAYER+0x115,1)[0]
        self.word(KITS+0xf4+kit*0x704+slot*4,bits(encoded))
    def combo(self,encoded):self.ranks(encoded,45)
    def carried(self,pain,active,slot=5):
        # A medicine in the player's own inventory slot (player+10Ch, its 23
        # slots at +10Ch..+168h), with the native medkit vtable and activity.
        inventory=self.read(PLAYER+0x10c)
        if not inventory:inventory=self.block(0x200);self.word(PLAYER+0x10c,inventory)
        item=self.medkit(pain,not pain);self.word(item,MEDKIT_VTABLE)
        self.emu.mem_write(item+0x2b0,bytes([int(active)]))
        self.word(inventory+0x10c+slot*4,item)
        return item
    def crouch(self,effective):
        # player+494h -> bt_character_controller -> its Bullet controller +201h
        wrapper=self.read(PLAYER+0x494)
        if not wrapper:
            wrapper=self.block(8);self.word(PLAYER+0x494,wrapper);self.word(wrapper,self.block(0x300))
        self.emu.mem_write(self.read(wrapper)+0x201,bytes([int(effective)]))
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
        ('artefact_onyx',None,1,20),('artefact_lifebone','artefact_spring',1,20),('artefact_spring',None,1,20),
        ('artefact_rattle',None,1,1),('unused','artefact_lifebone',1,20)]:
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

def run(data,padding=0,kit=0):
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
    g=MedicalGame(data,padding)
    g.emu.mem_write(PLAYER+0x115,bytes([kit]))
    g.emu.mem_write(g.places+0xc,bytes([kit]))
    g.word(g.places+kit*4,PLAYER)
    item=g.medkit();healing=g.medkit(False,True);other=g.medkit(False,False)
    # Potent Painkiller 0..3 (4..7 refused: no skill at all), Fast Bandaging
    # 0..2 (3 refused), each with the other at its own top rank.
    for rank in range(8):
        painkiller=rank if rank<=3 else 0
        g.ranks(rank|(2<<3))
        bandaging=2 if rank<=3 else 0
        for site,regs in [(0x33d41b,dict(ebp=item,ebx=PLAYER)),(0x33d852,dict(edi=item,ebp=PLAYER)),(0x33dd65,dict(edi=item,ebx=PLAYER))]:
            close(g.step(site,6,xmm=10000,**regs),10000+painkiller*1500)
        close(g.step(0x33d41b,6,xmm=1000,ebp=healing,ebx=PLAYER),1000)
        close(g.step(0x33d486,6,xmm=1000,ebp=healing,ebx=PLAYER),1000*(1-.05*bandaging))
        close(g.step(0x33d486,6,xmm=1000,ebp=other,ebx=PLAYER),1000)
        hit=g.block(0x50);damage=g.block(4);pierce=g.block(4);name=g.block(16)
        g.emu.mem_write(name,b'pain\0');g.word(hit,name);g.word(damage,bits(.5));g.word(pierce,bits(.3))
        g.call(0x33db30,(hit,damage,pierce),ecx=item)
        close(g.float(damage),.5*(.9-.01*painkiller));close(g.float(pierce),.3)
    for rank in range(8):
        g.ranks(3|(rank<<3))
        close(g.step(0x33d486,6,xmm=1000,ebp=healing,ebx=PLAYER),1000*(1-.05*rank) if rank<=2 else 1000)
    g.ranks((5<<6)|(5<<17)|(3<<12))
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
    for health,expected in [(1,4),(.3,4),(.29,4.6),(.19,4.6),(0,4)]:
        g.word(body+0xa0,bits(health));close(g.step(0x2dbc91,5,xmm=4,ebx=PLAYER,ebp=STACK+0x40),expected)
    g.emu.mem_write(PLAYER+0x33e,b'\0');g.step(0x32a460,8,xmm=.4,esi=body)
    assert g.emu.mem_read(PLAYER+0x33e,1)==b'\1'
    g.emu.mem_write(PLAYER+0x33e,b'\0');g.step(0x32a28b,8,xmm=.1,ebp=body)
    assert g.emu.mem_read(PLAYER+0x33e,1)==b'\1'
    # Medic is Cascade's Larkspur share: Cascade's 6 m (was 5 m), the edge included.
    ally=g.player(1,0,(0,6,0));far=g.player(2,0,(6.01,0,0));enemy=g.player(3,1,(1,0,0));dead=g.player(4,0,(1,0,0))
    g.emu.mem_write(dead+0x33c,b'\0')
    hit=g.block(0x50);owner=g.block(0x20);g.word(hit+0x20,owner)
    g.ranks(2<<21)
    for place,expected in [(0,.8),(1,.4),(2,.4),(3,.8),(19,.8)]:
        g.word(owner+4,place)
        g.step(0x2e50c7,5,stack=(0,0,bits(.8)),edi=g.read(PLAYER+0x2e0),esi=hit)
        close(g.float(STACK-0x100+8),expected)
    assert all(g.read(p+0xd0)==10 for p in [PLAYER,ally,far,enemy,dead])
    item=g.block(0x500);vt=g.block(0x100);g.word(item,vt);g.word(vt+0x80,0x312250)
    for at in [PLAYER,ally,far,enemy,dead]:
        for name in g.pools[at]:g.word(g.pools[at][name]+0xa0,bits(.1))
    resource=g.effect_resource();g.word(item+0x290,resource);g.word(item+0x494,3000)
    inventory=g.block(0x200);g.word(item+0x250,inventory);g.word(inventory+0x174,PLAYER+0x108)
    second=g.player(5,0,(1,0,0))
    source_effect=g.block(0x60);source_slot=g.block(4);g.word(source_slot,source_effect)
    g.word(source_effect+0x1c,1)
    for skill, inserted in [(0,1),(1<<20,1),(1<<20,0)]:
        # Native restoration clears +33F without changing the active roster or
        # serialized life state. Every peer must still choose the same allies.
        for at in [PLAYER,ally,second]:g.emu.mem_write(at+0x33f,bytes([inserted]))
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
    print('Medic: insertion-independent recipients; independent native resource emission; retained references balanced: PASS')
    for at in [PLAYER,far,enemy,dead]:close(g.float(g.pools[at]['body']+0xa0),.1)
    # Every body part of a recipient is full, the limbs too (the owner, 2026-11-14).
    for name in g.pools[ally]:close(g.float(g.pools[ally][name]+0xa0),g.float(g.pools[ally][name]+0x9c))
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
    print('medical skills: ranks 0..3/0..2/0..5, out-of-range refused; duration start/tick/restore; healing-only activation; protection; limb health; regeneration; Adrenalin damage/heal; allied damage; Medic radius/team/life filters and references: PASS')

    return (
        tuple(tuple((name,g.read(pool+0xa0)) for name,pool in g.pools[at].items())
              for at in [PLAYER,ally,far,enemy,dead,second]),
        tuple(g.read(at+0x114)&255 for at in g.effects),
        tuple((g.read(e+0x14),g.read(e+0x18),g.read(e+0x1c)) for e in g.emitted),
    )

def check_combo(data,kit=0):
    # Numb Legs (sprint stamina), Take Cover (medicine healing crouched) and
    # Unflinching (bullets' slow through the real stagger, with Spring), ranks
    # 0..2, 0..2 and 0..4 in modifier 45 (Unflinching three bits at 4),
    # painkiller on/off, effective crouch vs the key.
    outcome=[]
    for combo in range(128):
        numb,cover,unflinching=combo&3,(combo>>2)&3,(combo>>4)&7
        valid=numb<=2 and cover<=2 and unflinching<=4
        if not valid and combo not in (3,12,5<<4,6<<4,7<<4,(5<<4)|10,127):continue
        for active in (False,True):
            for crouched in (False,True):
                for spring in (False,True):
                    g=MedicalGame(data)
                    g.emu.mem_write(PLAYER+0x115,bytes([kit]))
                    g.combo(combo)
                    g.carried(False,True,3)          # an active bandage is no painkiller
                    pain=g.carried(True,active)
                    g.crouch(crouched)
                    g.emu.mem_write(PLAYER+0x330,struct.pack('<I',0x100 if not crouched else 0))  # the key alone is no crouch
                    r=lambda x:x if valid else 0
                    sprint=g.step(0x329aad,6,xmm=10,edi=PLAYER+0x6c0)
                    close(sprint,10*(1-.2*r(numb)) if active else 10)
                    healed=g.step(0x33d948,6,xmm=.5,edi=pain,ebp=PLAYER)
                    close(healed,.5*(1+.1*r(cover)) if crouched else .5)
                    s=g.spring(SPRING);g.call(PASSIVE_LINK,eax=s)
                    if spring:g.call(ACTIVE_LINK,eax=s)
                    recent=g.block(24)
                    g.emu.mem_write(recent,struct.pack('<IfIf',9900,.15,9800,.3))
                    g.word(PLAYER+0x2e8,recent);g.word(PLAYER+0x2ec,recent+16)
                    g.emu.mem_write(PLAYER+0x31c,struct.pack('<ff',.5,.5));g.word(PLAYER+0x2fc,MOVEMENT)
                    g.call(STAGGER,(10000,),eax=PLAYER)
                    slow=0 if spring else .45*((1-.1*r(unflinching)) if active else 1)
                    close(g.float(PLAYER+0x300),-slow)
                    outcome.append((combo,active,crouched,spring,sprint,healed,g.read(PLAYER+0x300)))
    print('Numb Legs, Take Cover, Unflinching: ranks 0..2/0..2/0..4, painkiller on/off, effective crouch, Spring on/off, ranks 3/3/5..7 refused: PASS')
    return outcome

def check_pressed_entry(data):
    # Start at the public, already-detoured action entry, not inside its body.
    # Rev45's fire_kind copied the activation body and silently bypassed Medic.
    g=MedicalGame(data)
    g.ranks(1<<20)
    ally=g.player(1,0,(1,0,0))
    pool=g.pools[ally]['body']
    g.word(pool+0xa0,bits(.1))
    g.word(PLAYER+0x108,HOLDER_VT)
    g.word(DM,g.read(PLAYER+0x2e0))
    inventory=g.block(0x200);g.word(inventory+0x174,PLAYER+0x108)
    vt=g.block(0x100);g.word(vt+0x80,0x312250)
    items=[]
    for slot in [13,14]:
        item=g.block(0x500);items.append(item)
        for off,value in [(0,vt),(0x250,inventory),(0x24c,1),(0x280,3),(0x48c,3000)]:g.word(item+off,value)
        g.word(inventory+0x10c+slot*4,item)
    g.call(0x313300,(1,1234),ecx=items[0])
    close(g.float(pool+0xa0),g.float(pool+0x9c))
    for item in items:
        assert g.read(item+0x280)==4 and g.read(item+0x2a0)==1234
        assert g.read(item+0x494)==3000
    # Repeated press and key-up cannot heal again while both items are active.
    g.word(pool+0xa0,bits(.1))
    for pressed in [1,0]:
        g.call(0x313300,(pressed,2234),ecx=items[0])
        close(g.float(pool+0xa0),.1)
    # Pressing the active item still fires another ready artefact of its kind.
    g.word(items[1]+0x280,3)
    g.call(0x313300,(1,3234),ecx=items[0])
    close(g.float(pool+0xa0),g.float(pool+0x9c))
    assert g.read(items[0]+0x2a0)==1234 and g.read(items[1]+0x2a0)==3234
    assert g.read(ally+0xd0)==10
    print('Medic: real detoured action entry, multiple Larkspurs, key-up, active/ready mix and references: PASS')

# Concentration (patch 94): breath_vibration_calculator::tick from after its
# profiler call to the store of apply_modifier's result (0x31D72D), the real
# getter of modifier 61 between: the site 0x31D727 is inside.
SWAY_FROM, SWAY_TO = 0x31D6F1, 0x31D72D

def check_sway(data,kit=0):
    # The dispersion B (the tick's first argument), the kit's sway sum S
    # (modifier 61, the gear's -20 % among them), Concentration's ranks 0..7
    # (5..7 refused: no skill at all) with or without Unflinching (refused
    # beside it), painkiller on/off: B x max(0, 1 + S - 0.05 x rank).
    outcome=[]
    for concentration in range(8):
        for unflinching in (0,1):
            for active in (False,True):
                g=MedicalGame(data)
                g.emu.mem_write(PLAYER+0x115,bytes([kit]))
                g.combo((concentration<<7)|(unflinching<<4))
                g.carried(False,True,3)          # an active bandage is no painkiller
                g.carried(True,active)
                valid=concentration<=4 and not (unflinching and concentration)
                rank=concentration if valid and active else 0
                calculator=g.block(0x80);g.word(calculator+0x3c,PLAYER)
                frame=g.block(0x20);g.word(frame+0xc,50)
                for s in (0,-.2,-.9,-1.5):
                    g.ranks(s,61)
                    for base in (0,.3,2):
                        g.word(frame+8,bits(base))
                        sway=g.step(SWAY_FROM,SWAY_TO-SWAY_FROM,edi=calculator,ebp=frame)
                        close(sway,base*max(0,1+s-.05*rank))
                        assert g.read(frame+8)==bits(base)
                        outcome.append((concentration,unflinching,active,s,base,sway))
    print('Concentration sway: modifier 61 -5% a rank added to the kit\'s (0 and -20% gear, -90%, -150%), 0..4 (5..7 and with Unflinching refused), painkiller on/off (a bandage none), no dispersion none, never below 0: PASS')
    return outcome

def check_regeneration(data,kit=0):
    # The kit's regeneration modifiers (10 seconds off the delay, 9 the limit
    # raised: the Regeneration booster's -2 and 1) at body_part::regenerate's
    # join 0x32A53F, then the whole native body_part::regenerate. Modifier 10
    # as the kit's own float, 9 as an item's node in its list (as a booster in
    # a pocket adds it), so both of the getter's sums are run.
    g=MedicalGame(data)
    g.emu.mem_write(PLAYER+0x115,bytes([kit]))
    container=KITS+0xf4+kit*0x704
    node=g.block(0x10)
    def booster(on):
        g.word(container+10*4,bits(-2 if on else 0))
        g.word(node+4,0);g.word(node+0xc,bits(1 if on else 0))
        g.word(container+0x110+9*0x14+4,node);g.emu.mem_write(container+0x120+9*0x14,b'\1')
    g.ranks(5<<6)
    g.step(0x2d706f,7,ebp=PLAYER)
    pools=g.pools[PLAYER]
    for on in [False,True]:
        booster(on)
        for name,rate,limit in [('body',.03,.5),('head',.03,.5),('left_leg',.03,.2),('pain',.1,1),('right_hand',0,0)]:
            pool=pools[name];delay=g.read(pool+0xac)
            assert delay==5500
            stack=[0]*12;stack[4]=bits(rate);stack[5]=delay;stack[6]=bits(limit);stack[9]=12345
            g.step(0x32a53f,6,stack=stack,eax=delay,edi=pool)
            regenerates=on and rate>0
            assert g.emu.reg_read(UC_X86_REG_EAX)==(3500 if regenerates else 5500),name
            close(g.float(STACK-0x100+0x18),1 if regenerates else limit)
            assert g.emu.reg_read(UC_X86_REG_EBP)==12345
    # The native regenerates the body from 10 %: to its 50 % without the
    # booster, to full with it; never before the delay.
    body=pools['body']
    for on,now,expected in [(False,5000,.1),(False,200000,.5),(True,3400,.1),(True,200000,1)]:
        booster(on)
        g.word(body+0xa0,bits(.1));g.word(body+0xa4,bits(.03));g.word(body+0xa8,bits(.5));g.word(body+0xb0,0)
        g.call(0x32a4b0,(now,now),edi=body)
        close(g.float(body+0xa0),expected)
        close(g.float(body+0xa8),.5)
    print('Regeneration booster: delay after Metabolism 5.5 -> 3.5 s, limit 50 %/20 % -> 100 %, pools without regeneration and kits without it unchanged; native regenerate to full: PASS')

def main():
    original=open(sys.argv[1],'rb').read()
    patched,_=base.build(original,'section',open(base.default_bin('section'),'rb').read(),None,open(base.late_join_bin(),'rb').read())
    check_pressed_entry(patched)
    assert check_sway(patched)==check_sway(patched,2)
    check_particle_capacity(patched)
    check_regeneration(patched);check_regeneration(patched,2)
    assert check_combo(patched)==check_combo(patched,2)
    assert run(patched)==run(patched,0x4000,2)==run(patched,0x8000,1)
    print("Medical simulation outcome identical across independent client memory layouts and all three kits: PASS")

if __name__=='__main__':main()

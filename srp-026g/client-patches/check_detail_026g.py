#!/usr/bin/env python3
"""Run real native model/material selection and texture requests with detail on/off.
Usage: python check_detail_026g.py original.exe [--out directory]
"""
import argparse
import json
import os
from pathlib import Path
import struct
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import *
import patch_026g as base
import check_fov_lod_026g as fov

OPTIONS = fov.WORK + 0xD000

def u32(emu, address):
    return struct.unpack('<I', emu.mem_read(address, 4))[0]

def write32(emu, address, value):
    emu.mem_write(address, struct.pack('<I', value))

def mode(emu, adaptive):
    write32(emu, 0x2260498, OPTIONS)
    emu.mem_write(OPTIONS + 0x132, bytes([adaptive]))

def run_until(emu, start, end):
    emu.emu_start(start, end, count=200000)
    assert emu.reg_read(UC_X86_REG_EIP) == end, hex(start)

def model(emu, distance, kind, availability=(1,1,1,0)):
    # Initialize native model layout with the existing FOV harness, then use
    # both actual distance- and screen-size-based content descriptors.
    fov.select_lod(emu, distance)
    params = (20.,40.,240.) if kind == 0 else (.4,.2,.001)
    emu.mem_write(fov.LODS, bytes(availability))
    emu.mem_write(fov.LODS+16, struct.pack('<I3fI',kind,*params,0))
    return fov.call(emu, fov.SELECT_LOD, [fov.MODEL,fov.MATRIX], {UC_X86_REG_EAX:fov.EYE}) & 255

def shader(emu, factor, quality, shader_lods):
    write32(emu, OPTIONS+0xE8, quality)
    emu.mem_write(OPTIONS+0x131, bytes([shader_lods]))
    emu.mem_write(fov.SURFACE+0x58, struct.pack('<f',factor))
    return fov.call(emu,0x158d20,[fov.SURFACE],{}) & 255

def texture_request(emu, bias):
    # Run the actual non-streamed request branch through its mip-cut write.
    manager=fov.WORK+0x1000
    # Far manager field: put its base below WORK so +88194h stays mapped.
    manager=fov.WORK+0x3000-0x88194
    write32(emu,0x226047c,manager)
    write32(emu,manager+0x88194,bias)
    esp=fov.STACK+0x8000
    emu.reg_write(UC_X86_REG_ESP,esp)
    run_until(emu,0x3d9c32,0x3d9c50)
    return u32(emu,esp+0x240),u32(emu,esp+0x244)

def menu(emu):
    tab,item,command,callback,allocator,vtable=(fov.WORK+i for i in (0x2000,0x2100,0x2200,0x2300,0x2400,0x2500))
    array=fov.WORK+0x2600
    write32(emu,tab,array); write32(emu,tab+8,2)
    write32(emu,0x1f1d9c4,allocator); write32(emu,0xaf30e0,allocator)
    write32(emu,allocator,vtable)
    alloc_callback=fov.RETURN+0x100
    write32(emu,vtable+0x28,alloc_callback)
    write32(emu,command,0xa28600)
    write32(emu,command+0x40,OPTIONS+0x132)
    write32(emu,command+0x48,OPTIONS+0x25e)
    def returned(uc,value=0,cleanup=0):
        esp=uc.reg_read(UC_X86_REG_ESP)
        uc.reg_write(UC_X86_REG_EAX,value)
        uc.reg_write(UC_X86_REG_EIP,u32(uc,esp))
        uc.reg_write(UC_X86_REG_ESP,esp+4+cleanup)
    calls=[]
    def intercept(uc,address,size,data):
        if address not in (0xe4f90,0xeb7c0,alloc_callback): return
        if address==0xe4f90:
            assert uc.reg_read(UC_X86_REG_EAX)==0x20
            answer,cleanup=item,4
        elif address==0xeb7c0:
            name=bytes(uc.mem_read(uc.reg_read(UC_X86_REG_ESI),24)).split(b'\0')[0]
            assert name==b'r_use_texture_streaming',name
            answer,cleanup=command,0
        else: answer,cleanup=callback,8
        calls.append(address); returned(uc,answer,cleanup)
    handle=emu.hook_add(UC_HOOK_CODE,intercept)
    for site in (0x262c08,0x262c17):
        esp=fov.STACK+0x8000
        emu.mem_write(esp,struct.pack('<6I',11,22,33,fov.RETURN,44,55))
        emu.reg_write(UC_X86_REG_ESP,esp); emu.reg_write(UC_X86_REG_EBX,tab)
        run_until(emu,site,fov.RETURN)
        assert emu.reg_read(UC_X86_REG_ESP)==esp+24
        assert u32(emu,array+19*4)==item
        assert u32(emu,item)==base.arena().symbols['detail_option_vtable']+4
        assert u32(emu,item+8)==command and u32(emu,item+0x10)==tab
        assert bytes(emu.mem_read(item+0x14,1))==b'\x13'
        assert u32(emu,item+0xc)==1, 'must be a native string selector, not a checkbox'
        assert bytes(emu.mem_read(item+0x1c,1))==b'\2'
    emu.hook_del(handle)
    option_vtable=u32(emu,item)
    # driven: initialize through options_tab::initialize_data at 0x263163
    for adaptive in (True,False):
        mode(emu,adaptive)
        emu.reg_write(UC_X86_REG_EDI,tab); emu.reg_write(UC_X86_REG_ESI,19)
        emu.reg_write(UC_X86_REG_ESP,fov.STACK+0x8000)
        run_until(emu,0x263163,0x26316f)
        assert bytes(emu.mem_read(item+0x1d,2))==bytes([int(not adaptive)])*2
    print("driven: initialize through options_tab::initialize_data at 0x263163")
    preset=fov.WORK+0x2800
    write32(emu,array+8*4,preset); write32(emu,preset+0x10,tab)
    emu.mem_write(preset+0x14,b'\x08'); emu.mem_write(preset+0x1c,b'\x06')
    # The preset file fills the original table at runtime. Use distinctive
    # values to prove all ten existing entries are forwarded unchanged.
    for quality in range(5):
        for i in range(10): emu.mem_write(0x1f1b188+(quality*10+i)*8,struct.pack('<IB3x',9+i,quality))
    owner,game,wrapper,movie=(fov.WORK+i for i in (0x3000,0x3100,0x3300,0x3400))
    write32(emu,tab+0x10,owner); write32(emu,owner,game)
    write32(emu,game+0x108,wrapper); write32(emu,wrapper+4,movie)
    params,arg,ret=(fov.WORK+i for i in (0x3500,0x3540,0x3580))
    updates=[]
    def external(uc,address,size,data):
        if address==0x511600: # GFx Movie::Invoke: only the player boundary is mocked.
            esp=uc.reg_read(UC_X86_REG_ESP)
            name=bytes(uc.mem_read(u32(uc,esp+4),15)).split(b'\0')[0]
            assert name==b'root.set_value',name
            argv,count=u32(uc,esp+12),u32(uc,esp+16)
            assert count==4
            assert all(u32(uc,argv+i*24+4)==4 for i in range(count))
            updates.append([u32(uc,argv+i*24+8) for i in range(count)])
            returned(uc,1,16)
        elif address==0xe4590: # CRT formatting requires Windows TLS.
            esp=uc.reg_read(UC_X86_REG_ESP); value=u32(uc,esp+8)
            assert value in (0,1)
            buffer=u32(uc,uc.reg_read(UC_X86_REG_EAX))
            uc.mem_write(buffer,str(value).encode()+b'\0'); returned(uc)
    handle=emu.hook_add(UC_HOOK_CODE,external)
    def choose(receiver,index,entry=0x261310):
        emu.mem_write(arg,struct.pack('<6I',0,4,index,0,0,0)) # native UInt, as all quality options
        emu.mem_write(ret,b'\0'*24); emu.mem_write(params,struct.pack('<2I',ret,arg))
        fov.call(emu,entry,[params],{UC_X86_REG_ECX:receiver})
        assert u32(emu,ret+4)==4 and u32(emu,ret+8)==index, 'native integer result'
    traces=[]; manual=[]
    for quality in range(5):
        updates.clear(); choose(preset,quality,0x2620a0)
        expected=[[2,9+i,quality,0] for i in range(10)]+[[2,19,int(quality==4),0]]
        assert updates==expected,(quality,updates)
        traces.append(list(updates))
        updates.clear(); choose(item,int(quality==4))
        assert not updates, 'matching preset must stay named'
        manual.append({'preset':quality,'choice':int(quality==4),'updates':list(updates)})
        updates.clear(); choose(item,int(quality!=4))
        assert updates==[[2,8,5,0]], 'manual mismatch must invoke the stock Custom update'
        manual.append({'preset':quality,'choice':int(quality!=4),'updates':list(updates)})
    updates.clear(); choose(preset,5,0x2620a0)
    assert not updates, 'Custom must not overwrite any individual option'
    updates.clear()
    # Default graphics uses the same original ten rows, plus Maximum.
    options=fov.WORK+0x3700; write32(emu,options+0xc,game)
    fov.call(emu,0x1ff460,[options],{})
    assert updates==[[2,9+i,4,0] for i in range(10)]+[[2,19,1,0]]
    # Repeat both directions, including genuine UInt return values and native
    # fill_value. No state reaches the renderer until the tab calls Apply.
    mode(emu,True)
    emu.mem_write(preset+0x1d,b'\x05')
    for choice in (1,0,1,0,1):
        updates.clear(); choose(item,choice)
        fov.call(emu,0x261260,[ret],{UC_X86_REG_ECX:item})
        assert u32(emu,ret+4)==4 and u32(emu,ret+8)==choice
        assert bytes(emu.mem_read(OPTIONS+0x132,1))==b'\1'
    # Cancel restores the saved integer via the stock revert / fill_value.
    emu.mem_write(item+0x1e,b'\0'); updates.clear()
    fov.call(emu,0x261300,[],{UC_X86_REG_ECX:item})
    assert updates==[[2,19,0,1]] and bytes(emu.mem_read(item+0x1d,2))==b'\0\0'
    for choice in (1,0):
        choose(item,choice)
        before=bytes(emu.mem_read(OPTIONS+0x132,1))
        # driven: Apply through options_tab::apply at 0x263000
        emu.reg_write(UC_X86_REG_EDI,tab); emu.reg_write(UC_X86_REG_EBX,19)
        emu.reg_write(UC_X86_REG_ESP,fov.STACK+0x8000)
        run_until(emu,0x263000,0x26300f)
        assert bytes(emu.mem_read(OPTIONS+0x132,1))==bytes([1-choice])
        assert bytes(emu.mem_read(OPTIONS+0x25e,1))==before
        assert bytes(emu.mem_read(item+0x1d,2))==bytes([choice])*2
        # Reopen reads the command written by Apply.
        emu.mem_write(item+0x1d,b'\xff\xff')
        emu.reg_write(UC_X86_REG_EDI,tab); emu.reg_write(UC_X86_REG_ESI,19)
        emu.reg_write(UC_X86_REG_ESP,fov.STACK+0x8000)
        run_until(emu,0x263163,0x26316f)
        assert bytes(emu.mem_read(item+0x1d,2))==bytes([choice])*2
    print("driven: Apply through options_tab::apply at 0x263000")
    emu.hook_del(handle)
    return {'presets':traces,'manual':manual,'manual_custom_cases':5,'arrow_directions':5,'apply_reopen_cases':2,'cancel_cases':1,'default_entries':11}

def grass(emu, distance, thresholds, available):
    patch=fov.WORK+0x4000
    write32(emu,patch+0x409c,available)
    emu.mem_write(OPTIONS+0x50,struct.pack('<2f',*thresholds))
    emu.reg_write(UC_X86_REG_EAX,patch)
    emu.reg_write(UC_X86_REG_ECX,OPTIONS)
    emu.reg_write(UC_X86_REG_XMM0,struct.unpack('<I',struct.pack('<f',distance*distance))[0])
    run_until(emu,0x1605d6,0x160634)
    return u32(emu,patch+0x4098)

def labels(emu):
    array=fov.WORK+0x9000
    size=20*0x4c
    emu.mem_write(array,b'\xa5'*(size+16))
    emu.reg_write(UC_X86_REG_EAX,array)
    emu.reg_write(UC_X86_REG_EDI,size)
    # Execute the native array's buffer_string constructors.
    run_until(emu,0x202181,0x2021b3)
    previous=bytes(emu.mem_read(array,19*0x4c))
    for flags in (0x202,0x246):
        emu.reg_write(UC_X86_REG_ESP,fov.STACK+0x8000)
        emu.reg_write(UC_X86_REG_ESI,array)
        emu.reg_write(UC_X86_REG_EFLAGS,flags)
        run_until(emu,0x2009b9,0x2009bf)
        assert emu.reg_read(UC_X86_REG_EFLAGS)==flags
        assert emu.reg_read(UC_X86_REG_EAX)==array+0x558
        label=bytes(emu.mem_read(array+19*0x4c+12,64)).split(b'\0')[0]
        assert label==b'st_distance_detail_option',label
        assert bytes(emu.mem_read(array,19*0x4c))==previous
        assert bytes(emu.mem_read(array+size,16))==b'\xa5'*16

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('original',type=Path)
    parser.add_argument('--out',type=Path)
    args=parser.parse_args()
    os.environ['SRP_026G_EXE']=str(args.original.resolve())
    original=args.original.read_bytes()
    data,_=base.build(original,'section',Path(base.default_bin('section')).read_bytes(),None,Path(base.late_join_bin()).read_bytes())
    native,patched=fov.image(original),fov.image(data)
    compared=forced=0
    examples=[]
    for angle in (50.,65.,90.,100.):
        for emu in (native,patched):
            mode(emu,True)
            fov.projection(emu,65.,1.)
        # Compare at stock FOV: wider-FOV compensation is an existing patch.
        for kind in (0,1):
            for distance in (1.,5.,10.,19.,20.,21.,39.,40.,41.,100.,239.,240.,241.,500.,1000.):
                a=model(native,distance,kind)
                mode(patched,True)
                b=model(patched,distance,kind)
                assert a==b,(kind,distance,a,b)
                compared+=1
                mode(patched,False)
                fov.projection(patched,angle,1.)
                assert model(patched,distance,kind)==0
                forced+=1
                if angle==65 and kind==0: examples.append({'distance_m':distance,'adaptive_lod':a,'full_lod':0})
                fov.projection(patched,65.,1.)
    for available,expected in (((0,0,0,0),0xaa),((1,0,0,0),0)):
        assert model(patched,500.,1,available)==expected
    shaders=0
    for quality in (0,1,2):
        for enabled in (False,True):
            for factor in (0.,.0001,.001,.1,1.,10.,100.):
                mode(native,True);mode(patched,True)
                a=shader(native,factor,quality,enabled)
                assert shader(patched,factor,quality,enabled)==a
                mode(patched,False)
                assert shader(patched,factor,quality,enabled)==0
                shaders+=1
    for bias in (0,1,2):
        mode(native,True);mode(patched,True)
        assert texture_request(native,bias)==texture_request(patched,bias)==(0xffffffff,bias)
        mode(patched,False)
        assert texture_request(patched,bias)==(0xffffffff,0)
    grass_cases=0
    for thresholds in ((40.,60.),(60.,40.)):
        for available in (0,1,2,3):
            for distance in (1.,39.,40.,41.,59.,60.,61.,100.,1000.):
                mode(native,True);mode(patched,True)
                assert grass(native,distance,thresholds,available)==grass(patched,distance,thresholds,available)
                mode(patched,False)
                assert grass(patched,distance,thresholds,available)==0
                grass_cases+=1
    menu_calls=menu(patched)
    labels(patched)
    result={'adaptive_model_cases_equal':compared,'full_detail_model_cases_lod0':forced,'shader_cases':shaders,'grass_cases':grass_cases,'texture_bias_cases':3,'native_menu':menu_calls,'distance_lod_examples':examples,'live_performance_measured':False}
    if args.out:
        args.out.mkdir(parents=True,exist_ok=True)
        (args.out/'native-check.json').write_text(json.dumps(result,indent=2)+'\n')
        (args.out/'survarium.detail.exe').write_bytes(data)
    print(json.dumps(result,indent=2))
    print('DETAIL PASS')

if __name__=='__main__':main()

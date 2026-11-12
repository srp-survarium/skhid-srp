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
    write32(emu,tab,array)
    write32(emu,0x1f1d9c4,allocator)
    write32(emu,0xaf30e0,allocator)
    write32(emu,allocator,vtable)
    alloc_callback=fov.RETURN+0x100
    write32(emu,vtable+0x28,alloc_callback)
    write32(emu,command,0xa28600)
    write32(emu,command+0x40,OPTIONS+0x132)
    write32(emu,command+0x48,OPTIONS+0x25e)
    calls=[]
    def intercept(uc,address,size,data):
        if address not in (0xe4f90,0xeb7c0,alloc_callback): return
        esp=uc.reg_read(UC_X86_REG_ESP)
        if address==0xe4f90:
            assert uc.reg_read(UC_X86_REG_EAX)==0x1c
            answer,cleanup=item,4
        elif address==0xeb7c0:
            name=bytes(uc.mem_read(uc.reg_read(UC_X86_REG_ESI),24)).split(b'\0')[0]
            assert name==b'r_use_texture_streaming',name
            answer,cleanup=command,0
        else:
            answer,cleanup=callback,8
        calls.append(address)
        uc.reg_write(UC_X86_REG_EAX,answer)
        uc.reg_write(UC_X86_REG_EIP,u32(uc,esp))
        uc.reg_write(UC_X86_REG_ESP,esp+4+cleanup)
    handle=emu.hook_add(UC_HOOK_CODE,intercept)
    for site in (0x262c08,0x262c17):
        esp=fov.STACK+0x8000
        emu.mem_write(esp,struct.pack('<6I',11,22,33,fov.RETURN,44,55))
        emu.reg_write(UC_X86_REG_ESP,esp)
        emu.reg_write(UC_X86_REG_EBX,tab)
        run_until(emu,site,fov.RETURN)
        assert emu.reg_read(UC_X86_REG_ESP)==esp+24
        assert emu.reg_read(UC_X86_REG_EAX)==tab
        assert u32(emu,array+19*4)==item
        assert u32(emu,item)==0xa06744
        assert u32(emu,item+8)==command
        assert u32(emu,item+0x10)==tab
        assert bytes(emu.mem_read(item+0x14,1))==b'\x13'
        assert u32(emu,item+0xc)==0
    emu.hook_del(handle)
    # Native options_item_bool initialize, then a pending checkbox edit and
    # native Cancel: the active renderer value must not change at either step.
    mode(emu,True)
    fov.call(emu,0x2617c0,[],{UC_X86_REG_ECX:item})
    assert bytes(emu.mem_read(item+0x18,2))==b'\1\1'
    emu.mem_write(item+0x18,b'\0')
    assert bytes(emu.mem_read(OPTIONS+0x132,1))==b'\1'
    emu.reg_write(UC_X86_REG_ECX,item)
    run_until(emu,0x261880,0x260fd0)
    assert bytes(emu.mem_read(item+0x18,2))==b'\1\1'
    assert bytes(emu.mem_read(OPTIONS+0x132,1))==b'\1'
    # Native Apply executes render_cc_bool, including its previous-value
    # tracking used by the renderer to decide that textures need reloading.
    emu.mem_write(item+0x18,b'\0')
    def format_bool(uc,address,size,data):
        # The CRT formatter needs Windows TLS. Emulate only "%u" here;
        # execute the real menu Apply and render_cc_bool parsing afterwards.
        if address != 0xe4590: return
        esp=uc.reg_read(UC_X86_REG_ESP)
        value=u32(uc,esp+8)
        assert value in (0,1)
        buffer=u32(uc,uc.reg_read(UC_X86_REG_EAX))
        uc.mem_write(buffer,str(value).encode()+b'\0')
        uc.reg_write(UC_X86_REG_EIP,u32(uc,esp))
        uc.reg_write(UC_X86_REG_ESP,esp+4)
    handle=emu.hook_add(UC_HOOK_CODE,format_bool)
    fov.call(emu,0x261820,[],{UC_X86_REG_ECX:item})
    emu.hook_del(handle)
    assert bytes(emu.mem_read(OPTIONS+0x132,1))==b'\0'
    assert bytes(emu.mem_read(OPTIONS+0x25e,1))==b'\1'
    return len(calls)

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
    result={'adaptive_model_cases_equal':compared,'full_detail_model_cases_lod0':forced,'shader_cases':shaders,'grass_cases':grass_cases,'texture_bias_cases':3,'menu_intercepted_allocations_and_lookups':menu_calls,'distance_lod_examples':examples,'live_performance_measured':False}
    if args.out:
        args.out.mkdir(parents=True,exist_ok=True)
        (args.out/'native-check.json').write_text(json.dumps(result,indent=2)+'\n')
        (args.out/'survarium.detail.exe').write_bytes(data)
    print(json.dumps(result,indent=2))
    print('DETAIL PASS')

if __name__=='__main__':main()

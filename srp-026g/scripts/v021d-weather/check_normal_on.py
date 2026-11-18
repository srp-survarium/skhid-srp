"""Independent inverse audit for source-built missing normal-blend programs."""
from pathlib import Path
import argparse
import json
import struct
import hashlib
import re
import subprocess
import tempfile
from check_vertex_blend import chunks,instructions,check_program
from check_normal_off import reflection

from shader_operands import operand, operands

def dead_coordinates(ops,at):
    block=ops[at-6:at]
    assert [x[0]&2047 for x in block] in ([56,0,15,15,0,56],[0,56,15,15,0,56])
    dead=set().union(*(operands(op)[0] for op in block))
    dead-=operands(ops[at])[0]
    for op in ops[at+1:]:
        if not dead:break
        assert op[0]&2047 in (0,14,15,16,24,49,50,51,52,54,55,56,69,75,47,25), ('unreviewed control flow',op[0]&2047)
        args=operands(op)
        assert not dead & set().union(*args[1:]), ('live removed coordinate',dead,op)
        dead-=args[0]
    assert not dead

def mask_taint(ops,mask_at):
    # Conservative component taint: unrelated live values can only add false
    # positives. The explicit original-selector override kills the weight.
    taint=operands(ops[mask_at])[0]
    weights=[(i,o) for i,o in enumerate(ops) if o[0]==0x08002038 and o[-3:]==[0x0020801a,0,14]]
    assert len(weights)==1
    weight_at,weight=weights[0];assert mask_at<weight_at
    weight_components=operands(weight)[0]
    for i in range(mask_at+1,len(ops)):
        if i==weight_at+1:
            taint-=weight_components
        if weight_at<i<=weight_at+24:continue
        op=ops[i];kind=op[0]&2047
        if kind==62:break
        if kind in (21,18):continue
        args=operands(op)
        if kind in (31,13):
            assert not taint & set().union(*args), 'mask affects control/discard'
            continue
        used=set().union(*args[1:])
        influenced=bool(used & taint)
        # Output operands have no temporary components: explicitly reject a
        # dependency reaching any output register or resource side effect.
        dest_type=(op[1]>>12)&255
        assert not (influenced and dest_type!=0), 'mask escapes original weight override'
        taint-=args[0]
        if influenced:taint|=args[0]


def variable_records(raw):
    u=lambda i:struct.unpack_from('<I',raw,i)[0]
    s=lambda i:raw[i:raw.index(b'\0',i)].decode()
    result={}
    for i in range(u(0)):
      at=u(4)+24*i
      for j in range(u(at+4)):
        p=u(at+8)+j*24;result[s(u(p))]=(p,u(p+4),u(p+8),u(p+12))
    return result

def verify(row,native,adapted,actual):
    assert hashlib.sha256(native).hexdigest()==row['source_sha256']
    check_program(native,adapted,row['source_path'])
    oh,old=chunks(adapted);nh,new=chunks(actual)
    assert oh==nh and old.keys()==new.keys()
    assert all(old[k]==new[k] for k in old if k not in (b'SHDR',b'RDEF'))
    ver,before=instructions(old[b'SHDR']);v,after=instructions(new[b'SHDR']);assert v==ver
    change=row['conversion'];remove=change['kind'].startswith('mask_off');alpha=change['kind'].endswith('alpha_on')
    key=row['source_path'];target=key
    if remove:
        matches=list(re.finditer(r'_([01]1)1([01]1)____',target));assert len(matches)==1
        m=matches[0];target=target[:m.start()]+f'_{m[1]}0{m[2]}____'+target[m.end():]
    if alpha:
        prefix='shaders/sm_4_0/gbuffer_pass.ps/';k=list(target[len(prefix):]);assert k[8]=='0';k[8]='1';target=prefix+''.join(k)
        coverage=change['coverage'];matches=list(re.finditer(r'_([01]1[01])([01])1____',target));assert len(matches)==1
        m=matches[0];target=target[:m.start()]+f'_{m[1]}'+('1' if coverage=='blended' else '0')+'1____'+target[m.end():]
    assert target==row['path'],(target,row['path'])
    if alpha:
        weight=[(i,o) for i,o in enumerate(after) if o[0]==0x08002038 and o[-3:]==[0x0020801a,0,14]];assert len(weight)==1
        i,o=weight[0];at=i+25;scratch=next(o[1] for o in after if o[0]&2047==104)-1
        sample=after[at];assert sample==[0x09000045,0x00100012,scratch,0x00101046,7,0x00107ff6,0,0x00106000,row['base_sampler']]
        cursor=at+1
        if coverage=='blended':
            component=((o[1]>>4)&15).bit_length()-1
            assert after[cursor]==[0x07000000,0x00100012,scratch,0x0010000a,scratch,0x00004001,0xbf800000]
            assert after[cursor+1]==[0x09000032,0x00100012,scratch,0x0010000a|(component<<4),o[2],0x0010000a,scratch,0x00004001,0x3f800000]
            cursor+=2
        assert after[cursor:cursor+3]==[
          [0x09000000,0x00100012,scratch,0x0010000a,scratch,0x8020802a,0x41,0,15],
          [0x07000031,0x00100012,scratch,0x0010000a,scratch,0x00004001,0],
          [0x0304000d,0x0010000a,scratch]]
        del after[at:cursor+3]
    if remove:
        slot=change['mask_slot'];samples=[i for i,o in enumerate(before) if o[0]==0x09000045 and o[6]==slot];assert len(samples)==1
        sample_at=samples[0];dead_coordinates(before,sample_at);mask_taint(before,sample_at)
        declarations=[i for i,o in enumerate(before) if o[0]&2047==88 and o[2]==slot];assert len(declarations)==1
        declaration=declarations[0];assert declaration<sample_at-6
        replacement_at=sample_at-7
        assert after[replacement_at]==[0x09000045,*before[sample_at][1:3],0x00101046,7,0x00107ff6,0,0x00106000,row['base_sampler']]
        after[replacement_at:replacement_at+1]=before[sample_at-6:sample_at+1]
        after.insert(declaration,before[declaration])
    assert before==after, (row['path'],'unrelated instruction change')
    # Recover reflection byte-for-byte, including unused records and names.
    expected=bytearray(old[b'RDEF']);oldvars=variable_records(expected);newvars=variable_records(new[b'RDEF'])
    if remove:
        at,offset,size,flags=oldvars['vertex_blend_tiled_mask_parameters'];assert (offset,size,flags)==(208,12,2)
        struct.pack_into('<I',expected,at+12,0)
        count,start=struct.unpack_from('<II',expected,8)
        def name(at):
            p=struct.unpack_from('<I',expected,at)[0];return expected[p:expected.index(0,p)].decode()
        ids=[i for i in range(count) if name(start+i*32)=='t_vertex_blended_tiled_mask'];assert len(ids)==1
        at=start+ids[0]*32;expected[at:start+(count-1)*32]=expected[at+32:start+count*32];expected[start+(count-1)*32:start+count*32]=bytes(32);struct.pack_into('<I',expected,8,count-1)
    if alpha:
        at,offset,size,flags=oldvars['alpha_ref_parameter'];assert (offset,size,flags)==(248,4,0);struct.pack_into('<I',expected,at+12,2)
    assert bytes(expected)==new[b'RDEF'],(row['path'],'unexpected reflection/binding delta')
    oldused,oldbindings=reflection(old);newused,newbindings=reflection(new)
    if remove:oldbindings.pop('t_vertex_blended_tiled_mask')
    assert oldbindings==newbindings
    assert old[b'ISGN']==new[b'ISGN'] and old[b'OSGN']==new[b'OSGN']

def check(porter, stock, patch, pack, blend_pack, patches, patch_cache):
    catalog = json.loads(Path(__file__).with_name('normal-on-programs.json').read_text())
    assert len(catalog) == 704
    manifest = json.loads((pack / 'pack.json').read_text())
    paths = {row['path'] for row in catalog}
    assert len(paths) == len(catalog) == len(manifest['operations'])
    assert {row['path'] for row in manifest['operations']} == paths
    assert all(row['op'] == 'add_file' for row in manifest['operations'])
    # Additions cannot shadow any existing native or published map behavior.
    archives = [stock] + [patch_cache / line.split()[0] for line in patches.read_text().splitlines()
                          if line.startswith('resources.db.patch_')]
    for archive in archives:
        existing = set(subprocess.check_output([str(porter), 'list', '--db', str(archive)], text=True).splitlines())
        assert paths.isdisjoint(existing), ('existing shader replaced', str(archive))
    cache = {}
    with tempfile.TemporaryDirectory() as tmp:
        def read(db, path):
            out = Path(tmp) / 'shader'
            subprocess.run([str(porter), 'extract', '--db', str(db), '--path', path, '--output', str(out)],
                           check=True, stdout=subprocess.DEVNULL)
            return out.read_bytes()
        for row in catalog:
            fingerprint = row['source_sha256']
            if fingerprint not in cache:
                cache[fingerprint] = read(stock, row['source_path'])
            native = cache[fingerprint]
            adapted = (blend_pack / 'files' / row['source_path']).read_bytes()
            actual = read(patch, row['path'])
            assert actual == (pack / 'files' / row['path']).read_bytes(), row['path']
            verify(row, native, adapted, actual)
    print(f'Normal-blend closure: {len(catalog)} added aliases from {len(cache)} pinned native programs; '
          'native instructions and bindings preserved, mask dependency eliminated, alpha cutoff verified')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('porter', 'stock', 'patch', 'pack', 'blend-pack', 'patches', 'patch-cache'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    check(args.porter, args.stock, args.patch, args.pack, args.blend_pack, args.patches, args.patch_cache)

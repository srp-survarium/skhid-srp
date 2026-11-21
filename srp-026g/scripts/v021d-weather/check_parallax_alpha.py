"""Verify the missing ordinary parallax family, native finish and painted cutoff."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
from check_vertex_alpha import geometry, original_block, SAMPLES, PROFILES, run
from check_vertex_blend import chunks, instructions


def assemble_for_check(header, cs):
    # Only a parser input for the mathematical checker, never emitted as a game asset.
    code = bytearray(b'DXBC'+bytes(16)+struct.pack('<III', 1, 0, len(cs))+bytes(4*len(cs)))
    for i, (tag, raw) in enumerate(cs.items()):
        struct.pack_into('<I', code, 32+4*i, len(code))
        code.extend(tag+struct.pack('<I', len(raw))+raw)
    struct.pack_into('<I', code, 24, len(code))
    return header+code


def template(native, vertex):
    header, cs = chunks(native)
    version, ops = instructions(cs[b'SHDR'])
    matches = [i for i,o in enumerate(ops) if o[0]&2047 == 13]
    assert len(matches) == 1
    discard = matches[0]
    temp = next(o for o in ops if o[0] == 0x02000068)
    scratch = temp[1]
    temp[1] += 1
    variable = vertex in (1,2)
    at = discard-16 if variable else discard
    if variable:
        removed = ops[at:discard-2]
        assert [o[0]&2047 for o in removed] == [0,56,15,15,0,56,69,0,56,56,47,56,25,56]
        assert ops[discard-2:] and ops[discard-2] == [0x07000000,0x00100082,0,0x0010003a,0,0x4001,0xbecccccd]
        assert ops[discard-1] == [0x07000031,0x00100082,0,0x0010003a,0,0x4001,0]
        assert ops[discard] == [0x0304000d,0x0010003a,0]
        # Every removed write is dead at the native finish boundary. r1.zw are
        # overwritten by the roughness sample (which reads r1.xy only); r0.w is
        # overwritten by roughness arithmetic; r3 is never referenced again.
        assert ops[discard+1] == [0x09000045,0x001000f2,1,0x00100046,1,0x00107e46,3,0x00106000,0]
        assert ops[discard+2] == [0x0b000032,0x00100082,0,0x0020803a,1,0,0x0010001a,1,0x0020802a,1,0]
        for instruction in ops[discard+3:]:
            for i, word in enumerate(instruction[:-1]):
                if word in (0x00100002,0x00100012,0x00100022,0x00100042,0x00100082,0x001000f2,
                            0x0010000a,0x0010001a,0x0010002a,0x0010003a,0x00100046):
                    assert instruction[i+1] != 3, 'removed mask temporary is live'
    else:
        assert ops[discard] == [0x0304000d,0x4001,0xffffffff]
    samples = [o for o in ops[:at] if o[0]&2047 == 69 and o[-4:-2] == [0x00107e46,1]]
    assert len(samples) == 1
    sample = samples[0]
    register = sample[2]
    assert sample[1] == 0x001000f2
    for instruction in ops[ops.index(sample)+1:at]:
        assert not (len(instruction)>2 and (instruction[1]>>12)&255 == 0
                    and instruction[2] == register and instruction[1]&0x80), 'base alpha overwritten'
    ops[at:discard+1] = [
        [0x09000000,0x00100012,scratch,0x0010003a,register,0x8020802a,0x41,1,15],
        [0x07000031,0x00100012,scratch,0x0010000a,scratch,0x4001,0],
        [0x0304000d,0x0010000a,scratch],
    ]
    if variable:
        declarations = [o for o in ops if o[0]&2047 == 88 and o[2] == 2]
        assert len(declarations) == 1
        ops.remove(declarations[0])
    words = [version, 2+sum(map(len,ops))]+[word for o in ops for word in o]
    cs[b'SHDR'] = struct.pack('<'+'I'*len(words),*words)
    r = bytearray(cs[b'RDEF'])
    u = lambda p:struct.unpack_from('<I',r,p)[0]
    name = lambda p:bytes(r[u(p):r.index(0,u(p))]).decode()
    changes = {'alpha_ref_parameter':(248,0,2)}
    if variable:
        changes.update(vertex_blend_parameters=(224,2,0),vertex_blend_tiled_mask_parameters=(208,2,0))
    found = set()
    for i in range(u(0)):
        p=u(4)+24*i
        for j in range(u(p+4)):
            v=u(p+8)+24*j
            if name(v) in changes:
                offset,before,after=changes[name(v)]
                assert (u(v+4),u(v+12)) == (offset,before)
                found.add(name(v));struct.pack_into('<I',r,v+12,after)
    assert found == set(changes)
    if variable:
        count,table=u(8),u(12)
        bindings=[bytes(r[table+i*32:table+(i+1)*32]) for i in range(count)]
        found=[i for i in range(count) if name(table+i*32)=='t_vertex_blended_tiled_mask']
        assert len(found)==1 and u(table+found[0]*32+20)==2
        del bindings[found[0]]
        r[table:table+count*32]=b''.join(bindings)+bytes(32)
        struct.pack_into('<I',r,8,count-1)
    cs[b'RDEF']=bytes(r)
    return assemble_for_check(header,cs)


def verify(read, stock, donor, patch, pack, stock_paths):
    here=Path(__file__).resolve().parent
    rows=json.loads((here/'parallax-alpha-programs.json').read_text())
    originals=json.loads((here/'vertex-alpha-programs.json').read_text())['geometry']['original']
    references={}
    for original in originals:
        blob=read(donor,original['path'])
        assert hashlib.sha256(blob).hexdigest()==original['sha256']
        block=original_block(blob)
        for profile,marker in PROFILES.items():
            for constant in (False,True):
                references[original['vertex'],marker,constant]=[
                    run(block,v,1. if constant else a,profile[:3],0.) for v,a in SAMPLES]
    expected=set()
    for row in rows:
        vertex=row['vertex']
        high=read(stock,row['high_path']);low=read(stock,row['low_path'])
        assert hashlib.sha256(high).hexdigest()==row['high_sha256']
        assert hashlib.sha256(low).hexdigest()==row['low_sha256']
        # All aliases and native low-quality parallax inactivity are checked from stock.
        assert high==read(stock,row['high_path'].replace('__0_02100','__2_12100'))
        low_dirt=row['high_path'].replace('/_3_', '/_0_')
        assert read(stock,low_dirt)==read(stock,low_dirt.replace('_10001_', '_00001_'))
        assert low==read(stock,row['low_path'].replace('__0_00000','__2_10000'))
        high_template=template(high,vertex)
        expected_targets={f'shaders/sm_4_0/gbuffer_pass.ps/_{q}___1001_10001_00____{vertex}_____0__{alias}00____00000____000__________0__'
                          for q in (0,3) for alias in ('0_000','2_100')}
        assert expected_targets==set(row['targets']) and not expected_targets & stock_paths
        expected.update(expected_targets)
        for path in row['targets']:
            actual=read(patch,path)
            assert actual==(pack/'files'/path).read_bytes()
            geometry(high_template if '/_3_' in path else low,actual,vertex,references)
    manifest=json.loads((pack/'pack.json').read_text())
    assert len(expected)==len(manifest['operations'])
    assert {o['path'] for o in manifest['operations']}==expected
    assert all(o['op']=='add_file' for o in manifest['operations'])
    print('Ordinary parallax alpha:28 absent paths; native height/finish inverse, dead mask removal, exact base alpha and7 original painted contracts PASS')


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for key in ('porter','stock','donor','patch','pack'):p.add_argument('--'+key,required=True,type=Path)
    a=p.parse_args()
    paths=set(subprocess.check_output([str(a.porter),'list','--db',str(a.stock),'--prefix','shaders/sm_4_0/gbuffer_pass.ps/'],text=True).splitlines())
    with tempfile.TemporaryDirectory(prefix='check-parallax-alpha-') as tmp:
        def read(db,path):
            out=Path(tmp)/'resource'
            subprocess.run([str(a.porter),'extract','--db',str(db),'--path',path,'--output',str(out)],check=True,stdout=subprocess.DEVNULL)
            return out.read_bytes()
        verify(read,a.stock,a.donor,a.patch,a.pack,paths)
if __name__=='__main__':main()

"""Verify original normal-off equations, ABI adaptations and native isolation."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import re

from check_vertex_blend import chunks, instructions

ORIGINAL = re.compile(r'^shaders/sm_4_0/gbuffer_pass.ps/_(?P<q>[03])___(?P<flags>[01]{10})__(?P<meta>[01]{2})____(?P<vertex>0|1|2|10|12|13|14)_____(?P<wind>[01])__(?P<mode>[012])_(?P<blend>[01]100)____(?P<features>[01]{5})____(?P<tail>[012]{3})__________(?P<end>[01])__$')


def input_signature(raw):
    count, offset = struct.unpack_from('<II', raw)
    assert offset == 8 and count <= 32 and len(raw) >= 8 + count * 24
    result = {}
    for i in range(count):
        at = 8 + i * 24
        name, ordinal, system, kind, register = struct.unpack_from('<IIIII', raw, at)
        semantic = raw[name:raw.index(b'\0', name)].decode()
        mask, used = raw[at + 20:at + 22]
        assert used & ~mask == 0
        key = semantic, ordinal, system, kind, register
        assert key not in result
        result[key] = mask
    return result


def compatible_inputs(original, native):
    source, target = input_signature(original), input_signature(native)
    return all(key in target and mask & ~target[key] == 0 for key, mask in source.items())


def reflection(code):
    raw = code[b'RDEF']
    def u(at):
        return struct.unpack_from('<I', raw, at)[0]
    def string(at):
        return raw[at:raw.index(b'\0', at)].decode()
    variables = {}
    for i in range(u(0)):
        buffer = u(4) + i * 24
        for j in range(u(buffer + 4)):
            var = u(buffer + 8) + j * 24
            if u(var + 12) & 2:
                variables[string(u(buffer)), string(u(var))] = (u(var + 4), u(var + 8))
    bindings = {string(u(u(12) + i * 32)): (u(u(12) + i * 32 + 4), u(u(12) + i * 32 + 20))
                for i in range(u(8))}
    return variables, bindings


import copy
from shader_operands import operands
import check_masked_blend

def all_variables(code):
 raw=bytearray(code[b'RDEF']);u=lambda i:struct.unpack_from('<I',raw,i)[0]
 for i in range(u(0)):
  cb=u(4)+i*24
  for j in range(u(cb+4)):
   at=u(cb+8)+j*24;struct.pack_into('<I',raw,at+12,2)
 return reflection(code|{b'RDEF':bytes(raw)})[0]

def variables(raw):
 u=lambda i:struct.unpack_from('<I',raw,i)[0];s=lambda i:raw[i:raw.index(b'\0',i)].decode();out={}
 for i in range(u(0)):
  cb=u(4)+24*i
  for j in range(u(cb+4)):
   at=u(cb+8)+24*j;out[(s(u(cb)),s(u(at)))]=(cb,at)
 return out

def verify(row,original,native,actual):
 assert hashlib.sha256(original).hexdigest()==row['original_sha256']
 assert hashlib.sha256(native).hexdigest()==row['native_reference_sha256']
 oh,old=chunks(original);_,reference=chunks(native);ah,new=chunks(actual)
 assert oh==ah and old.keys()==new.keys()
 assert compatible_inputs(old[b'ISGN'],reference[b'ISGN']) and old[b'OSGN']==reference[b'OSGN']
 used,bindings=reflection(old);native_used,native_bindings=reflection(reference);native_all=all_variables(reference)
 for k,value in used.items():
  target=native_used.get(k)
  if k[1]=='alpha_ref_parameter' and value==(248,4):target=native_all.get(k)
  if row['global_layout']=='scene_time_396_to_400' and k==('static_globals','scene_time'):
   assert value==(396,4) and target==(400,4)
  else:assert value==target,(row['original_path'],k,value,target)
 assert set(bindings)-set(native_bindings)<={'s_base','t_normal'}
 assert all(old[k]==new[k] for k in old if k not in (b'SHDR',b'RDEF'))
 version,before=instructions(old[b'SHDR']);v,after=instructions(new[b'SHDR']);assert version==v
 expected=bytearray(old[b'RDEF']);var=variables(expected)
 layout=row['blend_layout']
 if layout['kind']=='vertex_alpha':
  alpha=layout['alpha_register'];weight=layout['weight_operand'];assert (row['vertex'],alpha) in ((1,7),(2,9))
  wanted=[0x08000000,*weight,0x8010100a,0x41,alpha,0x00004001,0x3f800000]
  sites=[i for i,o in enumerate(before) if o==wanted];assert len(sites)==1;at=sites[0]+1
  temps=[(i,o) for i,o in enumerate(before) if o[0]&2047==104];assert len(temps)==1
  ti,to=temps[0];scratch=to[1]
  block=copy.deepcopy(after[at:at+20]);assert len(block)==20
  assert block[18]==[0x05000036,*weight,0x0010000a,scratch], 'wrong original second-texture coefficient'
  coord=layout['sample_coordinates'];texture=layout['sample_texture'];sampler=layout['sample_sampler'];cb=layout['material_buffer']
  assert bindings['t_base']==(2,texture) and bindings['deferred_material_parameters']==(0,cb)
  samples=[(i,o) for i,o in enumerate(before) if o[0]==0x09000045 and o[6]==texture];assert len(samples)==1
  si,sample=samples[0];assert sample[3:5]==coord and sample[7:]==[0x00106000,sampler]
  if coord[0]==0x00100046:
   assert si<at-1
   live={(coord[1],0),(coord[1],1)}
   assert all(not operands(o)[0]&live for o in before[si:at]),'parallax UV was overwritten'
  else:assert coord==[0x00101046,7]
  assert block[4]==[0x09000045,0x00100082,scratch,*coord,0x00107ff6,texture,0x00106000,sampler]
  # Normalize only bound resource slots and coefficient orientation; the existing
  # independently decoded original-equation checker validates every other word.
  for o in block:
   for j in range(1,len(o)-2):
    if o[j] in (0x0020801a,0x0020800a):
     assert o[j+1:j+3]==[cb,14];o[j+1]=0
  block[4][3:5]=[0x00101046,7];block[4][6]=0
  block[18]=[0x08000000,*weight,0x8010000a,0x41,scratch,0x00004001,0x3f800000]
  canonical_sample=[0x09000045,0x001000f2,0,0x00101046,7,0x00107e46,0,0x00106000,sampler]
  check_masked_blend.verify(block,[canonical_sample],scratch,weight,alpha)
  del after[at:at+20];assert after[ti]==[0x02000068,scratch+1];after[ti]=to
  _,p=var['deferred_material_parameters','vertex_blend_parameters']
  assert struct.unpack_from('<III',expected,p+4)==(160,12,0)
  struct.pack_into('<I',expected,p+4,224);struct.pack_into('<I',expected,p+12,2)
  assert native_all['deferred_material_parameters','vertex_blend_parameters']==(224,12)
 else:
  assert layout=={'kind':'constant_zero'} and row['vertex'] in (0,10,12,13,14)
 if row['global_layout']=='scene_time_396_to_400':
  slot=bindings['static_globals'][1];reads=0
  for op in after:
   for j in range(1,len(op)-2):
    if op[j:j+3]==[0x0020800a,slot,25]:op[j:j+3]=[0x0020803a,slot,24];reads+=1
  assert reads==1
  ds=[o for o in after if o[0]==0x04000059 and o[2]==slot];assert len(ds)==1 and ds[0][3]==26;ds[0][3]=25
  cb,p=var['static_globals','scene_time'];assert struct.unpack_from('<I',expected,cb+12)[0]==400
  assert struct.unpack_from('<III',expected,p+4)==(396,4,2)
  struct.pack_into('<I',expected,cb+12,416);struct.pack_into('<I',expected,p+4,400)
 else:assert row['global_layout']=='identical'
 assert before==after,(row['original_path'],'original instruction changed beyond bounded adapter')
 assert bytes(expected)==new[b'RDEF'],'unexpected constant/resource ABI change'

def check(porter, stock, donor, patch, pack):
    here = Path(__file__).resolve().parent
    contracts = json.loads((here / 'normal-off-programs.json').read_text())
    assert len(contracts) == 348
    paths = {path for row in contracts for path in row['paths']}
    assert len(paths) == sum(len(row['paths']) for row in contracts) == 1218
    manifest = json.loads((pack / 'pack.json').read_text())
    assert len(manifest['operations']) == len(paths)
    assert {op['path'] for op in manifest['operations']} == paths
    assert all(op['op'] == 'add_file' for op in manifest['operations'])
    shipped = set(subprocess.check_output([str(porter), 'list', '--db', str(stock)], text=True).splitlines())
    assert paths.isdisjoint(shipped), 'existing native shader changed'
    # All previously supplied paths remain supported; masked correctness now
    # takes precedence over the previous expansion's byte-identity optimization.
    previous = json.loads((here / 'normal-off-v2-programs.json').read_text())
    assert {p for row in previous for p in row['paths']} <= paths
    cache = {}
    with tempfile.TemporaryDirectory() as tmp:
        def read(db, path):
            out = Path(tmp) / 'shader'
            subprocess.run([str(porter), 'extract', '--db', str(db), '--path', path, '--output', str(out)],
                           check=True, stdout=subprocess.DEVNULL)
            return out.read_bytes()
        for row in contracts:
            key = 'original', row['original_sha256']
            if key not in cache:
                cache[key] = read(donor, row['original_path'])
            original = cache[key]
            key = 'native', row['native_reference_sha256']
            if key not in cache:
                cache[key] = read(stock, row['native_reference_path'])
            native = cache[key]
            for path in row['paths']:
                actual = read(patch, path)
                assert actual == (pack / 'files' / path).read_bytes(), path
                verify(row, original, native, actual)
        check_constant_alpha(here, lambda path: read(donor, path))
    print(f'Normal-off: {len(contracts)} pinned original/native contracts, {len(paths)} added paths; '
          'original linear instructions and alpha cutoffs, masked equation, base sampling, globals and native isolation verified')


def check_constant_alpha(here, read):
    examples = json.loads((here / 'constant-alpha-programs.json').read_text())
    assert {(r['quality'], r['vertex']) for r in examples} == {(q, v) for q in (0, 3) for v in (0, 10, 12, 13, 14)}
    for row in examples:
        source = read(row['path'])
        assert hashlib.sha256(source).hexdigest() == row['sha256']
        ops = instructions(chunks(source)[1][b'SHDR'])[1]
        first = [i for i, op in enumerate(ops) if op[0] == 0x08002000 and op[3:] == [0x0020800a, 0, 10, 0x00004001, 0x3f800000]]
        assert len(first) == 1
        i = first[0];dst = ops[i][1:3];component = ((dst[0] >> 4) & 15).bit_length() - 1
        src = [0x0010000a | (component << 4), dst[1]]
        assert ops[i+1:i+4] == [
            [0x0500002f, *dst, *src],
            [0x08000038, *dst, *src, 0x0020800a, 0, 10],
            [0x05000019, *dst, *src]], 'constant-alpha original equation differs'
    materials = json.loads((here / 'vertex-blend-inputs.json').read_text())['materials']
    factors = [r['original_blend']['material.g_stage.effect.constant_vertex_blended_factor.value']
               for r in materials if r['mode'] == 'masked']
    assert len(factors) == 13 and all(1 <= factor <= 2 for factor in factors)
    # For these authored factors saturate(factor+1) is exactly1; log2(1)=0,
    # factor*0=0, exp2(0)=1. Thus the original constant-alpha interpolation
    # coefficient is exactly the original unmasked program's coefficient1.


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('porter', 'stock', 'donor', 'patch', 'pack'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    check(args.porter, args.stock, args.donor, args.patch, args.pack)

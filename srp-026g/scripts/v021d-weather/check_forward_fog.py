"""Independent retail-block, scene-guard and native-fallback bytecode audit."""
import argparse,ctypes,hashlib,json,struct,subprocess,tempfile
from pathlib import Path

def chunks(blob):
 offset=4+struct.unpack_from('<I',blob)[0]*264;code=blob[offset:];assert code[:4]==b'DXBC'
 result={}
 for i in range(struct.unpack_from('<I',code,28)[0]):
  p=struct.unpack_from('<I',code,32+4*i)[0];n=struct.unpack_from('<I',code,p+4)[0];result[code[p:p+4]]=code[p+8:p+8+n]
 return blob[:offset],result

def ops(blob):
 raw=chunks(blob)[1][b'SHDR'];w=struct.unpack('<'+'I'*(len(raw)//4),raw);out=[];at=2
 while at<len(w):
  n=(w[at]>>24)&127;assert n;out.append(w[at:at+n]);at+=n
 assert at==len(w);return out

def split_guards(program,markers,globals):
 native=[];branches={};at=0
 while at<len(program):
  op=program[at]
  if not(len(op)==8 and op[0]==0x08000018 and op[3:7]==(0x0020803a,globals,18,0x00004001) and op[-1] in markers):
   native.append(op);at+=1;continue
  register=op[2];assert op[1]==0x00100082 and program[at+1]==(0x0304001f,0x0010003a,register)
  depth=1;end=at+2;otherwise=None
  while depth:
   code=program[end][0]&2047
   if code==31:depth+=1
   if code==21:depth-=1
   if code==18 and depth==1:otherwise=end
   end+=1
  assert otherwise is not None
  assert op[-1] not in branches
  branches[op[-1]]=(register,program[at+2:otherwise])
  fallback,nested=split_guards(program[otherwise+1:end-1],markers,globals)
  assert not(branches.keys()&nested.keys());branches.update(nested);native+=fallback;at=end
 return native,branches

def mapped(original,scratch,profile,globals):
 out=[original[0]];at=1
 while at<len(original):
  token=original[at];at+=1;ext=[]
  if token&0x80000000:ext=[original[at]];at+=1
  kind=(token>>12)&255;dim=(token>>20)&3;indices=list(original[at:at+dim]);at+=dim
  if kind==4:
   n=4 if token&3==2 else 1;out+=[token]+list(original[at:at+n]);at+=n;continue
  if kind==0:indices[0]+=scratch
  elif kind==8 and indices[0]==globals:
   if indices[1]==18:
    channel=(token>>4)&3
    if channel==0:token+=16
    else:
     assert channel==1;token=(token&~(0x00300000|0x000ff000|0x30))|0x00100000|0x30;indices=[profile]
   elif indices[1]==15 and (token>>2)&3==1:
    token=(token&~(0x00300000|0x000ff000))|0x00100000;indices=[profile]
  out += [token]+ext+indices
 out[0]=(out[0]&~0x7f000000)|(len(out)<<24);return tuple(out)

# Per family: the original program, its fog block, the globals buffer, its pin, and how the block's
# results reach the native tail (glass: r0.x; particles: colour r0.xzw and amount r0.y).
GLASS=lambda t:[(0x05000036,0x00100012,0,0x0010000a,t)]
PARTICLE=lambda t:[(0x05000036,0x001000d2,0,0x00100906,t),(0x05000036,0x00100022,0,0x0010001a,t+1)]
SPECS={
 'forward_base.ps':[('______0________1__010__1________0_0_____________1__0_0_0________0__',13,61,2,'a3ca3f7366221b6b80b00c4fe7a9c79b7a359730c29ba1f2780af7ca984855a9',GLASS),
  ('______0________2__010__3________0_0_____________0__0_0_0________0__',10,55,2,'22698bac066933e07f1785714f743ee0767809bd2579d0c55c70ab134454d6d9',PARTICLE)],
 'forward_probe_lighting.ps':[('_0___11_0___001___00___1________0_0_____________0__0_0_____________',134,183,3,'7d8b804f706e6d3bacb1bc3cffb46a97131b3492d0d6bd83ee84719c129b8437',None)],
 'forward_lighting.ps':[('_0__111_0___001__000___1___0____0_0_____________0__0_0_____________',389,439,3,'ae3cf994899c05c015fed0ec4ec0701391045f844eac9b9fb0d710edec8d1a66',None)]}

def main():
 ap=argparse.ArgumentParser()
 for k in ['porter','donor','stock','pack','profiles','patches','patch_cache','shader_indexer']:ap.add_argument('--'+k.replace('_','-'),type=Path,required=True)
 ap.add_argument('--patch',type=Path);a=ap.parse_args();manifest=json.loads((a.pack/'pack.json').read_text());specs=json.loads(a.profiles.read_text())['profiles'];assert len(specs)==15
 assert manifest['id']=='v021d-forward-fog' and manifest['version']=='3' and manifest['operations']
 expected_observed={
 'shaders/sm_4_0/forward_base.ps/______0________2_010__3________0_0______________0__0_0_0________0__':'3c12ca61033cb5c2bef08e2e96839a40bedce5ce3e6ce5a4e17912b469715bc2',
 'shaders/sm_4_0/forward_base.ps/______0________1_010__1________0_0______________1__0_0_0________0__':'11a0d17017593951bf8c850e2a5b5f7646413f03a196a1b236995822f5c88028',
 'shaders/sm_4_0/forward_probe_lighting.ps/_____11_0___001__00___1________0_0______________0__0_0__________0__':'b15ac0326fad1fbecb7b6e5457f05543a98d2461e2f9b241575136e98fc42046',
 'shaders/sm_4_0/forward_lighting.ps/____111_0___001_000___1___0____0_0______________0__0_0__________0__':'891830b564aa6b58fcdfaa6bd0c453b497483b274191ad96f9194d36df842299'}
 operations={op['path']:op for op in manifest['operations']};assert len(operations)==len(manifest['operations'])
 assert expected_observed.keys()<=operations.keys()
 assert all(operations[path]['base_sha256']==sha for path,sha in expected_observed.items())
 # Enumerate the final published shader layers independently of the builder.
 # Whole-DXBC identities include every reviewed native family variant; aliases
 # with different engine headers are still required and retain their own header.
 pins=json.loads(Path(__file__).with_name('forward-fog-inputs.json').read_text())
 supported={row['dxbc_digest'] for row in pins};assert len(supported)==19
 layers=[a.stock]
 archive_pins={a.stock.name:json.loads(a.profiles.read_text())['target_sha256']}
 for line in a.patches.read_text().splitlines():
  if line and not line.startswith('#'):
   name,sha,size,urls=line.split('\t',3);archive=a.patch_cache/name
   assert archive.stat().st_size==int(size);layers.append(archive);archive_pins[name]=sha
 effective={}
 for archive in layers:
  for line in subprocess.check_output([str(a.shader_indexer),str(archive)],text=True).splitlines():
   digest,path=line.split('\t',1)
   if path.startswith('shaders/sm_4_0/') and path.split('/')[-2] in SPECS:effective[path]=(archive,digest)
 expected={path for path,(_,digest) in effective.items() if digest in supported}
 assert operations.keys()==expected, ('effective forward alias coverage differs', sorted(expected-operations.keys()),sorted(operations.keys()-expected))
 provenance=json.loads((a.pack/'source-archives.json').read_text())
 assert provenance['published_manifest_sha256']==hashlib.sha256(a.patches.read_bytes()).hexdigest()
 used={effective[path][0].name for path in expected}
 assert {r['archive']:r['sha256'] for r in provenance['used_archives']}=={name:archive_pins[name] for name in used}
 markers={struct.unpack('<I',struct.pack('<f',s['marker']))[0]:s for s in specs};assert len(markers)==15
 with tempfile.TemporaryDirectory() as tmp:
  cache={}
  def read(db,path):
   key=(db,path)
   if key not in cache:
    dest=Path(tmp)/str(len(cache));subprocess.run([str(a.porter),'extract','--db',str(db),'--path',path,'--output',str(dest)],check=True,stdout=subprocess.DEVNULL);cache[key]=dest.read_bytes()
   return cache[key]
  # Donor uploads and the builder use single precision powf, including the
  # exponent. A double-precision approximation differs by several ULPs for
  # authored HDR colors above1; compare exact single-precision results.
  powf=ctypes.CDLL(None).powf;powf.argtypes=[ctypes.c_float,ctypes.c_float];powf.restype=ctypes.c_float
  colors={}
  for marker,spec in markers.items():
   raw=subprocess.check_output([str(a.porter),'dump-db-config','--db',str(a.donor),'--path',spec['project'],'--leaves'],text=True)
   values={k:json.loads(v.split(':',1)[1]) for line in raw.splitlines() for k,v in [line.split('\t',1)]}
   rgb=[v for k,v in values.items() if k.endswith('.properties.rayleigh_fog_color')];assert len(rgb)==1
   colors[marker]=[powf(v,2.2) for v in rgb[0][:3]]
  unique=set();coverage={}
  for operation in manifest['operations']:
   assert operation['op']=='replace_file';path=operation['path'];family=path.split('/')[-2];globals=SPECS[family][0][3]
   old=read(effective[path][0],path);assert hashlib.sha256(old).hexdigest()==operation['base_sha256'];new=(a.pack/'files'/path).read_bytes()
   if a.patch:assert read(a.patch,path)==new,'composed shader differs'
   header,old_chunks=chunks(old);new_header,new_chunks=chunks(new);assert header==new_header and old_chunks.keys()==new_chunks.keys()
   assert all(old_chunks[k]==new_chunks[k] for k in old_chunks if k!=b'SHDR')
   before=ops(old);after=ops(new);native,branches=split_guards(after,markers,globals);assert branches.keys()==markers.keys()
   oldtemp=next(op[1] for op in before if op[0]&2047==104);assert next(op[1] for op in after if op[0]&2047==104)==oldtemp+5
   assert [op for op in before if op[0]&2047!=104]==[op for op in native if op[0]&2047!=104], 'unmarked native behavior changed'
   for marker,(profile,branch) in branches.items():
    assert profile==oldtemp+4
    load=branch[0];assert load[:4]==(0x08000036,0x001000f2,profile,0x00004002) and load[-1]==0x7f800000
    rgb=struct.unpack('<3f',struct.pack('<3I',*load[4:7]));assert list(rgb)==colors[marker]
    body=branch[1:]
    spec=[s for s in SPECS[family] if s[5] is None or body[-len(s[5](oldtemp)):]==s[5](oldtemp)];assert len(spec)==1,'unknown result transfer'
    key,start,end,_,pinned,results=spec[0]
    if results is None:assert body[0]==(0x05000036,0x001000f2,oldtemp,0x00100e46,0);body=body[1:]
    else:body=body[:-len(results(oldtemp))]
    original=read(a.donor,'shaders/sm_4_0/'+family+'/'+key);assert hashlib.sha256(original).hexdigest()==pinned
    assert body==[mapped(op,oldtemp,profile,globals) for op in ops(original)[start:end]],'original fog block changed'
   unique.add(hashlib.sha256(new).hexdigest());coverage[family]=coverage.get(family,0)+1
  print(json.dumps(dict(status='PASS',families=coverage,effective_layers=len(layers),effective_forward_paths=len(effective),aliases=len(operations),unique_programs=len(unique),all15_scene_guards=True,unmarked_native_instructions='byte-identical',original_fog_blocks='exact after established register/schema bindings',composed=a.patch is not None),indent=2))
if __name__=='__main__':main()

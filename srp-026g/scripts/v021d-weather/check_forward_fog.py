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

def mapped(original,scratch,profile,globals,to_globals=None,inputs=None):
 out=[original[0]];at=1
 while at<len(original):
  token=original[at];at+=1;ext=[]
  if token&0x80000000:ext=[original[at]];at+=1
  kind=(token>>12)&255;dim=(token>>20)&3;indices=list(original[at:at+dim]);at+=dim
  if kind==4:
   n=4 if token&3==2 else 1;out+=[token]+list(original[at:at+n]);at+=n;continue
  if kind==0:indices[0]+=scratch
  elif kind==1 and inputs:
   assert indices[0]==inputs[0];indices[0]=inputs[1]
  elif kind==8 and indices[0]==globals:
   if to_globals is not None:indices[0]=to_globals
   if indices[1]==18:
    channel=(token>>4)&3
    if channel==0:token+=16
    else:
     assert channel==1;token=(token&~(0x00300000|0x000ff000|0x30))|0x00100000|0x30;indices=[profile]
   elif indices[1]==15 and (token>>2)&3==1:
    token=(token&~(0x00300000|0x000ff000))|0x00100000;indices=[profile]
  out += [token]+ext+indices
 out[0]=(out[0]&~0x7f000000)|(len(out)<<24);return tuple(out)

# --- Any other forward program: the native fog recognised by what it computes. ---
# A value per register component, by how it was computed (opcode and argument values), so the same
# computation is the same value in any instruction order and any registers. Independent of the builder.
DOT={15:2,16:3,17:4}
FOG_OPS={0,14,16,25,47,49,50,51,52,54,55,56,68,72,75} # add div dp3 exp log lt mad min max mov movc mul rsq sqrt sample_l
WRITES_FIRST=set([0,1,11,12,14,15,16,17,24,25,26,27,28,29,30,32,33,34,35,36,37,39,40,41,42,43,45,47,49,50,51,52,54,55,56,57,59,60,61]+list(range(64,76))+[79,80,82,83,84,85,86,87])
def operands(ins):
 if ins[0]&0x80000000:return None
 at=1;out=[]
 while at<len(ins):
  token=ins[at];at+=1;ext=None
  if token&0x80000000:
   ext=ins[at];at+=1
   if ext&0x80000000:return None
  dim=(token>>20)&3
  if any((token>>(22+3*i))&7 for i in range(dim)):return None
  kind=(token>>12)&255;vector=token&3==2;n=(4 if vector else 1) if kind==4 else 0
  indices=tuple(ins[at:at+dim]);at+=dim;literal=tuple(ins[at:at+n]);at+=n
  out.append(dict(kind=kind,vector=vector,mode=(token>>2)&3,bits=(token>>4)&255,ext=ext,indices=indices,literal=literal))
 return out if at==len(ins) else None
def written(o):return [c for c in range(4) if o['bits']&(1<<c)] if o['vector'] and o['mode']==0 else None
def component(o,lane):
 if not o['vector']:return None
 return (o['bits']>>(2*lane))&3 if o['mode']==1 else o['bits']&3 if o['mode']==2 else None
class Region:
 def __init__(self):self.held={};self.written=set();self.bound={}
 def bind(self,key,index):return self.bound.setdefault(key,index)==index
 def value(self,o,lane):
  if o['kind']==4:v=('literal',o['literal'][lane if o['vector'] else 0])
  elif o['kind']==0:
   c=component(o,lane);v=self.held.get((o['indices'][0],c)) if c is not None else None
   if v is None:return None
  elif o['kind']==1:
   c=component(o,lane)
   if c is None or not self.bind('input',o['indices'][0]):return None
   v=('position',c)
  elif o['kind']==8:
   c=component(o,lane)
   if c is None or not self.bind('globals',o['indices'][0]):return None
   v=('global',o['indices'][1],c)
  else:return None
  return ('modified',o['ext'],v) if o['ext'] is not None else v
 def step(self,ins):
  op=ins[0]&2047
  if op not in FOG_OPS:return False
  o=operands(ins)
  if not o or o[0]['kind']!=0 or o[0]['ext'] is not None:return False
  dest,sources=o[0],o[1:];to=written(dest)
  if to is None:return False
  token=ins[0]&0x80ffffff;new={}
  if op in DOT:
   args=tuple(tuple(self.value(x,lane) for lane in range(DOT[op])) for x in sources)
   if any(v is None for a in args for v in a):return False
   for c in to:new[c]=('dot',token,args)
  elif op==72:
   if len(sources)!=4:return False
   at,texture,sampler,level=sources
   if (at['kind'],texture['kind'],sampler['kind'],level['kind'])!=(0,7,6,4):return False
   if not self.bind('texture',texture['indices'][0]) or not self.bind('sampler',sampler['indices'][0]):return False
   coordinates=tuple(self.value(at,lane) for lane in range(4))
   if None in coordinates:return False
   for c in to:new[c]=('sample',token,coordinates,component(texture,c),level['literal'])
  else:
   for c in to:
    args=tuple(self.value(x,c) for x in sources)
    if None in args:return False
    new[c]=('lane',token,args)
  for c,v in new.items():
   self.held[(dest['indices'][0],c)]=v;self.written.add((dest['indices'][0],c))
  return True
# The three ways the native programs compute their fog: a program each, its instructions, and where
# they leave the colour (red, green, blue) and the amount.
NATIVE_FOGS=[
 ('forward_base.ps/______0________2_010__3________0_0______________0__0_0_0________0__','3c12ca61033cb5c2bef08e2e96839a40bedce5ce3e6ce5a4e17912b469715bc2',11,64,[(0,0),(0,2),(0,3),(0,1)]),
 ('forward_lighting.ps/____000_0___000_000___0___0____0_0______________0__0_0__________0__','9cb6d4aca31e98266d21670a6114c87c3758d78cbb6cd9d358db39bbd6e4a542',364,417,[(1,1),(1,2),(1,3),(1,0)]),
 ('forward_lighting.ps/____001_0___000_001___0___0____0_0______________1__0_0__________1__','13e5b75c2f554d81857206317c7b30bcd1afca915a61998a2a8e7bf03c0b12c7',407,460,[(1,0),(1,1),(1,2),(0,2)])]
# Every fogged 0.21 forward program computes this one block: colour in r0.xyz, amount in r1.y.
ORIGINAL_FOG=('forward_base.ps/______0________2__010__3________0_0_____________0__0_0_0________0__','22698bac066933e07f1785714f743ee0767809bd2579d0c55c70ab134454d6d9',10,55,[(0,0),(0,1),(0,2),(1,1)],2,6)
def fog_values(program,start,end,results):
 region=Region()
 for ins in program[start:end]:assert region.step(ins),'a pinned fog block is not a fog computation'
 return [region.held[slot] for slot in results]
def is_native_way(way):
 # colour: (texture.component*mie+rayleigh)/max(amount,tiny); amount: clamped.
 for c,v in enumerate(way[:3]):
  assert v[0]=='lane' and v[1]&0x27ff==14 and v[2][0][0]=='lane' and v[2][0][1]&0x27ff==50 and v[2][0][2][0][0]=='sample' and v[2][0][2][0][3]==c,'pinned native fog colour'
 assert way[3][0]=='lane' and way[3][1]&0x27ff==0x2036,'pinned native fog amount'
def find_fog(program,ways):
 """(start, end, result slots, input, globals, written) of the one run that computes a native fog, or None."""
 found=None
 for start,ins in enumerate(program):
  if ins[0]&2047!=0:continue
  region=Region()
  for end in range(start,len(program)):
   if not region.step(program[end]):break
   slots=None
   for way in ways:
    inverse={}
    for slot,v in region.held.items():inverse.setdefault(v,slot)
    if all(v in inverse for v in way):slots=[inverse[v] for v in way];break
   if slots:
    assert found is None,'ambiguous forward fog block'
    found=(start,end+1,slots,region.bound['input'],region.bound['globals'],set(region.written));break
 return found
def lanes(op,dest):
 if op in DOT:return list(range(DOT[op]))
 if dest is not None and op in WRITES_FIRST and op not in (45,61,69,70,71,72,73,74) and written(dest) is not None:return written(dest)
 return [0,1,2,3]
def reads_only_results(tail,slots,block_written):
 # A component written again is the program's own: to the end outside any branch, to the end of
 # its branch inside one.
 own=set();outer=[]
 for ins in tail:
  op=ins[0]&2047
  if op>=88:return False
  o=operands(ins)
  if o is None:return False
  wrote=2 if op in (38,77,78,81) else 1 if op in WRITES_FIRST else 0
  if len(o)<wrote or any(x['kind'] not in (0,2,13) for x in o[:wrote]):return False
  dest=o[0] if wrote==1 else None
  for x in o[wrote:]:
   if x['kind']==0:
    for lane in lanes(op,dest):
     c=component(x,lane)
     if c is None:return False
     slot=(x['indices'][0],c)
     if slot in block_written and slot not in own and slot not in slots:return False
   elif x['kind']==3:return False
  if op in (31,48,76):outer.append(set(own))
  elif op in (18,6,10):
   if not outer:return False
   own=set(outer[-1])
  elif op in (21,22,23):
   if not outer:return False
   own=outer.pop()
  else:
   for x in o[:wrote]:
    if x['kind']==0:
     to=written(x)
     if to is None:return False
     own|={(x['indices'][0],c) for c in to}
 return True
def adaptable(program,ways):
 """find_fog's answer when the builder's rule adapts the program, else None."""
 found=find_fog(program,ways)
 if not found:return None
 depth=0
 for ins in program[:found[0]]:
  op=ins[0]&2047;depth+=1 if op in (31,48,76) else -1 if op in (21,22,23) else 0
 if depth or not reads_only_results(program[found[1]:],found[2],found[5]):return None
 return found
def moves(slots,scratch):
 red,green,blue,amount=slots;out=[]
 if red[0]==green[0]==blue[0]:
  colour=[red[1],green[1],blue[1]];mask=sum(1<<c for c in colour)
  swizzle=sum((colour.index(c) if c in colour else 0)<<(2*c) for c in range(4))
  out.append((0x05000036,0x00100002|mask<<4,red[0],0x00100006|swizzle<<4,scratch))
 else:
  for i,(register,to) in enumerate((red,green,blue)):out.append((0x05000036,0x00100002|(1<<to)<<4,register,0x0010000a|i<<4,scratch))
 out.append((0x05000036,0x00100002|(1<<amount[1])<<4,amount[0],0x0010001a,scratch+1))
 return out

# Per family: the original program, its fog block, the globals buffer, its pin, and how the block's
# results reach the native tail (glass: r0.x; particles: colour r0.xzw and amount r0.y).
GLASS=lambda t:[(0x05000036,0x00100012,0,0x0010000a,t)]
SPECS={
 'forward_base.ps':[('______0________1__010__1________0_0_____________1__0_0_0________0__',13,61,2,'a3ca3f7366221b6b80b00c4fe7a9c79b7a359730c29ba1f2780af7ca984855a9',GLASS)  ],
 'forward_probe_lighting.ps':[('_0___11_0___001___00___1________0_0_____________0__0_0_____________',134,183,3,'7d8b804f706e6d3bacb1bc3cffb46a97131b3492d0d6bd83ee84719c129b8437',None)],
 'forward_lighting.ps':[('_0__111_0___001__000___1___0____0_0_____________0__0_0_____________',389,439,3,'ae3cf994899c05c015fed0ec4ec0701391045f844eac9b9fb0d710edec8d1a66',None)]}

def main():
 ap=argparse.ArgumentParser()
 for k in ['porter','donor','stock','pack','profiles','patches','patch_cache','shader_indexer']:ap.add_argument('--'+k.replace('_','-'),type=Path,required=True)
 ap.add_argument('--patch',type=Path);a=ap.parse_args();manifest=json.loads((a.pack/'pack.json').read_text());specs=json.loads(a.profiles.read_text())['profiles'];assert specs and len(specs)%3==0
 assert manifest['id']=='v021d-forward-fog' and manifest['version']=='5' and manifest['operations']
 expected_observed={
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
 supported={row['dxbc_digest'] for row in pins};assert len(supported)==len(pins)
 layers=[a.stock]
 archive_pins={a.stock.name:json.loads(a.profiles.read_text())['target_sha256']}
 for line in a.patches.read_text().splitlines():
  if line and not line.startswith('#'):
   name,sha,size,urls=line.split('\t',3);archive=a.patch_cache/name
   assert archive.stat().st_size==int(size);layers.append(archive);archive_pins[name]=sha
 # A released patch that carries a program of this pack (this version or an earlier one, released
 # before it) leaves the layer below it in force.
 def dxbc_digest(blob):
  at=blob.find(b'DXBC');return blob[at+4:at+20].hex()
 own={}
 for version in a.pack.parent.glob('v021d-forward-fog-*'):
  for path in operations:
   if (version/'files'/path).is_file():own.setdefault(path,set()).add(dxbc_digest((version/'files'/path).read_bytes()))
 effective={}
 for archive in layers:
  for line in subprocess.check_output([str(a.shader_indexer),str(archive)],text=True).splitlines():
   digest,path=line.split('\t',1)
   if path.startswith('shaders/sm_4_0/') and path.split('/')[-2] in SPECS and not (archive!=a.stock and digest in own.get(path,())):effective[path]=(archive,digest)
 expected={path for path,(_,digest) in effective.items() if digest in supported}
 assert operations.keys()==expected, ('effective forward alias coverage differs', sorted(expected-operations.keys())[:5],sorted(operations.keys()-expected)[:5])
 provenance=json.loads((a.pack/'source-archives.json').read_text())
 # Built over the release list of its day: the current one up to the rows released since.
 published=a.patches.read_bytes().splitlines(keepends=True)
 assert any(hashlib.sha256(b''.join(published[:n])).hexdigest()==provenance['published_manifest_sha256'] for n in range(len(published)+1))
 used={effective[path][0].name for path in expected}
 assert {r['archive']:r['sha256'] for r in provenance['used_archives']}=={name:archive_pins[name] for name in used}
 markers={struct.unpack('<I',struct.pack('<f',s['marker']))[0]:s for s in specs};assert len(markers)==len(specs)
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
  # The pinned fog computations, read independently of the builder.
  ways=[]
  for key,pinned,start,end,results in NATIVE_FOGS:
   blob=read(a.stock,'shaders/sm_4_0/'+key);assert hashlib.sha256(blob).hexdigest()==pinned
   way=fog_values(ops(blob),start,end,results);is_native_way(way);assert way not in ways;ways.append(way)
  key,pinned,start,end,results,original_globals,original_input=ORIGINAL_FOG
  blob=read(a.donor,'shaders/sm_4_0/'+key);assert hashlib.sha256(blob).hexdigest()==pinned
  original_fog=ops(blob)[start:end];original_way=fog_values(ops(blob),start,end,results)
  assert results==[(0,0),(0,1),(0,2),(1,1)] and original_way[3][0]=='lane' and original_way[3][1]&0x27ff==0x2036
  unique=set();coverage={};any_way={}
  for operation in manifest['operations']:
   assert operation['op']=='replace_file';path=operation['path'];family=path.split('/')[-2]
   old=read(effective[path][0],path);assert hashlib.sha256(old).hexdigest()==operation['base_sha256'];new=(a.pack/'files'/path).read_bytes()
   if a.patch:assert read(a.patch,path)==new,'composed shader differs'
   header,old_chunks=chunks(old);new_header,new_chunks=chunks(new);assert header==new_header and old_chunks.keys()==new_chunks.keys()
   assert all(old_chunks[k]==new_chunks[k] for k in old_chunks if k!=b'SHDR')
   before=ops(old);after=ops(new)
   guards={op[4] for op in after if len(op)==8 and op[0]==0x08000018 and op[3]==0x0020803a and op[5]==18 and op[6]==0x00004001 and op[-1] in markers};assert len(guards)==1
   globals=guards.pop();native,branches=split_guards(after,markers,globals);assert branches.keys()==markers.keys()
   oldtemp=next(op[1] for op in before if op[0]&2047==104);assert next(op[1] for op in after if op[0]&2047==104)==oldtemp+5
   assert [op for op in before if op[0]&2047!=104]==[op for op in native if op[0]&2047!=104], 'unmarked native behavior changed'
   for marker,(profile,branch) in branches.items():
    assert profile==oldtemp+4
    load=branch[0];assert load[:4]==(0x08000036,0x001000f2,profile,0x00004002) and load[-1]==0x7f800000
    rgb=struct.unpack('<3f',struct.pack('<3I',*load[4:7]));assert list(rgb)==colors[marker]
    body=branch[1:]
    key,start,end,pinned_globals,pinned,results=SPECS[family][0]
    exact=body[0]==(0x05000036,0x001000f2,oldtemp,0x00100e46,0) if results is None else body[-1:]==results(oldtemp)
    if exact:
     # One of the pinned glass programs: its own original block and result transfer.
     assert globals==pinned_globals
     body=body[1:] if results is None else body[:-len(results(oldtemp))]
     original=read(a.donor,'shaders/sm_4_0/'+family+'/'+key);assert hashlib.sha256(original).hexdigest()==pinned
     assert body==[mapped(op,oldtemp,profile,globals) for op in ops(original)[start:end]],'original fog block changed'
    else:
     # Any other program: the native fog found here by what it computes, the common original
     # block in its place, its results moved to where the rest of the program reads them.
     found=any_way.get(path) or adaptable(before,ways);assert found,'not a program the rule adapts';any_way[path]=found
     first,last,slots,position,found_globals,_=found;assert found_globals==globals
     assert body==[mapped(op,oldtemp,profile,original_globals,globals,(original_input,position)) for op in original_fog]+moves(slots,oldtemp),'original fog block or its results changed'
   if path in any_way:
    # The guards stand exactly where the native run stood, which stays whole behind them.
    first,last=any_way[path][:2];temps=[i for i,op in enumerate(before) if op[0]&2047==104][0]
    head=list(before[:first]);head[temps]=(before[temps][0],oldtemp+5)
    assert after[:first]==head and after[-(len(before)-last):]==list(before[last:]),'guards misplaced'
    tail=after[first:len(after)-(len(before)-last)];assert tail[-len(markers):]==[(0x01000015,)]*len(markers)
    assert tail[-len(markers)-(last-first):-len(markers)]==list(before[first:last]) and tail[-len(markers)-(last-first)-1]==(0x01000012,),'native fallback misplaced'
   unique.add(hashlib.sha256(new).hexdigest());coverage[family]=coverage.get(family,0)+1
  # What stays native is what the rule does not adapt: no native fog in one run, or more read of it.
  left=sorted(effective.keys()-operations.keys());reasons={}
  for path in left:
   program=ops(read(effective[path][0],path));found=find_fog(program,ways)
   assert adaptable(program,ways) is None,('an adaptable program was left native',path)
   fogged=any(op[i:i+2]==(g,18) for op in program for g in (1,2,3) for i in range(2,len(op)-1))
   reason='reads more of the fog' if found else 'fogged otherwise' if fogged else 'no fog';reasons[reason]=reasons.get(reason,0)+1
  print(json.dumps(dict(status='PASS',left_native=reasons,generic_programs=len(any_way),families=coverage,effective_layers=len(layers),effective_forward_paths=len(effective),aliases=len(operations),unique_programs=len(unique),scene_guards=len(markers),unmarked_native_instructions='byte-identical',original_fog_blocks='exact after established register/schema bindings',composed=a.patch is not None),indent=2))
if __name__=='__main__':main()

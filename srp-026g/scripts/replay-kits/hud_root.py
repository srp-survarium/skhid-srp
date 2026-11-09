#!/usr/bin/env python3
"""hud_root.py <hud.swf> <harness.swf> <out.swf>: the hud movie with the harness as its document class.

HUD's constructor needs the stage, which a root loaded by a Loader does not have yet, so a harness that
loads the hud gets a root that never ran its setup. Here the movie is the main one: the harness class
(which extends HUD) is added to it and named its root, everything else being the hud's own.
"""
from pathlib import Path
import re,struct,sys
sys.path.insert(0,str(Path(__file__).resolve().parents[3] / 'tools/actionscript'));import swf
DO_ABC,SYMBOL_CLASS=82,76
hud,harness,out=map(Path,sys.argv[1:4])
header,tags=swf.split(hud.read_bytes())
_,own=swf.split(harness.read_bytes())
abc=[(k,r) for k,r in own if k==DO_ABC]
if not abc: sys.exit('the harness has no ABC')
def rooted(payload):
 count=struct.unpack_from('<H',payload)[0];pos=2;entries=[];root=False
 for _ in range(count):
  char=struct.unpack_from('<H',payload,pos)[0];pos+=2
  end=payload.index(0,pos);name=payload[pos:end];pos=end+1
  if char==0: name=b'KitsHarness';root=True
  entries.append(struct.pack('<H',char)+name+b'\0')
 return root,struct.pack('<H',count)+b''.join(entries)

def u30(data,pos):
 value=shift=0
 while True:
  byte=data[pos];pos+=1;value|=(byte&127)<<shift;shift+=7
  if not byte&128: return value,pos
def encode(value):
 out=bytearray()
 while True:
  byte=value&127;value>>=7
  out.append(byte|(128 if value else 0))
  if not value: return bytes(out)
def names_of(abc):
 """The multiname indices of each plain name in an ABC file's constant pool."""
 pos=4;strings=[b'']
 for size in (None,None):
  count,pos=u30(abc,pos)
  for _ in range(max(count-1,0)): _,pos=u30(abc,pos)
 count,pos=u30(abc,pos);pos+=8*max(count-1,0)
 count,pos=u30(abc,pos)
 for _ in range(max(count-1,0)):
  size,pos=u30(abc,pos);strings.append(abc[pos:pos+size]);pos+=size
 count,pos=u30(abc,pos)
 for _ in range(max(count-1,0)):
  pos+=1;_,pos=u30(abc,pos)
 count,pos=u30(abc,pos)
 for _ in range(max(count-1,0)):
  n,pos=u30(abc,pos)
  for _ in range(n): _,pos=u30(abc,pos)
 count,pos=u30(abc,pos);found={}
 for index in range(1,count):
  kind=abc[pos];pos+=1
  if kind in (7,13): _,pos=u30(abc,pos);name,pos=u30(abc,pos)
  elif kind in (15,16): name,pos=u30(abc,pos)
  elif kind in (17,18): continue
  elif kind in (9,14): name,pos=u30(abc,pos);_,pos=u30(abc,pos)
  elif kind in (27,28): _,pos=u30(abc,pos);continue
  elif kind==29:
   _,pos=u30(abc,pos);n,pos=u30(abc,pos)
   for _ in range(n): _,pos=u30(abc,pos)
   continue
  else: sys.exit('multiname kind %d'%kind)
  found.setdefault(strings[name],[]).append(index)
 return found
def adapted(payload):
 """What Scaleform takes and Flash and Ruffle throw at, in this copy of HUD only: it hides and shows the
 stage while it sets up (#2071): `stage.visible = x` becomes nops; its layout reads `layoutData` off the
 plain Sprites among the root's children (#1069): HUD's `new Sprite()` become MovieClips, which answer."""
 start=payload.index(0,4)+1;abc=payload[start:];names=names_of(abc)
 def any_of(name): return b'(?:'+b'|'.join(re.escape(encode(i)) for i in names.get(name,[])) +b')'
 if not names.get(b'stage') or not names.get(b'visible'): return payload,0
 code=re.compile(b'\x5D'+any_of(b'stage')+b'\x66'+any_of(b'stage')+b'[\x26\x27]\x61'+any_of(b'visible'))
 fresh,n=code.subn(lambda m:b'\x02'*len(m.group(0)),abc)
 clip=encode(names[b'MovieClip'][0])
 def as_clip(m):
  plain=b'\x5D'+clip+b'\x4A'+clip+b'\x00'
  return plain+b'\x02'*(len(m.group(0))-len(plain))
 fresh,clips=re.subn(b'\x5D'+any_of(b'Sprite')+b'\x4A'+any_of(b'Sprite')+b'\x00',as_clip,fresh)
 print('new Sprite() made MovieClips:',clips)
 return payload[:start]+fresh,n
dropped=0;patched=[]
for k,r in tags:
 if k==DO_ABC:
  payload,n=adapted(swf.payload(r)) if swf.payload(r)[4:8]==b'HUD\0' else (None,0);dropped+=n
  if n: r=swf.tag(k,payload)
 patched.append((k,r))
tags=patched
print('stage.visible writes dropped:',dropped)
if not dropped: sys.exit('no stage.visible in the hud movie: is this still needed?')
result=[];done=False
for k,r in tags:
 if k==SYMBOL_CLASS and not done:
  root,payload=rooted(swf.payload(r))
  if root:
   result+=abc;r=swf.tag(k,payload);done=True
 result.append((k,r))
if not done: sys.exit('the hud movie names no root class')
out.write_bytes(swf.join(header,result))

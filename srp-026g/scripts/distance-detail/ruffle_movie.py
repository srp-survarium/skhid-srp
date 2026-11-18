#!/usr/bin/env python3
"""Test fixture adapter, never used by the release builder.

Stock Scaleform PlaceObject3 tags omit the SWF visibility byte when flag 0x20
is set, and omit the image class-name string for bitmap placements (0x10).
Ruffle rejects those tags, losing named children such as mc_value and card
sections. Supply visibility=false and clear the classless image flag in a
COPY for Ruffle. All graphics, transforms, names and ActionScript stay intact.
"""
from pathlib import Path
import sys,struct
sys.path.insert(0,str(Path(__file__).resolve().parents[3] / 'tools/actionscript'));import swf
src=Path(sys.argv[1]);out=Path(sys.argv[2]);count=0
def nested(data):
 pos=0;tags=[]
 while pos<len(data):
  start=pos;v=struct.unpack_from('<H',data,pos)[0];pos+=2;n=v&63
  if n==63:n=struct.unpack_from('<I',data,pos)[0];pos+=4
  pos+=n;tags.append((v>>6,data[start:pos]))
 return tags
def rewrite(tags):
 global count
 out=[]
 for k,r in tags:
  b=swf.payload(r)
  if k==39:r=swf.tag(k,b[:4]+b''.join(r for _,r in rewrite(nested(b[4:]))))
  elif k==70:
   b=bytearray(b)
   if b[0]&2 and b[1]&16 and not b[1]&8: b[1]&=~16;count+=1
   if b[1]&32: b.append(0);count+=1
   r=swf.tag(k,b)
  out.append((k,r))
 return out
h,t=swf.split(src.read_bytes());out.write_bytes(swf.join(h,rewrite(t)));print('Adapted native Scaleform image placements for Ruffle:',count)

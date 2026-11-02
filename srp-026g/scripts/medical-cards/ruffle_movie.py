#!/usr/bin/env python3
"""Test fixture adapter, never used by the release builder.

Stock Scaleform PlaceObject3 tags omit the SWF visibility byte when flag 0x20
is set, and omit the image class-name string for bitmap placements (0x10).
Some place a symbol by its class name alone (0x08, no character id), which
Ruffle does not support. Ruffle rejects those tags, losing named children such
as mc_value, card sections and the profile's CLIK controls. Supply
visibility=false, clear the classless image flag and place a class by its
symbol's character id in a COPY for Ruffle. All graphics, transforms, names
and ActionScript stay intact.

--slider-labels: the lobby's ammo sliders (one frame, no labels) also get the
labels AmmoSlider.draw goes to (default, disabled, focused). Scaleform goes on
past a label a clip lacks, Ruffle throws, and the lobby's own root stops there
in its setup, before the item cards' listeners.
"""
from pathlib import Path
import sys,struct
sys.path.insert(0,str(Path(__file__).resolve().parents[3] / 'tools/actionscript'));import swf
src=Path(sys.argv[1]);out=Path(sys.argv[2]);count=0
SLIDERS=['SrvSliderAmmo2','SrvSliderAmmoPD','SrvSliderAmmoCh'] if '--slider-labels' in sys.argv[3:] else []
def nested(data):
 pos=0;tags=[]
 while pos<len(data):
  start=pos;v=struct.unpack_from('<H',data,pos)[0];pos+=2;n=v&63
  if n==63:n=struct.unpack_from('<I',data,pos)[0];pos+=4
  pos+=n;tags.append((v>>6,data[start:pos]))
 return tags
def rewrite(tags,symbols):
 global count
 out=[]
 for k,r in tags:
  b=swf.payload(r)
  if k==39:r=swf.tag(k,b[:4]+b''.join(r for _,r in rewrite(nested(b[4:]),symbols)))
  elif k==70:
   b=bytearray(b)
   if b[1]&8 and not b[0]&2:
    end=b.index(0,4);name=b[4:end].decode()
    if name in symbols: b[4:end+1]=struct.pack('<H',symbols[name]);b[1]&=~8;b[0]|=2;count+=1
   if b[0]&2 and b[1]&16 and not b[1]&8: b[1]&=~16;count+=1
   if b[1]&32: b.append(0);count+=1
   r=swf.tag(k,b)
  out.append((k,r))
 return out
def labelled(tags,symbols):
 ids={symbols[n] for n in SLIDERS}
 out=[]
 for k,r in tags:
  if k==39 and struct.unpack_from('<H',swf.payload(r),0)[0] in ids:
   b=swf.payload(r);r=swf.tag(k,b[:4]+b''.join(swf.tag(43,l+b'\0') for l in (b'default',b'disabled',b'focused'))+b[4:])
  out.append((k,r))
 return out
h,t=swf.split(src.read_bytes());symbols={n:c for c,n in swf.symbol_classes(t)}
out.write_bytes(swf.join(h,labelled(rewrite(t,symbols),symbols)));print('Adapted native Scaleform image placements for Ruffle:',count)

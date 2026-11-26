#!/usr/bin/env python3
"""Draw 10 of the 11 tiles of initial/ (768 x 768 WebP) from seeded noise and the palettes below.

python3 initial_art.py [--out DIR]   (default: initial/ beside this script)
Pillow and NumPy. No input files: every shape comes from the seeds in this file. Written
2026-11-25 for the camouflage gallery and kept here so the tiles can be regenerated; with
NumPy 2.4.4 and Pillow 12.2.0 (as made) or 13.0.0 it reproduces them byte for byte. The 11th,
mm-14.webp, is a swatch of the real MM-14 drawn by classics_art.py; it replaced this script's MM-14
study (Pixel Steppe) on 2026-11-26. A row left None is a dropped tile: its place keeps every other
tile's grain seed (its index).
"""
import argparse
import numpy as np
from PIL import Image, ImageDraw
from pathlib import Path
import math
from random import Random
W=768
Y,X=np.mgrid[:W,:W]/W
def field(seed,w,h=None,x=X,y=Y,angular=False):
    h=h or w
    g=np.random.default_rng(seed).normal(0,1,(h,w))
    px=x*w; py=y*h
    xi=np.floor(px).astype(int); yi=np.floor(py).astype(int)
    u=px-xi; v=py-yi
    a=g[yi%h,xi%w]; b=g[yi%h,(xi+1)%w]; c=g[(yi+1)%h,xi%w]; d=g[(yi+1)%h,(xi+1)%w]
    if angular:
        return np.where(u>v,a+(b-a)*u+(d-b)*v,a+(d-c)*u+(c-a)*v)
    u=u*u*(3-2*u); v=v*v*(3-2*v)
    return (a*(1-u)+b*u)*(1-v)+(c*(1-u)+d*u)*v
def organic(seed,x=X,y=Y,wx=5,wy=5):
    return field(seed,wx,wy,x,y)+.41*field(seed+1,wx*2+1,wy*2+1,x,y)+.16*field(seed+2,wx*5,wy*5,x,y)
def angular(seed, density=6):
    # Quantized contours of overlapping triangular noise form broad, interlocking polygons.
    a=field(seed,density,density-1,angular=True,x=X+Y,y=Y)
    b=field(seed+2,density-1,density+1,angular=True,x=X,y=Y-X)
    c=field(seed+3,density,density,angular=True,x=X-Y,y=Y)
    return np.select([a>0.57,b>.21,c>.22],[3,2,1],0)
def periodic_poly(draw,points,fill):
    x0,y0=np.min(points,axis=0); x1,y1=np.max(points,axis=0)
    xs=range(math.ceil(-x1/W),math.floor((W-x0)/W)+1)
    ys=range(math.ceil(-y1/W),math.floor((W-y0)/W)+1)
    for i in xs:
        for j in ys:
            draw.polygon([(round(x+i*W),round(y+j*W)) for x,y in points],fill=fill)
def poly_img(arr):
    img=Image.fromarray(arr.astype('uint8'))
    return img,ImageDraw.Draw(img)
def rain(arr, seed, count, direction, fill):
    r=Random(seed)
    im,draw=poly_img(arr)
    for i in range(count):
        x,y=r.randrange(W),r.randrange(W)
        h=r.uniform(9,27)
        w=r.choices([1,2,3],[4,12,2])[0]
        dx=h*direction
        periodic_poly(draw,[(x,y),(x+w,y+1),(x+dx+w-.4,y+h),(x+dx,y+h-1)],fill)
    return np.array(im)
def snow():
    a=angular(119)
    # Fine dash islands interspersed with clean snow panels.
    b=rain(a,420,1800,.18,4)
    return np.where((field(121,8,6)>-.35)&(a!=3),b,a)
def tiger():
    y=Y+.072*field(125,3,3)+.016*field(97,6,7)+.005*field(122,12,16)
    a=field(187,5,19,X,y)+.45*field(109,7,43,X,y)+.14*field(115,14,87,X,y)
    r=field(102,8,13,X,y)+.25*field(21,15,35,X,y)
    return np.select([a>.35,(a>.10)&(r<.26),r>.30],[3,1,2],0)
def dazzle():
    # Panels rotate and interrupt sweeping diagonal ribbons.
    a=field(722,4,4,angular=True,x=X+Y,y=Y)
    p=(2*X+3*Y)%1; q=(4*X-3*Y+.19)%1; r=(X+3*Y)%1
    band1=np.select([p<.09,p<.31,p<.55,p<.67],[2,1,0,3],1)
    band2=np.select([q<.13,q<.5,q<.61],[2,3,1],0)
    band3=np.select([r<.11,r<.41,r<.51],[2,1,3],0)
    return np.where(a>.46,band2,np.where(a<-.60,band3,band1))
def urban():
    u=X+.06*field(170,3); v=Y+.05*field(199,3)
    return np.select([organic(182,u,v)>0.46,organic(72,u,v)>0.22,organic(294,u,v)>-.07],[3,2,1],0)
def dunes():
    a=np.select([organic(21,wx=3,wy=5)>.42,organic(5,wx=6,wy=4)>.13],[2,1],0)
    im,draw=poly_img(a)
    r=Random(7)
    for _ in range(52):
        cx=r.uniform(0,W); cy=r.uniform(0,W)
        for j in range(r.choice([3,4,5,7])):
            x=cx+r.gauss(0,15); y=cy+r.gauss(0,10)
            w=r.uniform(4,12); h=w*r.uniform(.6,1.1)
            path=[(x-w*.95,y-h*.23),(x-w*.4,y-h*.82),(x+w*.25,y-h),(x+w,y-h*.12),(x+w*.66,y+h*.67),(x+w*.1,y+h),(x-w*.79,y+h*.6)]
            periodic_poly(draw,path,3)
            periodic_poly(draw,[(x+(ix-x)*.66+1.2,y+(iy-y)*.64-2) for ix,iy in path],4)
    return np.array(im)
def birch():
    # Minute stepped edges around larger pale leaf islands, with shadow around some cells.
    u=np.floor(X*128)/128; v=np.floor(Y*128)/128
    a=organic(185,u,v,7,8)+.18*field(105,37,42,u,v)
    b=organic(292,u,v,4,5)
    return np.select([a>.60,(a>.39)&(b<.05),b>0.24],[3,2,1],0)
def ember():
    u=X+.02*field(810,6); v=Y+.026*field(811,6)
    a=organic(631,u,v,6,7)
    b=organic(624,u,v,9,10)
    c=organic(49,u,v,15,14)
    im,draw=poly_img(np.select([a>.55,b>.26,c>.5],[3,2,1],0))
    r=Random(77)
    for _ in range(210):
        x,y=r.randrange(W),r.randrange(W)
        base=im.getpixel((x,y))
        color=r.choice([max(0,base-1),min(3,base+1)])
        for j in range(r.randint(2,6)):
            cx=x+r.gauss(0,9); cy=y+r.gauss(0,7); w=r.uniform(2,5); h=r.uniform(2,6)
            periodic_poly(draw,[(cx-w,cy-.3*h),(cx-.5*w,cy-h),(cx+.8*w,cy-h),(cx+w,cy+.2*h),(cx+.2*w,cy+h),(cx-.7*w,cy+.7*h)],color)
    return np.array(im)
def tidal():
    u=np.floor(X*72)/72; v=np.floor(Y*72)/72
    a=organic(93,u,v,4,5)+.3*field(112,24,22,u,v)
    b=organic(563,u,v,6,5)+.28*field(34,22,22,u,v)
    return np.select([a>.51,b>.37,((a>-.04)&(b<.04))],[3,2,1],0)
rows=[
("frost-splinter","E6E6D9 BEC3B7 919C98 636E70 A5AEA7",snow),
("rust-tiger","A95533 CFA273 7C4C34 272B25",tiger),
("quarry","C2BC9A A29B73 78816C 4D5045",lambda:angular(881,5)),
("black-ice","232C30 3C5355 ADABA0 526D6A",dazzle),
("bruise","68565F 80767A 634858 392F39",urban),
None,  # Reed Rain, dropped 2026-11-26
("dune-chip","CDB58A AA916C AC7657 4F443D E5D9BE",dunes),
None,  # Night Grid, dropped 2026-11-26
("birch-leaf","45594B 52654D 91A184 B9C3A0",birch),
("ember-leaf","8E6040 BD8C50 AD6342 593F3D",ember),
("tidal-pixel","577B78 789B93 506672 35495A",tidal),
None]  # Violet Splinter, dropped 2026-11-26
def mosaic():
    u=X+.07*field(810,4,3); v=Y+.05*field(523,4,4)
    a=organic(251,u,v,5,9); b=organic(216,u,v,6,8); c=organic(211,u,v,8,10); d=organic(226,u,v,8,11)
    return np.select([a>.89,b>.53,c>.39,d>.26],[4,3,2,1],0)
# Row 12 was the MM-14 study.
rows.extend([None,("field-mosaic","9A9473 B0A388 837F5B 6D6E4F 635948",mosaic)])


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out',type=Path,default=Path(__file__).resolve().parent/'initial')
    out=parser.parse_args().out
    out.mkdir(parents=True,exist_ok=True)
    for i,row in enumerate(rows):
        if row is None:
            continue
        name,palette,fn=row
        p=np.array([[int(c[k:k+2],16) for k in (0,2,4)] for c in palette.split()],dtype='float32')
        color=p[fn()]
        # Subtle print mottling, periodic over each tile. No pre-lit bevels or shadows.
        grain=field(i+670,99,97)*.85+field(i+720,38,36)*.55+field(i+1000,18,17)*.60
        weave=.40*np.cos(2*np.pi*(X*256+Y*128))+.42*np.cos(2*np.pi*Y*256)
        image=Image.fromarray(np.rint(np.clip(color+grain[:,:,None]+weave[:,:,None],0,255)).astype('uint8'))
        image.save(out/(name+'.webp'),quality=95,method=6)
        print(name,palette,flush=True)


if __name__=='__main__':
    main()

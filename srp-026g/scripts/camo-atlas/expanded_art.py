"""Additional seamless prints for the atlas, composed at illustration scale.

All geometry in the ten textile, mineral and map studies is original. Museum and
USGS links identify visual references. The 75 other colorways rework CC0 source
tiles with custom palettes and periodic overprints.
"""
import math
from concurrent.futures import ThreadPoolExecutor
from io import BytesIO
from zipfile import ZipFile

import numpy as np
from PIL import Image, ImageDraw
from scipy.ndimage import map_coordinates

SIZE = 768
Y, X = np.mgrid[0:SIZE,0:SIZE].astype(np.float32) / SIZE
INKS = (
    ('Cobalt','182943 245ca5 82b2cc f4e7c9'),
    ('Oxblood','342738 78364a c27665 f1c9a7'),
    ('Glacial','233c50 517b91 a9c8ce f3efe6'),
    ('Citron','243c49 3f7679 acc35f f2df99'),
    ('Madder','392747 874c73 c68295 f2caba'),
    ('Verdigris','243941 287a78 89bba2 e8d5ac'),
    ('Saffron','302f40 76514a d8963f f4d99c'),
    ('Graphite','25272c 525965 a6aaa4 ebe7dc'),
    ('Orchid','2e2846 625181 ad84ad ecd5d7'),
    ('Lapis','212d47 374f92 6596be e5d0ae'),
    ('Satsuma','332f3c 884b3d d87c44 f4d097'),
    ('Aubergine','322a3e 604760 a87583 e2c1ab'),
    ('Riptide','213651 306783 5bb5aa e5d9b5'),
    ('Pewter','2d3b45 5f7279 aba9a0 eae1c7'),
    ('Carmine','352940 7c3956 c35c68 ebafa6'),
    ('Marigold','35313a 805744 c48e49 f2cf7c'),
    ('Ultramarine','262844 3d54a3 869cce e7dcba'),
    ('Seafoam','273e43 4e8682 a5d0b3 ece4c8'),
    ('Russet','322c36 73483d b87148 ddc8a1'),
    ('Icelandic','283749 547084 afc2c4 f3f3e7'),
    ('Cerise','342d42 763959 b95681 edb3b8'),
    ('Kingfisher','193441 1c6a79 5baca5 e1ac6e'),
    ('Sundown','352f46 705273 c38383 edc398'),
    ('Maritime','293641 3a6477 92afaa e8dbc0'),
    ('Garnet','352736 6f3644 b55e57 e4ad87'),
)
STUDIES = (
    ('Soundings','Stepped depth bands and fine contour loops wind around submerged peaks.',
     'https://www.usgs.gov/faqs/what-a-topographic-map','USGS · Contour maps'),
    ('Agate','Alternating mineral bands pool into broken eyes and softly folded ribbons.',
     'https://naturalhistory.si.edu/explore/collections/geogallery/10026355','Smithsonian · Banded agate'),
    ('Kasuri','Feathered edges break up staggered diamond-shaped resist marks.',
     'https://www.metmuseum.org/art/collection/search/45408','The Met · Warp and weft kasuri'),
    ('Cloud Hex','Cloud-shaped islands interrupt an outlined, nested hexagon repeat.',
     'https://www.metmuseum.org/art/collection/search/50803','The Met · Clouds and hexagons'),
    ('Wax Resist','Six-petal resist rosettes lie over mottled dye and wandering fine cracks.',
     'https://asia.si.edu/education/educator-resources/teaching-china-with-the-smithsonian/lesson-plans/batik-textile-connections-from-china-to-today/','Smithsonian · Batik textiles'),
    ('Crosscheck','Unequal warp and weft stripes cross with narrow pinstripes and twill marks.',
     'https://www.vam.ac.uk/dundee/info/scotland-s-oldest-tartan-discovered-by-scottish-tartans-authority','V&A Dundee · Glen Affric tartan'),
    ('Fishbone','Broken narrow chevrons run through staggered bands of woven color.',
     'https://www.metmuseum.org/art/collection/search/454599','The Met · Herringbone silk fragment'),
    ('Terrazzo','Three sizes of angular inlay chips sit in a contrasting mineral ground.',
     'https://www.si.edu/object/gems-and-minerals-exhibit-national-museum-natural-history%3Asiris_sic_9363','Smithsonian · Terrazzo floor'),
    ('Screenprint','A dot screen swells and contracts over broad areas of flat printed color.',
     'https://www.metmuseum.org/art/collection/search/265373','The Met · Halftone screen'),
    ('Beadwork','Stepped diamond shapes form a beaded band with small contrasting accents.',
     'https://artsandculture.google.com/asset/%D0%A8%D0%B8%D0%B9%D0%BD%D1%96-%D0%B1%D1%96%D1%81%D0%B5%D1%80%D0%BD%D1%96-%D0%BF%D1%80%D0%B8%D0%BA%D1%80%D0%B0%D1%81%D0%B8-%D2%90%D0%B5%D1%80%D0%B4%D0%B0%D0%BD%D0%B8-%D0%90%D0%B2%D1%82%D0%BE%D1%80-%D0%BD%D0%B5%D0%B2%D1%96%D0%B4%D0%BE%D0%BC%D0%B8%D0%B9/GgGw8mhbXFm5rw?hl=uk','National Museum of the History of Ukraine · Geometric beadwork'),
)


def field(rng, nx, ny, x=X, y=Y, order=3):
    values=rng.uniform(-1,1,(ny,nx)).astype(np.float32)
    return map_coordinates(values,[y*ny,x*nx],order=order,mode='grid-wrap')


def closest_cell(x,y,nx,ny):
    """Offset rows wrap over an even row count. Return signed position inside nearest hex."""
    row=np.floor(y*ny)
    best=np.full_like(x,9.)
    for dr in (-1,0,1):
        r=row+dr
        xx=(x*nx-.5*(r%2)+.5)%1-.5
        yy=y*ny-r
        metric=xx*xx+yy*yy*.75
        use=metric<best
        if dr==-1:
            closest_x=xx
            closest_y=yy
        else:
            closest_x=np.where(use,xx,closest_x)
            closest_y=np.where(use,yy,closest_y)
        best=np.minimum(metric,best)
    return closest_x,closest_y


def polygon_wrap(draw, points, color):
    xmin,ymin=points.min(axis=0)
    xmax,ymax=points.max(axis=0)
    for i in range(math.ceil(-xmax/SIZE),math.floor((SIZE-xmin)/SIZE)+1):
        for j in range(math.ceil(-ymax/SIZE),math.floor((SIZE-ymin)/SIZE)+1):
            draw.polygon([tuple(p) for p in points+[i*SIZE,j*SIZE]], fill=color)


def terrazzo(rng,c):
    im=Image.fromarray(c.astype('uint8'))
    pen=ImageDraw.Draw(im)
    for count,size in ((68,32),(135,12),(330,4)):
        for _ in range(count):
            points=rng.integers(3,7)
            angles=np.arange(points)*(2*math.pi/points)+rng.uniform(-.3,.3,points)+rng.uniform(0,2*math.pi)
            lengths=size*rng.uniform(.49,1.12,points)
            vertices=np.stack((np.cos(angles)*lengths,np.sin(angles)*lengths*rng.uniform(.45,1.15)),axis=1)
            polygon_wrap(pen,np.round(rng.uniform(0,SIZE,2)+vertices).astype(int),int(rng.choice([0,2,3])))
    return np.asarray(im)


def design(family,variant):
    rng=np.random.default_rng(15001+variant*127+family*37)
    a=field(rng,4+variant%2,4)
    b=field(rng,6,7)
    x=X+rng.uniform(0,1)
    y=Y+rng.uniform(0,1)
    if family==0:
        height=a+.27*b+.07*field(rng,14,14)
        levels=height*(4+variant%3)
        c=np.select([levels%1<.12,levels%3<.24,height>.18],[3,0,2],1)
    elif family==1:
        u=x+.055*a;v=y+.055*b
        xx,yy=closest_cell(u,v,2+variant%3,4)
        bands=np.sqrt(xx*xx+(.8*yy)**2)*6+.62*field(rng,6,7,u,v)
        c=np.select([bands%1<.09,bands%2<.59,bands%1<.48],[3,0,2],1)
    elif family==2:
        u=x+.022*field(rng,5,86); v=y+.008*field(rng,65,5)
        xx,yy=closest_cell(u,v,5+variant%4,6+2*(variant%2))
        d=np.abs(xx)+.68*np.abs(yy)
        c=np.select([d<.10,(d>.18)&(d<.27),d<.44,b>.35],[3,3,2,1],0)
    elif family==3:
        xx,yy=closest_cell(x,y,7+variant%3,8)
        d=np.maximum(abs(xx)*2,abs(xx)+1.5*abs(yy))
        c=np.select([d>.91,(d>.62)&(d<.71),d<.18],[3,2,2],0)
        c=np.where(a+.18*b>.34,np.where(b>.28,3,1),c)
    elif family==4:
        xw=x+.009*a;yw=y+.012*b
        xx,yy=closest_cell(xw,yw,4+variant%3,6)
        rad=np.sqrt(xx**2+yy**2)
        petals=np.cos(6*np.arctan2(yy,xx)-variant*.24)
        rim=.24+.12*petals
        leaf=(abs(rad-rim)<.022)|(rad<.066)|(abs(rad-.47)<.018)
        crack=abs(b+.29*field(rng,17,18))<.024
        c=np.select([leaf,crack,rad<rim,a>.22],[3,2,2,1],0)
    elif family==5:
        # Unequal stripe widths form a sett. The crossings alternate warp/weft.
        n=2+variant%3
        tx=(x*n)%1;ty=(y*n)%1
        widths=np.array([.033,.09,.20,.24,.40,.52,.62,.68,.75,.82,.84,.91])
        widths=np.sort(widths+rng.uniform(-.016,.016,widths.size))
        colors=np.array([3,1,0,2,1,2,1,3,0,1,3,0,1])
        warp=colors[np.digitize(tx,widths)]
        weft=colors[np.digitize(ty,widths)]
        c=np.where(((x+y)*128)%1<.52,warp,weft)
    elif family==6:
        n=8+4*(variant%3)
        tx=x*n
        phase=np.where(tx.astype(int)%2==0,tx%1,1-tx%1)
        band=(y*(11+variant%4)+phase*.64)%1
        c=np.select([band<.10,band<.26,(np.floor(tx)+np.floor(y*4))%4==0],[3,2,2],0)
        c=np.where((y*4)%1<.17,np.where(band<.42,0,1),c)
    elif family==7:
        c=terrazzo(rng,np.ones_like(a))
    elif family==8:
        u=x+y;v=y-x
        freq=18+variant
        dx=(u*freq+.5)%1-.5;dy=(v*freq+.5)%1-.5
        rad=np.sqrt(dx*dx+dy*dy)
        threshold=np.clip(.27+.27*a+.055*b,.06,.63)
        c=np.select([rad<threshold*.69,rad<threshold,a>.23,b<-.35],[0,1,2,1],3)
    else:
        # Keep motif period an exact divisor of tile dimensions.
        n=48+24*(variant%2)
        u=np.floor(x*n);v=np.floor(y*n)
        xx=((u+6*((v//12)%2))%12)-6; yy=v%12-6
        d=abs(xx)+abs(yy)
        c=np.select([d==5,(d==2)|(d==9),d<=1,d<=5],[3,2,3,1],0)
        c=np.where((v%24==0)|(v%24==1),np.where((u//2)%2==0,2,1),c)
    return c.astype('uint8'),a


def save_original(out,index):
    family,variant=index%len(STUDIES),index//len(STUDIES)
    name,description,url,label=STUDIES[family]
    color,inks=INKS[(variant*7+family*13)%len(INKS)]
    title=f'{color} {name}'
    values=np.array([list(bytes.fromhex(h)) for h in inks.split()],dtype=np.float32)
    indices,grain=design(family,variant)
    rgb=values[indices]+grain[:,:,None]*1.6
    image=Image.fromarray(np.clip(rgb,0,255).astype('uint8'))
    slug=title.lower().replace(' ','-')
    image.save(out/(slug+'.webp'),quality=94,method=4)
    return (slug,title,name,description+' Original atlas print.',url,label+' · visual reference',
            ' '.join('#'+h.upper() for h in inks.split()))


REMIX = (
    'Tiger','Net','Cheetah_Dots','Seigaiha','M90_Basic_Small','Brush_Strokes_Big',
    'DBDU','Classic','Desert_Night','Dotty','Tiger_Vertical_Dotty','Fractal_Polygon',
    'Watercolor_Dots','Lines_Thin','Polygon',
)


def save_reprint(out,index,archives,families,member):
    number,style=index%len(REMIX),index//len(REMIX)
    motif=REMIX[number]
    pack,_,family,details,_,_=next(row for row in families if row[1]==motif)
    # Different source realizations and different macro overlays keep variants distinct at 512px.
    theme=('Desert','Urban','Woodland','Urban','Desert')[style]
    variant=(number+style)%3+1
    with ZipFile(archives[pack-1]) as archive:
        src=archive.read(member(pack,motif,theme,variant))
    with Image.open(BytesIO(src)) as source:
        base=np.asarray(source.convert('L').resize((SIZE,SIZE),Image.Resampling.LANCZOS),dtype=np.float32)
    if style%2:base=base.T
    rng=np.random.default_rng(9571+31*number+17*style)
    a,b=field(rng,5,4),field(rng,4,5)
    x=X+.021*a+rng.random();y=Y+.021*b+rng.random()
    val=map_coordinates(base,[y*SIZE,x*SIZE],order=1,mode='grid-wrap')
    low,high=np.percentile(val,(1,99))
    val=np.clip((val-low)/(high-low),0,1)
    # Five separate printing treatments use broad layers so the changes survive mipmapping.
    if style==0: val=np.where(a>.3,.19+val*.78,val*.82)
    elif style==1: val=np.where((a+.35*b)>.32,.18+val*.69,val)
    elif style==2: val=np.where(abs(a+.35*b)<.15,.88-val*.25,val*.87)
    elif style==3: val=np.where(((X+Y+.09*a)*4)%1<.18,.14+val*.49,val)
    else: val=np.where(b>.25,.9-val*.56,val*.91)
    color,inks=INKS[(style*8+number*7+2)%len(INKS)]
    stops=np.array([list(bytes.fromhex(h)) for h in inks.split()])
    ramp=np.stack([np.interp(np.arange(256),[0,85,170,255],channel) for channel in stops.T],axis=1)
    image=Image.fromarray(np.rint(ramp).astype(np.uint8)[(val*255).astype('uint8')])
    title=f'{color} {family}'
    slug=title.lower().replace(' ','-')
    image.save(out/(slug+'.webp'),quality=94,method=4)
    return (slug,title,family,details+'. Atlas color separation and overprint on CC0 artwork.',
            f'https://snowycocoon.itch.io/camouflage-texture-pack-{pack}',f'SnowyCocoon · Free Pack {pack} · CC0',
            ' '.join('#'+h.upper() for h in inks.split()))


def expanded(out,archives,families,member):
    with ThreadPoolExecutor(max_workers=4) as pool:
        original=list(pool.map(lambda i:save_original(out,i),range(90)))
        reprints=list(pool.map(lambda i:save_reprint(out,i,archives,families,member),range(75)))
    return original+reprints

#!/usr/bin/env python3
"""Import the atlas's 285 additional tiles from the author's two unmodified FREE ZIPs.

Usage: python3 import_art.py Camouflage_Pattern_Pack_1_FREE.zip Camouflage_Pattern_Pack_2_FREE.zip
Pillow 12.1+, NumPy and SciPy. The catalog order is part of the game dictionary.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import csv
from hashlib import file_digest
from io import BytesIO
from pathlib import Path
from zipfile import ZipFile

import numpy as np
from PIL import Image

from expanded_art import expanded

HERE = Path(__file__).resolve().parent
ARCHIVES = (
    'cb0ed66f0e6afc0735532af8827282dccc239d9c9085e66ce7cac23f8c104e2f',
    'edc78e404b10803ddf77b5498e8bac97326802e289db2841afabbf755c7bdda5',
)
# Archive, motif, display family, visible design, art-directed alternate name, ink ramp.
FAMILIES = [
    (2,'Seigaiha','Wave Fan','Overlapping fans of concentric arcs','Porcelain Tide','203549 346976 7ca59e e2d6b5'),
    (2,'Fractal_Stars','Starfield','Fine interlocking stars and square flecks','Orchid Static','27233d 55436a 927397 d0acc1'),
    (1,'Tiger','Tiger Stripe','Long, irregular horizontal bands','Cinnabar Tiger','312821 653b37 b44e3c d8a976'),
    (2,'Watercolor_Dots','Watercolor','Translucent pools of pigment with soft edges','Sea Glass','23404b 4c797c 90b0a1 d9d6b2'),
    (1,'Net','Netting','Crossing bands around open, uneven cells','Copper Mesh','253c38 547366 927553 c49c71'),
    (2,'M90_Basic_Small','Cut Paper','Large, straight-edged paper-like shapes','Glacier Cut','364857 69808d a7b8bd e8ebe1'),
    (1,'DBDU','Stone Chip','Tiny outlined stones over broad islands of color','Salt Crater','413f46 8b767d c9a99b efdfbd'),
    (2,'Brush_Strokes_Big','Broadbrush','Broad, broken strokes layer across the tile','Kingfisher','203746 32757a 8ab1a2 e3a366'),
    (1,'Classic','Cloud','Rounded patches overlap in a loose field','Mulberry Cloud','332e3d 625063 966d79 c7aba3'),
    (2,'Desert_Night','Broken Grid','A square lattice breaks around organic islands','Amber Circuit','2a302b 555441 9a7948 dbb97a'),
    (1,'Noodles','Scribble','Short curved marks form a tight wandering scribble','Persimmon Ink','40363c 875344 c97c57 eac7a1'),
    (2,'Strichtarn','Rain Streak','Slender vertical dashes on a quieter ground','Copper Rain','293e41 506862 a58b69 d7b791'),
    (1,'Polygon','Shard','Flat polygons meet in abrupt facets','Basalt Shard','31323c 5a586e 9291a0 c4bdab'),
    (2,'Cheetah_Dots','Rosette','Dark-edged organic rosettes drift across the ground','Honey Rosette','3c302b 765540 b98d5b e9c996'),
    (1,'Pixel','Pixel Grain','Layers of granular digital clusters','Juniper Pixel','253934 496659 899c80 cbbfa3'),
    (2,'Fractal_Chunks','Fracture','Small rock-like fragments meet along thin borders','Rose Breccia','433b45 735e72 a9808a d9bfa7'),
    (1,'Cells_Big','Riverstone','Wide irregular cells fit together like river stones','Malachite','213e3c 456f65 8baf8e d1c4a2'),
    (2,'Lines_Fat','Ribbon','Wide rippling ribbons run down the surface','Night Silk','232338 4f4565 827699 b7a5b6'),
    (1,'Dotty','Pebble','Small rounded spots overlap like pebbles','Saffron Gravel','414338 777346 b6a056 e0ca8e'),
    (2,'Fractal_Polygon','Crackle','Fractured polygons break into finer grain at their edges','Opal Fault','38454f 698187 9fab9c d9c8b3'),
    (1,'Tiger_Vertical_Dotty','Reed Stripe','Vertical stripes carry a light scattering of small dots','Coral Reeds','2c4649 578178 b88071 e0b6a2'),
    (2,'Brush_Strokes_Small','Drybrush','Fine, overlapping brush dabs vary in direction','Sienna Wash','3d3732 6b5444 a17f5b d8c39a'),
    (1,'Cells_Small','Shingle','Dense little cells create a fine shingle texture','Blue Shingle','263647 4b6879 879fa4 c1c9bc'),
    (2,'M90_Basic_Large','Mosaic','Angular fragments interlock at several scales','Iris Mosaic','3b344b 655a77 9387a3 c7b5bd'),
    (2,'Lines_Thin','Woodgrain','Thin wavering lines curl around knots','Walnut Burl','352d2a 634237 986b4c cda67a'),
]
# These source files have their own palette and printing treatment.
SPECIALS = [
    (1,'Blue_Tiger','Harbor Tiger','Tiger Stripe'), (1,'Dotty_Tiger','Reef Tiger','Reed Stripe'),
    (1,'Woodland_Pixel','Lichen Pixel','Pixel Grain'), (1,'Polygon_Dark','Black Thicket','Shard'),
    (2,'Fractal_Stars','Mauve Static','Starfield'), (2,'Watercolor_Dots','Inkstone','Watercolor'),
    (2,'Desert_Night','Signal Grid','Broken Grid'), (2,'Cheetah_Dots','Silver Rosette','Rosette'),
    (2,'Lines_Thin','Fired Clay','Woodgrain'), (2,'M90_Basic_Small','Citron Cut','Cut Paper'),
    (1,'Seigaiha','Olive Tide','Wave Fan'), (1,'Strichtarn_Dark','Ash Rain','Rain Streak'),
]
EXTRAS = [
    ('Seigaiha','Vermilion Tide','3b3336 794950 b96a58 e8b98f'),
    ('Net','Polar Mesh','2d414d 66848c a3bcbb e9e9d8'),
    ('Brush_Strokes_Big','Amethyst Wash','343049 615c81 9285a2 cdb8bf'),
    ('M90_Basic_Small','Ochre Cut','3c393a 766146 b58d58 d8c69c'),
    ('Watercolor_Dots','Petal Wash','4e4553 8e707b c7999f e5c5ad'),
    ('Cheetah_Dots','Lagoon Rosette','243e44 427679 80a799 c8c4a3'),
    ('Desert_Night','Ultraviolet Grid','272836 474359 78617f ba92a4'),
    ('Tiger','Salt Tiger','424850 879197 b7b9ac e7e4cf'),
]


def select():
    for index, (pack,motif,family,details,name,inks) in enumerate(FAMILIES):
        yield pack,motif,'Urban',index%3+1,family,name,inks,details
    for pack,motif,name,family in SPECIALS:
        details=next(item[3] for item in FAMILIES if item[2]==family)
        yield pack,motif,'Base',2,family,name,'',details
    for theme in ('Woodland','Desert','Urban'):
        for index,(pack,motif,family,details,_,_) in enumerate(FAMILIES):
            variant=(index+{'Woodland':1,'Desert':0,'Urban':2}[theme])%3+1
            yield pack,motif,theme,variant,family,f'{theme} {family}','',details
    for motif,name,inks in EXTRAS:
        pack,_,family,details,_,_=next(row for row in FAMILIES if row[1]==motif)
        yield pack,motif,'Urban',3,family,name,inks,details


def member(pack,motif,theme,variant):
    # The large M90 sheets in Pack 2 omit the otherwise consistent "_Pattern" token.
    pattern='' if motif=='M90_Basic_Large' and theme!='Base' else '_Pattern'
    return f'Camouflage_Pattern_Pack_{pack}_FREE/1K/{theme}/Camouflage_{motif}{pattern}_{theme}_1K_{variant}.png'


def recolor(image,inks):
    # Transfer the source ink density through an explicit four-stop palette.
    # Use only the occupied luminance range so low-contrast source sheets retain their detail.
    gray=np.asarray(image.convert('L'))
    low,high=np.percentile(gray,(0.5,99.5))
    stops=np.array([list(bytes.fromhex(h)) for h in inks.split()])
    source=np.linspace(low,high,len(stops))
    ramp=np.stack([np.interp(np.arange(256),source,channel) for channel in stops.T],axis=1)
    return Image.fromarray(np.rint(ramp).astype(np.uint8)[gray])


def palette(image):
    quant=image.resize((128,128),Image.Resampling.BOX).quantize(colors=5)
    colors=np.array(quant.getpalette(),dtype=np.uint8).reshape(-1,3)
    return ' '.join('#'+bytes(colors[i]).hex().upper() for count,i in sorted(quant.getcolors(),reverse=True))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('free_packs',type=Path,nargs=2,metavar='ZIP')
    parser.add_argument('--out',type=Path,default=HERE/'art')
    args=parser.parse_args()
    for path,expected in zip(args.free_packs,ARCHIVES):
        with path.open('rb') as stream:
            if file_digest(stream,'sha256').hexdigest()!=expected:
                parser.error(f'Unexpected archive: {path.name}')
    args.out.mkdir(parents=True,exist_ok=True)
    def convert(spec):
        pack,motif,theme,variant,family,name,inks,details=spec
        origin=member(pack,motif,theme,variant)
        with ZipFile(args.free_packs[pack-1]) as archive:
            source=archive.read(origin)
        with Image.open(BytesIO(source)) as opened:
            image=opened.convert('RGB')
        assert image.size==(1024,1024),origin
        if inks:
            image=recolor(image,inks)
            description=f'{details}. Atlas recolor of SnowyCocoon’s {theme.lower()} tile.'
            palette_text=' '.join('#'+color.upper() for color in inks.split())
        else:
            description=f'{details}. Original SnowyCocoon colorway.'
            palette_text=palette(image)
        slug=name.lower().replace(' ','-')
        image.save(args.out/(slug+'.webp'),quality=94,method=4)
        return (slug,name,family,description,f'https://snowycocoon.itch.io/camouflage-texture-pack-{pack}',
                f'SnowyCocoon · Free Pack {pack} · CC0',palette_text)
    with ThreadPoolExecutor(max_workers=4) as pool:
        rows=list(pool.map(convert,select()))
    rows.extend(expanded(args.out,args.free_packs,FAMILIES,member))
    assert len(rows)==len({r[0] for r in rows})==285
    with (args.out/'catalog.tsv').open('w',encoding='utf8',newline='') as stream:
        writer=csv.writer(stream,delimiter='\t',lineterminator='\n')
        writer.writerow(('slug','name','family','description','source_url','source_label','palette'))
        writer.writerows(rows)


if __name__=='__main__':
    main()

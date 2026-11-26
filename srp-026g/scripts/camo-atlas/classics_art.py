#!/usr/bin/env python3
"""Draw the tiles of classics/ (768 x 768 WebP): real camouflage patterns from free swatches.

python3 classics_art.py fetch DIR                 download the swatches below into DIR
python3 classics_art.py build --sources DIR [--out DIR]

Each row crops a square of a free swatch from Wikimedia Commons (SWATCHES: the file, its
licence and the exact bytes used, by sha256), evening out a photograph's lighting where asked,
and makes it periodic with a minimum-cost seam on the wrapped edges. classics/SOURCES.txt says
where each one comes from.
One swatch replaces an older tile of initial/ and is written there (INITIAL), keeping its item.
Pillow, NumPy and SciPy; with NumPy 2.4.4, Pillow 13.0.0 and SciPy 1.17.1 it reproduces
classics/*.webp and initial/mm-14.webp byte for byte. Each row's id is its item in the game dictionary.
"""
import argparse
import csv
import hashlib
import time
import urllib.request
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.ndimage import gaussian_filter

HERE = Path(__file__).resolve().parent
SIZE = 768
COMMONS = 'https://commons.wikimedia.org/wiki/File:'

# name: (Commons file, download (the original or a standard thumbnail), its sha256, licence, author)
SWATCHES = {
    'lizard': ('French_Lizard_Pattern_Swatch_from_the_1950s.png',
               'https://thumb.wikimedia.org/wikipedia/commons/thumb/c/c0/French_Lizard_Pattern_Swatch_from_the_1950s.png/3840px-French_Lizard_Pattern_Swatch_from_the_1950s.png',
               '509994ed27ced6b3a775cc415f0ee8c298809698f1e444cbd7b57c81eafba31d', 'CC0', 'VoltigeurFR'),
    'erdl': ('Brown-dominant_%22highland%22_ERDL_camouflage_pattern_swatch.svg',
             'https://thumb.wikimedia.org/wikipedia/commons/thumb/a/a0/Brown-dominant_%22highland%22_ERDL_camouflage_pattern_swatch.svg/3840px-Brown-dominant_%22highland%22_ERDL_camouflage_pattern_swatch.svg.png',
             '81c7eb807ce57aa500241f26e2cefd4fe35163fbe8e1df4a111622e902a69fae', 'public domain (U.S. Government work)',
             'Illegitimate Barrister'),
    'dcu': ('DCU_camo_swatch.png',
            'https://upload.wikimedia.org/wikipedia/commons/8/82/DCU_camo_swatch.png',
            'f992a0dc395116e626d7ea214296ffd61870872e0ab17eec153055813caadf4c', 'public domain (U.S. Government work)',
            'Illegitimate Barrister'),
    'brush': ('Phase_I_All-Over_Brush_Urban_Camo_Close-Up_Swatch.png',
              'https://thumb.wikimedia.org/wikipedia/commons/thumb/6/6c/Phase_I_All-Over_Brush_Urban_Camo_Close-Up_Swatch.png/1280px-Phase_I_All-Over_Brush_Urban_Camo_Close-Up_Swatch.png',
              'a79a871defdae8dfe603529c3781a6dddb8b7d0def8071a8243a982b60aa5e2b', 'public domain (U.S. Government work)',
              'SergeantSelfExplanatory'),
    'cce': ('Bariolage_Centre-Europe.png',
            'https://upload.wikimedia.org/wikipedia/commons/1/1a/Bariolage_Centre-Europe.png',
            '793ca83c12eec6ac8308fd077a446fe9fd4f118d14eef779ea40074d89c04902', 'CC0', 'Youri BRIAND'),
    'bme': ('Bariolage_multi-environnement_(B.M.E.).png',
            'https://upload.wikimedia.org/wikipedia/commons/d/df/Bariolage_multi-environnement_%28B.M.E.%29.png',
            '10dfc0408264ab3ba89398f047bf3235517145404df93d7fbeb6bdc1a38cfd0e', 'CC0', 'Youri BRIAND'),
    'hu2015': ('2015M.png',
               'https://upload.wikimedia.org/wikipedia/commons/4/4e/2015M.png',
               '371316112e21753557e804e750af2631a64720c2540c6c9bc97b7a03dbe067f4', 'CC0', 'Shotya'),
    'mm14': ('MM14.jpg',
             'https://upload.wikimedia.org/wikipedia/commons/c/cd/MM14.jpg',
             '2355623bc74b08cf21ba632c9003dcc5b1488a3a39bedb045a227633ded42017', 'CC BY-SA 4.0',
             'Lucas Cavalcante Mesquita'),
    'rhodesian': ('Rhodesian_Brushstroke_pattern.jpg',
                  'https://upload.wikimedia.org/wikipedia/commons/1/19/Rhodesian_Brushstroke_pattern.jpg',
                  '4b1657794e12f6fe9c810bde0e38fcdea09a8ca4d847f058808b55ecc88b277d', 'CC BY-SA 3.0', 'Katangais'),
    'tropentarn': ('Tropentarn_Fabric_(cropped_2).jpg',
                   'https://thumb.wikimedia.org/wikipedia/commons/thumb/2/2d/Tropentarn_Fabric_%28cropped_2%29.jpg/1920px-Tropentarn_Fabric_%28cropped_2%29.jpg',
                   'c754c6d27774b12d7b3c1e5b0fe3d16e5661cdb25f6ab67a1aca013d922e8c2d', 'CC BY-SA 4.0', 'Murchison-Eye'),
    'bgdpm': ('Bulgarian_M2003_DPM_camouflage.jpg',
              'https://thumb.wikimedia.org/wikipedia/commons/thumb/c/c3/Bulgarian_M2003_DPM_camouflage.jpg/3840px-Bulgarian_M2003_DPM_camouflage.jpg',
              '8e9eee6572ab15f4eb0eb5b851c23c74971cd5ec87c5a6d0e2d7e0271b72ca74', 'CC BY-SA 4.0', 'Spektov'),
    'bgsplinter': ('Bulgarian_Splinter_camouflage.jpg',
                   'https://thumb.wikimedia.org/wikipedia/commons/thumb/2/2a/Bulgarian_Splinter_camouflage.jpg/3840px-Bulgarian_Splinter_camouflage.jpg',
                   '8c5393f2ffe5d562c49ff7363311c10452982f29402c5170616261269dc308ee', 'CC BY-SA 4.0', 'Spektov'),
    'nfpblue': ('M19_NFP_Blue_-_Dutch_pattern.jpg',
                'https://upload.wikimedia.org/wikipedia/commons/9/9a/M19_NFP_Blue_-_Dutch_pattern.jpg',
                '67d2731157410be98125e41fe9161fc16f78008131bbe1e013f62c6fff74e3c8', 'CC BY-SA 4.0',
                'Ministerie van Defensie (Netherlands)'),
}


def seam(cost):
    """The top-to-bottom path of least summed cost through a rows x columns array."""
    rows, cols = cost.shape
    total = cost.copy()
    back = np.zeros((rows, cols), int)
    for r in range(1, rows):
        prev = total[r - 1]
        options = np.stack([np.r_[np.inf, prev[:-1]], prev, np.r_[prev[1:], np.inf]])
        step = options.argmin(0)
        back[r] = np.arange(cols) + step - 1
        total[r] += options.min(0)
    path = np.zeros(rows, int)
    path[-1] = total[-1].argmin()
    for r in range(rows - 1, 0, -1):
        path[r - 1] = back[r, path[r]]
    return path


def wrap_columns(crop, n):
    """rows x (n + o) -> rows x n whose last column continues into its first.

    The first o columns take the o columns past n left of a least-difference seam,
    so the tile's right edge meets the pixels that followed it in the swatch."""
    o = crop.shape[1] - n
    after, start = crop[:, n:], crop[:, :o]
    path = seam(np.abs(after.astype(np.int32) - start).sum(2).astype(np.float64))
    tile = crop[:, :n].copy()
    take = np.arange(o)[None, :] < path[:, None]
    tile[:, :o][take] = after[take]
    return tile


def periodic(crop, n):
    """(n + o) x (n + o) swatch crop -> n x n tile that wraps on both axes."""
    rows = wrap_columns(crop, n)
    return wrap_columns(rows.transpose(1, 0, 2), n).transpose(1, 0, 2)


def swatch(sources, name, x, y, n, overlap, repeats=False, light=0):
    file, _, digest, _, _ = SWATCHES[name]
    data = (sources / name).read_bytes()
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError(f'{sources / name}: not the expected {file}')
    with Image.open(sources / name) as image:
        pixels = np.asarray(image.convert('RGB'))
    if light:
        # A photograph: divide out its broad light and shade (a blur of light radius) at constant mean.
        values = pixels.astype(np.float32)
        shade = gaussian_filter(values.mean(2), light, mode='reflect')
        pixels = np.clip(np.rint(values * (shade.mean() / shade)[:, :, None]), 0, 255).astype(np.uint8)
    span = n + overlap
    if repeats:
        # The swatch is itself a repeat: the crop may run over its edges.
        crop = pixels.take(range(y, y + span), 0, mode='wrap').take(range(x, x + span), 1, mode='wrap')
    else:
        crop = pixels[y:y + span, x:x + span]
    if crop.shape[:2] != (span, span):
        raise ValueError(f'{name}: crop outside the swatch')
    return Image.fromarray(periodic(crop, n)).resize((SIZE, SIZE), Image.Resampling.LANCZOS)


# id (the item; 3107-3110 were drawn studies, dropped 2026-11-26), slug, name, family, description,
# and the swatch: (name, x, y, n, overlap[, repeats[, light]])
ROWS = [
    (3099, 'lizard', 'Lizard', 'Lizard',
     'Brushed horizontal strokes of green and brown over a pale ground, from a 1950s French swatch.',
     ('lizard', 1280, 0, 1500, 200)),
    (3100, 'erdl-highland', 'ERDL Highland', 'ERDL',
     'Brown-dominant leaf pattern with black branches, from the U.S. ERDL highland swatch.',
     ('erdl', 704, 0, 1850, 260)),
    (3101, 'three-color-desert', 'Three-Color Desert', 'Desert Tricolor',
     'Broad khaki clouds and thin brown ridges on sand, from the U.S. desert uniform swatch.',
     ('dcu', 256, 640, 700, 100, True)),
    (3102, 'urban-brush', 'Urban Brush', 'All-Over Brush',
     'Sweeping dry-brush marks in plum, rose and cream, from a 2002 U.S. Army trial swatch.',
     ('brush', 0, 0, 740, 100)),
    (3103, 'centre-europe', 'Centre Europe', 'Centre Europe',
     'Wide horizontal shapes in green, brown, sand and black, from the French CCE swatch.',
     ('cce', 384, 0, 1091, 128, True)),
    (3104, 'all-terrain', 'All Terrain', 'Multi-Environment',
     'Layered gradients of sand, olive and brown with small bright flecks, from the French BME swatch.',
     ('bme', 320, 0, 2250, 300)),
    (3105, 'danube-leaf', 'Danube Leaf', 'Leaf Mosaic',
     'Interlocking leaf shapes in ochre, olive and brown with pale veins, from the Hungarian 2015M swatch.',
     ('hu2015', 0, 0, 960, 140)),
    (3106, 'brushstroke', 'Brushstroke', 'Brushstroke',
     'Diagonal green and brown dry-brush strokes on sand, from a Rhodesian brushstroke swatch.',
     ('rhodesian', 0, 0, 600, 90)),
    (3111, 'tropentarn', 'Tropentarn', 'Desert Fleck',
     'Brown, green and sand flecks on a pale khaki ground, from a German Tropentarn fabric photograph.',
     ('tropentarn', 576, 0, 1150, 150, False, 150)),
    (3112, 'balkan-dpm', 'Balkan DPM', 'Disruptive Pattern',
     'Interlocking brush shapes in brown, olive, grey, navy and cream, from the Bulgarian M2003 DPM.',
     ('bgdpm', 1792, 768, 1800, 240, False, 150)),
    (3113, 'balkan-splinter', 'Balkan Splinter', 'Splinter Rain',
     'Grey and plum splinters over a beige ground with rain marks, from the Bulgarian splinter pattern.',
     ('bgsplinter', 1792, 704, 1800, 240)),
    (3114, 'nfp-blue', 'NFP Blue', 'Mottle',
     'Fine navy and slate mottling, from the Dutch M19 NFP Blue pattern.',
     ('nfpblue', 0, 0, 740, 90)),
]

# Rows of initial/catalog.tsv whose tile is a swatch made here (it replaced a study under the same item).
INITIAL = [
    (2812, 'mm-14', ('mm14', 0, 192, 640, 96)),
]


def palette(image):
    quant = image.resize((128, 128), Image.Resampling.BOX).quantize(colors=5)
    colors = np.array(quant.getpalette(), dtype=np.uint8).reshape(-1, 3)
    return ' '.join('#' + bytes(colors[i]).hex().upper() for count, i in sorted(quant.getcolors(), reverse=True))


def build(sources, out, initial):
    out.mkdir(parents=True, exist_ok=True)
    rows = []
    for item, slug, name, family, description, how in ROWS:
        image = swatch(sources, *how)
        file, _, _, licence, author = SWATCHES[how[0]]
        url, label = COMMONS + file, f'Wikimedia Commons · {author} · {licence}'
        image.save(out / (slug + '.webp'), quality=94, method=4)
        rows.append((item, slug, name, family, description, url, label, palette(image)))
        print(slug, flush=True)
    for item, slug, how in INITIAL:
        image = swatch(sources, *how)
        image.save(initial / (slug + '.webp'), quality=94, method=4)
        print(f'{slug} (initial/, palette {palette(image)})', flush=True)
    with (out / 'catalog.tsv').open('w', encoding='utf-8', newline='') as stream:
        writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
        writer.writerow(('id', 'slug', 'name', 'family', 'description', 'source_url', 'source_label', 'palette'))
        writer.writerows(rows)


def fetch(target):
    target.mkdir(parents=True, exist_ok=True)
    for name, (_, url, digest, _, _) in SWATCHES.items():
        path = target / name
        while not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            request = urllib.request.Request(url, headers={'User-Agent': 'camo-atlas-fetch/1.0'})
            try:
                path.write_bytes(urllib.request.urlopen(request, timeout=300).read())
            except OSError as error:
                print(f'{name}: {error}; retrying in a minute', flush=True)
                time.sleep(60)
        print(name, digest, flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)
    get = commands.add_parser('fetch')
    get.add_argument('dir', type=Path)
    make = commands.add_parser('build')
    make.add_argument('--sources', required=True, type=Path)
    make.add_argument('--out', type=Path, default=HERE / 'classics')
    make.add_argument('--initial', type=Path, default=HERE / 'initial')
    args = parser.parse_args()
    if args.command == 'fetch':
        fetch(args.dir)
    else:
        build(args.sources, args.out, args.initial)


if __name__ == '__main__':
    main()

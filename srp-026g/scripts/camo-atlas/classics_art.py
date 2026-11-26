#!/usr/bin/env python3
"""Draw the tiles of classics/ (768 x 768 WebP): real camouflage patterns and studies of classic ones.

python3 classics_art.py fetch DIR                 download the swatches below into DIR
python3 classics_art.py build --sources DIR [--out DIR]

Swatch rows crop a square of a public-domain, CC0 or otherwise free swatch from Wikimedia
Commons (SWATCHES: the file, its licence and the exact bytes used, by sha256) and make it
periodic with a minimum-cost seam on the wrapped edges. Study rows are drawn from the seeds
in this file and read no file. classics/SOURCES.txt says where each one comes from.
Pillow, NumPy and SciPy; with NumPy 2.4.4, Pillow 13.0.0 and SciPy 1.17.1 it reproduces
classics/*.webp byte for byte. Catalog order is part of the game dictionary.
"""
import argparse
import csv
import hashlib
import math
import time
import urllib.request
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from scipy.ndimage import map_coordinates

HERE = Path(__file__).resolve().parent
SIZE = 768
COMMONS = 'https://commons.wikimedia.org/wiki/File:'

# name: (Commons file, download (the original or a standard thumbnail), its sha256, licence, author)
SWATCHES = {
    'lizard': ('French_Lizard_Pattern_Swatch_from_the_1950s.png',
               'https://upload.wikimedia.org/wikipedia/commons/thumb/c/c0/French_Lizard_Pattern_Swatch_from_the_1950s.png/3840px-French_Lizard_Pattern_Swatch_from_the_1950s.png',
               '509994ed27ced6b3a775cc415f0ee8c298809698f1e444cbd7b57c81eafba31d', 'CC0', 'VoltigeurFR'),
    'erdl': ('Brown-dominant_%22highland%22_ERDL_camouflage_pattern_swatch.svg',
             'https://upload.wikimedia.org/wikipedia/commons/thumb/a/a0/Brown-dominant_%22highland%22_ERDL_camouflage_pattern_swatch.svg/3840px-Brown-dominant_%22highland%22_ERDL_camouflage_pattern_swatch.svg.png',
             '81c7eb807ce57aa500241f26e2cefd4fe35163fbe8e1df4a111622e902a69fae', 'public domain (U.S. Government work)',
             'Illegitimate Barrister'),
    'dcu': ('DCU_camo_swatch.png',
            'https://upload.wikimedia.org/wikipedia/commons/8/82/DCU_camo_swatch.png',
            'f992a0dc395116e626d7ea214296ffd61870872e0ab17eec153055813caadf4c', 'public domain (U.S. Government work)',
            'Illegitimate Barrister'),
    'brush': ('Phase_I_All-Over_Brush_Urban_Camo_Close-Up_Swatch.png',
              'https://upload.wikimedia.org/wikipedia/commons/thumb/6/6c/Phase_I_All-Over_Brush_Urban_Camo_Close-Up_Swatch.png/1280px-Phase_I_All-Over_Brush_Urban_Camo_Close-Up_Swatch.png',
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


def swatch(sources, name, x, y, n, overlap, repeats=False):
    file, _, digest, _, _ = SWATCHES[name]
    data = (sources / name).read_bytes()
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError(f'{sources / name}: not the expected {file}')
    with Image.open(sources / name) as image:
        pixels = np.asarray(image.convert('RGB'))
    span = n + overlap
    if repeats:
        # The swatch is itself a repeat: the crop may run over its edges.
        crop = pixels.take(range(y, y + span), 0, mode='wrap').take(range(x, x + span), 1, mode='wrap')
    else:
        crop = pixels[y:y + span, x:x + span]
    if crop.shape[:2] != (span, span):
        raise ValueError(f'{name}: crop outside the swatch')
    return Image.fromarray(periodic(crop, n)).resize((SIZE, SIZE), Image.Resampling.LANCZOS)


# Studies: drawn at twice the tile size on a periodic plane and filtered down.
N = SIZE * 2
Y, X = np.mgrid[0:N, 0:N].astype(np.float32) / N


def field(rng, nx, ny, x=X, y=Y):
    values = rng.uniform(-1, 1, (ny, nx)).astype(np.float32)
    return map_coordinates(values, [y * ny, x * nx], order=3, mode='grid-wrap')


def fbm(rng, base, octaves, x=X, y=Y, sx=1, sy=1):
    out = np.zeros_like(X)
    for octave in range(octaves):
        scale = base * 2 ** octave
        out += .5 ** octave * field(rng, max(1, int(scale * sx)), max(1, int(scale * sy)), x, y)
    return out


def polygon(draw, points, ink):
    points = np.asarray(points, dtype=np.float64)
    for dx in (-N, 0, N):
        for dy in (-N, 0, N):
            moved = points + (dx, dy)
            if moved[:, 0].max() >= 0 and moved[:, 0].min() <= N and moved[:, 1].max() >= 0 and moved[:, 1].min() <= N:
                draw.polygon([tuple(p) for p in moved], fill=ink)


def brushstroke(rng):
    """Green, then brown dry-brush strokes on a diagonal over a sand ground."""
    image = Image.new('L', (N, N), 0)
    draw = ImageDraw.Draw(image)
    for ink, count in ((1, 16), (2, 13)):
        for _ in range(count):
            cx, cy = rng.uniform(0, N, 2)
            angle = math.radians(-32 + rng.normal(0, 9))
            length = rng.uniform(.30, .52) * N
            width = rng.uniform(.045, .085) * N
            ux, uy = math.cos(angle), math.sin(angle)
            bend = rng.normal(0, .05) * N
            wobble = rng.normal(0, 1, 6)
            t = np.linspace(-.5, .5, 40)
            profile = np.sin(np.pi * (t + .5)) ** .45
            left, right = [], []
            for i, ti in enumerate(t):
                px = cx + ux * ti * length - uy * bend * (ti ** 2 * 4 - 1)
                py = cy + uy * ti * length + ux * bend * (ti ** 2 * 4 - 1)
                w = width * profile[i] * (1 + .18 * math.sin(7 * ti + wobble[0]) + .1 * math.sin(17 * ti + wobble[1]))
                jl, jr = rng.normal(0, .05 * width, 2)
                left.append((px - uy * (w + jl), py + ux * (w + jl)))
                right.append((px + uy * (w + jr), py - ux * (w + jr)))
            polygon(draw, left + right[::-1], ink)
            for end in (-1, 1):
                for _ in range(rng.integers(5, 10)):
                    off = rng.uniform(-.8, .8) * width
                    s0, s1 = end * length * .42, end * length * rng.uniform(.5, .68)
                    th = rng.uniform(.04, .12) * width
                    a0 = (cx + ux * s0 - uy * off, cy + uy * s0 + ux * off)
                    off1 = off + rng.normal(0, .1 * width)
                    a1 = (cx + ux * s1 - uy * off1, cy + uy * s1 + ux * off1)
                    polygon(draw, [(a0[0] - uy * th, a0[1] + ux * th), a1, (a0[0] + uy * th, a0[1] - ux * th)], ink)
    return np.asarray(image)


def frogskin(rng):
    """Overlapping rounded spots in four inks, the largest first."""
    image = Image.new('L', (N, N), 0)
    draw = ImageDraw.Draw(image)
    for ink, count, low, high in ((1, 70, .028, .06), (2, 95, .018, .045), (3, 120, .010, .028), (4, 60, .008, .02)):
        for _ in range(count):
            cx, cy = rng.uniform(0, N, 2)
            r = rng.uniform(low, high) * N
            h = rng.normal(0, 1, (4, 2))
            a = np.linspace(0, 2 * np.pi, 40, endpoint=False)
            rr = r * (1 + .16 * np.sin(2 * a + h[0, 0]) + .1 * np.sin(3 * a + h[1, 0]) + .06 * np.sin(5 * a + h[2, 0]))
            stretch = rng.uniform(.8, 1.3)
            polygon(draw, np.stack([cx + rr * np.cos(a) * stretch, cy + rr * np.sin(a) / stretch], 1), ink)
    return np.asarray(image)


def alpenflage(rng):
    """Soft green and brown patches under a scatter of small red-brown dots."""
    a, b, c = fbm(rng, 4, 3), fbm(rng, 5, 3), fbm(rng, 6, 3)
    image = Image.fromarray(np.select([a > .32, b > .3, c > .28], [1, 2, 3], 0).astype(np.uint8))
    draw = ImageDraw.Draw(image)
    for _ in range(420):
        cx, cy = rng.uniform(0, N, 2)
        for _ in range(rng.integers(1, 5)):
            x, y = cx + rng.normal(0, .012 * N), cy + rng.normal(0, .012 * N)
            r = rng.uniform(.0035, .008) * N
            a = np.linspace(0, 2 * np.pi, 16, endpoint=False)
            rr = r * (1 + .1 * rng.normal(0, 1, 16))
            polygon(draw, np.stack([x + rr * np.cos(a), y + rr * np.sin(a)], 1), 4)
    return np.asarray(image)


def klmk(rng):
    """Stepped, perforated dark ribbons running across a light ground."""
    u, v = np.floor(X * 72) / 72, np.floor(Y * 120) / 120
    a = fbm(rng, 3, 4, u, v, sx=.7, sy=2.4)
    holes = fbm(rng, 14, 2, u, v, sx=.8, sy=1.4)
    ink = ((a > 0) & (holes < .42)).astype(np.uint8)
    ink[(a > -.12) & (a <= 0) & (holes > .2)] = 1
    return ink


def leiber(rng):
    """Green and red-brown patches, short black squiggles and pale flecks."""
    green, brown, squiggle, fleck = fbm(rng, 4, 3), fbm(rng, 5, 3), fbm(rng, 5, 4), fbm(rng, 40, 1)
    ink = np.zeros(X.shape, np.uint8)
    ink[green > .38] = 1
    ink[brown > .48] = 2
    ink[(np.abs(squiggle) < .07) & (fbm(rng, 9, 2) > -.1)] = 3
    ink[fleck > .78] = 4
    return ink


def study(draw, palette, seed):
    rng = np.random.default_rng(seed)
    inks = np.array([list(bytes.fromhex(h)) for h in palette.split()], dtype=np.float32)
    image = Image.fromarray(inks[draw(rng)].astype(np.uint8)).resize((SIZE, SIZE), Image.Resampling.LANCZOS)
    # A faint periodic print grain and weave, as on the other drawn tiles.
    y, x = np.mgrid[0:SIZE, 0:SIZE].astype(np.float32) / SIZE
    grain = map_coordinates(rng.uniform(-1, 1, (96, 96)).astype(np.float32), [y * 96, x * 96], order=1, mode='grid-wrap')
    weave = .9 * np.cos(2 * np.pi * (x * 256 + y * 128)) + .8 * np.cos(2 * np.pi * y * 256)
    pixels = np.asarray(image, dtype=np.float32) + (2 * grain + weave)[:, :, None]
    return Image.fromarray(np.clip(np.rint(pixels), 0, 255).astype(np.uint8))


# slug, name, family, description, how: ('swatch', name, x, y, n, overlap[, repeats]) or ('study', function, palette, seed)
ROWS = [
    ('lizard', 'Lizard', 'Lizard',
     'Brushed horizontal strokes of green and brown over a pale ground, from a 1950s French swatch.',
     ('swatch', 'lizard', 1280, 0, 1500, 200)),
    ('erdl-highland', 'ERDL Highland', 'ERDL',
     'Brown-dominant leaf pattern with black branches, from the U.S. ERDL highland swatch.',
     ('swatch', 'erdl', 704, 0, 1850, 260)),
    ('three-color-desert', 'Three-Color Desert', 'Desert Tricolor',
     'Broad khaki clouds and thin brown ridges on sand, from the U.S. desert uniform swatch.',
     ('swatch', 'dcu', 256, 640, 700, 100, True)),
    ('urban-brush', 'Urban Brush', 'All-Over Brush',
     'Sweeping dry-brush marks in plum, rose and cream, from a 2002 U.S. Army trial swatch.',
     ('swatch', 'brush', 0, 0, 740, 100)),
    ('centre-europe', 'Centre Europe', 'Centre Europe',
     'Wide horizontal shapes in green, brown, sand and black, from the French CCE swatch.',
     ('swatch', 'cce', 384, 0, 1091, 128, True)),
    ('all-terrain', 'All Terrain', 'Multi-Environment',
     'Layered gradients of sand, olive and brown with small bright flecks, from the French BME swatch.',
     ('swatch', 'bme', 320, 0, 2250, 300)),
    ('danube-leaf', 'Danube Leaf', 'Leaf Mosaic',
     'Interlocking leaf shapes in ochre, olive and brown with pale veins, from the Hungarian 2015M swatch.',
     ('swatch', 'hu2015', 0, 0, 960, 140)),
    ('brushstroke', 'Brushstroke', 'Brushstroke',
     'Diagonal green and brown dry-brush strokes on sand. An original study of the Rhodesian pattern.',
     ('study', brushstroke, 'C5A57A 4A5A30 6B4527', 11)),
    ('frogskin', 'Frogskin', 'Frogskin',
     'Rounded spots in olive, brown, forest and black on pale green. An original study of the 1940s spot patterns.',
     ('study', frogskin, 'A9AD84 6E7445 7B5A3B 3E4A2C 2A2A22', 5)),
    ('alpenflage', 'Alpenflage', 'Alpine Dot',
     'Soft green and brown patches under a scatter of red-brown dots. An original study of the Swiss TAZ 83.',
     ('study', alpenflage, 'C7BE9F 8E9A6C 4F5C3B 7A5B3D 9C4A33', 83)),
    ('klmk', 'KLMK', 'Stepped Ribbon',
     'Stepped, perforated dark green ribbons across a light ground. An original study of the Soviet KLMK.',
     ('study', klmk, 'AFB28F 4B5A37', 68)),
    ('leibermuster', 'Leibermuster', 'Squiggle',
     'Green and red-brown patches, black squiggles and pale flecks on buff. An original study of the 1945 German pattern.',
     ('study', leiber, 'C2B994 667346 8A5A3E 24231F E3DDC8', 45)),
]


def palette(image):
    quant = image.resize((128, 128), Image.Resampling.BOX).quantize(colors=5)
    colors = np.array(quant.getpalette(), dtype=np.uint8).reshape(-1, 3)
    return ' '.join('#' + bytes(colors[i]).hex().upper() for count, i in sorted(quant.getcolors(), reverse=True))


def build(sources, out):
    out.mkdir(parents=True, exist_ok=True)
    rows = []
    for slug, name, family, description, how in ROWS:
        if how[0] == 'swatch':
            image = swatch(sources, *how[1:])
            file, _, _, licence, author = SWATCHES[how[1]]
            url, label = COMMONS + file, f'Wikimedia Commons · {author} · {licence}'
        else:
            _, draw, inks, seed = how
            image = study(draw, inks, seed)
            url, label = '', 'Original study · classics_art.py'
        image.save(out / (slug + '.webp'), quality=94, method=4)
        rows.append((slug, name, family, description, url, label, palette(image)))
        print(slug, flush=True)
    with (out / 'catalog.tsv').open('w', encoding='utf-8', newline='') as stream:
        writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
        writer.writerow(('slug', 'name', 'family', 'description', 'source_url', 'source_label', 'palette'))
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
    args = parser.parse_args()
    if args.command == 'fetch':
        fetch(args.dir)
    else:
        build(args.sources, args.out)


if __name__ == '__main__':
    main()

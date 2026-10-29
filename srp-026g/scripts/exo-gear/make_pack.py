#!/usr/bin/env python3
"""make_pack.py: the content pack that paints over the STK2 exoskeleton boots' and gloves' black areas
(2026-10-29; ../../docs/notes/exo-gear.md).

usage: python3 -I make_pack.py --live <folder> --out <packs folder>/exo-gear-<version> [--previews <folder>]

--live      the released files at their game paths (`srp-game-data get --db <stock> --patch <each listed
            patch>... --out <folder> <path>...`): every path of LIVE, checked by its sha256.
--out       the pack's folder (a new one): pack.json, files/ and icons/, for `vostok-port pack` and the
            manifest's `pack` row (release/content.tsv).
--previews  optional: before/after renders of the four icons and the four textures' edited areas.

The Duty and Freedom exoskeleton sets (packs duty-exo@20, freedom-exo@4, from STK2) paint the boots'
upper shafts and side patch and the gloves' palms and wrists solid black: STK2 hid them under the
trousers and sleeves, 0.26g's kits show them, as black where a sock or the glove's leather should be.
The pack paints those black pixels over with the fabric beside them (each reflected across the edge
of the black area, so the weave goes on), in the four textures both sets use (the meshes are shared);
the soles' undersides stay black. The textures keep their format (DXT1, 2048x2048, 10 mips): only the
4x4 blocks with painted pixels are encoded again, each mip from the painted top level. The lobby icons
of the four pieces are rendered again from the painted textures, over their own cells
(`replaces_sha256`: the porter checks the cell holds the icon it replaces).
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import sys
import zlib

import numpy as np
from PIL import Image
from scipy import ndimage

PACK_ID = 'exo-gear'
VERSION = '1'
MODELS = 'models/ported/duty_026'
BOOT = MODELS + '/duty_03_boot.skinned_model/render/legs_s/converted_model'
HAND = MODELS + '/duty_03_hand.skinned_model/render/torso_s/converted_model'
MOVIE = 'flash_movies/lobby_menu.swf'
SYMBOL = 'LobbyMenu_EquipmentIcons'
# The released files: sha256 (the lobby movie changes every release: its cells are what the
# pack pins, by `replaces_sha256`).
LIVE = {
    BOOT: '2250bbc752ec086d74c33fd8e9522bddd758a66ceceea1805d088c0db9704d54',
    HAND: 'c7abc7d877fcacc71d719bcbaa9a92e22422a66b6edd12a1526ad597c50800ea',
    'textures/ported/duty_026/legs_diffuse.dds': 'b360aa2b13ddf6c9ec5c7a059dfa9d9034330a0b71e59dcf529e322e14d75003',
    'textures/ported/duty_026/torso_diffuse.dds': '3fdfcccb9a1542c798248b02d796c6e87ac78394939b98330c025510c7822db3',
    'textures/ported/freedom_026/legs_diffuse.dds': 'e33cd80ebb717d8491c0c9f7b179de4e6804fe9af3f2da18da4c86ea3d911f51',
    'textures/ported/freedom_026/torso_diffuse.dds': '25e6ad30d4b2320c4cdcfb5829818dde34146806218a25fa880421571983eeb0',
    MOVIE: None,
}
# (piece, set, mesh, texture, its lobby cell [row, col] of SYMBOL)
PIECES = [
    ('boots', 'duty', BOOT, 'textures/ported/duty_026/legs_diffuse.dds', (7, 6)),
    ('boots', 'freedom', BOOT, 'textures/ported/freedom_026/legs_diffuse.dds', (9, 6)),
    ('gloves', 'duty', HAND, 'textures/ported/duty_026/torso_diffuse.dds', (6, 6)),
    ('gloves', 'freedom', HAND, 'textures/ported/freedom_026/torso_diffuse.dds', (8, 6)),
]
BLACK = 0.04           # flat black: darker than this (0..1),
GRAIN = 0.01           # with no grain: the local deviation (9x9) under this,
AREA = 400             # in areas of at least this many pixels
CELL = (171, 83)       # the equipment atlas's cells; rows 91 pixels apart
ROW_PITCH = 91


def sha256(data):
    return hashlib.sha256(data).hexdigest()


# --- the meshes (converted_model: 48-byte vertices, then u16 triangles) ---

def mesh(data):
    count = struct.unpack_from('<I', data, 8)[0]
    raw = np.frombuffer(data[12:12 + 48 * count], np.uint8).reshape(count, 48)
    floats = raw.copy().view('<f4').reshape(count, 12)
    faces = np.frombuffer(data, dtype='<u2', offset=24 + 48 * count).reshape(-1, 3).astype(np.int64)
    normals = raw[:, 28:31][:, ::-1].astype(float) / 127.5 - 1
    return floats[:, :3].astype(float), floats[:, 10:12].astype(float), normals, faces


# --- DXT1 (BC1) ---

def dds_levels(data):
    """The header and each mip level's block bytes of a DXT1 DDS."""
    height, width = struct.unpack_from('<II', data, 12)
    mips = struct.unpack_from('<I', data, 28)[0]
    assert data[84:88] == b'DXT1', 'not DXT1'
    levels, at = [], 128
    for level in range(mips):
        w, h = max(1, width >> level), max(1, height >> level)
        size = max(1, (w + 3) // 4) * max(1, (h + 3) // 4) * 8
        levels.append((w, h, at, size))
        at += size
    # The ported textures end with a byte past their last mip; it stays as it is.
    assert at <= len(data) <= at + 16, 'DDS size does not match its mips'
    return levels


def unpack565(c):
    r = ((c >> 11) & 31) * 255 // 31
    g = ((c >> 5) & 63) * 255 // 63
    b = (c & 31) * 255 // 31
    return np.stack([r, g, b], -1).astype(np.float64)


def decode(blocks, w, h):
    """DXT1 blocks to an (h, w, 3) float image (0..255)."""
    bw, bh = max(1, (w + 3) // 4), max(1, (h + 3) // 4)
    b = np.frombuffer(blocks, np.uint8).reshape(bh * bw, 8)
    c0 = b[:, 0].astype(np.int64) | b[:, 1].astype(np.int64) << 8
    c1 = b[:, 2].astype(np.int64) | b[:, 3].astype(np.int64) << 8
    p0, p1 = unpack565(c0), unpack565(c1)
    four = (c0 > c1)[:, None]
    p2 = np.where(four, (2 * p0 + p1) / 3, (p0 + p1) / 2)
    p3 = np.where(four, (p0 + 2 * p1) / 3, 0)
    palette = np.stack([p0, p1, p2, p3], 1)
    bits = b[:, 4:8].astype(np.int64)
    idx = np.stack([(bits[:, row] >> (2 * col)) & 3 for row in range(4) for col in range(4)], 1)
    pixels = np.take_along_axis(palette, idx[:, :, None].repeat(3, 2), 1)
    image = pixels.reshape(bh, bw, 4, 4, 3).transpose(0, 2, 1, 3, 4).reshape(bh * 4, bw * 4, 3)
    return image[:h, :w]


def pack565(rgb):
    r = np.clip(np.round(rgb[..., 0] * 31 / 255), 0, 31).astype(np.int64)
    g = np.clip(np.round(rgb[..., 1] * 63 / 255), 0, 63).astype(np.int64)
    b = np.clip(np.round(rgb[..., 2] * 31 / 255), 0, 31).astype(np.int64)
    return r << 11 | g << 5 | b


def encode(pixels):
    """(n, 16, 3) float blocks to DXT1 bytes (four-colour mode: endpoints at the colour extremes
    along the block's principal axis)."""
    n = len(pixels)
    mean = pixels.mean(1, keepdims=True)
    centred = pixels - mean
    cov = np.einsum('nki,nkj->nij', centred, centred)
    axis = np.linalg.eigh(cov)[1][:, :, -1]
    t = np.einsum('nki,ni->nk', centred, axis)
    lo = mean[:, 0] + axis * t.min(1)[:, None]
    hi = mean[:, 0] + axis * t.max(1)[:, None]
    c0, c1 = pack565(hi), pack565(lo)
    swap = c0 < c1
    c0, c1 = np.where(swap, c1, c0), np.where(swap, c0, c1)
    p0, p1 = unpack565(c0), unpack565(c1)
    palette = np.stack([p0, p1, (2 * p0 + p1) / 3, (p0 + 2 * p1) / 3], 1)
    distance = ((pixels[:, :, None, :] - palette[:, None, :, :]) ** 2).sum(-1)
    idx = distance.argmin(2)
    idx = np.where((c0 == c1)[:, None], 0, idx)
    bits = np.zeros(n, np.int64)
    for k in range(16):
        bits |= idx[:, k].astype(np.int64) << (2 * k)
    out = np.zeros((n, 8), np.uint8)
    out[:, 0], out[:, 1] = c0 & 255, c0 >> 8
    out[:, 2], out[:, 3] = c1 & 255, c1 >> 8
    for k in range(4):
        out[:, 4 + k] = (bits >> (8 * k)) & 255
    return out.tobytes()


# --- the painting ---

def downward(points, faces):
    """The faces facing down: the soles' undersides, which stay as they are."""
    normal = np.cross(points[faces][:, 1] - points[faces][:, 0], points[faces][:, 2] - points[faces][:, 0])
    normal /= np.linalg.norm(normal, axis=1)[:, None] + 1e-12
    return normal[:, 1] < -0.5


def raster(uv, faces, w, h):
    """The pixels the faces cover in texture space."""
    from PIL import ImageDraw
    canvas = Image.new('L', (w, h), 0)
    draw = ImageDraw.Draw(canvas)
    for face in faces:
        draw.polygon([tuple((uv[v] % 1) * [w, h]) for v in face], fill=255)
    return np.asarray(canvas) > 0


def paint(image, points, uv, faces):
    """The image with its flat black areas (under the mesh's faces, not its soles) painted over
    by the fabric beside them; and the mask of the painted pixels. Flat black: darker than BLACK,
    no grain (local deviation under GRAIN), in areas of at least AREA pixels; dark leather with
    a weave stays."""
    h, w = image.shape[:2]
    island = ndimage.binary_dilation(raster(uv, faces, w, h), iterations=2)
    soles = ndimage.binary_dilation(raster(uv, faces[downward(points, faces)], w, h), iterations=4)
    lum = image.mean(-1) / 255
    mean = ndimage.uniform_filter(lum, 9)
    deviation = np.sqrt(np.maximum(ndimage.uniform_filter(lum ** 2, 9) - mean ** 2, 0))
    flat = (lum < BLACK) & (deviation < GRAIN) & island & ~soles
    labels, count = ndimage.label(flat)
    sizes = ndimage.sum(flat, labels, range(1, count + 1))
    mask = np.isin(labels, 1 + np.nonzero(sizes >= AREA)[0])
    # Two pixels more: the black's soft edge goes too.
    mask = ndimage.binary_dilation(mask, iterations=2) & island & ~soles
    valid = island & ~mask & ~flat & ~soles
    _, (iy, ix) = ndimage.distance_transform_edt(~valid, return_indices=True)
    yy, xx = np.nonzero(mask)
    ny, nx = iy[yy, xx], ix[yy, xx]
    # Reflected across the edge: the fabric's grain goes on; the nearest pixel where that falls off.
    ry, rx = np.clip(2 * ny - yy, 0, h - 1), np.clip(2 * nx - xx, 0, w - 1)
    ok = valid[ry, rx]
    out = image.copy()
    out[yy, xx] = np.where(ok[:, None], image[ry, rx], image[ny, nx])
    return out, mask


def repaint(dds, points, uv, faces):
    """The DDS with its black areas painted over: only the blocks with painted pixels change."""
    levels = dds_levels(dds)
    w0, h0, at0, size0 = levels[0]
    top = decode(dds[at0:at0 + size0], w0, h0)
    painted, mask = paint(top, points, uv, faces)
    out = bytearray(dds)
    for level, (w, h, at, size) in enumerate(levels):
        factor = 1 << level
        if factor > 1:
            image = painted.reshape(h, factor, w, factor, 3).mean((1, 3))
            changed = mask.reshape(h, factor, w, factor).any((1, 3))
        else:
            image, changed = painted, mask
        bw, bh = max(1, (w + 3) // 4), max(1, (h + 3) // 4)
        padded = np.zeros((bh * 4, bw * 4, 3))
        padded[:h, :w] = image
        blocks = padded.reshape(bh, 4, bw, 4, 3).transpose(0, 2, 1, 3, 4).reshape(bh * bw, 16, 3)
        flags = np.zeros((bh * 4, bw * 4), bool)
        flags[:h, :w] = changed
        touched = np.nonzero(flags.reshape(bh, 4, bw, 4).any((1, 3)).reshape(-1))[0]
        if len(touched):
            encoded = np.frombuffer(encode(blocks[touched]), np.uint8).reshape(-1, 8)
            for k, block in enumerate(touched):
                out[at + 8 * block:at + 8 * block + 8] = encoded[k].tobytes()
    return bytes(out), painted, mask


# --- the icons ---

def render(points, uv, normals, faces, texture, yaw, pitch, roll, size=(684, 332)):
    """Orthographic, depth-tested, lit like the porter's Duty icons (resource-porter
    scripts/render-duty-026-icons.py), fitted to `size`."""
    yaw, pitch, roll = np.radians([yaw, pitch, roll])
    toward = np.array([np.sin(yaw) * np.cos(pitch), np.sin(pitch), np.cos(yaw) * np.cos(pitch)])
    right = np.cross([0, 1, 0], toward)
    right /= np.linalg.norm(right)
    up = np.cross(toward, right)
    right, up = right * np.cos(roll) + up * np.sin(roll), up * np.cos(roll) - right * np.sin(roll)
    basis = np.stack([right, up, toward], axis=1)
    pp = points @ basis
    used = np.unique(faces)
    lo, hi = pp[used].min(0), pp[used].max(0)
    scale = min((size[0] - 32) / (hi[0] - lo[0]), (size[1] - 16) / (hi[1] - lo[1]))
    centre = (lo + hi) / 2
    screen = (pp[:, :2] - centre[:2]) * [scale, -scale] + [size[0] / 2, size[1] / 2]
    light = right * -0.45 + up * 0.75 + toward * 0.8
    light /= np.linalg.norm(light)
    tex = texture / 255
    out = np.zeros((size[1], size[0], 4), np.uint8)
    depth = np.full((size[1], size[0]), -np.inf)
    for face in faces:
        q, z = screen[face], pp[face, 2]
        x0, y0 = np.maximum(np.floor(q.min(0)).astype(int), 0)
        x1, y1 = np.minimum(np.ceil(q.max(0)).astype(int), [size[0] - 1, size[1] - 1])
        if x0 > x1 or y0 > y1:
            continue
        den = (q[1, 1] - q[2, 1]) * (q[0, 0] - q[2, 0]) + (q[2, 0] - q[1, 0]) * (q[0, 1] - q[2, 1])
        if abs(den) < 1e-8:
            continue
        yy, xx = np.mgrid[y0:y1 + 1, x0:x1 + 1]
        xx, yy = xx + .5, yy + .5
        a = ((q[1, 1] - q[2, 1]) * (xx - q[2, 0]) + (q[2, 0] - q[1, 0]) * (yy - q[2, 1])) / den
        b = ((q[2, 1] - q[0, 1]) * (xx - q[2, 0]) + (q[0, 0] - q[2, 0]) * (yy - q[2, 1])) / den
        c = 1 - a - b
        dz = a * z[0] + b * z[1] + c * z[2]
        visible = (a >= 0) & (b >= 0) & (c >= 0) & (dz > depth[y0:y1 + 1, x0:x1 + 1])
        if not visible.any():
            continue
        weights = np.stack([a[visible], b[visible], c[visible]], axis=1)
        tuv = weights @ uv[face]
        tx = np.clip((tuv[:, 0] % 1) * (tex.shape[1] - 1), 0, tex.shape[1] - 1).astype(int)
        ty = np.clip((tuv[:, 1] % 1) * (tex.shape[0] - 1), 0, tex.shape[0] - 1).astype(int)
        normal = weights @ normals[face]
        normal /= np.maximum(np.linalg.norm(normal, axis=1)[:, None], 1e-8)
        illumination = .38 + .75 * np.maximum(normal @ light, 0)
        rgb = np.clip(tex[ty, tx] * illumination[:, None] * [1.05, 1.01, .94], 0, 1)
        patch = out[y0:y1 + 1, x0:x1 + 1]
        patch[visible, :3] = (rgb * 255).astype(np.uint8)
        patch[visible, 3] = 255
        depth[y0:y1 + 1, x0:x1 + 1][visible] = dz[visible]
    return Image.fromarray(out)


def icon(piece, points, uv, normals, faces, texture):
    """The piece's icon (171x83 RGBA), posed as the icons it replaces: the boots as a pair
    brought together, seen from the front right and a little above; one glove (the left of the
    mesh) from its back, the fingers down to the left."""
    points = points.copy()
    if piece == 'boots':
        points[:, 0] -= np.sign(points[:, 0]) * .065
        picture = render(points, uv, normals, faces, texture, 45, 15, 0)
    else:
        one = faces[points[faces][:, :, 0].mean(1) < 0]
        picture = render(points, uv, normals, one, texture, -90, 6, 45)
    return picture.resize(CELL, Image.Resampling.LANCZOS)


# --- the movie's atlas ---

def atlas(movie):
    """The equipment atlas's premultiplied ARGB pixels of the lobby movie."""
    data = movie
    if data[:3] == b'CWS':
        data = data[:8] + zlib.decompress(data[8:])
    at = 8 + (5 + 4 * (data[8] >> 3) + 7) // 8 + 4
    tags = []
    while at < len(data):
        header = struct.unpack_from('<H', data, at)[0]
        at += 2
        size = header & 63
        if size == 63:
            size = struct.unpack_from('<I', data, at)[0]
            at += 4
        tags.append((header >> 6, data[at:at + size]))
        at += size
    char = None
    for kind, payload in tags:
        if kind != 76:
            continue
        at = 2
        for _ in range(struct.unpack_from('<H', payload)[0]):
            ident = struct.unpack_from('<H', payload, at)[0]
            end = payload.index(0, at + 2)
            if payload[at + 2:end] == SYMBOL.encode():
                char = ident
            at = end + 1
    for kind, payload in tags:
        if kind == 36 and struct.unpack_from('<H', payload)[0] == char:
            _, fmt, w, h = struct.unpack_from('<HBHH', payload)
            assert fmt == 5
            return np.frombuffer(zlib.decompress(payload[7:]), np.uint8).reshape(h, w, 4)
    raise SystemExit(f'{MOVIE}: no {SYMBOL} bitmap')


def cell(argb, row, col):
    x, y = (col - 1) * CELL[0], (row - 1) * ROW_PITCH
    return argb[y:y + CELL[1], x:x + CELL[0]]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--live', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--previews', type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: exists; the pack goes in a new folder')
    live = {}
    for path, short in LIVE.items():
        data = (args.live / path).read_bytes()
        if short and sha256(data) != short:
            sys.exit(f'--live: {path} is not the released file (sha256 {short})')
        live[path] = data
    argb = atlas(live[MOVIE])
    files, operations, icons, painted = {}, [], [], {}
    for piece, kit, model, texture, (row, col) in PIECES:
        points, uv, normals, faces = mesh(live[model])
        if texture not in painted:
            dds, image, mask = repaint(live[texture], points, uv, faces)
            painted[texture] = image
            files['files/' + texture] = dds
            operations.append({'op': 'replace_file', 'path': texture, 'base_sha256': sha256(live[texture])})
            print(f'{texture}: {int(mask.sum())} pixels painted over')
            if args.previews:
                args.previews.mkdir(parents=True, exist_ok=True)
                ys, xs = np.nonzero(raster(uv, faces, *image.shape[1::-1]))
                box = (xs.min(), ys.min(), xs.max(), ys.max())
                before = decode(live[texture][128:128 + dds_levels(live[texture])[0][3]], *image.shape[1::-1])
                Image.fromarray(np.concatenate([before[box[1]:box[3], box[0]:box[2]], image[box[1]:box[3], box[0]:box[2]]], 1)
                                .astype(np.uint8)).save(args.previews / (texture.replace('/', '_') + '.png'))
        picture = icon(piece, points, uv, normals, faces, painted[texture])
        source = f'icons/{SYMBOL}/r{row:02}c{col:02}.png'
        out = io.BytesIO()
        picture.save(out, format='PNG', optimize=False)
        files[source] = out.getvalue()
        icons.append({'row': row, 'col': col, 'source': source,
                      'replaces_sha256': sha256(cell(argb, row, col).tobytes())})
        if args.previews:
            old = cell(argb, row, col).astype(np.float64)
            alpha = old[:, :, 0:1]
            rgb = np.where(alpha > 0, old[:, :, 1:] * 255 / np.maximum(alpha, 1), 0)
            before = Image.fromarray(np.concatenate([rgb, alpha], 2).astype(np.uint8), 'RGBA')
            sheet = Image.new('RGBA', (CELL[0] * 6, CELL[1] * 3), (233, 230, 220, 255))
            sheet.alpha_composite(before.resize((CELL[0] * 3, CELL[1] * 3)), (0, 0))
            sheet.alpha_composite(picture.resize((CELL[0] * 3, CELL[1] * 3)), (CELL[0] * 3, 0))
            sheet.convert('RGB').save(args.previews / f'{kit}-{piece}.png')
    operations.append({'op': 'atlas_icons_add', 'swf': MOVIE, 'symbol': SYMBOL, 'icons': icons})
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'the Duty and Freedom exoskeleton boots and gloves without their black areas (the boots\' '
                       'upper shafts and side patch, the gloves\' palms and wrists, painted over with the fabric '
                       'beside them in both sets\' textures) and their lobby icons rendered again',
        'operations': operations,
        'notes': ['made by srp-026g/scripts/exo-gear/make_pack.py from the released files'],
    }
    for path, data in files.items():
        target = args.out / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(painted)} textures and {len(icons)} icons, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()

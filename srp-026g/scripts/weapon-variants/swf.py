"""An icon atlas of the lobby movie: its DefineBitsLossless2 bitmap (premultiplied ARGB) as RGBA."""
import struct
import zlib

import numpy as np


def atlas(movie, symbol):
    """The bitmap the movie exports as `symbol`, as premultiplied RGBA [h, w, 4]."""
    if movie[:3] == b'CWS':
        body = zlib.decompress(movie[8:])
    elif movie[:3] == b'FWS':
        body = movie[8:]
    else:
        raise ValueError('not a SWF')
    at = (5 + 4 * (body[0] >> 3) + 7) // 8 + 4
    images, symbols = {}, {}
    while at < len(body):
        tag, = struct.unpack_from('<H', body, at)
        at += 2
        size = tag & 63
        if size == 63:
            size, = struct.unpack_from('<I', body, at)
            at += 4
        chunk = body[at:at + size]
        at += size
        if tag >> 6 == 36:
            ident, fmt, width, height = struct.unpack_from('<HBHH', chunk)
            if fmt == 5:
                argb = np.frombuffer(zlib.decompress(chunk[7:]), np.uint8).reshape(height, width, 4)
                images[ident] = argb[..., [1, 2, 3, 0]]
        elif tag >> 6 == 76:
            count, = struct.unpack_from('<H', chunk)
            pos = 2
            for _ in range(count):
                ident, = struct.unpack_from('<H', chunk, pos)
                end = chunk.index(0, pos + 2)
                symbols[chunk[pos + 2:end].decode()] = ident
                pos = end + 1
    return images[symbols[symbol]]


def occupied(bitmap, cell=(171, 83), pitch=91):
    """{(row, col): any pixel drawn} for every cell of the bitmap (1-based)."""
    h, w = bitmap.shape[:2]
    return {(r, c): bool(bitmap[(r - 1) * pitch:(r - 1) * pitch + cell[1], (c - 1) * cell[0]:c * cell[0], 3].any())
            for r in range(1, (h - cell[1]) // pitch + 2) for c in range(1, w // cell[0] + 1)}

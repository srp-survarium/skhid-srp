"""SWF containers and ABC blocks, as much as the movie build needs: tags, DoABC, SymbolClass, class names,
and bitmaps from PNG and JPEG files."""
import hashlib
import os
from pathlib import Path
import struct
import tempfile
import zlib

DO_ABC, DO_ABC_OLD, SYMBOL_CLASS, END = 82, 72, 76, 0
DEFINE_BITS_LOSSLESS2 = 36
DEFINE_BITS_JPEG2 = 21
DEFINE_BITS_JPEG3 = 35
DEFINE_SPRITE, SHOW_FRAME, PLACE_OBJECT2 = 39, 1, 26
# The tags that define a character, its id their first field.
DEFINES = {2, 6, 7, 10, 11, 14, 20, 21, 22, 32, 33, 34, 35, 36, 37, 39, 46, 48, 60, 75, 83, 84, 87, 90, 91}


def uncompressed(data):
    """The movie's bytes as an uncompressed (FWS) SWF."""
    if data[:3] == b'FWS':
        return data
    if data[:3] == b'CWS':
        return b'FWS' + data[3:8] + zlib.decompress(data[8:])
    raise ValueError('Not a SWF (or an LZMA one, which is not supported)')


def compressed(data, like):
    """`data` (FWS) compressed as `like` is."""
    return b'CWS' + data[3:8] + zlib.compress(data[8:], 9) if like[:3] == b'CWS' else data


def split(data):
    """(header, [(kind, raw tag bytes)]) of a SWF."""
    data = uncompressed(data)
    pos = 8 + (5 + 4 * (data[8] >> 3) + 7) // 8 + 4
    header, tags = data[:pos], []
    while pos < len(data):
        start = pos
        word, = struct.unpack_from('<H', data, pos)
        pos += 2
        kind, size = word >> 6, word & 63
        if size == 63:
            size, = struct.unpack_from('<I', data, pos)
            pos += 4
        pos += size
        if pos > len(data):
            raise ValueError('Truncated SWF tag')
        tags.append((kind, data[start:pos]))
        if kind == END:
            break
    if not tags or tags[-1][0] != END:
        raise ValueError('Missing SWF End')
    return header, tags


def join(header, tags):
    body = header[8:] + b''.join(raw for _, raw in tags)
    return b'FWS' + header[3:4] + struct.pack('<I', len(body) + 8) + body


def payload(raw):
    word, = struct.unpack_from('<H', raw)
    return raw[6 if word & 63 == 63 else 2:]


def tag(kind, body):
    return struct.pack('<HI', kind << 6 | 63, len(body)) + body


def do_abc(raw):
    """(flags, name, abc bytes) of a DoABC tag."""
    body = payload(raw)
    flags, = struct.unpack_from('<I', body)
    end = body.index(b'\0', 4)
    return flags, body[4:end].decode('utf-8'), body[end + 1:]


def symbol_classes(tags):
    """Every (character id, class name) the SymbolClass tags bind."""
    result = []
    for kind, raw in tags:
        if kind != SYMBOL_CLASS:
            continue
        body = payload(raw)
        count, = struct.unpack_from('<H', body)
        pos = 2
        for _ in range(count):
            cid, = struct.unpack_from('<H', body, pos)
            end = body.index(b'\0', pos + 2)
            result.append((cid, body[pos + 2:end].decode('utf-8')))
            pos = end + 1
    return result


def character_ids(tags):
    """Every character id the movie's tags define."""
    return {struct.unpack_from('<H', payload(raw))[0] for kind, raw in tags if kind in DEFINES}


def tags_of(body):
    """[(kind, raw tag bytes)] of a tag list (a DefineSprite's control tags), up to its End."""
    pos, out = 0, []
    while pos < len(body):
        start = pos
        word, = struct.unpack_from('<H', body, pos)
        pos += 2
        kind, size = word >> 6, word & 63
        if size == 63:
            size, = struct.unpack_from('<I', body, pos)
            pos += 4
        pos += size
        if pos > len(body):
            raise ValueError('Truncated sprite tag')
        out.append((kind, body[start:pos]))
        if kind == END:
            break
    return out


def placed(raw):
    """(depth, character id or None) of a PlaceObject2 tag."""
    body = payload(raw)
    flags, depth = body[0], struct.unpack_from('<H', body, 1)[0]
    return depth, (struct.unpack_from('<H', body, 3)[0] if flags & 0x02 else None)


def with_character(raw, cid):
    """A PlaceObject2 tag that places `cid` (it must place a character already)."""
    word, = struct.unpack_from('<H', raw)
    at = (6 if word & 63 == 63 else 2) + 3
    return raw[:at] + struct.pack('<H', cid) + raw[at + 2:]


def symbol_class(bindings):
    """A SymbolClass tag binding each (character id, class name)."""
    body = struct.pack('<H', len(bindings))
    for cid, name in bindings:
        body += struct.pack('<H', cid) + name.encode('utf-8') + b'\0'
    return tag(SYMBOL_CLASS, body)


def kept(what, data, make):
    """make() (bytes: a pure function of `data` and this file's code), made once: kept in the
    checks' cache (bitmaps/<what>-<SHA-256 of this file and data>, its own SHA-256 before it).
    A PNG's pixels are decoded and premultiplied a byte at a time: seconds a movie, every build."""
    cache = os.environ.get('SKHID_CHECKS_CACHE') or Path(os.environ.get('XDG_CACHE_HOME') or Path.home() / '.cache') / 'skhid-checks'
    path = Path(cache) / 'bitmaps' / f'{what}-{hashlib.sha256(SOURCE + data).hexdigest()}'
    try:
        found = path.read_bytes()
        if hashlib.sha256(found[32:]).digest() == found[:32]:
            return found[32:]
    except OSError:
        pass
    made = make()
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as partial:
            partial.write(hashlib.sha256(made).digest() + made)
        os.replace(partial.name, path)
    except OSError:
        pass
    return made


SOURCE = hashlib.sha256(Path(__file__).read_bytes()).digest()


def png(data):
    """(width, height, RGBA bytes) of an 8-bit RGBA or RGB, non-interlaced PNG."""
    made = kept('png', data, lambda: (lambda width, height, pixels: struct.pack('<II', width, height)
                                      + zlib.compress(pixels, 1))(*png_pixels(data)))
    return (*struct.unpack_from('<II', made), zlib.decompress(made[8:]))


def png_pixels(data):
    """png(), decoded here."""
    if data[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError('Not a PNG')
    pos, header, idat = 8, None, b''
    while pos < len(data):
        size, kind = struct.unpack_from('>I4s', data, pos)
        chunk = data[pos + 8:pos + 8 + size]
        if zlib.crc32(data[pos + 4:pos + 8 + size]) != struct.unpack_from('>I', data, pos + 8 + size)[0]:
            raise ValueError(f'PNG chunk {kind!r}: bad CRC')
        pos += 12 + size
        if kind == b'IHDR':
            header = struct.unpack('>IIBBBBB', chunk)
        elif kind == b'IDAT':
            idat += chunk
        elif kind == b'IEND':
            break
    if header is None:
        raise ValueError('PNG without IHDR')
    width, height, depth, colour, _, _, interlace = header
    if depth != 8 or colour not in (2, 6) or interlace:
        raise ValueError('Only 8-bit RGB or RGBA, non-interlaced PNG files are supported')
    channels = 4 if colour == 6 else 3
    stride = width * channels
    raw = zlib.decompress(idat)
    if len(raw) != height * (stride + 1):
        raise ValueError('PNG data of the wrong size')
    rows, previous = [], bytearray(stride)
    for y in range(height):
        kind, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for x in range(stride):
            left = line[x - channels] if x >= channels else 0
            up = previous[x]
            corner = previous[x - channels] if x >= channels else 0
            if kind == 1:
                line[x] = (line[x] + left) & 0xff
            elif kind == 2:
                line[x] = (line[x] + up) & 0xff
            elif kind == 3:
                line[x] = (line[x] + (left + up) // 2) & 0xff
            elif kind == 4:
                p = left + up - corner
                pa, pb, pc = abs(p - left), abs(p - up), abs(p - corner)
                line[x] = (line[x] + (left if pa <= pb and pa <= pc else up if pb <= pc else corner)) & 0xff
            elif kind:
                raise ValueError(f'PNG filter {kind}')
        rows.append(bytes(line))
        previous = line
    pixels = b''.join(rows)
    if channels == 3:
        pixels = b''.join(pixels[i:i + 3] + b'\xff' for i in range(0, len(pixels), 3))
    return width, height, pixels


def bitmap(cid, width, height, rgba):
    """A DefineBitsLossless2 tag (32-bit ARGB, premultiplied, as Flash keeps it) of RGBA pixels."""
    def packed():
        argb = bytearray()
        for i in range(0, len(rgba), 4):
            r, g, b, a = rgba[i:i + 4]
            argb += bytes((a, r * a // 255, g * a // 255, b * a // 255))
        return zlib.compress(bytes(argb), 9)
    return tag(DEFINE_BITS_LOSSLESS2, struct.pack('<HBHH', cid, 5, width, height) + kept('argb', rgba, packed))


def jpeg(cid, data):
    """A DefineBitsJPEG2 tag (the JPEG whole, its tables in it) of a baseline JPEG file, and its
    (width, height). The game's own renders are JPEG (DefineBits); a progressive one Flash does not
    read."""
    if data[:2] != b'\xff\xd8' or data[-2:] != b'\xff\xd9':
        raise ValueError('not a JPEG file (SOI ... EOI)')
    pos = 2
    while pos + 4 <= len(data):
        if data[pos] != 0xFF:
            raise ValueError(f'a JPEG marker expected at {pos}')
        marker = data[pos + 1]
        length, = struct.unpack_from('>H', data, pos + 2)
        if marker == 0xC0:
            height, width = struct.unpack_from('>HH', data, pos + 5)
            return tag(DEFINE_BITS_JPEG2, struct.pack('<H', cid) + data), (width, height)
        if 0xC1 <= marker <= 0xCF and marker not in (0xC4, 0xC8, 0xCC):
            raise ValueError('a JPEG must be baseline (SOF0)')
        pos += 2 + length
    raise ValueError('no frame (SOF0) in the JPEG')


def jpeg_alpha(cid, data, alpha):
    """A DefineBitsJPEG3 tag: a baseline JPEG file (jpeg()) and its alpha, one byte a pixel (zlib), for a
    picture with transparent parts far smaller than a lossless one. Its colour is premultiplied by the
    alpha (black where it is transparent), as Flash keeps a bitmap."""
    _, (width, height) = jpeg(cid, data)
    if len(alpha) != width * height:
        raise ValueError(f'the alpha is {len(alpha)} bytes, the JPEG {width} x {height}')
    return tag(DEFINE_BITS_JPEG3, struct.pack('<HI', cid, len(data)) + data + zlib.compress(alpha, 9))


FILE_ATTRIBUTES, DEFINE_SHAPE = 69, 2
# FileAttributes' flag: the movie is ActionScript 3's (it has no code; a movie AS3 loads is one).
AS3 = 0x08
# A bitmap fill drawn clipped (not repeated) and smoothed.
CLIPPED_SMOOTHED = 0x41


class _Bits:
    """SWF bit fields, most significant bit first."""
    def __init__(self):
        self.bits = []

    def unsigned(self, value, n):
        self.bits += [(value >> (n - 1 - i)) & 1 for i in range(n)]
        return self

    def signed(self, value, n):
        return self.unsigned(value & ((1 << n) - 1), n)

    def bytes(self):
        bits = self.bits + [0] * (-len(self.bits) % 8)
        return bytes(int(''.join(map(str, bits[i:i + 8])), 2) for i in range(0, len(bits), 8))


def _width(*values):
    """The bits a signed field needs for every value."""
    return max(max(abs(v), 1).bit_length() + 1 for v in values)


def _rect(xmax, ymax):
    n = _width(xmax, ymax)
    return _Bits().unsigned(n, 5).signed(0, n).signed(xmax, n).signed(0, n).signed(ymax, n).bytes()


def picture_movie(define, width, height, version):
    """A movie of its own that shows one picture and does nothing else: `define` (a DefineBits tag of
    character 1, e.g. jpeg_alpha(1, ...)) filling a width x height pixel rectangle, smoothed, on its one
    frame; no code (an AS3 movie, for Loader.loadBytes of an AS3 movie). Compressed."""
    w, h = width * 20, height * 20
    scale = 20 << 16
    n = _width(scale)
    matrix = _Bits().unsigned(1, 1).unsigned(n, 5).signed(scale, n).signed(scale, n).unsigned(0, 1).unsigned(0, 5)
    edges = _Bits().unsigned(1, 4).unsigned(0, 4)
    # a style change: fill style 1 on the right, moved to (0, 0)
    edges.unsigned(0, 1).unsigned(0, 1).unsigned(0, 1).unsigned(1, 1).unsigned(0, 1).unsigned(1, 1)
    edges.unsigned(1, 5).signed(0, 1).signed(0, 1).unsigned(1, 1)
    n = _width(w, h)
    for dx, dy in ((w, 0), (0, h), (-w, 0), (0, -h)):
        edges.unsigned(1, 1).unsigned(1, 1).unsigned(n - 2, 4).unsigned(1, 1).signed(dx, n).signed(dy, n)
    edges.unsigned(0, 6)
    shape = struct.pack('<H', 2) + _rect(w, h) + bytes([1, CLIPPED_SMOOTHED]) + struct.pack('<H', 1) \
        + matrix.bytes() + bytes([0]) + edges.bytes()
    short = lambda kind, body: struct.pack('<H', kind << 6 | len(body)) + body
    tags = [short(FILE_ATTRIBUTES, struct.pack('<I', AS3)), define, tag(DEFINE_SHAPE, shape),
            short(PLACE_OBJECT2, bytes([0x02]) + struct.pack('<HH', 1, 2)), short(SHOW_FRAME, b''), short(END, b'')]
    body = _rect(w, h) + struct.pack('<HH', 30 << 8, 1) + b''.join(tags)
    return b'CWS' + bytes([version]) + struct.pack('<I', len(body) + 8) + zlib.compress(body, 9)


class _Reader:
    def __init__(self, data):
        self.data, self.pos = data, 0

    def u8(self):
        self.pos += 1
        return self.data[self.pos - 1]

    def u16(self):
        self.pos += 2
        return struct.unpack_from('<H', self.data, self.pos - 2)[0]

    def u30(self):
        result = shift = 0
        for _ in range(5):
            byte = self.u8()
            result |= (byte & 0x7f) << shift
            shift += 7
            if not byte & 0x80:
                break
        return result

    def skip(self, n):
        self.pos += n
        if self.pos > len(self.data):
            raise ValueError('Truncated ABC')


def abc_definitions(data):
    """Parse a whole ABC block; its classes (qualified names) and script count.

    Every record is read, so a block that parses here is well formed as far as
    its layout goes (it says nothing of the code's meaning)."""
    r = _Reader(data)
    r.u16(), r.u16()
    for size in (None, None):              # int, uint pools
        for _ in range(max(r.u30() - 1, 0)):
            r.u30()
    r.skip(8 * max(r.u30() - 1, 0))        # double pool
    strings = ['']
    for _ in range(max(r.u30() - 1, 0)):
        n = r.u30()
        strings.append(r.data[r.pos:r.pos + n].decode('utf-8', 'replace'))
        r.skip(n)
    namespaces = [(0, 0)]
    for _ in range(max(r.u30() - 1, 0)):
        namespaces.append((r.u8(), r.u30()))
    for _ in range(max(r.u30() - 1, 0)):   # namespace sets
        for _ in range(r.u30()):
            r.u30()
    multinames = [None]
    for _ in range(max(r.u30() - 1, 0)):
        kind = r.u8()
        if kind in (0x07, 0x0d):           # QName(A)
            multinames.append((r.u30(), r.u30()))
        elif kind in (0x0f, 0x10):         # RTQName(A)
            multinames.append((None, r.u30()))
        elif kind in (0x11, 0x12):         # RTQNameL(A)
            multinames.append(None)
        elif kind in (0x09, 0x0e):         # Multiname(A)
            name, _ = r.u30(), r.u30()
            multinames.append((None, name))
        elif kind in (0x1b, 0x1c):         # MultinameL(A)
            r.u30()
            multinames.append(None)
        elif kind == 0x1d:                 # TypeName
            r.u30()
            for _ in range(r.u30()):
                r.u30()
            multinames.append(None)
        else:
            raise ValueError(f'Unknown multiname kind {kind:#x}')
    methods = r.u30()
    for _ in range(methods):
        params = r.u30()
        r.u30()
        for _ in range(params):
            r.u30()
        r.u30()
        flags = r.u8()
        if flags & 0x08:
            for _ in range(r.u30()):
                r.u30(), r.u8()
        if flags & 0x80:
            for _ in range(params):
                r.u30()
    for _ in range(r.u30()):               # metadata
        r.u30()
        items = r.u30()
        for _ in range(2 * items):
            r.u30()

    def traits():
        for _ in range(r.u30()):
            r.u30()
            kind = r.u8()
            if kind & 0x0f in (0, 6):
                r.u30(), r.u30()
                if r.u30():
                    r.u8()
            elif kind & 0x0f in (1, 2, 3, 4, 5):
                r.u30(), r.u30()
            else:
                raise ValueError(f'Unknown trait kind {kind & 0x0f}')
            if kind & 0x40:
                for _ in range(r.u30()):
                    r.u30()

    def qualified(index):
        ns, name = multinames[index]
        package = strings[namespaces[ns][1]] if ns is not None else ''
        return f'{package}.{strings[name]}' if package else strings[name]

    classes = []
    count = r.u30()
    for _ in range(count):
        name = r.u30()
        r.u30()
        flags = r.u8()
        if flags & 0x08:
            r.u30()
        for _ in range(r.u30()):
            r.u30()
        r.u30()
        traits()
        classes.append(qualified(name))
    for _ in range(count):
        r.u30()
        traits()
    scripts = r.u30()
    for _ in range(scripts):
        r.u30()
        traits()
    for _ in range(r.u30()):               # method bodies
        r.u30(), r.u30(), r.u30(), r.u30(), r.u30()
        r.skip(r.u30())
        for _ in range(r.u30()):
            r.u30(), r.u30(), r.u30(), r.u30(), r.u30()
        traits()
    if r.pos != len(data):
        raise ValueError('Bytes after the ABC block')
    return classes, scripts

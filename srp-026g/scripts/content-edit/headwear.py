"""Render-only headwear bone. The animation skeleton and clips stay byte-for-byte.

The combined model reads inverse binds in file order (0x15A000). Slot 69
duplicates Head's bind; only the selected surfaces use it. The client fills
its pose with Head while worn and the cosmetic body's pose after death.
"""
import math
import struct


def bind_records(data):
    count, = struct.unpack_from('<H', data)
    records, at = [], 2
    for _ in range(count):
        end = data.index(0, at)
        name = data[at:end].decode('ascii')
        frame = data[end + 1:end + 37]
        if len(frame) != 36 or not all(math.isfinite(x) for x in struct.unpack('<9f', frame)):
            raise ValueError('invalid bind frame')
        records.append((name, frame))
        at = end + 37
    if at != len(data) or len({n for n, _ in records}) != count:
        raise ValueError('invalid bind records')
    return records


def append_headwear_bind(data):
    records = bind_records(data)
    if len(records) != 69 or records[44][0] != 'Head':
        raise ValueError('expected the native 69-bone render bind, Head at 44')
    result = struct.pack('<H', 70) + data[2:] + b'SrpHeadwear\0' + records[44][1]
    assert bind_records(result) == records + [('SrpHeadwear', records[44][1])]
    return result


# Bones a surface may be skinned to and still become one rigid piece: Neck, Head.
HEAD_BONES = (21, 44)


def rigid_headwear(data, expected_vertices, allow_bones=()):
    """Bind every vertex to the headwear bone. A surface also skinned to other
    native bones is refused unless its edit names them (`allow_bones`): a piece
    with a collar or a neck guard then flies off whole, the guard with it."""
    allowed = set(HEAD_BONES)
    for bone in allow_bones:
        if not isinstance(bone, int) or not 0 <= bone < 69 or bone in allowed:
            raise ValueError(f'allow_bones: {bone!r} is not another native bone')
        allowed.add(bone)
    chunk, size, count = struct.unpack_from('<III', data)
    if chunk != 3 or count != expected_vertices or count == 0 or (size - 4) % count:
        raise ValueError('unexpected headwear vertex chunk')
    stride = (size - 4) // count
    if stride not in (36, 40, 44, 48):
        raise ValueError('unknown skin layout')
    index_at = 8 + size
    kind, index_size, indices = struct.unpack_from('<III', data, index_at)
    if kind != 4 or index_size != 4 + indices * 2 or len(data) != index_at + 8 + index_size:
        raise ValueError('invalid index chunk')
    if any(i >= count for (i,) in struct.iter_unpack('<H', data[index_at + 12:])):
        raise ValueError('vertex index outside mesh')
    result = bytearray(data)
    changed = set()
    for i in range(count):
        at = 12 + i * stride
        stored = (stride - 36) // 4
        weights = list(struct.unpack_from('<' + 'f' * stored, data, at + 16))
        weights.append(1.0 - sum(weights))
        if any(not math.isfinite(w) or not -1e-5 <= w <= 1.00001 for w in weights):
            raise ValueError('invalid skin weights')
        # The Biker's three blended neck vertices are deliberately rigidified;
        # refuse other body parts unless named, so a hood never enters by mistake.
        extra = {data[at + 12 + j] for j, w in enumerate(weights) if w > 1e-6} - allowed
        if extra:
            raise ValueError(f'headwear has influences beyond Head/Neck: bones {sorted(extra)}')
        result[at + 12:at + 16] = bytes([69] * 4)
        changed.update(range(at + 12, at + 16))
        if stored:
            struct.pack_into('<' + 'f' * stored, result, at + 16, 1.0, *([0.0] * (stored - 1)))
            changed.update(range(at + 16, at + 16 + 4 * stored))
    # Positions, normals, UVs and topology must stay exact, including padding.
    assert all(a == b or i in changed for i, (a, b) in enumerate(zip(data, result)))
    return bytes(result)

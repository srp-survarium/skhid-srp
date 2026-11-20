#!/usr/bin/env python3
"""Use existing atlas mips that fit the native 0.26g streaming pools."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def native_atlas(raw, spec):
    if sha(raw) != spec['sha256']:
        raise ValueError('source DDS differs from reviewed input')
    if raw[:4] != b'DDS ' or struct.unpack_from('<I', raw, 4)[0] != 124:
        raise ValueError('expected legacy DDS')
    height, width = struct.unpack_from('<II', raw, 12)
    mips = struct.unpack_from('<I', raw, 28)[0]
    fourcc = raw[84:88]
    if (width, height, mips) != (4096, 4096, 11) or fourcc not in (b'DXT1', b'DXT5'):
        raise ValueError('unexpected atlas dimensions, mip chain or compression')
    block_bytes = 8 if fourcc == b'DXT1' else 16
    sizes = [max(1, (width >> i) // 4) ** 2 * block_bytes for i in range(mips)]
    if len(raw) != 128 + sum(sizes) + 1 or raw[-1] not in (0, 1):
        raise ValueError('unexpected payload or Vostok sRGB trailer')
    out = bytearray(raw[:128] + raw[128 + sizes[0]:])
    struct.pack_into('<II', out, 12, 2048, 2048)
    # Vostok's linear size covers the full payload, including its sRGB byte.
    struct.pack_into('<I', out, 20, len(out) - 128)
    struct.pack_into('<I', out, 28, mips - 1)
    assert out[128:] == raw[128 + sizes[0]:]
    return bytes(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--porter', type=Path, required=True)
    parser.add_argument('--patch22', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    specs = json.loads((HERE / 'textures.json').read_text())
    made, operations = {}, []
    with tempfile.TemporaryDirectory(prefix='radar-textures-') as temp:
        for number, spec in enumerate(specs):
            source = Path(temp) / str(number)
            subprocess.run([str(args.porter), 'extract', '--db', str(args.patch22),
                            '--path', spec['file'], '--output', str(source)],
                           check=True, capture_output=True)
            raw = source.read_bytes()
            made[spec['file']] = native_atlas(raw, spec)
            operations.append({'op': 'replace_file', 'path': spec['file'], 'base_sha256': sha(raw)})
    manifest = {'format_version': 1, 'id': 'radar-v011e-textures', 'version': '1',
                'target_build': '0.26g', 'operations': operations,
                'notes': ['Use the existing 2048 mip of the shared vehicle atlas for native streaming.',
                          'No recompression; retain every remaining mip and the sRGB trailer.']}
    args.out.mkdir(parents=True, exist_ok=False)
    for path, raw in made.items():
        dest = args.out / 'files' / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(raw)
    raw = (json.dumps(manifest, indent=2) + '\n').encode()
    (args.out / 'pack.json').write_bytes(raw)
    print(f"radar-v011e-textures@1: {len(made)} textures; pack.json {sha(raw)}")


if __name__ == '__main__':
    main()

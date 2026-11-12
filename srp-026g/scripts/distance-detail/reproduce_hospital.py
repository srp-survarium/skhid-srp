#!/usr/bin/env python3
"""Measure native LOD decisions at the reported hospital camera poses.

This executes original x86 code in Unicorn; it is not a rendered reproduction.
See reproduction.txt for the evidence and limitations. No game is launched.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'client-patches'))
import check_fov_lod_026g as native
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_XMM0, UC_X86_REG_EAX, UC_X86_REG_EDI, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_ESP

MODEL = 'ported/v011e/level/03/hospital'
VIEWS = [
    {'eye': [-55.869, 11.447, -15.181], 'yaw': 358.5, 'pitch': -3.7},
    {'eye': [-55.871, 11.448, -15.186], 'yaw': 3.9, 'pitch': -3.5},
]


class Archive:
    def __init__(self, porter, database, cache):
        self.porter = str(porter)
        self.database = str(database)
        # Separate caches even when input archives share a basename.
        key = hashlib.sha256(str(database.resolve()).encode()).hexdigest()[:16]
        self.cache = cache / key

    def command(self, operation, path, *extra):
        return subprocess.check_output(
            [self.porter, operation, '--db', self.database, '--path', path, *map(str, extra)],
            text=True,
        )

    def leaves(self, path):
        return {key: json.loads(value.split(':', 1)[1])
                for key, value in (line.split('\t', 1) for line in
                                   self.command('dump-db-config', path, '--leaves').splitlines())}

    def read(self, path):
        output = self.cache / path
        if not output.exists():
            output.parent.mkdir(parents=True, exist_ok=True)
            self.command('extract', path, '--output', output)
        return output.read_bytes()


def mesh(blob):
    kind, size = struct.unpack_from('<II', blob)
    if kind != 3:
        raise ValueError('expected converted-model vertex chunk')
    end = 8 + size
    declaration_end = next((i for i in range(8, min(end, 256), 8)
                            if blob[i:i + 8] == bytes([255, 0, 0, 0, 17, 0, 0, 0])), None)
    at = declaration_end + 8 if declaration_end is not None else 8
    count, = struct.unpack_from('<I', blob, at)
    start = at + 4
    stride = (end - start) // count
    if start + stride * count != end:
        raise ValueError('unexpected vertex stride')
    positions = [struct.unpack_from('<3f', blob, start + i * stride) for i in range(count)]
    index_count, = struct.unpack_from('<I', blob, end + 8)
    return positions, index_count // 3


def view_projection(eye, yaw, pitch, projection):
    yaw, pitch = map(math.radians, (yaw, pitch))
    z = (math.sin(yaw) * math.cos(pitch), math.sin(pitch), math.cos(yaw) * math.cos(pitch))
    x = (math.cos(yaw), 0, -math.sin(yaw))
    up = (-math.sin(yaw) * math.sin(pitch), math.cos(pitch), -math.cos(yaw) * math.sin(pitch))
    view = [x[0], up[0], z[0], 0, x[1], up[1], z[1], 0, x[2], up[2], z[2], 0,
            -sum(a*b for a, b in zip(x, eye)), -sum(a*b for a, b in zip(up, eye)),
            -sum(a*b for a, b in zip(z, eye)), 1]
    return [sum(view[i*4+k] * projection[k*4+j] for k in range(4))
            for i in range(4) for j in range(4)]


def texture_selection(emu, eye, sphere, factor):
    # Observed ABI at 0x1c8640 and its call at 0x1c22cc. cdecl cleanup.
    # The texture dimensions/texture scalar/texel factor/bias below are fixtures,
    # not captured runtime descriptors. Eye and authored sphere come from evidence.
    sphere_at = native.WORK + 0x7000
    dimensions, output = sphere_at + 0x100, sphere_at + 0x200
    emu.mem_write(sphere_at, struct.pack('<4f', *sphere))
    emu.mem_write(dimensions, struct.pack('<2f', 1024, 1024))
    emu.mem_write(native.EYE, struct.pack('<3f', *eye))
    factor_bits, = struct.unpack('<I', struct.pack('<f', factor))
    args = (native.EYE, 2560, 1440, factor_bits, 0x3f800000, output + 4, 0)
    esp = native.STACK + 0x8000
    emu.mem_write(esp, struct.pack('<8I', native.RETURN, *args))
    for register, value in ((UC_X86_REG_ESP, esp), (UC_X86_REG_EAX, dimensions),
                            (UC_X86_REG_EDX, sphere_at), (UC_X86_REG_EDI, output)):
        emu.reg_write(register, value)
    emu.emu_start(0x1c8640, native.RETURN, count=10000)
    if emu.reg_read(UC_X86_REG_ESP) != esp + 4:
        raise AssertionError('texture calculator did not preserve the cdecl stack')
    return {'mips': emu.reg_read(UC_X86_REG_EAX),
            'projected_factor': struct.unpack('<f', emu.mem_read(output, 4))[0],
            'squared_distance': struct.unpack('<f', emu.mem_read(output + 4, 4))[0]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', type=Path, required=True, help='original 0.26g executable')
    parser.add_argument('--patched-exe', type=Path, help='optional build to compare adaptive and forced detail')
    parser.add_argument('--porter', type=Path, required=True, help='vostok-port executable')
    parser.add_argument('--resources', type=Path, required=True, help='archive containing hospital models/materials')
    parser.add_argument('--project-db', type=Path, required=True, help='archive containing the map client_project')
    parser.add_argument('--project', default='projects/v011e_level_03/day_2/client_project')
    parser.add_argument('--cache', type=Path, required=True, help='empty or matching-input extraction cache')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    resources = Archive(args.porter, args.resources, args.cache)
    project = Archive(args.porter, args.project_db, args.cache).leaves(args.project)
    placements = [key.rsplit('.', 1)[0] for key, value in project.items()
                  if key.endswith('.lib_name') and value == MODEL]
    if len(placements) != 1:
        raise ValueError(f'expected one hospital placement, found {placements}')
    placement = {name: project[placements[0] + '.' + name] for name in ('position', 'rotation', 'scale')}
    if placement != {'position': [0, 0, 0], 'rotation': [0, 0, 0], 'scale': [1, 1, 1]}:
        raise ValueError('this fixture supports the shipped identity hospital placement only')

    prefix = 'models/' + MODEL + '.model'
    settings = resources.leaves(prefix + '/settings')
    surface_names = [value for key, value in settings.items() if key.startswith('lod_hierrarchy.LOD0.surfaces[')]
    counts = [sum(key.startswith(f'lod_hierrarchy.LOD{i}.surfaces[') for key in settings) for i in range(3)]
    if settings['lod_switching.type'] != 1 or settings['lod_switching.default_params'] is not True:
        raise ValueError('unexpected hospital LOD parameters')
    model_properties = resources.leaves(prefix + '/render/export_properties')
    model_bounds = model_properties['bounding_box.min'] + model_properties['bounding_box.max']
    lod_mesh_counts = []
    for lod in range(2):
        vertices = triangles = 0
        for key, name in settings.items():
            if not key.startswith(f'lod_hierrarchy.LOD{lod}.surfaces['):
                continue
            positions, num_triangles = mesh(resources.read(prefix + '/render/' + name + '/converted_model'))
            vertices += len(positions)
            triangles += num_triangles
            props = resources.leaves(prefix + '/render/' + name + '/export_properties')
            for d in range(3):
                model_bounds[d] = min(model_bounds[d], props['bounding_box.min'][d])
                model_bounds[d + 3] = max(model_bounds[d + 3], props['bounding_box.max'][d])
        lod_mesh_counts.append({'lod': lod, 'surfaces': counts[lod], 'vertices': vertices, 'triangles': triangles})
    surfaces = []
    for name in surface_names:
        positions, triangles = mesh(resources.read(prefix + '/render/' + name + '/converted_model'))
        properties = resources.leaves(prefix + '/render/' + name + '/export_properties')
        material = settings['material_settings.' + name + '.material_name']
        material_values = resources.leaves('material_instances/' + material + '.material')
        flags = {key: value for key, value in material_values.items()
                 if re.search(r'\.use_(nmap|parallax|detail_nmap|tditail_diffuse)\.value$', key)}
        surfaces.append({'name': name, 'vertices': len(positions), 'triangles': triangles,
                         'min': [min(p[d] for p in positions) for d in range(3)],
                         'max': [max(p[d] for p in positions) for d in range(3)],
                         'sphere': properties['bounding_sphere.center'] + [properties['bounding_sphere.radius']],
                         'material': material, 'material_flags': flags})

    original = args.exe.read_bytes()
    emu = native.image(original)
    projection = native.projection(emu, 65, 1, width=2560, height=1440)
    options = native.WORK + 0xD000
    emu.mem_write(0x2260498, struct.pack('<I', options))
    emu.mem_write(options + 0x131, b'\1')
    # Existing helper initializes the native model and descriptor object layout.
    native.select_lod(emu, 10)
    emu.mem_write(native.MODEL + 0x14C, struct.pack('<16f', *[float(i % 5 == 0) for i in range(16)]))
    emu.mem_write(native.ORIGINAL + 0x108, struct.pack('<6f', *model_bounds))
    emu.mem_write(native.LODS, bytes([*counts, 0]) + bytes(12) + struct.pack('<I3fI', 1, 0., 0., 0., 1))
    default_thresholds = struct.unpack('<3f', emu.mem_read(0xA7A814, 12))
    model_results = []
    projected_sizes = []
    def observe_size(uc, address, size, data):
        bits = uc.reg_read(UC_X86_REG_XMM0) & 0xFFFFFFFF
        projected_sizes.append(struct.unpack('<f', struct.pack('<I', bits))[0])
    emu.hook_add(UC_HOOK_CODE, observe_size, begin=0x16F2AC, end=0x16F2AC)
    for camera in VIEWS:
        emu.mem_write(native.MATRIX, struct.pack('<16f', *view_projection(**camera, projection=projection)))
        emu.mem_write(native.EYE, struct.pack('<3f', *camera['eye']))
        model_results.append(native.call(emu, native.SELECT_LOD, [native.MODEL, native.MATRIX],
                                         {UC_X86_REG_EAX: native.EYE}) & 255)
        for surface in surfaces:
            emu.mem_write(native.SURFACE + 0x20, struct.pack('<6f', *surface['min'], *surface['max']))
            emu.mem_write(native.SURFACE + 0x38, struct.pack('<I', native.MATERIAL))
            emu.mem_write(native.MATERIAL + 0x48, bytes(4))
            native.call(emu, native.SCREEN_FACTOR, [native.MATRIX],
                        {UC_X86_REG_ESI: native.SURFACE, UC_X86_REG_EDI: native.EYE})
            factor, = struct.unpack('<f', emu.mem_read(native.SURFACE + 0x58, 4))
            shader_lods = []
            for quality in (0, 1):
                emu.mem_write(options + 0xE8, struct.pack('<I', quality))
                shader_lods.append(native.call(emu, 0x158D20, [native.SURFACE], {}) & 255)
            surface.setdefault('views', []).append({'screen_factor': factor, 'shader_lods_quality_0_1': shader_lods})
    factors = (.001, .01, .1, 1., 10., 100.)
    for surface in surfaces:
        surface['texture_factor_sweep'] = [
            {'texel_factor': factor, 'views': [texture_selection(emu, camera['eye'], surface['sphere'], factor)
                                              for camera in VIEWS]}
            for factor in factors]
    patched_results = None
    if args.patched_exe:
        patched = native.image(args.patched_exe.read_bytes())
        # Copy only fixture memory, then let the patched projection initialize its FOV state.
        patched.mem_write(native.WORK, bytes(emu.mem_read(native.WORK, 0x10000)))
        patched.mem_write(0x2260498, struct.pack('<I', options))
        patched_projection = native.projection(patched, 65, 1, width=2560, height=1440)
        patched_results = {}
        for adaptive in (True, False):
            patched.mem_write(options + 0x132, bytes([adaptive]))
            lods = []
            for camera in VIEWS:
                patched.mem_write(native.MATRIX, struct.pack('<16f', *view_projection(**camera, projection=patched_projection)))
                patched.mem_write(native.EYE, struct.pack('<3f', *camera['eye']))
                lods.append(native.call(patched, native.SELECT_LOD, [native.MODEL, native.MATRIX], {UC_X86_REG_EAX: native.EYE}) & 255)
            patched_results['adaptive' if adaptive else 'forced'] = lods
        if patched_results['adaptive'] != model_results or patched_results['forced'] != [0, 0]:
            raise AssertionError(f'unexpected patched model choices: {patched_results}')
    result = {'kind': 'original native function measurements, not a rendered reproduction',
              'exe_sha256': hashlib.sha256(original).hexdigest(),
              'resources': str(args.resources), 'project_db': str(args.project_db),
              'project': args.project, 'placement': placements[0], 'transform': placement,
              'camera_assumptions': {'fov': 65, 'resolution': [2560, 1440], 'views': VIEWS},
              'default_screen_lod_thresholds': default_thresholds, 'model_bounds': model_bounds, 'lod_mesh_counts': lod_mesh_counts, 'model_projected_sizes': projected_sizes,
              'lod_surface_counts': counts, 'model_lods': model_results, 'patched_model_lods': patched_results,
              'surfaces': surfaces,
              'model_transition': model_results[0] != model_results[1],
              'shader_transitions': sum(s['views'][0]['shader_lods_quality_0_1'] != s['views'][1]['shader_lods_quality_0_1'] for s in surfaces),
              'texture_fixture_transitions': sum(t['views'][0]['mips'] != t['views'][1]['mips'] for s in surfaces for t in s['texture_factor_sweep'])}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(f"hospital: {len(surfaces)} surfaces; model LODs {model_results}; patched {patched_results}; shader transitions {result['shader_transitions']}; texture fixture transitions {result['texture_fixture_transitions']}; output {args.output}")


if __name__ == '__main__':
    main()

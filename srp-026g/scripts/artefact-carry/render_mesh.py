#!/usr/bin/env python3
"""Offline shaded carry mesh preview; requires numpy, Pillow and vostok-port.

Uses stock vertex weights/inverse bind matrices and the manifest's pose clips.
This is a geometry/framing study with an explicit reference eye, not a capture
of the engine's camera, lighting, material or locomotion animation.
"""
import argparse
import math
from pathlib import Path
import struct
import subprocess
import json
import sys
import numpy as np
from PIL import Image, ImageDraw
from preview import content, pose, rotation

HERE = Path(__file__).resolve().parent
CHAR = 'models/character/human/black_market/'
ITEM = 'models/item/artefacts/'


def matrix(r, p):
    result = np.eye(4)
    result[:3, :3], result[3, :3] = r, p
    return result


def bind_pose(data):
    count, = struct.unpack_from('<H', data)
    offset, result = 2, {}
    for _ in range(count):
        end = data.index(0, offset)
        name = data[offset:end].decode()
        v = struct.unpack_from('<9f', data, end + 1)
        result[name] = matrix(np.diag(v[6:]) @ rotation(v[3:6]), v[:3])
        offset = end + 37
    assert offset == len(data)
    return result


def mesh(data, frames=None):
    kind, size, count = struct.unpack_from('<III', data)
    stride = (size - 4) // count
    assert kind == 3 and stride in (28, 32, 48)
    vertices = np.array([struct.unpack_from('<3f', data, 12 + i * stride) for i in range(count)])
    if frames is not None:
        assert stride == 48
        indices = np.array([struct.unpack_from('<4B', data, 24 + i * stride) for i in range(count)])
        weights = np.array([struct.unpack_from('<3f', data, 28 + i * stride) for i in range(count)])
        weights = np.c_[weights, 1 - weights.sum(axis=1)]
        vertices = np.einsum('ni,nijk,nj->nk', weights, frames[indices], np.c_[vertices, np.ones(count)])[:, :3]
    kind, index_size, count = struct.unpack_from('<III', data, 8 + size)
    assert kind == 4 and index_size == 4 + 2 * count and count % 3 == 0
    triangles = np.array(struct.unpack_from('<' + str(count) + 'H', data, 20 + size)).reshape(-1, 3)
    return vertices, triangles


def native_pose(exe, world, names, parents, eye, now):
    """Run the installed pose hooks on the full stock rig, without the game."""
    sys.path.insert(0, str(HERE.parents[1] / 'client-patches'))
    from check_artefact_carry_026g import Run, COMMAND
    g = Run(exe.read_bytes())
    p = g.players[0]
    g.input(p, 0, COMMAND)
    g.tick(p, now)
    instance, _ = g.renderer()
    count = len(names) - 1
    assert names[0] == 'object_movement'
    skeleton = g.block(0x110 + 28 * len(names))
    g.word(skeleton + 0x108, len(names))
    hit = {'Root', 'Hip', 'Spin', 'Spin_1', 'Neck', 'Head', 'LeftArm', 'LeftForeArm', 'LeftHand',
           'RightArm', 'RightForeArm', 'RightHand', 'LeftUpLeg', 'LeftLeg', 'LeftFoot', 'RightUpLeg', 'RightLeg', 'RightFoot'}
    for name in list(hit):
        while name in parents and parents[name] is not None:
            name = parents[name]
            hit.add(name)
    for i, name in enumerate(names):
        text = g.block(len(name) + 1)
        g.emu.mem_write(text, name.encode() + b'\0')
        g.word(skeleton + 0x110 + i * 28, text)
        parent = names.index(parents[name]) if parents[name] else 0
        g.word(skeleton + 0x114 + i * 28, skeleton + 0x110 + parent * 28)
        g.emu.mem_write(skeleton + 0x128 + i * 28, bytes([3 if name in hit else 1]))
    g.word(g.read(p + 0xF290) + 0x10C, skeleton)
    g.word(p + 0x408, skeleton)
    g.bones = g.block(count * 64)
    g.word(g.frame - 0x14, g.bones)
    g.word(g.frame - 0x1C, count)
    raw = b''.join(struct.pack('<16f', *matrix(*world[name]).flatten()) for name in names[1:])
    g.emu.mem_write(g.bones, raw)
    g.emu.mem_write(p + 0x44C, struct.pack('<16f', *matrix(*eye).flatten()))
    g.site(0x247716, [0x24771C], ebx=p, ebp=g.frame)
    rendered = bytes(g.emu.mem_read(g.bones, count * 64))
    result = dict(world)
    for i, name in enumerate(names[1:]):
        m = np.array(struct.unpack_from('<16f', rendered, i * 64)).reshape(4, 4)
        result[name] = m[:3, :3], m[3, :3]
    damage = bytearray(raw)
    for i, name in enumerate(names[1:]):
        if name not in hit:
            damage[i*64:(i+1)*64] = struct.pack('<16f', *([float('nan')] * 16))
    g.emu.mem_write(g.bones, bytes(damage))
    g.site(0x2DA769, [0x2DA76F], edi=p, esi=g.bones, ebx=count)
    collided = bytes(g.emu.mem_read(g.bones, count * 64))
    for i, name in enumerate(names[1:]):
        expected = rendered if name in hit else damage
        assert collided[i*64:(i+1)*64] == expected[i*64:(i+1)*64], name
    def attachment(bounds):
        resource = g.read(instance + 0x290)
        g.emu.mem_write(resource + 0x108, struct.pack('<6f', *bounds))
        g.emu.mem_write(g.bones, raw)
        g.site(0x247716, [0x24771C], ebx=p, ebp=g.frame)
        return np.array(g.rendered[-1][2]).reshape(4, 4)
    return result, attachment


def render(meshes, eye, fov, yaw=0, camera=None):
    width, height = 640, 360
    pixels = np.full((height, width, 3), [25, 32, 40], dtype=np.uint8)
    depth = np.full((height, width), np.inf)
    camera = np.array(rotation([0, yaw, 0])) if camera is None else np.array(camera).T
    focal = height / (2 * math.tan(math.radians(fov) / 2))
    for vertices, triangles, color in meshes:
        vertices = (vertices - eye) @ camera
        for triangle in triangles:
            v = vertices[triangle]
            z = v[:, 2]
            if min(z) < .02:
                continue
            screen = v[:, :2] / z[:, None] * [focal, -focal] + [width / 2, height / 2]
            lo = np.maximum(np.floor(screen.min(axis=0)).astype(int), 0)
            hi = np.minimum(np.ceil(screen.max(axis=0)).astype(int), [width - 1, height - 1])
            if np.any(lo > hi):
                continue
            x, y = np.meshgrid(np.arange(lo[0], hi[0] + 1) + .5, np.arange(lo[1], hi[1] + 1) + .5)
            a, b, c = screen
            den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
            if abs(den) < 1e-8:
                continue
            u = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / den
            t = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / den
            q = 1 - u - t
            inv = u / z[0] + t / z[1] + q / z[2]
            d = np.divide(1, inv, out=np.full_like(inv, np.inf), where=inv > 0)
            region = depth[lo[1]:hi[1] + 1, lo[0]:hi[0] + 1]
            inside = (u >= 0) & (t >= 0) & (q >= 0) & (d < region)
            normal = np.cross(v[1] - v[0], v[2] - v[0])
            normal /= max(np.linalg.norm(normal), 1e-8)
            light = .35 + .65 * abs(np.dot(normal, [.2, .5, -.84]))
            pixels[lo[1]:hi[1] + 1, lo[0]:hi[0] + 1][inside] = np.clip(np.array(color) * light, 0, 255)
            region[inside] = d[inside]
    return Image.fromarray(pixels)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--db', required=True, type=Path)
    parser.add_argument('--porter', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--exe', required=True, type=Path, help='built candidate exe, executed only in Unicorn')
    parser.add_argument('--time', type=int, default=800, help='simulation time after raising, in ms')
    parser.add_argument('--eye', nargs=3, type=float, help='override the native Item eye position')
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    cache = args.out / 'assets'
    cache.mkdir(exist_ok=True)

    def extract(path):
        target = cache / path.replace('/', '__')
        if not target.exists():
            subprocess.run([str(args.porter), 'extract', '--db', str(args.db), '--path', path, '--output', str(target)], check=True, stdout=subprocess.DEVNULL)
        return target.read_bytes()

    specs = json.loads((HERE / 'pose.json').read_text())['make_pose_clip']
    source, names_data = [extract(p) for p in specs[0]['files']]
    names = [content.name_at(names_data, content.rec(names_data, at)[0]).decode()
             for at in content.children(names_data, content.find(names_data, 'bones_names'))]
    parents = {}
    for path, kind, _ in content.walk(extract('animations/skeletons/scavengers_01.skeleton')):
        if path and kind == content.T_NAMED:
            parts = path.split('/')[1:]
            parents[parts[-1]] = parts[-2] if len(parts) > 1 else None
    source_world = pose(content.make_pose_clip(source, names_data, specs[0]), names, parents, 0)
    eye_r, eye_p = source_world['Item']
    if args.eye is not None: eye_p = args.eye
    world, attachment = native_pose(args.exe, source_world, names, parents, (eye_r, eye_p), args.time)
    def skin_for(world):
        skin = []
        for model, surface, color in [('black_market_01_hand_hud', 'black_market_01_hand_s', [160, 170, 153]),
                                      ('black_market_01_torso_s1', 'black_market_01_torso_s', [91, 117, 86])]:
            prefix = CHAR + model + '.skinned_model/render/'
            bind = bind_pose(extract(prefix + 'bind_pose'))
            frames = np.array([np.linalg.inv(m) @ matrix(*world[n]) for n, m in bind.items()])
            v, t = mesh(extract(prefix + surface + '/converted_model'), frames)
            skin.append((v, t, color))
        return skin
    skin = skin_for(world)
    flower_spec = json.loads((HERE.parent / 'content-edit/artefact-carry-flower-1.json').read_text())['make_held_flower'][0]
    flower = content.make_held_flower({p: extract(p) for p in flower_spec['files']}, flower_spec)
    artefacts = [('Spring', [extract(ITEM + 'artefact_spring.model/render/artefacts_s/converted_model')]),
                 ('Onyx', [extract(ITEM + 'artefact_onyx.model/render/' + n + '/converted_model') for n in ('artefacts_s', 'artefacts_s1')]),
                 ('Larkspur', [next(v for p, v in flower.items() if p.endswith('/converted_model'))])]
    def objects_for(attachment, skin, name, surfaces):
        parsed = [mesh(data) for data in surfaces]
        if name == 'Larkspur':
            properties = flower[flower_spec['output'] + 'render/export_properties']
        else:
            properties = extract(ITEM + 'artefact_' + name.lower() + '.model/render/export_properties')
        def bounds(which):
            offset, _, _, kind, _ = content.rec(properties, content.find(properties, 'bounding_box/' + which))
            assert kind == 7
            return np.array(struct.unpack_from('<3f', properties, offset))
        low, high = bounds('min'), bounds('max')
        transform = attachment([*low, *high])
        objects = list(skin)
        for v, t in parsed:
            objects.append((v @ transform[:3, :3] + transform[3, :3], t, [180, 78, 178] if name == 'Spring' else [103, 145, 188]))
        return objects

    sheet = Image.new('RGB', (1920, 1200), (15, 20, 26))
    labels = ImageDraw.Draw(sheet)
    for row, (name, surfaces) in enumerate(artefacts):
        objects = objects_for(attachment, skin, name, surfaces)
        for column, fov in enumerate((50, 75, 100)):
            x, y = column * 640, row * 400
            sheet.paste(render(objects, np.array(eye_p), fov, camera=eye_r), (x, y + 32))
            labels.text((x + 15, y + 10), f'{name} | vertical FOV {fov} | native pose, offline mesh', fill='white')
    sheet.save(args.out / 'framing.png')
    pitch_sheet = Image.new('RGB', (1920, 1200), (15, 20, 26))
    labels = ImageDraw.Draw(pitch_sheet)
    for column, angle in enumerate((-40, 0, 40)):
        pitched = np.array(rotation([math.radians(angle), 0, 0])) @ np.array(eye_r)
        posed, pitched_attachment = native_pose(args.exe, source_world, names, parents, (pitched, eye_p), args.time)
        posed_skin = skin_for(posed)
        for row, (name, surfaces) in enumerate(artefacts):
            objects = objects_for(pitched_attachment, posed_skin, name, surfaces)
            x, y = column * 640, row * 400
            pitch_sheet.paste(render(objects, np.array(eye_p), 75, camera=pitched), (x, y + 32))
            labels.text((x + 15, y + 10), f'{name} | eye pitch {angle:+} deg | FOV 75', fill='white')
    pitch_sheet.save(args.out / 'pitch.png')
    side = Image.new('RGB', (1280, 400), (15, 20, 26))
    objects = objects_for(attachment, skin, *artefacts[0])
    for column, (label, position, yaw) in enumerate([('Teammate, front', [0, 1.4, 1.2], math.pi), ('Side: wrist and finger curl', [.8, 1.4, .25], -math.pi / 2)]):
        side.paste(render(objects, np.array(position), 50, yaw=yaw), (column * 640, 32))
        ImageDraw.Draw(side).text((column * 640 + 15, 10), label, fill='white')
    side.save(args.out / 'teammate.png')
    hand_r, hand_p = map(np.array, world['RightHand'])
    attachment = np.array([-.13, .15, 0]) @ hand_r + hand_p
    (args.out / 'reference.json').write_text(json.dumps({'eye': list(eye_p), 'wrist': hand_p.tolist(), 'attachment': attachment.tolist(), 'max_dimension': .16,
        'note': 'Installed native pose hooks on the stock rig, viewed from its Item eye bone. Engine materials and locomotion are not simulated.'}, indent=2) + '\n')
    print(args.out / 'framing.png')


if __name__ == '__main__':
    main()

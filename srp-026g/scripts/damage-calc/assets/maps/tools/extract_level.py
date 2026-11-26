#!/usr/bin/env python3
"""Extract what a top-down render of one 0.26g project needs from resources.db.

    extract_level.py DBGRAB RESOURCES_DB PROJECT(e.g. level_01/day_2) OUT_DIR [PACK_FILES_DIR]
                     [--patch resources.db.patch_<n> ...] [--manifest FILE]

With PACK_FILES_DIR (a content pack's files/ tree, e.g. a ported map), files
are taken from it first and from the database otherwise.

With --patch (repeatable), the patches are mounted over the database as the
game mounts them: a file is read from the highest-numbered patch that has it,
else from the database. Use it for maps a released patch added (London in
patch 14, the 0.11e maps in patch 22).

With --manifest, FILE gets what was read as JSON: "files", every archive path
extracted, and "absent", every path looked for and found in no archive (a
missing texture): a record of what a render was made from (build_all.py
renders a map again only with --rerender or when it has no render yet).

OUT_DIR/project            decoded client_project (bincfg JSON)
OUT_DIR/models/<lib>.model/{settings,render/<surface>/converted_model}
OUT_DIR/material_instances/<name>.material
OUT_DIR/tex/<name>.png      diffuse textures downscaled to <= 256 px (DDS dropped)
Only LOD0 render data, materials and their diffuse texture are kept, to stay small.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / 'gen-data-026g'))  # bincfg
import bincfg  # noqa: E402
from PIL import Image  # noqa: E402

DIFFUSE_SLOTS = ('texture_diffuse', 'texture_detail_0', 'texture_vertex_blended_diffuse')


OVERLAY = None   # a pack's files/ directory, searched before the database
PATCHES = []     # (patch number, path, set of its file paths), lowest first
FETCHED = set()  # the archive paths extracted
ABSENT = set()   # the paths looked for and not found


def patch_number(path):
    return int(Path(path).name.rsplit('_', 1)[1])


def load_patches(dbgrab, paths):
    out = []
    for path in sorted(paths, key=patch_number):
        listing = subprocess.run([dbgrab, path, 'list'], check=True, capture_output=True, text=True).stdout
        out.append((patch_number(path), path, {line.split('\t', 1)[0] for line in listing.splitlines() if line}))
    return out


def grab_one(dbgrab, db, prefix, dest):
    """Extract every file of one archive under prefix into dest, restoring '/' layout."""
    if OVERLAY is not None:
        src = OVERLAY / prefix.rstrip('/')
        if src.is_file():
            (dest / prefix).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dest / prefix)
            return 1
        if prefix.endswith('/') and src.is_dir():
            shutil.copytree(src, dest / prefix.rstrip('/'), dirs_exist_ok=True)
            return 1
    # dbgrab writes a path with '/' as '__', which is ambiguous where a name
    # ends or starts with '_' (surface "l3terr_"), so the real paths come from
    # the archive's listing.
    listing = subprocess.run([dbgrab, db, 'list', prefix], check=True, capture_output=True, text=True).stdout
    real = {path.replace('/', '__'): path for path in (line.split('\t', 1)[0] for line in listing.splitlines())
            if path.startswith(prefix)}
    if not real:
        return 0
    with tempfile.TemporaryDirectory(dir=dest.parent if dest.parent.exists() else None) as tmp:
        subprocess.run([dbgrab, db, 'get', prefix, tmp], check=True, capture_output=True)
        n = 0
        for name in sorted(os.listdir(tmp)):
            if name not in real:
                continue
            target = dest / real[name]
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(os.path.join(tmp, name), target)
            FETCHED.add(real[name])
            n += 1
        return n


def grab(dbgrab, db, prefix, dest):
    """Extract every file under prefix into dest, restoring '/' layout: from the
    pack, else from the database with each patch that has a file under prefix
    laid over it in order (the highest patch's copy of a path wins)."""
    if OVERLAY is not None and (OVERLAY / prefix.rstrip('/')).exists():
        return grab_one(dbgrab, db, prefix, dest)
    n = grab_one(dbgrab, db, prefix, dest)
    for _, path, files in PATCHES:
        if any(f.startswith(prefix) for f in files):
            n += grab_one(dbgrab, path, prefix, dest)
    if not n:
        ABSENT.add(prefix)
    return n


def leaves(d, p=''):
    if isinstance(d, dict):
        for k, v in d.items():
            yield from leaves(v, f'{p}.{k}' if p else k)
    elif isinstance(d, list):
        for i, v in enumerate(d):
            yield from leaves(v, f'{p}[{i}]')
    else:
        yield p, d


def main():
    global OVERLAY, PATCHES
    args, patches, manifest = [], [], None
    it = iter(sys.argv[1:])
    for a in it:
        if a == '--patch':
            patches.append(next(it))
        elif a == '--manifest':
            manifest = Path(next(it))
        else:
            args.append(a)
    dbgrab, db, project, out = args[0], args[1], args[2], Path(args[3])
    if len(args) > 4:
        OVERLAY = Path(args[4])
    PATCHES = load_patches(dbgrab, patches)
    out.mkdir(parents=True, exist_ok=True)
    grab(dbgrab, db, f'projects/{project}/client_project', out)
    proj = bincfg.load(out / 'projects' / project / 'client_project')
    (out / 'project.json').write_text(json.dumps(proj, default=list))
    libs = sorted({o['lib_name'] for o in proj.get('render_visuals', [])})
    materials = set()
    for lib in libs:
        base = f'models/{lib}.model/'
        if not (out / base / 'settings').exists():
            grab(dbgrab, db, base + 'settings', out)
            grab(dbgrab, db, base + 'render/', out)
        sp = out / base / 'settings'
        if not sp.exists():
            print('missing model', lib, file=sys.stderr)
            continue
        s = bincfg.load(sp)
        for surf, ms in s.get('material_settings', {}).items():
            if isinstance(ms, dict) and ms.get('material_name'):
                materials.add(ms['material_name'])
        # drop non-LOD0 surfaces to save space
        lod0 = set(s.get('lod_hierrarchy', {}).get('LOD0', {}).get('surfaces', []))
        rd = out / base / 'render'
        if rd.exists():
            for d in rd.iterdir():
                if d.is_dir() and d.name not in lod0:
                    shutil.rmtree(d)
    textures = set()
    for m in sorted(materials):
        p = out / 'material_instances' / (m + '.material')
        if not p.exists():
            grab(dbgrab, db, f'material_instances/{m}.material', out)
        if not p.exists():
            print('missing material', m, file=sys.stderr)
            continue
        for path, v in leaves(bincfg.load(p)):
            if isinstance(v, str) and v and path.endswith('.value') and path.split('.')[-2] in DIFFUSE_SLOTS:
                textures.add(v)
    (out / 'tex').mkdir(exist_ok=True)
    for t in sorted(textures):
        png = out / 'tex' / (t + '.png')
        if png.exists():
            continue
        grab(dbgrab, db, f'textures/{t}.dds', out)
        dds = out / 'textures' / (t + '.dds')
        if not dds.exists():
            print('missing texture', t, file=sys.stderr)
            continue
        try:
            im = Image.open(dds).convert('RGBA')
            if max(im.size) > 256:
                im = im.resize((256, 256 * im.size[1] // im.size[0]), Image.BOX)
            png.parent.mkdir(parents=True, exist_ok=True)
            im.save(png)
        except Exception as e:  # noqa: BLE001
            print('bad texture', t, e, file=sys.stderr)
        dds.unlink()
    shutil.rmtree(out / 'textures', ignore_errors=True)
    if manifest:
        manifest.write_text(json.dumps({'files': sorted(FETCHED), 'absent': sorted(ABSENT)}, indent=0))
    print(json.dumps({'project': project, 'models': len(libs), 'materials': len(materials),
                      'textures': len(textures)}))


if __name__ == '__main__':
    main()

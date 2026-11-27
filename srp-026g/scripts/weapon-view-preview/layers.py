"""The Weapon view window's gun layers (flash/main_menu/bitmaps/SrpPreview*.png), offline.

python layers.py <scripts/sight-alignment> <weapon-modules pack files> <preview_eyes JSON> <out folder>

The AK-12 (363) aimed at 0.69d0's 32.5 degrees, 450x253 with alpha, as patch 161 draws it:
- over its iron sights: the gun's `aim` locator with its aim_offset (iron-sights-1.json's -0.0506;
  revision 73's 0, weapon-view-069.json's aim_offset_026g: patch 218 moves it no nearer than that),
  at -20, 0, +25 and +50 cm of sight distance (SrpPreviewIron*);
- through the EOTech 552: the bare gun with the fit's parts and optic at the crate's placements, the eye
  moved along its line by Relief::shift (pose.rs) at the same distances (SrpPreviewRedDot*);
- through the 1P69 Hyperon: its aimed model alone at its own eye (the game hides the gun and the
  hands; a magnified scope's eye never moves) (SrpPreviewScope).
The JSON is `cargo run -p weapon-modules --example preview_eyes -- <weapon_modules.srpm> 363 eotech_552
zenit_1p69`. Pillow and NumPy.
"""
import json, math, struct, sys
from pathlib import Path
import numpy as np
from PIL import Image
sys.path.insert(0, sys.argv[1])
import model, render
from model import Bone, HALF, TO_VIEW, matrix, transform, Mesh

pack = Path(sys.argv[2])
dump = json.loads(Path(sys.argv[3]).read_text())
out = Path(sys.argv[4])
out.mkdir(exist_ok=True)
W, H = 450, 253
FOV = 32.5
AIM_OFFSET, AIM_OFFSET_026G = -0.0506, 0.0
DISTANCES = {-20: "M20", 0: "0", 25: "25", 50: "50"}
LEAST_RELIEF = 0.06


def bind(directory):
    """The model's bones at its bind pose (render/bind_pose)."""
    data = (directory / "render/bind_pose").read_bytes()
    rest, at = [], 2
    for _ in range(struct.unpack_from("<H", data)[0]):
        end = data.index(0, at)
        values = struct.unpack_from("<9f", data, end + 1)
        rest.append(Bone(data[at:end].decode(), matrix(values[:3], values[3:6], values[6:]), None))
        at = end + 37
    for b in rest:
        b.pose = b.rest
    return rest


def affine(twelve):
    m = np.eye(4)
    a = np.asarray(twelve).reshape(4, 3)
    m[:3, :3], m[3, :3] = a[:3], a[3]
    return m


def placed(rig, at):
    return HALF @ affine(at["offset"]) @ rig[at["bone"]].rest


def moved(meshes, m):
    return [Mesh(x.name, transform(x.points, m), x.triangles, x.uv, x.material) for x in meshes]


def shot(meshes, camera, name):
    focal = (H / 2) / math.tan(math.radians(FOV) / 2)
    image, depth = render.render(meshes, np.linalg.inv(camera) @ TO_VIEW, (W, H), focal, [pack])
    rgba = np.dstack([np.asarray(image), np.where(np.isfinite(depth), 255, 0).astype(np.uint8)])
    Image.fromarray(rgba, "RGBA").save(out / f"{name}.png", optimize=True)
    print(name)


def relief_shift(along, farther):
    """pose.rs Relief::shift for an eye in the gun's plane."""
    if farther == 0:
        return 0.0
    return min(along - farther, max(along, -LEAST_RELIEF)) - along


whole = pack / "models" / (dump["bare"].replace("_bare", "") + ".skinned_model")
bare = pack / "models" / (dump["bare"] + ".skinned_model")
rig = bind(whole)
gun = model.meshes(whole, rig)
aim = model.locator_matrix(model.locator(whole, "aim"), rig)
for cm, suffix in DISTANCES.items():
    offset = min(AIM_OFFSET - cm / 100.0, AIM_OFFSET_026G)
    shot(gun, matrix((0, 0, offset)) @ HALF @ aim, f"SrpPreviewIron{suffix}")
brig = bind(bare)
body = model.meshes(bare, brig)
for name, fit in dump["fits"].items():
    optic = model.meshes(pack / "models" / ((fit["aimed"] or fit["optic"]) + ".model"))
    meshes = moved([m for m in optic if "lense_1p" not in m.name], placed(brig, fit["frame"]))
    eye = placed(brig, fit["eye"])
    if fit["own"]:
        shot(meshes, eye, "SrpPreviewScope")
        continue
    meshes += body
    for part in fit["parts"]:
        meshes += moved(model.meshes(pack / "models" / (part["model"] + ".model")), placed(brig, part["at"]))
    for cm, suffix in DISTANCES.items():
        shot(meshes, matrix((0, 0, relief_shift(fit["along"], cm / 100.0))) @ eye, f"SrpPreviewRedDot{suffix}")

"""The Weapon view window's gun layers (flash/main_menu/bitmaps/SrpPreview*), offline.

python layers.py <scripts/sight-alignment> <weapon-modules pack files> <preview_eyes JSON> <out folder>

The AK-12 (363) aimed, as patch 161 draws it (a JPEG and its alpha, <class>.jpg and <class>.alpha.png):
- over its iron sights: the gun's `aim` locator with its aim_offset (iron-sights-1.json's -0.0506;
  revision 73's 0, weapon-view-069.json's aim_offset_026g: patch 218 moves it no nearer than that),
  at -20, 0, +25 and +50 cm of sight distance (SrpPreviewIron<cm><tier>);
- through the EOTech 552: the bare gun with the fit's parts and optic at the crate's placements, the eye
  moved along its line by Relief::shift (pose.rs) at the same distances (SrpPreviewRedDot<cm><tier>);
- through the 1P69 Hyperon: its aimed model alone at its own eye (the game hides the gun and the
  hands; a magnified scope's eye never moves) (SrpPreviewScope).
Each view's layer is made at a few angles (TIERS): the widest its sliders reach (the iron sights' at
"Weapon at the world FOV" and World FOV 100: 94.8; the EOTech's 77.7; the scope's 32.5), so the whole
gun is there at every setting, and narrower ones, so the preview, which draws the narrowest tier at or
over the angle, never enlarges a tier more than its next one's ratio (at most 2.3 times its pixels at
the widest step, about the pane's on-screen size at 1440p). 1440x810, drawn at twice that and
downsampled (anti-aliased edges). Emissive materials (the EOTech's dot, the 1P69's) are drawn in their
constant emissive colour over the rest, as the game's additive forward pass shows them (the
collimator's red).
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
W, H = 1440, 810
SUPER = 2
# Each view's tiers: (suffix, degrees), widest first.
TIERS = {"Iron": [("W", 94.8), ("M", 52.0), ("N", 28.0)], "RedDot": [("W", 77.7), ("M", 45.0), ("N", 26.0)],
         "Scope": [("", 32.5)]}
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


def glow(material):
    """The constant emissive colour (0-255) of an additive forward material, or None."""
    path = pack / "material_instances" / (material + ".material")
    if not path.exists():
        return None
    effect = model.config(path)["material"].get("forward", {}).get("effect", {})
    if not effect or effect.get("use_temissive", {}).get("value", True):
        return None
    colour = effect.get("constant_emissive", {}).get("value")
    if not colour:
        return None
    gain = float(effect.get("constant_emissive_multiplier", {}).get("value", 1))
    return np.clip(np.asarray(colour[:3], float) * gain, 0, 1) * 255


def shot(meshes, camera, view, name):
    """The view's layer at each of its tiers."""
    lit = [m for m in meshes if glow(m.material) is None]
    glowing = [m for m in meshes if glow(m.material) is not None]
    for suffix, fov in TIERS[view]:
        w, h = W * SUPER, H * SUPER
        focal = (h / 2) / math.tan(math.radians(fov) / 2)
        to_view = np.linalg.inv(camera) @ TO_VIEW
        image, depth = render.render(lit, to_view, (w, h), focal, [pack])
        rgb = np.asarray(image).astype(float)
        alpha = np.isfinite(depth).astype(float)
        for mesh in glowing:
            _, near = render.render([mesh], to_view, (w, h), focal, [pack])
            seen = np.isfinite(near) & (near <= depth)
            rgb[seen] = glow(mesh.material)
            alpha[seen] = 1.0
        # premultiplied, so the downsampled edges keep their colour
        rgba = np.dstack([rgb * alpha[..., None], alpha * 255]).astype(np.float32)
        small = np.stack([np.asarray(Image.fromarray(rgba[..., i]).resize((W, H), Image.LANCZOS)) for i in range(4)], -1)
        a = np.clip(small[..., 3], 0, 255)
        colour = np.where(a[..., None] > 0, small[..., :3] * 255 / np.maximum(a[..., None], 1e-3), 0)
        # a JPEG of the colour premultiplied by the alpha (black where transparent), and the alpha beside
        # it (<class>.alpha.png, grey): one DefineBitsJPEG3, a tenth of a lossless bitmap's size
        premultiplied = np.clip(colour * a[..., None] / 255, 0, 255).astype(np.uint8)
        Image.fromarray(premultiplied, "RGB").save(out / f"{name}{suffix}.jpg", quality=92, progressive=False)
        grey = np.clip(a, 0, 255).astype(np.uint8)
        Image.fromarray(np.dstack([grey] * 3), "RGB").save(out / f"{name}{suffix}.alpha.png", optimize=True)
        print(name + suffix)


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
    shot(gun, matrix((0, 0, offset)) @ HALF @ aim, "Iron", f"SrpPreviewIron{suffix}")
brig = bind(bare)
body = model.meshes(bare, brig)
for name, fit in dump["fits"].items():
    optic = model.meshes(pack / "models" / ((fit["aimed"] or fit["optic"]) + ".model"))
    meshes = moved([m for m in optic if "lense_1p" not in m.name], placed(brig, fit["frame"]))
    eye = placed(brig, fit["eye"])
    if fit["own"]:
        shot(meshes, eye, "Scope", "SrpPreviewScope")
        continue
    meshes += body
    for part in fit["parts"]:
        meshes += moved(model.meshes(pack / "models" / (part["model"] + ".model")), placed(brig, part["at"]))
    for cm, suffix in DISTANCES.items():
        shot(meshes, matrix((0, 0, relief_shift(fit["along"], cm / 100.0))) @ eye, "RedDot", f"SrpPreviewRedDot{suffix}")

"""The Weapon view window's world (flash/main_menu/bitmaps/SrpPreviewWorld*.jpg), from two captures.

python world.py <scripts/sight-alignment> <character assets> <captures folder> <poses JSON> <out folder>

The poses JSON (vostok.json) holds one view twice: "yard" at 100 degrees vertical (the widest world the
preview draws: the iron sights' at uro_fov 100 is 94.8) and "yard-scope" at 20 (the 1P69's 16.25, sharp);
the folder holds capture.py's yard.png and yard-scope.png of it (1920x1080). A stock soldier (a
scavenger's outfit, the lobby's idle pose with an AK) is rendered at the pose's "target" facing the
camera with each capture's projection and laid over it: SrpPreviewWorld and SrpPreviewWorldScope, at
640x360 (the 100-degree frame's 60 degrees are about the preview's 169 px), baseline JPEGs as
flash_tree.py takes them. The character assets are a folder of `srp-game-data get`'s
models/character/human/{scavengers,base}/... and the animation below, with their textures.
"""
import json, math, sys
from pathlib import Path
import numpy as np
from PIL import Image, ImageEnhance
sys.path.insert(0, sys.argv[1])
import model, render
from model import matrix, TO_VIEW

root, captures, out = Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[5])
poses = {p["name"]: p for p in json.loads(Path(sys.argv[4]).read_text())["poses"]}
out.mkdir(exist_ok=True)
ANIMATION = "resources/animations/single/weapons/ak/player/stand/3rd_view/on_site/idle_lobby"
PARTS = ["scavengers/scavenger_04_torso_s1", "scavengers/scavenger_04_legs_s1", "scavengers/scavenger_04_hand",
         "base/base_character_head"]
W, H = 1920, 1080
# The renderer lights flatly; the capture's sunlit yard is brighter.
BRIGHTER = 1.35


def soldier(pose):
    """The soldier's meshes standing at the pose's target, facing its camera."""
    eye, target = np.array(pose["position"], dtype=float), np.array(pose["target"], dtype=float)
    at = matrix(tuple(target), angles=(0, math.atan2(eye[0] - target[0], eye[2] - target[2]), 0))
    meshes = []
    for part in PARTS:
        folder = root / f"models/character/human/{part}.skinned_model"
        for m in model.meshes(folder, model.bones(root, folder, ANIMATION, 0.0)):
            meshes.append(model.Mesh(m.name, model.transform(m.points, at), m.triangles, m.uv, m.material))
    return meshes


def world(name, bitmap):
    pose = poses[name]
    eye, forward = np.array(pose["position"], dtype=float), np.array(pose["forward"], dtype=float)
    # the camera's rows: right, up, forward, place
    z = forward / np.linalg.norm(forward)
    x = np.cross([0.0, 1.0, 0.0], z)
    x /= np.linalg.norm(x)
    camera = np.eye(4)
    camera[0, :3], camera[1, :3], camera[2, :3], camera[3, :3] = x, np.cross(z, x), z, eye
    focal = (H / 2) / math.tan(math.radians(pose["fov"]) / 2)
    image, depth = render.render(soldier(pose), np.linalg.inv(camera) @ TO_VIEW, (W, H), focal, [root])
    scene = Image.open(captures / f"{name}.png").convert("RGB")
    scene.paste(ImageEnhance.Brightness(image).enhance(BRIGHTER), (0, 0),
                Image.fromarray(np.where(np.isfinite(depth), 255, 0).astype(np.uint8)))
    ys, xs = np.nonzero(np.isfinite(depth))
    print(name, "the soldier at", xs.min(), xs.max(), ys.min(), ys.max())
    scene.save(out / f"{name}-full.png")
    scene.resize((640, 360), Image.LANCZOS).save(out / f"{bitmap}.jpg", quality=86, progressive=False)


world("yard", "SrpPreviewWorld")
world("yard-scope", "SrpPreviewWorldScope")

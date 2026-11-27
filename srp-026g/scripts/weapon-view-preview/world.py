"""The Weapon view window's worlds (flash/main_menu/bitmaps/SrpPreviewWorld*.jpg), from three captures.

python world.py <scripts/sight-alignment> <character assets> <captures folder> <poses JSON> <out folder>
                <weapon-modules pack files> <content packs folder> <preview_eyes JSON>

The poses JSON (vostok.json) holds one view three times: "lane" at 100 degrees vertical (the widest world
the preview draws: the iron sights' at uro_fov 100 is 94.8), "lane-60" at 60 (the worlds of 60 degrees
and less: the narrowest, the iron sights' at World FOV 50, 45.9, is 792 of its 1080 rows) and
"lane-scope" at 20 (the 1P69's 16.25). The folder holds capture.py's lane.png, lane-60.png and
lane-scope.png of them (2560x1440, 1920x1080 and 2560x1440; the game's highest settings).

Its "soldiers" (soldiers.py) are rendered with each capture's projection and laid over it: each standing
on the walk grid's floor (crates/mapdata: a floor a spawn reaches, its height there) facing the camera
turned a little, the AK-12 at his shoulder (the third person's aimed idle), bright decals on his outfit.
The renderer lights flatly; the capture's sunlit ground is brighter (BRIGHTER). Saved: SrpPreviewWorld
2250x1266 (lane: its narrowest view, 60.1 degrees, is 615 rows), SrpPreviewWorld60 1920x1080 and
SrpPreviewWorldScope 1440x810 (16.25 of its 20 degrees: 650 rows), baseline JPEGs at quality 95 as
flash_tree.py takes them: the 800x450 pane is 600 rows at 1440p.
The character assets are a folder of `srp-game-data get`'s models/character/human/{scavengers,base}/...,
the skeleton and the AK's third person clips, with their textures.
"""
import json
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageEnhance

sys.path.insert(0, sys.argv[1])
sys.path.insert(0, str(Path(__file__).resolve().parent))
import render  # noqa: E402
import soldiers  # noqa: E402
from model import TO_VIEW  # noqa: E402

root, captures, out = Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[5])
spec = json.loads(Path(sys.argv[4]).read_text())
poses = {p["name"]: p for p in spec["poses"]}
pack, packs = Path(sys.argv[6]), Path(sys.argv[7])
bare = json.loads(Path(sys.argv[8]).read_text())["bare"]
out.mkdir(exist_ok=True)
SIZES = {"lane": (2250, 1266), "lane-60": (1920, 1080), "lane-scope": (1440, 810)}
BITMAPS = {"lane": "SrpPreviewWorld", "lane-60": "SrpPreviewWorld60", "lane-scope": "SrpPreviewWorldScope"}
BRIGHTER = 1.35

render.texture = soldiers.texture(render.texture)
people = soldiers.soldiers(root, pack, bare, spec["soldiers"], packs)


def world(name):
    pose = poses[name]
    eye, forward = np.array(pose["position"], dtype=float), np.array(pose["forward"], dtype=float)
    # the camera's rows: right, up, forward, place
    z = forward / np.linalg.norm(forward)
    x = np.cross([0.0, 1.0, 0.0], z)
    x /= np.linalg.norm(x)
    camera = np.eye(4)
    camera[0, :3], camera[1, :3], camera[2, :3], camera[3, :3] = x, np.cross(z, x), z, eye
    scene = Image.open(captures / f"{name}.png").convert("RGB")
    W, H = scene.size
    focal = (H / 2) / math.tan(math.radians(pose["fov"]) / 2)
    image, depth = render.render(people, np.linalg.inv(camera) @ TO_VIEW, (W, H), focal, [root, pack])
    scene.paste(ImageEnhance.Brightness(image).enhance(BRIGHTER), (0, 0),
                Image.fromarray(np.where(np.isfinite(depth), 255, 0).astype(np.uint8)))
    scene.save(out / f"{name}-full.png")
    scene.resize(SIZES[name], Image.LANCZOS).save(out / f"{BITMAPS[name]}.jpg", quality=95, progressive=False)
    print(name, "written")


for name in BITMAPS:
    world(name)

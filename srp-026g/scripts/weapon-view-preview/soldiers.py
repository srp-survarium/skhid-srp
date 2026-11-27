"""Stock soldiers for the preview's world: posed as the game's third person aims, the AK-12 in their hands,
bright decals on their outfit. Used by world.py (and closeup() to look at one).

A soldier is the scavenger 04 outfit (torso, legs, hands) and the base head, posed by the AK's
`aimed_on_site_idle` at its first frame on the whole skeleton, standing at a floor point facing a
point (the camera). The AK-12 (the weapon-modules pack's whole model at its bind pose) is held at the
skeleton's Weapon bone, as the game draws a held gun (HALF @ the bone, as layers.py's hip view).

Decals: the game bakes them into the outfit's textures (the camouflage bake), which this renderer cannot
do; each decal is a square of its own picture (alpha-tested, as the renderer draws every texture) 1.5 cm
in front of the body: the chest (30 cm) and each thigh's front (22 cm). The decals are the content packs'
own textures (Snufkin, the bottled fumo, Neco-Arc, the number stencils).
"""
import math
import struct
from pathlib import Path

import numpy as np
from PIL import Image

import model
import render
from model import Bone, HALF, clip, config, matrix, resource, transform, Mesh

ANIMATION = "resources/animations/single/weapons/ak/player/stand/3rd_view/on_site/aimed_on_site_idle"
PARTS = ["scavengers/scavenger_04_torso_s1", "scavengers/scavenger_04_legs_s1", "scavengers/scavenger_04_hand",
         "base/base_character_head"]


def bind(directory):
    """A model's bones at its bind pose (render/bind_pose)."""
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


def skeleton(root, directory, animation, time=0.0):
    """Every bone of the model's skeleton (the Weapon and Item bones too, which no mesh is bound to) at the
    clip's `time`, standing at the origin."""
    parents = {}

    def descend(node, parent=None):
        for name, child in node.items():
            if name != "mask" and isinstance(child, dict):
                parents[name] = parent
                descend(child, name)

    descend(config(root / resource(config(directory / "skeleton")["skeleton"], ".skeleton")))
    local = clip(root, animation, time)
    world = {}

    def resolve(name):
        if name not in world:
            parent = parents[name]
            world[name] = local.get(name, np.eye(4)) @ (resolve(parent) if parent else np.eye(4))
        return world[name]

    return {name: resolve(name) for name in parents}


def gun(pack, bare):
    """The AK-12's whole model at its bind pose (preview_eyes' `bare` names it)."""
    whole = pack / "models" / (bare.replace("_bare", "") + ".skinned_model")
    return model.meshes(whole, bind(whole))


# Where the decals go, on the outfit at its bind pose (standing, facing +z): (bone it moves with, height,
# side offset, size): the chest and each thigh's front.
SPOTS = [("Chest", 1.30, 0.0, 0.30), ("RightUpLeg", 0.60, 0.12, 0.22), ("LeftUpLeg", 0.60, -0.12, 0.22)]


def sticker(body, height, side, size, decal):
    """A decal as a square facing the viewer in front of the body's surface at (side, height): the front-most
    point of the body there, 1.5 cm out. Its material names the decal file (texture() reads it)."""
    near = body[(np.abs(body[:, 1] - height) < 0.04) & (np.abs(body[:, 0] - side) < 0.04)]
    front = (near[:, 2].max() if len(near) else 0.15) + 0.015
    h = size / 2
    # u runs the viewer's left to right: the soldier's +x to -x
    points = np.array([[side + h, height + h, front], [side - h, height + h, front],
                       [side - h, height - h, front], [side + h, height - h, front]])
    uv = np.array([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
    return Mesh("decal", points, np.array([[0, 1, 2], [0, 2, 3]]), uv, "decal:" + str(decal))


def texture(original):
    """render.texture, and a decal's own picture for a sticker's material."""
    cache = {}

    def read(roots, material):
        if material.startswith("decal:"):
            if material not in cache:
                cache[material] = np.asarray(Image.open(material[6:]).convert("RGBA"))
            return cache[material]
        return original(roots, material)

    return read


def soldiers(root, pack, bare, spec, decal_root):
    """The meshes of every soldier of `spec` ([{place, facing, decals: [chest, left thigh, right thigh]}]) and
    the texture function painting their decals."""
    bones = skeleton(root, root / f"models/character/human/{PARTS[0]}.skinned_model", ANIMATION)
    weapon = HALF @ bones["Weapon"]
    held = gun(pack, bare)
    parts = []
    for part in PARTS:
        folder = root / f"models/character/human/{part}.skinned_model"
        parts.append(model.meshes(folder, model.bones(root, folder, ANIMATION, 0.0)))
    # the outfit at its bind pose (for where a decal sits) and each bone's move from it to the pose (a decal
    # moves with its bone, as the skin around it)
    rest = []
    moves = {}
    for part in PARTS[:2]:
        folder = root / f"models/character/human/{part}.skinned_model"
        posed = model.bones(root, folder, ANIMATION, 0.0)
        rest += [m.points for m in model.meshes(folder, [Bone(b.name, b.rest, b.rest) for b in posed])]
        for b in posed:
            moves[b.name] = np.linalg.inv(b.rest) @ b.pose
    body = np.vstack(rest)
    meshes = []
    for one in spec:
        place, facing = np.asarray(one["place"], float), np.asarray(one["facing"], float)
        # turned `turn` degrees from the point it faces, so the rifle shows its side
        # (a yaw of +90 degrees turns +z to -x)
        yaw = math.atan2(place[0] - facing[0], facing[2] - place[2]) + math.radians(one.get("turn", 0))
        at = matrix(tuple(place), angles=(0, yaw, 0))
        for group in parts:
            for m in group:
                meshes.append(Mesh(m.name, transform(m.points, at), m.triangles, m.uv, m.material))
        for m in held:
            meshes.append(Mesh(m.name, transform(m.points, weapon @ at), m.triangles, m.uv, m.material))
        for (bone, height, side, size), decal in zip(SPOTS, one["decals"]):
            if decal:
                d = sticker(body, height, side, size, decal_root / decal)
                meshes.append(Mesh(d.name, transform(d.points, moves[bone] @ at), d.triangles, d.uv, d.material))
    return meshes

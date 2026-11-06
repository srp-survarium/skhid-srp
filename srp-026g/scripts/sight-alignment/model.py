"""Read 0.26g sight geometry and reproduce the retail affine aim alignment.

Matrices use engine row vectors. No game process, server, or archive is modified.
"""

from dataclasses import dataclass
from pathlib import Path
import math
import struct
import sys

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "content-edit"))
import content_edit as ce


def config(path):
    data = Path(path).read_bytes()

    def read(at):
        value, _, _, kind, count = ce.rec(data, at)
        if kind == ce.T_NAMED:
            return {
                ce.name_at(data, ce.rec(data, c)[1]).decode(): read(c)
                for c in ce.children(data, at)
            }
        if kind == ce.T_INDEXED:
            return [read(c) for c in ce.children(data, at)]
        if kind == ce.T_STR:
            return data[value : value + count - 1].decode()
        if kind == ce.T_BOOL:
            return bool(value & 1)
        if kind in (ce.T_INT, ce.T_FLOAT):
            return ce.as_number(kind, value & 0xFFFFFFFF)
        if kind in (6, 7, 8):
            return list(struct.unpack_from("<%df" % (kind - 4), data, value))
        raise ValueError(f"{path}: unsupported config type {kind}")

    return read(0)


def rotation(angles):
    sx, sy, sz = np.sin(angles)
    cx, cy, cz = np.cos(angles)
    return np.array(
        [
            [cy * cz, -cy * sz, sy],
            [sy * sx * cz + cx * sz, -sx * sy * sz + cx * cz, -sx * cy],
            [-cx * sy * cz + sx * sz, cx * sy * sz + sx * cz, cx * cy],
        ]
    )


def matrix(position=(0.0, 0.0, 0.0), angles=(0.0, 0.0, 0.0), scale=(1.0, 1.0, 1.0)):
    out = np.eye(4)
    out[:3, :3] = np.diag(scale) @ rotation(angles)
    out[3, :3] = position
    return out


HALF = matrix(angles=(0.0, math.pi, 0.0))
TO_VIEW = np.diag([1.0, 1.0, -1.0, 1.0])  # D3D's +Z projection to preview -Z


def transform(points, m):
    return points @ m[:3, :3] + m[3, :3]


def resource(name, suffix):
    return name.removeprefix("resources/") + suffix


def curve(pairs, time):
    """Retail cubic B-spline, including its repeated/end-knot rules.

    Port of resource-porter skeleton_animation::Channel::span_coefficients,
    itself transcribed from Vostok poly_curve_inline.h get_spline_params.
    The final control point (-1e9 in retail clips) is never sampled.
    """
    knots, points = np.asarray(pairs).T
    spans = [i for i in range(3, len(knots) - 1) if knots[i + 1] - knots[i] >= 1e-7]
    if not spans:
        raise ValueError("animation channel has no nonempty cubic span")
    time = np.clip(time, knots[spans[0]], knots[-1])
    i = next((i for i in reversed(spans) if knots[i] <= time), spans[0])

    def knot(j):
        return knots[j] if 0 <= j < len(knots) else knots[-1]

    dm1, d0, d1, d2, d3 = [knot(j + 1) - knot(j) for j in range(i - 2, i + 3)]
    k0 = 1 / (d1 * (d1 + d0) * (d1 + d0 + dm1))
    k11 = 1 / (d1 * (d1 + d0) * (d1 + d0 + d2))
    k12 = 1 / (d1 * (d1 + d2) * (d1 + d0 + d2))
    k3 = 1 / (d1 * (d1 + d2) * (d1 + d2 + d3))
    factors = np.array(
        [
            [
                0,
                k11 * d1 * d0 * d0,
                k0 * d1 * d1 * (dm1 + d0) + k11 * d1 * d0 * (d1 + d2),
                k0 * d1**3,
            ],
            [
                0,
                k11 * (2 * d0 * d1 - d0 * d0) + k12 * d0 * (d1 + d2),
                k0 * (d1 * d1 - 2 * d1 * (dm1 + d0))
                + k11 * (-d0 * d1 + (d1 - d0) * (d1 + d2))
                + k12 * (d1 + d2) ** 2,
                -k0 * 3 * d1 * d1,
            ],
            [
                0,
                k11 * (d1 - 2 * d0) + k12 * (d1 + d2 - d0) + k3 * (d1 + d2 + d3),
                k0 * (dm1 + d0 - 2 * d1)
                + k11 * (-2 * d1 - d2 + d0)
                - 2 * k12 * (d1 + d2),
                k0 * 3 * d1,
            ],
            [k3, -k11 - k12 - k3, k0 + k11 + k12, -k0],
        ]
    )
    c = factors @ points[[i, i - 1, i - 2, i - 3]]
    t = time - knots[i]
    return c[0] + t * (c[1] + t * (c[2] + t * c[3]))


def clip(root, name, time):
    path = root / resource(name, ".b-spline")
    names = config(root / resource(name, ".bones_names"))["bones_names"]
    data = path.read_bytes()
    bones, events, _ = ce.CLIP_HEAD.unpack_from(data)
    if bones != len(names):
        raise ValueError(f"{path}: bone count mismatch")
    at = ce.CLIP_HEAD.size + bones * ce.CLIP_BONE + events * ce.CLIP_EVENT
    out = {}
    for name in names:
        values = []
        for _ in range(9):
            n = struct.unpack_from("<I", data, at)[0]
            pairs = np.frombuffer(data, "<f4", n * 2, at + 4).reshape(n, 2)
            values.append(curve(pairs, time))
            at += 4 + n * 8
        out[name] = matrix(values[:3], values[3:6], values[6:])
    return out


@dataclass
class Bone:
    name: str
    rest: np.ndarray
    pose: np.ndarray


def bones(root, directory, animation, time):
    data = (directory / "render/bind_pose").read_bytes()
    rest, at = {}, 2
    for _ in range(struct.unpack_from("<H", data)[0]):
        end = data.index(0, at)
        name = data[at:end].decode()
        values = struct.unpack_from("<9f", data, end + 1)
        rest[name] = matrix(values[:3], values[3:6], values[6:])
        at = end + 37
    if at != len(data):
        raise ValueError(f"{directory}: trailing bind-pose bytes")
    skeleton = config(directory / "skeleton")["skeleton"]
    parents = {}

    def descend(node, parent=None):
        for name, child in node.items():
            if name != "mask" and isinstance(child, dict):
                parents[name] = parent
                descend(child, name)

    descend(config(root / resource(skeleton, ".skeleton")))
    local = clip(root, animation, time)
    posed, visiting = {}, set()

    def resolve(name):
        if name in posed:
            return posed[name]
        if name in visiting:
            raise ValueError("cyclic skeleton")
        visiting.add(name)
        parent = parents[name]
        parent = parent if parent in rest else None
        inherited = resolve(parent) if parent else np.eye(4)
        default = rest[name] @ np.linalg.inv(rest[parent]) if parent else rest[name]
        posed[name] = local.get(name, default) @ inherited
        visiting.remove(name)
        return posed[name]

    return [Bone(n, m, resolve(n)) for n, m in rest.items()]


@dataclass
class Mesh:
    name: str
    points: np.ndarray
    triangles: np.ndarray
    uv: np.ndarray
    material: str


def meshes(directory, rig=None):
    settings = config(directory / "settings")
    out = []
    for name in settings["lod_hierrarchy"]["LOD0"]["surfaces"]:
        data = (directory / "render" / name / "converted_model").read_bytes()
        kind, size, n = struct.unpack_from("<3I", data)
        if kind != 3 or n == 0 or (size - 4) % n:
            raise ValueError(f"{directory}/{name}: bad vertices")
        stride = (size - 4) // n
        raw = np.frombuffer(data, np.uint8, n * stride, 12).reshape(n, stride)
        p = raw[:, :12].copy().view("<f4").reshape(n, 3).astype(float)
        if rig is not None:
            if stride not in (36, 40, 44, 48):
                raise ValueError(
                    f"{directory}/{name}: unsupported skin stride {stride}"
                )
            count = (stride - 36) // 4 + 1
            ids = raw[:, 12 : 12 + count]
            w = np.ones((n, count))
            if count > 1:
                w[:, :-1] = (
                    raw[:, 16 : 16 + 4 * (count - 1)]
                    .copy()
                    .view("<f4")
                    .reshape(n, count - 1)
                )
                w[:, -1] = 1 - w[:, :-1].sum(axis=1)
            if ids.max() >= len(rig):
                raise ValueError(f"{directory}/{name}: bad bone index")
            posed = np.zeros_like(p)
            for i, bone in enumerate(rig):
                weight = np.where(ids == i, w, 0).sum(axis=1)
                if weight.any():
                    posed += (
                        transform(p, np.linalg.inv(bone.rest) @ bone.pose)
                        * weight[:, None]
                    )
            p = posed
            uv = raw[:, -8:].copy().view("<f4").reshape(n, 2).astype(float)
        elif stride == 28:
            uv = raw[:, -4:].copy().view("<f2").reshape(n, 2).astype(float)
        else:
            raise ValueError(f"{directory}/{name}: unsupported static stride {stride}")
        at = 8 + size
        kind, size, count = struct.unpack_from("<3I", data, at)
        if kind != 4 or count % 3 or size != 4 + 2 * count:
            raise ValueError("bad triangle chunk")
        ix = np.frombuffer(data, "<u2", count, at + 12).reshape(-1, 3).copy()
        if ix.max() >= n:
            raise ValueError("triangle outside mesh")
        out.append(
            Mesh(name, p, ix, uv, settings["material_settings"][name]["material_name"])
        )
    return out


def model_dir(root, name):
    for suffix in (".skinned_model", ".model"):
        path = root / "models" / (name + suffix)
        if path.is_dir():
            return path
    raise FileNotFoundError(name)


@dataclass(frozen=True)
class Locator:
    position: tuple[float, float, float]
    rotation: tuple[float, float, float]
    bone: int | None

    @classmethod
    def parse(cls, raw):
        def vector(key, default=None):
            value = raw.get(key, default)
            if (
                value is None
                or len(value) != 3
                or not all(math.isfinite(v) for v in value)
            ):
                raise ValueError(f"locator {key}: expected three finite numbers")
            return tuple(value)

        # Retail stores a u16 index; bone_name is editor metadata only.
        index = raw.get("bone_idx", 65535)
        if not isinstance(index, int) or not -1 <= index <= 65535:
            raise ValueError("locator bone_idx: outside u16 range")
        return cls(
            vector("position"),
            vector("rotation", (0.0, 0.0, 0.0)),
            None if index in (-1, 65535) else index,
        )


def locator(directory, name):
    found = [
        l
        for l in config(directory / "render/export_properties")["locators"]
        if l["name"] == name
    ]
    if len(found) != 1:
        raise ValueError(f"{directory}: expected one {name} locator, got {len(found)}")
    return Locator.parse(found[0])


def locator_matrix(loc, rig):
    m = matrix(loc.position, loc.rotation)
    if loc.bone is not None:
        # Retail load_properties consumes bone_idx, not the editor's bone_name.
        b = rig[loc.bone]
        m = m @ b.pose
    return m


def weapon(root, cfg, time=0):
    directory = model_dir(root, cfg["object"]["model"])
    animation = (
        cfg["states"]
        .get("aimed_idle", {})
        .get("animations", cfg["states"]["idle"]["animations"])[0]
    )
    rig = bones(root, directory, animation, time)
    aim = locator(directory, "aim")
    # calculate_aim_alignment 0x244D40 -> calculate_locator 0x243BC0:
    # translate(0,0,aim_offset) * rotation_y(pi) * locator * bone.
    camera = (
        matrix((0, 0, cfg["parameters"].get("aim_offset", 0)))
        @ HALF
        @ locator_matrix(aim, rig)
    )
    view = np.linalg.inv(camera) @ TO_VIEW
    return directory, rig, aim, view, meshes(directory, rig)

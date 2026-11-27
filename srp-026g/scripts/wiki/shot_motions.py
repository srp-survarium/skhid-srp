"""A weapon's two fixed motions on a shot, from its own clips, for the wiki's simulators.

    python -I shot_motions.py <raw> <json> <out.json>

<raw> holds the game's files ('/' as '__', as released.extract writes them), <json> the
configs dump_json.py decoded. For every weapon config with a fire state, out.json has, under the
config's json file name:

    camera  what the hands' shot clip does to the camera (the bone `Item`, the eyes and where a
            bullet starts): {"hip": curve, "aimed": curve} from `states.fire.user_animations[0]`
            and `[4]`, a curve {"up": [...], "side": [...]} in degrees; no "aimed" where it is
            the hip's;
    gun     what the gun's own shot clip does to its body (the bone `offset`, which the mesh and
            the sights hang on) in front of the camera, as the aimed view shows it since patch
            172: {"back": [...] mm toward the eye, "up": [...], "side": [...] degrees}.

Each list is the motion from the clip's first frame at STEPS + 1 even moments of one pass of the
clip, which a shot plays in 60 / rounds_per_minute seconds whatever its frames (the fire state's
speed, 0x30E5C0). A motion that stays under STILL is left out: the hands of most automatic guns
move nothing of the camera.

The angles are the bones' own (Euler x, y about their parents): up is -x and side -y. For the
camera, `on_site_camera_look` turns `Item` +67.5 degrees of x looking down; for the gun, the body
carried in the aim locator's frame (calculate_aim_alignment 0x244D40) turns up by -x and to the
camera's right by -y, and slides back by +z, on the six guns checked (HK416, L85A2, AK-74M, SVD,
Beretta 93R, Mosin: within 0.01 degrees while the clip's roll is small). The clips start
within a degree of level (the Winchester 1.1 degrees off); a gun whose `offset` starts LEVEL or
more off, where those axes would no longer be the camera's, is left out.
"""
import json
import math
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "sight-alignment"))
import model  # noqa: E402  (the retail B-spline and the clip's header)

STEPS = 32
STILL = {"deg": 0.02, "mm": 0.05}
LEVEL = math.radians(5)  # an `offset` turned more than this at the clip's start is not read (its axes are not the camera's)


def raw_path(raw, resource, suffix):
    return Path(raw) / (resource.removeprefix("resources/").replace("/", "__") + suffix)


def tracks(raw, resource):
    """{bone: nine channels of (knot, value) pairs} of a clip, or None when the game has no such clip."""
    clip, names = raw_path(raw, resource, ".b-spline"), raw_path(raw, resource, ".bones_names")
    if not clip.is_file() or not names.is_file():
        return None
    data, bones = clip.read_bytes(), model.config(names)["bones_names"]
    count, events, _ = model.ce.CLIP_HEAD.unpack_from(data)
    if count != len(bones):
        raise ValueError(f"{clip}: {count} bones, {len(bones)} names")
    at, out = model.ce.CLIP_HEAD.size + count * model.ce.CLIP_BONE + events * model.ce.CLIP_EVENT, {}
    for bone in bones:
        channels = []
        for _ in range(9):
            n = struct.unpack_from("<I", data, at)[0]
            channels.append([struct.unpack_from("<ff", data, at + 4 + 8 * i) for i in range(n)])
            at += 4 + 8 * n
        out[bone] = channels
    return out


def sampled(channel, scale):
    """The channel's change from the clip's start at STEPS + 1 even moments, times `scale`."""
    end = channel[-1][0]
    values = [float(model.curve(channel, end * i / STEPS)) for i in range(STEPS + 1)]
    return [round((value - values[0]) * scale, 3) for value in values]


def moving(curve, units):
    return any(abs(value) >= STILL[units[key]] for key, values in curve.items() for value in values)


def camera(raw, resource):
    bones = tracks(raw, resource)
    if not bones or "Item" not in bones:
        return None
    item = bones["Item"]
    curve = {"up": sampled(item[3], -math.degrees(1)), "side": sampled(item[4], -math.degrees(1))}
    return curve if moving(curve, {"up": "deg", "side": "deg"}) else None


def gun(raw, resource):
    bones = tracks(raw, resource)
    if not bones or "offset" not in bones:
        return None
    offset = bones["offset"]
    if any(abs(float(model.curve(offset[axis], 0))) > LEVEL for axis in (3, 4, 5)):
        return None
    curve = {"back": sampled(offset[2], 1000), "up": sampled(offset[3], -math.degrees(1)), "side": sampled(offset[4], -math.degrees(1))}
    return curve if moving(curve, {"back": "mm", "up": "deg", "side": "deg"}) else None


def motions(raw, cfg):
    """A weapon config's motions: {"camera": {"hip", "aimed"}, "gun"}, each present only when it moves."""
    fire = cfg.get("states", {}).get("fire", {})
    hands, item = fire.get("user_animations") or [], fire.get("animations") or []
    views = {view: camera(raw, hands[slot]) for view, slot in (("hip", 0), ("aimed", 4)) if slot < len(hands)}
    # The aimed clip is nearly always the hip's: written only where it differs.
    shown = {view: curve for view, curve in views.items() if curve and (view == "hip" or curve != views.get("hip"))}
    out = {"camera": shown, "gun": gun(raw, item[0]) if item else None}
    return {key: value for key, value in out.items() if value}


def main():
    raw, source, target = sys.argv[1:4]
    out = {}
    for file in sorted(Path(source).glob("gameplay__items__weapons__*.options.json")):
        found = motions(raw, json.loads(file.read_text(encoding="utf-8")))
        if found:
            out[file.name] = found
    Path(target).write_text(json.dumps(out, separators=(",", ":")))
    print(f"{target}: {len(out)} weapons with a shot motion, {sum('camera' in m for m in out.values())} of them moving the camera")


if __name__ == "__main__":
    main()

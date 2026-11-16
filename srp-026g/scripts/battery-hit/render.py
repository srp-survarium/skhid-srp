"""A small software renderer for the offline previews (the artefact carry's
render_mesh.py way): flat-shaded, depth-tested triangles, a pinhole camera
with the game's vertical FOV (65 degrees, the stock projection 0x242900) and
near plane (0.05 m, update_camera 0x1F355E). Geometry whose cut by the near
plane is on screen is drawn red and counted: the game would show it cut open
(a triangle that passes the plane off screen is clipped unseen)."""
import math

import numpy as np
from PIL import Image, ImageDraw

import clips

NEAR = 0.05
FOV = 65.0
W, H = 480, 270


def look_at(eye, target, up=(0, 1, 0)):
    """Camera rows (right, up, forward) for a camera at `eye` looking at `target`."""
    f = np.array(target, float) - eye
    f /= np.linalg.norm(f)
    r = np.cross(np.array(up, float), f)
    r /= np.linalg.norm(r)
    u = np.cross(f, r)
    return np.array([r, u, f])


def cut_on_screen(v, near, focal, half):
    """Does the near plane's cut of the triangle v (eye space) touch the
    screen (`half`: its half width and height at `focal`)?"""
    ends = [v[a] + (v[b] - v[a]) * (near - v[a, 2]) / (v[b, 2] - v[a, 2])
            for a, b in ((0, 1), (1, 2), (2, 0)) if (v[a, 2] < near) != (v[b, 2] < near)]
    p, q = (end[:2] / near * focal for end in ends)
    lo, hi = 0.0, 1.0            # the part of p -> q inside the screen (Liang-Barsky)
    for at, step, limit in ((p[0], q[0] - p[0], half[0]), (p[1], q[1] - p[1], half[1])):
        for side in (1.0, -1.0):
            room, speed = limit - side * at, side * step
            if abs(speed) < 1e-12:
                if room < 0:
                    return False
            elif speed > 0:
                hi = min(hi, room / speed)
            else:
                lo = max(lo, room / speed)
    return lo <= hi


def draw(meshes, eye, axes, label='', fov=FOV, near=NEAR, size=(W, H)):
    width, height = size
    pixels = np.full((height, width, 3), [28, 34, 42], dtype=np.uint8)
    depth = np.full((height, width), np.inf)
    focal = height / (2 * math.tan(math.radians(fov) / 2))
    clipped = 0
    for vertices, triangles, color in meshes:
        v_all = (vertices - eye) @ np.array(axes).T
        for triangle in triangles:
            v = v_all[triangle]
            z = v[:, 2]
            if max(z) <= 0:
                continue
            hot = min(z) < near
            if hot:
                # in view at all? (a vertex in front inside the frustum's sides)
                front = v[z > 1e-3]
                s = front[:, :2] / front[:, 2:3] * focal
                if not (len(front) and np.any((np.abs(s[:, 0]) < width / 2) & (np.abs(s[:, 1]) < height / 2))):
                    continue
                if cut_on_screen(v, near, focal, (width / 2, height / 2)):
                    clipped += 1
                    col = np.array([255, 30, 30])
                else:
                    col = np.array(color)
                v = v.copy()
                v[:, 2] = np.maximum(v[:, 2], near)
                z = v[:, 2]
            else:
                col = np.array(color)
            screen = v[:, :2] / z[:, None] * [focal, -focal] + [width / 2, height / 2]
            lo = np.maximum(np.floor(screen.min(axis=0)).astype(int), 0)
            hi = np.minimum(np.ceil(screen.max(axis=0)).astype(int), [width - 1, height - 1])
            if np.any(lo > hi):
                continue
            x, y = np.meshgrid(np.arange(lo[0], hi[0] + 1) + .5, np.arange(lo[1], hi[1] + 1) + .5)
            a, b, c = screen
            den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
            if abs(den) < 1e-9:
                continue
            u = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / den
            t = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / den
            q = 1 - u - t
            inv = u / z[0] + t / z[1] + q / z[2]
            d = np.divide(1, inv, out=np.full_like(inv, np.inf), where=inv > 0)
            region = depth[lo[1]:hi[1] + 1, lo[0]:hi[0] + 1]
            inside = (u >= 0) & (t >= 0) & (q >= 0) & (d < region)
            normal = np.cross(v[1] - v[0], v[2] - v[0])
            normal /= max(np.linalg.norm(normal), 1e-9)
            light = .35 + .65 * abs(np.dot(normal, [.2, .5, -.84]))
            pixels[lo[1]:hi[1] + 1, lo[0]:hi[0] + 1][inside] = np.clip(col * light, 0, 255)
            region[inside] = d[inside]
    image = Image.fromarray(pixels)
    if label:
        ImageDraw.Draw(image).text((6, 4), label, fill=(240, 240, 240))
    return image, clipped


def fp_camera(world):
    """The first-person camera: the Item bone. computed_head_transform
    (0x2DA4B0) stores the bone "Item" (0x9DE38C) of the player's animation
    times his transform (+40Ch) as the eye (+44Ch); update_camera (0x1F3480,
    the player's own view: camera mode +358h 0 or 4) hands +44Ch to the game
    camera (0x2422B0) unchanged and sets the near plane +8Ch to 0.05 (0x9CC878)
    times the zoom (1 unaimed; a carrier aims nothing); the projection
    (0x242900, patch 22) is uro_fov degrees vertical, 65.0 by default and in
    the owner's user.cfg. Item is a child of Root: the carry's body moves do
    not move the view, the look pitch does (Rig.world's look)."""
    r, p = world['Item']
    return p, r


def penetration(battery_vertices, body_vertices, margin=0.0):
    """Body vertices inside the battery's box (its own axes from its vertices' PCA)."""
    c = battery_vertices.mean(axis=0)
    u, s, vt = np.linalg.svd(battery_vertices - c, full_matrices=False)
    local = (battery_vertices - c) @ vt.T
    lo, hi = local.min(axis=0) - margin, local.max(axis=0) + margin
    b = (body_vertices - c) @ vt.T
    return int(np.sum(np.all((b > lo) & (b < hi), axis=1)))

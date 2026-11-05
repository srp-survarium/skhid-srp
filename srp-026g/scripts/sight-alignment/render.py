"""Small depth-buffered preview renderer; lighting is illustrative, geometry is measured."""

import numpy as np
from PIL import Image
from model import config, transform


def texture(roots, material):
    for root in roots:
        path = root / "material_instances" / (material + ".material")
        if path.exists():
            break
    else:
        return np.full((1, 1, 4), 255, np.uint8)
    cfg = config(path)["material"]
    for stage in ("g_stage", "forward"):
        effect = cfg.get(stage, {}).get("effect", {})
        name = effect.get("texture_diffuse", {}).get("value")
        if name:
            for root in roots:
                path = root / "textures" / (name + ".dds")
                if path.exists():
                    im = Image.open(path).convert("RGBA")
                    im.thumbnail((1024, 1024))
                    return np.asarray(im)
    return np.full((1, 1, 4), 255, np.uint8)


def render(meshes, view, size, focal, roots=(), center=None):
    width, height = size
    cx, cy = center or (width / 2, height / 2)
    rgb = np.full((height, width, 3), (60, 65, 70), np.uint8)
    depth = np.full((height, width), np.inf)
    for mesh in meshes:
        p = transform(mesh.points, view)
        tex = (
            texture(roots, mesh.material)
            if roots
            else np.full((1, 1, 4), (160, 160, 160, 255), np.uint8)
        )
        z = -p[:, 2]
        with np.errstate(divide="ignore", invalid="ignore"):
            xy = np.column_stack([cx + focal * p[:, 0] / z, cy - focal * p[:, 1] / z])
        for ix in mesh.triangles:
            if (z[ix] < 0.002).any():
                continue
            a, b, c = xy[ix]
            lo = np.maximum(np.floor(xy[ix].min(0)), (0, 0)).astype(int)
            hi = np.minimum(np.ceil(xy[ix].max(0)), (width - 1, height - 1)).astype(int)
            if (lo > hi).any():
                continue
            den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
            if abs(den) < 1e-9:
                continue
            yy, xx = np.mgrid[lo[1] : hi[1] + 1, lo[0] : hi[0] + 1] + 0.5
            u = ((b[1] - c[1]) * (xx - c[0]) + (c[0] - b[0]) * (yy - c[1])) / den
            v = ((c[1] - a[1]) * (xx - c[0]) + (a[0] - c[0]) * (yy - c[1])) / den
            weights = np.stack([u, v, 1 - u - v], -1)
            reciprocal = (weights / z[ix]).sum(-1)
            valid = (weights >= -1e-8).all(-1) & (reciprocal > 0)
            dz = 1 / np.maximum(reciprocal, 1e-20)
            sl = np.s_[lo[1] : hi[1] + 1, lo[0] : hi[0] + 1]
            valid &= dz < depth[sl]
            if not valid.any():
                continue
            uv = (weights / z[ix]) @ mesh.uv[ix] * dz[:, :, None]
            tx = (
                (np.mod(uv[:, :, 0], 1) * tex.shape[1])
                .astype(int)
                .clip(0, tex.shape[1] - 1)
            )
            ty = (
                (np.mod(uv[:, :, 1], 1) * tex.shape[0])
                .astype(int)
                .clip(0, tex.shape[0] - 1)
            )
            sample = tex[ty, tx]
            valid &= sample[:, :, 3] > 127
            normal = np.cross(p[ix[1]] - p[ix[0]], p[ix[2]] - p[ix[0]])
            normal /= max(np.linalg.norm(normal), 1e-20)
            shade = 0.55 + 0.25 * abs(normal[1]) + 0.20 * abs(normal[2])
            rgb[sl][valid] = (
                (sample[:, :, :3] * shade).clip(0, 255).astype(np.uint8)[valid]
            )
            depth[sl][valid] = dz[valid]
    return Image.fromarray(rgb), depth

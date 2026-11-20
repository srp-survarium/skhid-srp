"""Transfer donor mounts through a measured rigid correspondence of weapon meshes."""
from dataclasses import dataclass
from pathlib import Path
import sys
import shutil
import struct

import numpy as np
from scipy.spatial import cKDTree

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'rifle-scopes'))
import scopes_lib as lib


def points(directory):
    return np.unique(np.concatenate([lib.positions(directory / 'render' / surface / 'converted_model')
                                    for surface in lib.surfaces(directory)]), axis=0)


@dataclass(frozen=True)
class Frame:
    rotation: np.ndarray
    translation: np.ndarray
    matched: float
    error: float

    def locator(self, position):
        half = np.array([-1., 1., -1.])
        return ((np.asarray(position) * half) @ self.rotation + self.translation) * half


def fit_frame(live, donor):
    """Robust rigid ICP; changed magazines/handguards must not pull the receiver.

    A donor point counts as evidence only when a live vertex is within 0.2 mm.
    Reject weak matches instead of silently assuming identical model origins.
    """
    target, source = points(live), points(donor)
    tree = cKDTree(target)
    sample = source[::max(1, len(source) // 2000)]

    def score(rotation, translation):
        distances, _ = tree.query(source @ rotation + translation)
        matched = distances < .0002
        return float(matched.mean()), float(np.sqrt(np.mean(distances[matched] ** 2))) if matched.any() else float('inf')

    identity = np.eye(3)
    fraction, error = score(identity, np.zeros(3))
    if fraction >= .7:
        return Frame(identity, np.zeros(3), fraction, error)
    seeds = [np.zeros(3), np.median(target, axis=0) - np.median(source, axis=0),
             target.min(0) - source.min(0), target.max(0) - source.max(0)]
    best = Frame(identity, np.zeros(3), fraction, error)
    for seed in seeds:
        rotation, translation = np.eye(3), seed.copy()
        for _ in range(70):
            placed = sample @ rotation + translation
            distance, index = tree.query(placed)
            keep = distance <= np.quantile(distance, .6)
            a, b = sample[keep], target[index[keep]]
            ac, bc = a.mean(0), b.mean(0)
            u, _, vt = np.linalg.svd((a - ac).T @ (b - bc))
            sign = np.diag([1., 1., np.linalg.det(u @ vt)])
            next_rotation = u @ sign @ vt
            next_translation = bc - ac @ next_rotation
            change = np.max(np.abs(next_rotation - rotation)) + np.max(np.abs(next_translation - translation))
            rotation, translation = next_rotation, next_translation
            if change < 1e-10:
                break
        fraction, error = score(rotation, translation)
        if (fraction, -error) > (best.matched, -best.error):
            best = Frame(rotation, translation, fraction, error)
    if best.matched < .5:
        raise ValueError(f'{live.name} vs {donor.name}: only {best.matched:.1%} of donor vertices match within 0.2 mm')
    return best


def rotated_static(source, rotation, out):
    """Rotate a mount's positions and shading basis into the optic's local frame.

    Some barrel brackets expose an upright optic socket from a rotated root.
    Translating such a bracket alone turns the mount upside down.
    """
    np.testing.assert_allclose(rotation @ rotation.T, np.eye(3), atol=1e-6)
    if abs(np.linalg.det(rotation) - 1) > 1e-6:
        raise ValueError('mount rotation reflects or scales geometry')
    shutil.copytree(source, out)
    for mesh in out.glob('render/*/converted_model'):
        data = bytearray(lib.static_026(mesh.read_bytes()))
        _, _, n = struct.unpack_from('<3I', data)
        raw = np.frombuffer(data, np.uint8, n * 28, 12).reshape(n, 28).copy()
        position = raw[:, :12].copy().view('<f4').reshape(n, 3) @ rotation
        raw[:, :12] = position.astype('<f4').view(np.uint8).reshape(n, 12)
        for i, basis in enumerate(lib.basis(raw)):
            value = np.clip(np.round(((basis @ rotation) + 1) * 127.5), 0, 255).astype(np.uint8)
            raw[:, 12 + i * 4:15 + i * 4] = value[:, ::-1]
        data[12:12 + 28 * n] = raw.tobytes()
        mesh.write_bytes(data)
    return out

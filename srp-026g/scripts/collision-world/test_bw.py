"""Tests of the capture format and the walk on a made-up memory image (no game needed).

    python -m unittest discover -s srp-026g/scripts/collision-world -p 'test_*.py'
"""
import os
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bw_walk  # noqa: E402
import bwmem  # noqa: E402


class FakeMem(bwmem.Mem):
    def __init__(self):
        self.pages = {}

    def put(self, addr, data):
        self.pages[addr] = bytearray(data)

    def read(self, addr, n):
        for base, data in self.pages.items():
            if base <= addr and addr + n <= base + len(data):
                return bytes(data[addr - base:addr - base + n])
        return None


def block(size, fields):
    data = bytearray(size)
    for offset, fmt, *values in fields:
        struct.pack_into("<" + fmt, data, offset, *values)
    return data


IDENTITY = (1.0, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, 1.0, 0)


def image():
    """A world of two static bodies: a scaled box, and a mesh of two triangles under a compound with a cylinder."""
    mem = FakeMem()
    world, dyn, array = 0x100000, 0x101000, 0x102000
    obj_a, obj_b = 0x110000, 0x111000
    box, compound, cylinder, mesh = 0x120000, 0x121000, 0x122000, 0x123000
    children, interface, parts = 0x124000, 0x125000, 0x126000
    verts, indices, faces = 0x127000, 0x128000, 0x129000
    wrap_a, wrap_b, shapes_faces = 0x130000, 0x131000, 0x132000
    proxy_a, proxy_b, user = 0x133000, 0x134000, 0x135000
    mem.put(world, block(0x80, [(0, "I", bw_walk.VT_PHYSICS_WORLD), (0x3C, "I", dyn),
                                (0x48, "3f", -1, -2, -3), (0x54, "3f", 4, 5, 6)]))
    mem.put(dyn, block(0x400, [(0, "I", 0x9E45E4), (0x08, "i", 2), (0x0C, "i", 2), (0x10, "I", array)]))
    mem.put(array, struct.pack("<2I", obj_a, obj_b))
    for obj, shape, proxy, origin in ((obj_a, box, proxy_a, (1.0, 2.0, 3.0)), (obj_b, compound, proxy_b, (0, 0, 0))):
        mem.put(obj, block(bw_walk.OBJ_BYTES, [
            (0, "I", 0x9E40B8), (0x10, "12f", *IDENTITY), (0x40, "3f", *origin), (0xC8, "I", proxy),
            (0xCC, "I", shape), (0xD4, "I", shape), (0xD8, "I", 1), (0xEC, "f", 0.5), (0xF4, "I", 2),
            (0xF8, "I", user)]))
    mem.put(proxy_a, block(0x40, [(4, "HH", 2, 4), (0x10, "3f", 0, 0, 0), (0x20, "3f", 1, 1, 1)]))
    mem.put(proxy_b, block(0x40, [(4, "HH", 10, 20), (0x10, "3f", 0, 0, 0), (0x20, "3f", 1, 1, 1)]))
    mem.put(user, block(bw_walk.USER_BYTES, [(0, "I", 0xA1F258)]))
    mem.put(box, block(0x80, [(0, "I", 0x9E4AFC), (4, "I", 0), (8, "I", wrap_a), (0x10, "4f", 2, 1, 1, 0),
                              (0x20, "4f", 1.96, 0.96, 0.96, 0), (0x30, "f", 0.04)]))
    mem.put(compound, block(0x80, [(0, "I", 0x9E405C), (4, "I", 31), (8, "I", wrap_b), (0x10, "i", 2),
                                   (0x18, "I", children), (0x48, "f", 0.0), (0x50, "4f", 1, 1, 1, 0)]))
    mem.put(children, block(0xA0, [(0, "12f", *IDENTITY), (0x30, "3f", 0, 1, 0), (0x40, "I", cylinder), (0x44, "I", 13),
                                   (0x50, "12f", *IDENTITY), (0x90, "I", mesh), (0x94, "I", 21)]))
    mem.put(cylinder, block(0x80, [(0, "I", 0x9E4B9C), (4, "I", 13), (0x10, "4f", 1, 1, 1, 0),
                                   (0x20, "4f", 0.46, 0.96, 0.46, 0), (0x30, "f", 0.04), (0x40, "I", 1)]))
    mem.put(mesh, block(0x80, [(0, "I", bw_walk.VT_MESH_SHAPE), (4, "I", 21), (0x30, "I", interface),
                               (0x60, "I", faces)]))
    mem.put(interface, block(0x60, [(0, "I", bw_walk.VT_MESH_INTERFACE), (0x10, "4f", 1, 1, -1, 0), (0x24, "i", 1),
                                    (0x2C, "I", parts)]))
    mem.put(parts, struct.pack("<8I", 2, indices, 12, 4, verts, 12, 2, 0))
    mem.put(verts, struct.pack("<12f", 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 1))
    mem.put(indices, struct.pack("<6I", 0, 1, 2, 0, 2, 3))
    mem.put(faces, struct.pack("<2H", (1 << 14) | 67, 0xFFFF))
    mem.put(wrap_a, block(bw_walk.WRAP_BYTES, [(0, "I", 0x9E31EC), (0x108, "I", box), (0x110, "I", shapes_faces)]))
    mem.put(wrap_b, block(bw_walk.WRAP_BYTES, [(0, "I", 0x9E31EC), (0x108, "I", compound), (0x10C, "I", faces),
                                               (0x110, "I", shapes_faces)]))
    mem.put(shapes_faces, block(0x400, [(0, "H", 18)]))
    return mem, world


class WalkTest(unittest.TestCase):
    def check(self, walker, world):
        a, b = world["objects"]
        self.assertEqual((a["group"], a["mask"]), (2, 4))
        self.assertEqual(a["transform"]["origin"], [1.0, 2.0, 3.0])
        box = walker.shapes[a["shape"]]
        self.assertEqual(box["kind"], "box")
        self.assertAlmostEqual(box["margin"], 0.04, places=6)
        compound = walker.shapes[b["shape"]]
        self.assertEqual([walker.shapes[c["shape"]]["kind"] for c in compound["children"]],
                         ["cylinder", "triangle_mesh"])
        self.assertEqual(walker.shapes[compound["children"][0]["shape"]]["up_axis"], 1)
        mesh = walker.shapes[compound["children"][1]["shape"]]
        part = walker.meshes[mesh["mesh"]]["parts"][0]
        self.assertEqual((part["triangles"], part["vertices"], part["index_stride"]), (2, 4, 12))
        self.assertEqual(walker.meshes[mesh["mesh"]]["scaling"][:3], [1.0, 1.0, -1.0])
        self.assertEqual(walker.problems, [])

    def test_walk_and_capture_round_trip(self):
        mem, world_addr = image()
        recorder = bwmem.Recorder(mem)
        walker = bw_walk.Walker(recorder)
        self.check(walker, walker.world(world_addr))
        with tempfile.TemporaryDirectory() as folder:
            path = os.path.join(folder, "world.bwc")
            recorder.save(path, {"module_base": 0x10000, "world": world_addr})
            capture = bwmem.Capture(path)
            self.assertEqual(capture.meta["world"], world_addr)
            again = bw_walk.Walker(capture, chase=False)
            self.check(again, again.world(world_addr))
            # the mesh's own bytes are in the capture
            self.assertEqual(capture.need(0x129000, 4), struct.pack("<2H", (1 << 14) | 67, 0xFFFF))

    def test_ranges_merge(self):
        mem = FakeMem()
        mem.put(0x1000, bytes(range(64)))
        recorder = bwmem.Recorder(mem)
        recorder.read(0x1000, 16)
        recorder.read(0x1008, 24)
        recorder.read(0x1030, 8)
        self.assertEqual([(a, len(d)) for a, d in recorder.merged()], [(0x1000, 32), (0x1030, 8)])


class SupportTest(unittest.TestCase):
    def test_bullet_shapes(self):
        try:
            import numpy as np
            import bw_diff
        except ImportError:
            self.skipTest("numpy is not installed")
        d = np.array([[1.0, 0, 0], [0, 1.0, 0], [0.6, 0.8, 0]])
        box = {"kind": "box", "implicit_dims": [1.96, 0.96, 0.96, 0], "margin": 0.04}
        # faces are exact, a rounded edge is short of the sharp corner
        self.assertTrue(np.allclose(bw_diff.support_local(box, d)[:2], [2.0, 1.0]))
        self.assertLess(bw_diff.support_local(box, d)[2], 0.6 * 2.0 + 0.8 * 1.0)
        cylinder = {"kind": "cylinder", "implicit_dims": [0.46, 0.96, 0.46, 0], "margin": 0.04, "up_axis": 1}
        self.assertTrue(np.allclose(bw_diff.support_local(cylinder, d)[:2], [0.5, 1.0]))
        capsule = {"kind": "capsule", "implicit_dims": [0.38, 0.52, 0.38, 0], "margin": 0.04, "up_axis": 1}
        self.assertTrue(np.allclose(bw_diff.support_local(capsule, d)[:2], [0.38, 0.9]))
        sphere = {"kind": "sphere", "implicit_dims": [0.5, 0, 0, 0], "local_scaling": [2.0, 2.0, 2.0, 0], "margin": 0.5}
        self.assertTrue(np.allclose(bw_diff.support_local(sphere, d), [1.0, 1.0, 1.0]))


if __name__ == "__main__":
    unittest.main()

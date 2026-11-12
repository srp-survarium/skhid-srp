"""Walks the game's Bullet world in a memory source (bwmem) and returns it as plain data.

Layouts are the 0.26g exe's (build 2777), each from the exe (addresses are image VAs, base 0x10000;
`delta` = the module's load base - 0x10000). Where each was read: docs/notes/collision_world.md.

  bullet_physics_world            vtable 0x9E2EBC (ctor 0x74950); +0x3C btSoftRigidDynamicsWorld* (0x74E36)
  btCollisionWorld                +0x08 object count, +0x10 btCollisionObject** (btAlignedObjectArray at +4)
  btCollisionObject               +0x10 world transform (3 rows x 4 floats, origin 4 floats), +0xC8 broadphase
                                  proxy, +0xCC shape, +0xD4 root shape, +0xD8 flags, +0xF4 internal type,
                                  +0xF8 user object pointer (0x758F4, 0x75958, harness README)
  btBroadphaseProxy               +0x00 client object, +0x04 group (i16), +0x06 mask (i16), +0x10 aabb min, +0x20 max
  btCollisionShape                +0x04 shape type, +0x08 user pointer (the vostok bt_collision_shape, 0x7A05E)
  btConvexInternalShape           +0x10 local scaling, +0x20 implicit dimensions, +0x30 margin
  btCapsuleShape / btCylinderShape +0x40 up axis (btConvexInternalShape is 0x40 with its padding)
  btCompoundShape (0x60)          +0x10 child count, +0x18 children (0x50 each: transform, +0x40 shape,
                                  +0x44 type, +0x48 margin), +0x48 margin, +0x50 local scaling
  btBvhTriangleMeshShape (0x70)   +0x0C margin, +0x10 aabb min, +0x20 max, +0x30 mesh interface, +0x40 bvh,
                                  +0x60 face data (u16 a triangle) (ctor 0x79EE0)
  btTriangleIndexVertexArray      vtable 0x9E3248; +0x10 scaling, +0x24 mesh count, +0x2C meshes (0x20 each:
                                  +0 triangles, +4 index base, +8 index stride, +C vertices, +10 vertex base,
                                  +14 vertex stride, +18 index type, +1C vertex type) (0x7A6E0)
  vostok bt_collision_shape (0x118) +0x108 bt shape, +0x10C triangle face data, +0x110 shapes face data (0x7A000)
"""
import struct

VT_PHYSICS_WORLD = 0x9E2EBC
VT_MESH_INTERFACE = 0x9E3248
VT_MESH_SHAPE = 0x9E3284

SHAPE_NAMES = {
    0: "box", 1: "triangle", 4: "convex_hull", 8: "sphere", 9: "multi_sphere", 10: "capsule",
    11: "cone", 13: "cylinder", 14: "uniform_scaling", 21: "triangle_mesh",
    22: "scaled_triangle_mesh", 25: "gimpact", 27: "empty", 28: "static_plane", 31: "compound",
    32: "softbody",
}

OBJ_BYTES = 0x200
SHAPE_BYTES = 0x80
USER_BYTES = 0x200
WRAP_BYTES = 0x118


def looks_like_pointer(value):
    return 0x10000 <= value < 0xFFFF0000 and value % 4 == 0


class Walker:
    def __init__(self, mem, delta=0, chase=True):
        self.mem = mem
        self.delta = delta
        self.chase = chase
        self.shapes = {}
        self.meshes = {}
        self.wrappers = {}
        self.users = {}
        self.problems = []

    # -- helpers ---------------------------------------------------------------------------
    def touch(self, addr, size):
        """Reads a block so a Recorder keeps it; shrinks when the tail is unreadable."""
        while size > 0:
            data = self.mem.read(addr, size)
            if data is not None:
                return data
            if size < 16:
                break
            size //= 2
        return None

    def chase_pointers(self, data, size=0x100, depth=1):
        if not self.chase or data is None:
            return
        for (value,) in struct.iter_unpack("<I", data[: len(data) & ~3]):
            if looks_like_pointer(value):
                got = self.touch(value, size)
                if got is not None and depth > 1:
                    self.chase_pointers(got, size, depth - 1)

    def transform(self, addr):
        f = self.mem.f32s(addr, 16)
        return {"basis": [list(f[0:3]), list(f[4:7]), list(f[8:11])], "origin": list(f[12:15])}

    # -- the world -------------------------------------------------------------------------
    def world(self, world_addr):
        m = self.mem
        self.touch(world_addr, 0x80)
        dyn = m.u32(world_addr + 0x3C)
        self.touch(dyn, 0x400)
        count = m.i32(dyn + 0x08)
        data = m.u32(dyn + 0x10)
        if not (0 <= count <= 200000) or (count and not looks_like_pointer(data)):
            raise ValueError(f"world {world_addr:#x}: dynamics world {dyn:#x} has count {count}, data {data:#x}")
        ptrs = m.u32s(data, count) if count else ()
        objects = []
        for index, ptr in enumerate(ptrs):
            try:
                objects.append(self.collision_object(ptr, index))
            except MemoryError as error:
                self.problems.append(f"object {index} at {ptr:#x}: {error}")
        return {
            "world": world_addr,
            "dynamics_world": dyn,
            "dynamics_vtable": m.u32(dyn) - self.delta,
            "aabb_min": list(m.f32s(world_addr + 0x48, 3)),
            "aabb_max": list(m.f32s(world_addr + 0x54, 3)),
            "objects": objects,
        }

    def collision_object(self, addr, index):
        m = self.mem
        raw = self.touch(addr, OBJ_BYTES)
        if raw is None:
            raise MemoryError(f"unreadable {addr:#x}")
        proxy = m.u32(addr + 0xC8)
        shape = m.u32(addr + 0xCC)
        root = m.u32(addr + 0xD4)
        user = m.u32(addr + 0xF8)
        out = {
            "index": index,
            "addr": addr,
            "vtable": m.u32(addr) - self.delta,
            "transform": self.transform(addr + 0x10),
            "flags": m.u32(addr + 0xD8),
            "activation": m.u32(addr + 0xE4),
            "friction": m.f32(addr + 0xEC),
            "restitution": m.f32(addr + 0xF0),
            "internal_type": m.u32(addr + 0xF4),
            "user": user,
            "shape": shape,
            "root_shape": root,
        }
        if looks_like_pointer(proxy):
            self.touch(proxy, 0x40)
            out["group"], out["mask"] = struct.unpack("<HH", m.need(proxy + 4, 4))
            out["aabb_min"] = list(m.f32s(proxy + 0x10, 3))
            out["aabb_max"] = list(m.f32s(proxy + 0x20, 3))
        else:
            out["group"] = out["mask"] = None
        if looks_like_pointer(user) and user not in self.users:
            data = self.touch(user, USER_BYTES)
            self.users[user] = {"vtable": (m.u32(user) - self.delta) if data else None}
            self.chase_pointers(data, 0x100, 2)
        if looks_like_pointer(shape):
            self.shape(shape)
        if looks_like_pointer(root) and root != shape:
            self.shape(root)
        return out

    # -- shapes ----------------------------------------------------------------------------
    def shape(self, addr):
        if addr in self.shapes:
            return self.shapes[addr]
        m = self.mem
        raw = self.touch(addr, SHAPE_BYTES)
        if raw is None:
            self.problems.append(f"shape {addr:#x} unreadable")
            self.shapes[addr] = {"addr": addr, "kind": "unreadable"}
            return self.shapes[addr]
        kind = m.u32(addr + 4)
        wrapper = m.u32(addr + 8)
        out = {
            "addr": addr,
            "vtable": m.u32(addr) - self.delta,
            "type": kind,
            "kind": SHAPE_NAMES.get(kind, f"type_{kind}"),
            "wrapper": wrapper,
        }
        self.shapes[addr] = out
        if looks_like_pointer(wrapper):
            self.wrapper(wrapper)
        if kind in (0, 4, 8, 9, 10, 11, 13):
            out["local_scaling"] = list(m.f32s(addr + 0x10, 4))
            out["implicit_dims"] = list(m.f32s(addr + 0x20, 4))
            out["margin"] = m.f32(addr + 0x30)
            if kind in (10, 11, 13):
                out["up_axis"] = m.u32(addr + 0x40)
            if kind == 4:
                # btConvexHullShape: point array after the polyhedral base; kept raw for the offline parser
                self.chase_pointers(raw, 0x4000)
        elif kind == 31:
            count = m.i32(addr + 0x10)
            data = m.u32(addr + 0x18)
            out["margin"] = m.f32(addr + 0x48)
            out["local_scaling"] = list(m.f32s(addr + 0x50, 4))
            out["aabb_min"] = list(m.f32s(addr + 0x20, 3))
            out["aabb_max"] = list(m.f32s(addr + 0x30, 3))
            children = []
            if 0 < count <= 4096 and looks_like_pointer(data):
                self.touch(data, count * 0x50)
                for i in range(count):
                    child = data + i * 0x50
                    child_shape = m.u32(child + 0x40)
                    children.append({
                        "transform": self.transform(child),
                        "shape": child_shape,
                        "type": m.u32(child + 0x44),
                        "margin": m.f32(child + 0x48),
                    })
                    if looks_like_pointer(child_shape):
                        self.shape(child_shape)
            elif count:
                self.problems.append(f"compound {addr:#x}: count {count}, data {data:#x}")
            out["children"] = children
        elif kind == 21:
            out["margin"] = m.f32(addr + 0x0C)
            out["aabb_min"] = list(m.f32s(addr + 0x10, 3))
            out["aabb_max"] = list(m.f32s(addr + 0x20, 3))
            out["bvh"] = m.u32(addr + 0x40)
            out["face_data"] = m.u32(addr + 0x60)
            interface = m.u32(addr + 0x30)
            out["mesh_interface"] = interface
            if looks_like_pointer(interface):
                out["mesh"] = self.mesh_interface(interface, out["face_data"])
        elif kind == 22:
            # btScaledBvhTriangleMeshShape: +0x10 local scaling, +0x20 child (kept raw too)
            out["local_scaling"] = list(m.f32s(addr + 0x10, 4))
            child = m.u32(addr + 0x20)
            out["child"] = child
            if looks_like_pointer(child):
                self.shape(child)
        else:
            self.chase_pointers(raw, 0x100)
        return out

    def mesh_interface(self, addr, face_data):
        if addr in self.meshes:
            return addr
        m = self.mem
        self.touch(addr, 0x60)
        count = m.i32(addr + 0x24)
        data = m.u32(addr + 0x2C)
        out = {
            "addr": addr,
            "vtable": m.u32(addr) - self.delta,
            "scaling": list(m.f32s(addr + 0x10, 4)),
            "parts": [],
        }
        self.meshes[addr] = out
        if not (0 < count <= 64) or not looks_like_pointer(data):
            self.problems.append(f"mesh interface {addr:#x}: count {count}, data {data:#x}")
            return addr
        self.touch(data, count * 0x20)
        for i in range(count):
            part = data + i * 0x20
            tris, ibase, istride, verts, vbase, vstride, itype, vtype = m.u32s(part, 8)
            entry = {
                "triangles": tris, "index_base": ibase, "index_stride": istride,
                "vertices": verts, "vertex_base": vbase, "vertex_stride": vstride,
                "index_type": itype, "vertex_type": vtype,
            }
            out["parts"].append(entry)
            if tris > 20_000_000 or verts > 20_000_000 or istride > 64 or vstride > 64:
                self.problems.append(f"mesh part {part:#x}: {entry}")
                continue
            if self.touch(vbase, verts * vstride) is None:
                self.problems.append(f"mesh part {part:#x}: vertices unreadable")
            if self.touch(ibase, tris * istride) is None:
                self.problems.append(f"mesh part {part:#x}: indices unreadable")
            if i == 0 and looks_like_pointer(face_data):
                if self.touch(face_data, tris * 2) is None:
                    self.problems.append(f"mesh part {part:#x}: face data unreadable")
        return addr

    def wrapper(self, addr):
        if addr in self.wrappers:
            return
        m = self.mem
        raw = self.touch(addr, WRAP_BYTES)
        if raw is None:
            self.wrappers[addr] = None
            return
        out = {
            "addr": addr,
            "vtable": m.u32(addr) - self.delta,
            "bt_shape": m.u32(addr + 0x108),
            "tri_face_data": m.u32(addr + 0x10C),
            "shapes_face_data": m.u32(addr + 0x110),
        }
        self.wrappers[addr] = out
        if looks_like_pointer(out["shapes_face_data"]):
            self.touch(out["shapes_face_data"], 0x400)
        self.chase_pointers(raw, 0x100, 2)


def find_worlds(live, delta):
    """Addresses whose first dword is bullet_physics_world's vtable and whose +0x3C is a world."""
    worlds = []
    for addr in live.scan_u32(VT_PHYSICS_WORLD + delta):
        dyn = live.read(addr + 0x3C, 4)
        if dyn is None:
            continue
        (dyn,) = struct.unpack("<I", dyn)
        head = live.read(dyn, 0x18) if looks_like_pointer(dyn) else None
        if head is None:
            continue
        count, _cap, data = struct.unpack_from("<iiI", head, 8)
        if 0 <= count <= 200000:
            worlds.append((addr, dyn, count))
    return worlds

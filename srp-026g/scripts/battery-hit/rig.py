"""The carrier and his battery offline: the stock rig, skinned meshes and
clips out of resources.db (extracted by `extract.sh`), posed with the
engine's own spline basis (clips.py) and attached as the game attaches the
battery (compute_carried_transform 0x344E40: the battery skeleton's root at
Ry(pi) x the player's Weapon bone, which the stock carry clips put exactly
on both hands' contacts)."""
import math
import struct
from pathlib import Path

import numpy as np

import clips

CHAR = 'models_character_human_black_market_'
KEY = 'animations_single_weapons_key_objects_'
HANDS = KEY + 'player_nonadditive_stand_3rd_view_on_site_hands_only_'
ITEM = KEY + 'item_stand_3rd_view_'


def matrix(r, p):
    m = np.eye(4)
    m[:3, :3], m[3, :3] = r, p
    return m


def bind_pose(data):
    count, = struct.unpack_from('<H', data)
    offset, result = 2, {}
    for _ in range(count):
        end = data.index(0, offset)
        name = data[offset:end].decode()
        v = struct.unpack_from('<9f', data, end + 1)
        result[name] = matrix(np.diag(v[6:]) @ clips.rotation(v[3:6]), v[:3])
        offset = end + 37
    assert offset == len(data)
    return result


def mesh_data(data):
    """A skinned mesh: per vertex its position, four bone indices and as many
    weights as the stride holds (48: 3, 44: 2, 40: 1, 36: none; the last
    weight is the rest of 1), then 20 bytes this preview does not read."""
    kind, size, count = struct.unpack_from('<III', data)
    stride = (size - 4) // count
    extra = (stride - 36) // 4
    assert kind == 3 and stride in (36, 40, 44, 48)
    vertices = np.array([struct.unpack_from('<3f', data, 12 + i * stride) for i in range(count)])
    indices = np.array([struct.unpack_from('<4B', data, 24 + i * stride) for i in range(count)])
    weights = np.zeros((count, 4))
    for i in range(count):
        w = struct.unpack_from('<%df' % extra, data, 28 + i * stride) if extra else ()
        weights[i, :extra] = w
        weights[i, extra] = 1 - sum(w)
    kind, index_size, icount = struct.unpack_from('<III', data, 8 + size)
    assert kind == 4
    triangles = np.array(struct.unpack_from('<%dH' % icount, data, 20 + size)).reshape(-1, 3)
    return vertices, indices, weights, triangles


class Skinned:
    def __init__(self, bind, data, color):
        self.bind = bind
        self.names = list(bind)
        self.vertices, self.indices, self.weights, self.triangles = mesh_data(data)
        self.color = color

    def pose(self, world):
        frames = np.array([np.linalg.inv(self.bind[n]) @ matrix(*world[n]) for n in self.names])
        v = np.c_[self.vertices, np.ones(len(self.vertices))]
        return np.einsum('ni,nijk,nj->nk', self.weights, frames[self.indices], v)[:, :3]


class Rig:
    def __init__(self, assets):
        a = Path(assets)
        self.a = a
        self.parents = clips.skeleton_parents((a / 'animations_skeletons_scavengers_01.skeleton').read_bytes())
        self.item_parents = clips.skeleton_parents((a / 'animations_skeletons_weapons_electrobox.skeleton').read_bytes())
        self.names = clips.parse_names((a / (HANDS + 'idle.bones_names')).read_bytes())
        self.item_names = clips.parse_names((a / (ITEM + 'idle_electrobox.bones_names')).read_bytes())
        self.item_idle = self.clip(ITEM + 'idle_electrobox')
        part = lambda model, surface, color: Skinned(
            bind_pose((a / (CHAR + model + '.skinned_model_render_bind_pose')).read_bytes()),
            (a / (CHAR + model + '.skinned_model_render_' + surface + '_converted_model')).read_bytes(), color)
        self.hud = [part('black_market_01_hand_hud', 'black_market_01_hand_s', [170, 160, 140])]
        self.body = [part('black_market_01_torso_s1', 'black_market_01_torso_s', [91, 117, 86]),
                     part('black_market_01_legs_s1', 'black_market_01_legs_s', [80, 90, 70]),
                     part('black_market_01_boot', 'black_market_01_boot_s', [60, 50, 40]),
                     part('black_market_01_hand', 'black_market_01_hand_s', [170, 160, 140]),
                     part('black_market_01_helmet', 'black_market_01_helmet_s', [70, 80, 70])]
        head_bind = bind_pose((a / 'models_character_human_base_base_character_head.skinned_model_render_bind_pose').read_bytes())
        self.body.append(Skinned(head_bind, (a / 'models_character_human_base_base_character_head.skinned_model_render_black_market_01_head_s_converted_model').read_bytes(), [190, 160, 140]))
        self.battery = Skinned(bind_pose((a / 'models_weapons_electrobox.skinned_model_render_bind_pose').read_bytes()),
                               (a / 'models_weapons_electrobox.skinned_model_render_electrobox_s_converted_model').read_bytes(),
                               [200, 170, 40])

    def clip(self, stem):
        return clips.parse_clip((self.a / (stem + '.b-spline')).read_bytes())[0]

    def world(self, clip, t):
        return clips.world_pose(clips.local_pose(clip, self.names, t), self.parents, self.names)

    def battery_world(self, world, t=0):
        """The battery skeleton's bones in the player's model space."""
        root = matrix(clips.rotation([0, math.pi, 0]), [0, 0, 0]) @ matrix(*world['Weapon'])
        local = clips.world_pose(clips.local_pose(self.item_idle, self.item_names, t), self.item_parents, self.item_names)
        out = {}
        for n, (r, p) in local.items():
            m = matrix(r, p) @ root
            out[n] = (m[:3, :3], m[3, :3])
        return out

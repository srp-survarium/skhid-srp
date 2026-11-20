"""Place the two modular magazines for the offline side-profile render.

AKMSU's shipped bind bone is not its assembled idle position. The source
pack's (weapons-069@11) PPSh drum has a quarter-turn around that socket;
undo it for the profile icon, including its cartridge surface and tangent
frame. weapons-069@15's model has the drum without it (README).
"""
import struct
import numpy as np
import renderer


def basis(angles):
    x,y,z = angles
    sx,sy,sz = np.sin([x,y,z]); cx,cy,cz = np.cos([x,y,z])
    return np.array([[cy*cz,-cy*sz,sy],
                     [sy*sx*cz+cx*sz,-sx*sy*sz+cx*cz,-sx*cy],
                     [-cx*sy*cz+sx*sz,cx*sy*sz+sx*cz,cx*cy]])


def binds(path):
    data = path.read_bytes(); at = 2; out = {}
    for i in range(struct.unpack_from('<H',data)[0]):
        end = data.index(0,at); name = data[at:end].decode()
        values = struct.unpack_from('<9f',data,end+1); at = end+37
        assert np.allclose(values[6:],1), 'scaled bind pose is unsupported'
        out[name] = (i,np.array(values[:3]),basis(values[3:6]))
    return out


def constant_clip(path):
    """These shipped item idle tracks are constant; refuse a moving clip."""
    names = renderer.config_strings(path.with_suffix('.bones_names').read_bytes())
    names = [names[f'bones_names.{i}'] for i in range(len(names))]
    data = path.with_suffix('.b-spline').read_bytes()
    bones,events,kind = struct.unpack_from('<HBB',data)
    assert bones == len(names)
    at = 4+bones*0x48+events*0x10
    out = {}
    for name in names:
        channels = []
        for _ in range(9):
            count, = struct.unpack_from('<I',data,at); at += 4
            points = np.array(struct.unpack_from('<'+'f'*(count*2),data,at)).reshape(-1,2)
            at += count*8
            # The last knot's value is an unused -1e9 sentinel.
            values = points[:-1,1]
            assert len(values) and np.allclose(values,values[0]), f'{name}: moving idle track'
            channels.append(values[0])
        assert np.allclose(channels[6:],1)
        out[name] = (np.array(channels[:3]),basis(channels[3:6]))
    assert at == len(data)
    return out


def apply(files, model, parts):
    if '/akmsu/' in model:
        prefix = 'ak_762x39_30_round_sb7_mag_'
        clip = constant_clip(files/'animations/single/weapons/akmsu_069/item/stand/3rd_view/idle')
        root_t,root_r = clip['wpn_ak74']
        mag_t,mag_r = clip['magazin']
        target_t,target_r = mag_t@root_r+root_t,mag_r@root_r
    elif '/ppsh/' in model:
        prefix = 'ppsh_762x25_71_round_mag_'
        target_t = target_r = None
    else:
        return
    bone,origin,rotation = binds(files/model/'render/bind_pose')['magazin']
    if target_t is None:
        target_t = origin
        target_r = rotation@basis([0,-np.pi/2,0])
    turn = rotation.T@target_r
    selected = [p for p in parts if p.name.startswith(prefix)]
    assert len(selected) == 2, 'magazine and cartridges must move together'
    for part in selected:
        # All these module vertices have one influence: the magazine bone.
        data = (files/model/'render'/part.name/'converted_model').read_bytes()
        kind,size,count = struct.unpack_from('<III',data)
        assert kind == 3 and (size-4)//count == 36
        vertices = np.frombuffer(data,np.uint8,count*36,12).reshape(count,36)
        assert np.all(vertices[:,12:16] == [bone,0,0,0])
        part.position = (part.position-origin)@turn+target_t
        for attr in ('normal','tangent','bitangent'):
            setattr(part,attr,getattr(part,attr)@turn)

"""Held flower crops keep native vertex data and repair model bounds/counts."""
import hashlib
import struct
import unittest
import content_edit as ce
from test_content_edit import config

class HeldFlower(unittest.TestCase):
    def setUp(self):
        self.prefix = 'models/full.model/'
        positions = [(0.,0.,0.), (0.,.5,0.), (.1,.5,0.), (0.,.7,.1)]
        vertices = b''.join(struct.pack('<3f',*p) + bytes([i+10])*20 for i,p in enumerate(positions))
        mesh = struct.pack('<III',3,4+len(vertices),4)+vertices+struct.pack('<III6H',4,16,6,0,1,2,1,2,3)
        bounds = {'bounding_box': {'min': (0.,0.,0.), 'max': (.1,.7,.1)}}
        surface = {**bounds, 'bounding_sphere': {'center': (0.,0.,0.), 'radius': 1.},
                   'source': {'vertices': 4, 'indices': 6, 'primitives': 2}}
        self.sources = {self.prefix+'render/export_properties':config(bounds),
            self.prefix+'render/surface/export_properties':config(surface),
            self.prefix+'render/surface/converted_model':mesh,
            self.prefix+'settings': config({'material':'original'})}
        self.spec = {'source':self.prefix, 'output':'models/held.model/', 'minimum_y':.425,
                     'sha256':{p:hashlib.sha256(b).hexdigest() for p,b in self.sources.items()}}

    def test_only_flower_survives_with_compact_indices_original_uvs_and_correct_bounds(self):
        original = {p:bytes(b) for p,b in self.sources.items()}
        out=ce.make_held_flower(self.sources,self.spec)
        self.assertEqual(original,self.sources)
        self.assertTrue(all(p.startswith('models/held.model/') for p in out))
        mesh=out['models/held.model/render/surface/converted_model']
        self.assertEqual(struct.unpack_from('<III',mesh),(3,100,3))
        self.assertEqual(struct.unpack_from('<III3H',mesh,108),(4,10,3,0,1,2))
        for i in range(3): self.assertEqual(mesh[24+i*32:44+i*32],bytes([i+11])*20)
        positions=[struct.unpack_from('<3f',mesh,12+i*32) for i in range(3)]
        self.assertAlmostEqual(max(p[1] for p in positions)-min(p[1] for p in positions),.2)
        for name in ['render/export_properties','render/surface/export_properties']:
            b=out['models/held.model/'+name]
            for key,fn in [('min',min),('max',max)]:
                at=ce.rec(b,ce.find(b,'bounding_box/'+key))[0]
                self.assertEqual(struct.unpack_from('<3f',b,at),tuple(fn(p[i] for p in positions) for i in range(3)))
        self.assertEqual(out['models/held.model/settings'],original[self.prefix+'settings'])

    def test_stale_source_and_empty_crop_are_refused(self):
        self.spec['minimum_y']=1.
        with self.assertRaises(ValueError):ce.make_held_flower(self.sources,self.spec)
        self.spec['sha256'][self.prefix+'settings']='bad'
        with self.assertRaises(ValueError):ce.make_held_flower(self.sources,self.spec)

if __name__=='__main__': unittest.main()

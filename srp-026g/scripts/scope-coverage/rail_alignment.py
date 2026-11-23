"""Measure an optic against the socket of the rail actually present in its mesh."""
import numpy as np

import mounts


class Rails:
    def __init__(self, donor, builder):
        self.builder = builder
        self.donor = donor
        model, lib = builder.model, builder.lib
        self.optics = {r[0]: f'weapons/modules/{r[2]}/{r[2].split("/")[-1]}'
                       for r in builder.sf.survey.rows('optics.tsv')}
        self.optics['shop_scope_pu_3_5x'] = 'weapons/modules/scopes/pu_35x22_scope/pu_35x22_scope'
        self.rails = []
        game = model.config(donor / 'gameplay/game.options')
        seen = set()
        for name, value in game['weapon_modules'].items():
            if 'picatinnyrail' not in name:
                continue
            directory = lib.model_dir(donor, value.get('model', ''))
            if directory is None or directory in seen:
                continue
            seen.add(directory)
            for loc in model.config(directory / 'render/export_properties').get('locators', []):
                if loc['name'] in ('scope_point', 'picatinny_scope_point'):
                    self.rails.append((name, directory, loc))

    def measure(self, scope, idle):
        """Required translation of the optic, keeping the mounted rail in place."""
        b = self.builder
        native = b.lib.model_dir(self.donor, self.optics[scope])
        if native is None:
            raise ValueError(f'{scope}: missing native geometry for rail measurement')
        origin = b.sf.translation(idle, native)
        if origin is None:
            return np.zeros(3), [], None
        found = []
        for name, directory, loc in self.rails:
            translation = b.sf.translation(idle, directory)
            if translation is None:
                continue
            rotation = mounts.local(loc)[:3, :3]
            if not np.allclose(rotation, np.eye(3), atol=1e-6):
                raise ValueError(f'{idle}: rotated rail socket needs a rigid optic correction')
            found.append((name, np.array(loc['position']) + translation - origin))
        if not found:
            return np.zeros(3), [], None
        for _, shift in found[1:]:
            np.testing.assert_allclose(shift, found[0][1], atol=.0002,
                                       err_msg=f'{idle}: ambiguous rail sockets')
        own = set(b.lib.surfaces(native))
        shift = found[0][1]
        return (shift if abs(shift).max() > .0002 else np.zeros(3)), own, found[0][0]

    def shifted(self, scope, idle, aimed, shift, own, compose, work, name):
        """Move the optic and reticle as one; rails and replacement gun parts stay."""
        b = self.builder
        outputs = []
        for source, suffix in ((idle, ''), (aimed, '_aim')):
            names = b.lib.surfaces(source)
            moving = [s for s in names if s in own or b.rs.align.emitter(s) or
                      s in (b.lib.GRID, b.lib.EYEPIECE_LENS, b.lib.BACKGROUND)]
            fixed = [s for s in names if s not in moving]
            if not moving:
                raise ValueError(f'{scope}: cannot identify optic surfaces in {source}')
            target_name = f'weapons/attach/coverage/centered_{name}{suffix}'
            target = work / f'models/{target_name}.model'
            parts = [(source, shift, moving)] + ([(source, np.zeros(3), fixed)] if fixed else [])
            b.rs.compose(compose, parts, {}, target)
            outputs.append((target_name, target))
        return outputs

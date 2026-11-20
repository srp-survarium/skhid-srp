"""Builds and caches the samples of every decoded recording."""
import multiprocessing
import pathlib
import zlib

import numpy as np

from . import load
from .harness import Samples, is_test, match_samples

# Delay sets: None is the realistic one.
DELAYS = {'real': None, '50': 50, '100': 100, '150': 150, '250': 250}


def _one(job):
    path, out, every = job
    target = pathlib.Path(out) / (pathlib.Path(path).name + '.npz')
    if target.exists():
        return str(target)
    match = load.load(path)
    arrays = {}
    for label, delay in DELAYS.items():
        rng = np.random.default_rng(zlib.crc32(f'{match.name}/{label}'.encode()))
        s = match_samples(match, every, delay, rng)
        if s is None:
            continue
        for k, v in s.__dict__.items():
            if k not in ('names', 'match'):
                arrays[f'{label}/{k}'] = v
    tmp = target.with_suffix('.tmp.npz')
    np.savez(tmp, **arrays)
    tmp.rename(target)
    return str(target)


def build(data, cache, every_ticks, workers=12):
    cache = pathlib.Path(cache)
    cache.mkdir(parents=True, exist_ok=True)
    jobs = [(str(d), str(cache), every_ticks) for d in load.match_dirs(data)]
    with multiprocessing.Pool(workers) as pool:
        return pool.map(_one, jobs, chunksize=4)


def read(cache, label, test, limit=None):
    """The samples of one delay set of the train or the test matches."""
    parts = []
    for f in sorted(pathlib.Path(cache).glob('*.npz')):
        name = f.name[:-len('.npz')]
        if is_test(name) != test:
            continue
        z = np.load(f)
        keys = [k for k in z.files if k.startswith(label + '/')]
        if not keys:
            continue
        fields = {k.split('/', 1)[1]: z[k] for k in keys}
        n = len(fields['delay'])
        parts.append(Samples(match=np.zeros(n, np.int32), names=[name], **fields))
        if limit and len(parts) >= limit:
            break
    return Samples.concat(parts)

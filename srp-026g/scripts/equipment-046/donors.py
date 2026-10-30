"""The donor builds' and the released game's resources, read with srp-game-data.

A donor build is a game folder of resources.NN.db volumes (0.31a2: 22, 0.46c2:
36). A later volume replaces a path an earlier one has, as the game mounts
them (0.46c2's last volumes are its patches). The released game is the stock
0.26g resources.db with the published patches of base-patches.tsv over it.
srp-game-data follows directory hard links, which older porter scans missed.
"""
from collections import defaultdict
import hashlib
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
DONORS = ('031', '046')


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def safe(path):
    if '..' in Path(path).parts or Path(path).is_absolute():
        raise ValueError(f'unsafe resource path: {path}')
    return path


class Layers:
    """One resource view: the paths it has and how to extract them."""

    def __init__(self, binary, owners):
        self.binary = binary
        self.owners = owners  # path -> list of srp-game-data layer arguments

    @classmethod
    def donor(cls, binary, folder):
        volumes = sorted(Path(folder).glob('resources.[0-9][0-9].db'))
        if not volumes:
            raise ValueError(f'{folder}: no resources.NN.db volumes')
        owners = {}
        for volume in volumes:
            for path in listing(binary, ['--db', str(volume)]):
                owners[path] = ['--db', str(volume)]
        return cls(binary, owners)

    @classmethod
    def base(cls, binary, stock, patches):
        layers = ['--db', str(stock)]
        for line in (HERE / 'base-patches.tsv').read_text().splitlines():
            if not line or line.startswith('#'):
                continue
            name, checksum = line.split('\t')
            path = Path(patches) / name
            if digest(path) != checksum:
                raise ValueError(f'{path}: published patch checksum changed')
            layers += ['--patch', str(path)]
        return cls(binary, {path: layers for path in listing(binary, layers)})

    def __contains__(self, path):
        return path in self.owners

    def paths(self):
        return self.owners.keys()

    def extract(self, paths, destination, stage):
        """Write `paths` under `destination`; `stage` is a scratch folder."""
        by_layers = defaultdict(list)
        for path in sorted(set(paths)):
            if path not in self.owners:
                raise ValueError(f'missing resource: {path}')
            by_layers[tuple(self.owners[path])].append(safe(path))
        for index, (layers, selected) in enumerate(sorted(by_layers.items())):
            part = Path(stage) / str(index)
            pathlist = Path(stage) / f'{index}.paths'
            pathlist.parent.mkdir(parents=True, exist_ok=True)
            pathlist.write_text(''.join(path + '\n' for path in selected))
            subprocess.run([str(self.binary), 'get', *layers, '--paths', str(pathlist), '--out', str(part)],
                           check=True, capture_output=True)
            for path in selected:
                target = Path(destination) / path
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.move(part / path, target)
        shutil.rmtree(stage, ignore_errors=True)


def listing(binary, layers):
    result = subprocess.run([str(binary), 'files', *layers], check=True, capture_output=True, text=True)
    return {line.split('\t')[0] for line in result.stdout.splitlines()}

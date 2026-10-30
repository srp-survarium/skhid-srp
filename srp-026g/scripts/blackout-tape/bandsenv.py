"""Where the Blackout band tools find the game and keep their work: environment variables.

BANDS_GAME   the 0.26g game folder: its resources.db, then every resources.db.patch_<n> in it
             in number order (later wins), as the game mounts them
VOSTOK_PORT  the vostok-port binary (skhid-srp private/release-tools/vostok-port)
BANDS_POSER  resource-porter's pose-blackout-bands example (branch feat/blackout-bands:
             cargo build --release --example pose-blackout-bands)
BANDS_WORK   a work folder: extracted game files, posed models, measures, renders

The scripts need python3 with numpy and pillow, e.g.
  nix-shell -p 'python3.withPackages(ps: [ps.numpy ps.pillow])'
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)


def need(name):
    value = os.environ.get(name)
    if not value:
        sys.exit('%s is not set (see bandsenv.py)' % name)
    return value


GAME = need('BANDS_GAME')
VOSTOK_PORT = need('VOSTOK_PORT')
WORK = need('BANDS_WORK')
POSER = os.environ.get('BANDS_POSER', '')
os.makedirs(WORK, exist_ok=True)


def layers():
    """resources.db and its patches, in the order the game mounts them."""
    patches = sorted((int(m.group(1)), f) for f in os.listdir(GAME)
                     for m in [re.fullmatch(r'resources\.db\.patch_(\d+)', f)] if m)
    return [os.path.join(GAME, 'resources.db')] + [os.path.join(GAME, f) for _, f in patches]


def work(*parts):
    path = os.path.join(WORK, *parts)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    return path

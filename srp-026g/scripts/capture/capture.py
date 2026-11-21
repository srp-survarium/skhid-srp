#!/usr/bin/env python3
"""Pictures of a map from given camera poses, headless, in one command.

    capture.py --map <maps_dict id> --poses poses.json --out <dir>
               --game <game folder> --exe <exe with patch 175>
               [--content <resources.db.patch_N> ...] [--game-data <server-game-data.db>]

The game goes from its start straight into the map: no server, sign-in,
lobby or click. `srp-map-sur` (this tree's match server) writes a replay of
the map with nobody in it; the game is started with `-match_replay=` of it and
`SKHID_CAPTURE=<script>` (exe patch 175, docs/notes/capture.md), which puts
the camera at each pose, saves the scene before the interface as
`<out>/<name>.png` and ends the game. Everything runs on a display of its own
(Xorg's dummy driver at 60 Hz, openbox; never the desktop's), in a Wine prefix
of its own, and is stopped afterwards. Run it through the heavy-job runner:

    ~/.cache/skhid-heavy/run --gpu python3 srp-026g/scripts/capture/capture.py ...

poses.json: a list of poses, or {"capture": {defaults}, "poses": [...]}. A
pose: "name" (letters, digits, _ - .), "position" [x, y, z], and either
"forward" [x, y, z] or "yaw" and "pitch" in degrees (yaw 0 looks along +z, 90
along +x; pitch up positive); "hfov" (horizontal degrees, default 90) or
"fov" (vertical); "near" (0.1), "far" (1500), "settle_ms" (4000). The
defaults: those keys, plus "wait_ms" (15000, the world's first streaming at
the first pose), "size" ("1920x1080", the game's window) and "user_cfg"
({"r_texture_quality": "2", ...}, written into the prefix's user.cfg).

--game is a game folder whose resources.db and patches are linked (never
written), e.g. the owner's install's `game`; --content adds a built patch
(release/build-content's `resources.db.patch_<n>`) over them, and its folder's
server-game-data.db is the map's game data unless --game-data names one.
--exe is an exe built with patch_026g.py from a tree that has patch 175.
"""
import argparse
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
SRP = HERE.parents[1]
REPO = SRP.parent
WINE = os.environ.get('SKHID_CAPTURE_WINE', '/nix/store/qssax4fwpxbi6ffrlqv3l2py2n9y8nqs-wine-wow64-staging-11.8/bin/wine')
# The main checkout's private/ (a worktree's git common folder is the main checkout's .git).
PRIVATE = Path(subprocess.run(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=HERE,
                              capture_output=True, text=True).stdout.strip() or REPO / '.git').parent / 'private'
TEMPLATE = Path(os.environ.get('SKHID_CAPTURE_PREFIX_TEMPLATE', PRIVATE / 'local-clients/prefix-template'))
DLLS = ('bugtrap.dll', 'd3d11.dll', 'dxgi.dll', 'skhid_voice.dll')
DEFAULTS = {'hfov': 90.0, 'near': 0.1, 'far': 1500.0, 'settle_ms': 4000, 'wait_ms': 15000, 'size': '1920x1080',
            'user_cfg': {}}
NAME = re.compile(r'[A-Za-z0-9_-][A-Za-z0-9_.-]{0,47}')


def log(*a):
    print(time.strftime('[%H:%M:%S]'), *a, flush=True)


def windows(path):
    return 'Z:' + str(Path(path).resolve()).replace('/', '\\')


def nix_out(attr):
    return subprocess.run(['nix', 'build', '--no-link', '--print-out-paths', f'nixpkgs#{attr}'], check=True,
                          capture_output=True, text=True).stdout.split()[0]


def modeline(w, h):
    # CVT reduced blanking; the dummy driver takes no pixel clock over 300 MHz,
    # so a big mode runs at 30 Hz. Never 0 Hz: DXVK divides by it (Xvfb).
    ht, vt = w + 160, h + 30
    hz = 60 if ht * vt * 60 / 1e6 <= 295 else 30
    return f'"{w}x{h}" {ht * vt * hz / 1e6:.2f} {w} {w + 48} {w + 80} {ht} {h} {h + 3} {h + 8} {vt} +hsync -vsync'


def script_of(spec, out, aspect):
    """The game's script (capture.rs) from poses.json."""
    if isinstance(spec, list):
        spec = {'poses': spec}
    base = dict(DEFAULTS, **spec.get('capture', {}))
    lines = [f'folder {windows(out)}', f"wait {int(base['wait_ms'])}"]
    names = set()
    for raw in spec['poses']:
        p = dict(base, **{k: v for k, v in raw.items() if v is not None})
        name = p['name']
        if not NAME.fullmatch(name) or name in names:
            raise SystemExit(f'pose name {name!r}: letters, digits, _ - . (at most 48), each once')
        names.add(name)
        if 'forward' in p:
            forward = p['forward']
        else:
            yaw, pitch = math.radians(p['yaw']), math.radians(p['pitch'])
            forward = [math.sin(yaw) * math.cos(pitch), math.sin(pitch), math.cos(yaw) * math.cos(pitch)]
        fov = p['fov'] if 'fov' in raw else math.degrees(2 * math.atan(math.tan(math.radians(p['hfov']) / 2) / aspect))
        values = [*p['position'], *forward, fov, p['near'], p['far']]
        lines.append(f'pose {name} ' + ' '.join(f'{v:.6f}' for v in values) + f" {int(p['settle_ms'])}")
    return base, names, '\n'.join(lines) + '\n'


class Run:
    def __init__(self, a):
        self.a = a
        self.work = a.runtime / 'run'
        self.children = []

    def spawn(self, name, argv, env, cwd=None):
        out = (self.work / f'{name}.log').open('wb')
        child = subprocess.Popen(argv, env=env, cwd=cwd, stdout=out, stderr=subprocess.STDOUT,
                                 stdin=subprocess.DEVNULL, start_new_session=True)
        self.children.append((name, child))
        return child

    def replay(self):
        sur = self.work / 'map.sur'
        tool = SRP / 'target/release/srp-map-sur'
        if not tool.exists():
            log('building srp-map-sur')
            subprocess.run(['nix', 'develop', str(SRP), '--command', 'cargo', 'build', '--release', '-p',
                            'match-server', '--bin', 'srp-map-sur'], cwd=SRP, check=True)
        env = dict(os.environ, SRP_GAME_DATA=str(self.a.game_data))
        done = subprocess.run([str(tool), str(self.a.map), str(sur), '--minutes', '60'], env=env,
                              capture_output=True, text=True)
        if done.returncode:
            raise SystemExit(done.stderr.strip() or 'srp-map-sur failed')
        log(done.stdout.strip())
        return sur

    def game(self):
        game = self.a.runtime / 'game'
        x86 = game / 'binaries/x86'
        x86.mkdir(parents=True, exist_ok=True)
        for old in game.glob('resources.db*'):
            old.unlink()
        for entry in sorted(self.a.game.glob('resources.db*')):
            (game / entry.name).symlink_to(entry.resolve())
        for patch in self.a.content:
            if not re.fullmatch(r'resources\.db\.patch_\d+', patch.name):
                raise SystemExit(f'{patch}: a content patch is resources.db.patch_<n>')
            link = game / patch.name
            if link.is_symlink():
                link.unlink()
            link.symlink_to(patch.resolve())
        if not (game / 'resources').exists():
            shutil.copytree(self.a.game / 'resources', game / 'resources')
        for dll in DLLS:
            if (self.a.game / 'binaries/x86' / dll).exists():
                shutil.copyfile(self.a.game / 'binaries/x86' / dll, x86 / dll)
        shutil.copyfile(self.a.exe, x86 / 'survarium_skhid.capture.exe')
        self.game_dir = game

    def prefix(self, user_cfg, size):
        prefix = self.a.runtime / 'prefix'
        if not prefix.exists():
            subprocess.run(['cp', '-a', '--reflink=auto', str(TEMPLATE), str(prefix)], check=True)
        managed = {'account_name', 'account_password', 'account_digest', 'store_user_password', 'r_fullscreen',
                   'r_resolution', 'ui'} | set(user_cfg)
        for user in prefix.glob('drive_c/users/*'):
            if user.name in ('Public', 'All Users') or not user.is_dir():
                continue
            folder = user / 'Documents/Survarium'
            folder.mkdir(parents=True, exist_ok=True)
            path = folder / 'user.cfg'
            lines = [x for x in (path.read_text().splitlines() if path.exists() else [])
                     if (x.split(maxsplit=1) or [''])[0] not in managed]
            lines += ['r_fullscreen off', f'r_resolution {size}'] + [f'{k} {v}' for k, v in user_cfg.items()]
            path.write_text('\n'.join(lines) + '\n')
        self.wineprefix = prefix

    def display(self, w, h):
        xorg, dummy, openbox = nix_out('xorg.xorgserver'), nix_out('xf86-video-dummy'), nix_out('openbox')
        number = next(n for n in range(150, 260) if not Path(f'/tmp/.X11-unix/X{n}').exists()
                      and not Path(f'/tmp/.X{n}-lock').exists())
        conf = self.work / 'xorg.conf'
        conf.write_text(f'''Section "Device"
  Identifier "dummy"
  Driver "dummy"
  VideoRam 256000
EndSection
Section "Monitor"
  Identifier "mon"
  HorizSync 5.0 - 1000.0
  VertRefresh 5.0 - 200.0
  Modeline {modeline(w, h)}
EndSection
Section "Screen"
  Identifier "scr"
  Device "dummy"
  Monitor "mon"
  DefaultDepth 24
  SubSection "Display"
    Depth 24
    Modes "{w}x{h}"
  EndSubSection
EndSection
''')
        (self.work / 'xorg.conf.d').mkdir(exist_ok=True)
        env = {k: v for k, v in os.environ.items() if k not in ('WAYLAND_DISPLAY', 'XDG_SESSION_TYPE', 'DISPLAY')}
        self.spawn('xorg', [f'{xorg}/bin/Xorg', f':{number}', '-ac', '-noreset', '-novtswitch', '-nolisten', 'tcp',
                            '-config', str(conf), '-configdir', str(self.work / 'xorg.conf.d'),
                            '-modulepath', f'{xorg}/lib/xorg/modules,{dummy}/lib/xorg/modules',
                            '-logfile', str(self.work / 'xorg-server.log')], env)
        for _ in range(100):
            if Path(f'/tmp/.X11-unix/X{number}').exists():
                break
            time.sleep(.1)
        else:
            raise SystemExit(f'Xorg did not start; see {self.work}/xorg-server.log')
        self.env = dict(env, DISPLAY=f':{number}')
        self.spawn('openbox', [f'{openbox}/bin/openbox'], self.env)
        time.sleep(1)
        log('display', self.env['DISPLAY'])

    def client(self, sur, script):
        self.wine_env = dict(self.env, WINEPREFIX=str(self.wineprefix), WINEDEBUG='-all', DXVK_FRAME_RATE='60',
                             WINEDLLOVERRIDES='mscoree,mshtml,qwave=;winemenubuilder.exe=d;d3d11,dxgi=n,b',
                             SKHID_CAPTURE=windows(script))
        self.started = time.time()
        return self.spawn('client', [WINE, 'survarium_skhid.capture.exe', f'-match_replay={windows(sur)}',
                                     '-mt_allocator=768', '-minimum_resources_memory_size=768'],
                          self.wine_env, cwd=self.game_dir / 'binaries/x86')

    def patches_log(self):
        logs = [p for p in (self.game_dir / 'logs').glob('srp_patches.*.log') if p.stat().st_mtime >= self.started - 5]
        return max(logs, key=lambda p: p.stat().st_mtime) if logs else None

    def stop(self):
        if getattr(self, 'wine_env', None):
            subprocess.run([str(Path(WINE).parent / 'wineserver'), '-k'], env=self.wine_env, timeout=30)
        for _, child in reversed(self.children):
            if child.poll() is None:
                try:
                    os.killpg(child.pid, signal.SIGTERM)
                    child.wait(timeout=15)
                except ProcessLookupError:
                    pass
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--map', type=int, required=True, help='a maps_dict id (the map, weather and mode)')
    ap.add_argument('--poses', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--game', type=Path, required=True, help='a game folder: its resources.db* are linked')
    ap.add_argument('--exe', type=Path, required=True, help='an exe with patch 175')
    ap.add_argument('--content', type=Path, action='append', default=[], help='a resources.db.patch_<n> over them')
    ap.add_argument('--game-data', type=Path, help="server-game-data.db (default: --content's folder's, or SRP_GAME_DATA)")
    ap.add_argument('--runtime', type=Path, default=Path.home() / '.cache/skhid-capture/runtime',
                    help='the capture game folder, its Wine prefix and the run logs (not on /mnt/hdd: Wine needs ":" in names)')
    ap.add_argument('--timeout', type=int, default=900, help='seconds before the game is stopped')
    a = ap.parse_args()
    a.runtime = a.runtime.resolve()
    if ' ' in str(a.runtime):
        # Wine quotes an argument with a space and escapes its quotes: the game's
        # own command-line reader then takes the quotes into the replay's path.
        raise SystemExit('--runtime: a folder without spaces in its path')
    if a.game_data is None:
        near = [c.parent / 'server-game-data.db' for c in a.content if (c.parent / 'server-game-data.db').exists()]
        a.game_data = near[-1] if near else (Path(os.environ['SRP_GAME_DATA']) if 'SRP_GAME_DATA' in os.environ else None)
    if a.game_data is None or not a.game_data.exists():
        raise SystemExit('no game data: --game-data, a --content beside its server-game-data.db, or SRP_GAME_DATA')
    spec = json.loads(a.poses.read_text())
    base = dict(DEFAULTS, **(spec.get('capture', {}) if isinstance(spec, dict) else {}))
    w, h = (int(v) for v in str(base['size']).split('x'))
    a.out = a.out.resolve()
    a.out.mkdir(parents=True, exist_ok=True)
    base, names, text = script_of(spec, a.out, w / h)
    for name in names:
        (a.out / f'{name}.png').unlink(missing_ok=True)
    run = Run(a)
    if run.work.exists():
        shutil.rmtree(run.work)
    run.work.mkdir(parents=True)
    script = run.work / 'capture.txt'
    script.write_text(text)
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    code = None
    try:
        sur = run.replay()
        run.game()
        run.prefix(base['user_cfg'], f'{w}x{h}')
        run.display(w + 128, h + 128)
        client = run.client(sur, script)
        log(f'game started: map {a.map}, {len(names)} poses, {w}x{h}')
        try:
            code = client.wait(timeout=a.timeout)
        except subprocess.TimeoutExpired:
            log(f'the game did not end in {a.timeout} s')
    finally:
        run.stop()
    patches = run.patches_log()
    lines = [x for x in patches.read_text(errors='replace').splitlines() if 'capture:' in x] if patches else []
    for line in lines:
        print('  ' + line)
    missing = sorted(n for n in names if not (a.out / f'{n}.png').exists())
    if code != 0 or missing:
        raise SystemExit(f'capture failed (game exit {code}; missing {", ".join(missing) or "none"}); '
                         f'logs: {run.work}, {patches or run.game_dir / "logs"}')
    log(f'{len(names)} pictures in {a.out}')


if __name__ == '__main__':
    main()

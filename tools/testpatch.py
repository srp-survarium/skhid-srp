"""tools/test-patch <setup> [<input>...] [--name <name>] [--exe <name>] [--revision <n>]

The quick way to put a change in front of the owner: a resources.db.patch_<n> of exactly the
files that differ from what the setup's game mounts, in about a minute (a content build takes ~20).

  <setup>   a local game's folder (its config.json, as run.py --config takes it)
  <input>   each one of
              <pack folder>   a content pack (<id>-<version> with its pack.json): the files of its
                              add_file and replace_file operations. Its other operations (tables,
                              config_set, icons) are composed by vostok-port, which this does not
                              run: when they differ from those of the newest earlier version
                              beside it, it is refused.
              <movie tree>    srp-026g/flash/<movie>: compiled (seconds), its code put into the
                              setup's movie, whose graphics (the packs' icons) stay as they are
              <file>=<path>   a file at <path> of resources.db, as it is; a .swf at flash_movies/
                              gives only its code, as a tree does
  --name    the patch's folder: <setup>/content-<name>-<n> (default: the first input's name)
  --exe     patch_026g.py of this tree into <setup>/game/binaries/x86/survarium_skhid.<name>.exe
            (seconds; never over one that runs), at --revision <n>, else the setup's
            SRP_MIN_PATCH_REVISION, else REVISION

What differs from the setup's game (srp-game-data compare: every archive of its game folder, as
the game mounts them) is packed as resources.db.patch_<n>, n the next free number there, and
linked beside the others; nothing is overwritten. A pack's file that an edit list of
release/content.tsv lays on is left out: the setup has it with the list applied, the pack's file
without; one the pack changed is named, as the patch does not show that change. Then it says whether the server needs new game data (it reads gameplay/,
the texts and the projects' server cuts): if not, the setup's stays; if so, it builds it beside
the patch and says what the setup needs with it. Last, the run.py command that starts it all.
"""
import argparse
import filecmp
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / 'release'), str(ROOT / 'tools' / 'actionscript')]
PATCH = re.compile(r'^resources\.db\.patch_([1-9][0-9]{0,2})$')
FILE_OPS = ('add_file', 'replace_file')
RUN_PY = ROOT / 'srp-026g' / 'scripts' / 'local-clients' / 'run.py'


class Refused(Exception):
    pass


def layers(game):
    """The --db/--patch arguments of what the game in `game` mounts, and the patches' numbers."""
    numbers = sorted(int(m[1]) for p in game.iterdir() if (m := PATCH.match(p.name)) and int(m[1]) <= 255)
    args = ['--db', str(game / 'resources.db')]
    for n in numbers:
        args += ['--patch', str(game / f'resources.db.patch_{n}')]
    return args, numbers


def pack_files(pack):
    """{path: file} of a pack folder's file operations, and its other operations."""
    spec = json.loads((pack / 'pack.json').read_text())
    files = {op['path']: pack / 'files' / op['path'] for op in spec['operations'] if op['op'] in FILE_OPS}
    return spec, files, [op for op in spec['operations'] if op['op'] not in FILE_OPS]


def earlier_version(pack, spec):
    """The newest folder <id>-<k> beside `pack` with k below its version, or None."""
    found = []
    for folder in pack.parent.glob(f"{spec['id']}-*"):
        k = folder.name[len(spec['id']) + 1:]
        if k.isdigit() and int(k) < int(spec['version']) and (folder / 'pack.json').is_file():
            found.append((int(k), folder))
    return max(found)[1] if found else None


def other_ops_changed(pack):
    """Why a pack's operations other than files cannot be tested here, or None."""
    spec, _, others = pack_files(pack)
    earlier = earlier_version(pack, spec)
    before = pack_files(earlier)[2] if earlier else []
    canonical = lambda ops: sorted(json.dumps(op, sort_keys=True) for op in ops)
    if canonical(others) == canonical(before):
        return None
    kinds = sorted({op['op'] for op in others + before})
    against = f"{earlier.name}'s" if earlier else 'none (no earlier version beside it)'
    return (f"{pack.name}: its {', '.join(kinds)} operations differ from {against}: those are "
            "composed (vostok-port); give the composed files as <file>=<path>, or build the content "
            "(release/build-content --slow)")


def edit_lists(tree):
    """The edit rows' names of release/content.tsv, in order."""
    rows = (line.split('\t') for line in (tree / 'release' / 'content.tsv').read_text(encoding='utf-8').splitlines()
            if line and not line.startswith('#'))
    return [row[1] for row in rows if row[0] == 'edit']


def edited(tree, paths):
    """{path: [edit list]} of `paths` an edit list of the manifest reads (build_content.edit_paths)."""
    import build_content
    known = sorted(paths)
    folder = build_content.edits_dir(tree)
    found = {}
    for name in edit_lists(tree):
        if name.endswith('.tsv'):
            ce = build_content.own_content_edit()
            read = ce.stats_paths(ce.stats_category(name), known)
        else:
            read = build_content.edit_paths(json.loads((folder / name).read_text(encoding='utf-8')), known)
        for path in set(read) & set(paths):
            found.setdefault(path, []).append(name)
    return found


def chosen(lines, laid_on):
    """What to pack of srp-game-data compare's `lines`: ([(path, server)], [(path, lists)] left out)."""
    packed, left = [], []
    for line in lines:
        words = line.split()
        if words[0] == 'same':
            continue
        path = words[1]
        if path in laid_on:
            left.append((path, laid_on[path]))
        else:
            packed.append((path, words[-1] == 'server'))
    return packed, left


def without_bitmaps(movie, names):
    """`movie` without the bitmaps bound to `names`: their defining tags and their SymbolClass entries."""
    import swf
    header, tags = swf.split(movie)
    gone = {cid for cid, name in swf.symbol_classes(tags) if name in names}
    kept = []
    for kind, raw in tags:
        if kind in (swf.DEFINE_BITS_LOSSLESS2, swf.DEFINE_BITS_JPEG2, swf.DEFINE_BITS_JPEG3) \
                and struct.unpack_from('<H', swf.payload(raw))[0] in gone:
            continue
        if kind == swf.SYMBOL_CLASS:
            left = [(cid, name) for cid, name in swf.symbol_classes([(kind, raw)]) if name not in names]
            if not left:
                continue
            raw = swf.symbol_class(left)
        kept.append((kind, raw))
    return swf.compressed(swf.join(header, kept), movie)


def code_into(current, built, tree=None):
    """The setup's movie `current` with the code of `built`: its graphics (the packs' icons) kept,
    and the bitmaps of `tree` (the movie tree built) put in: added where it lacks them, in place of
    its own where it has them (a tree's bitmaps/ are its source: an earlier test's stay otherwise),
    and those an earlier test added that the tree no longer binds left out."""
    import flash_tree
    import swf
    code_tags = [t for t in swf.split(built)[1] if t[0] in (swf.DO_ABC, swf.DO_ABC_OLD)]
    bound = lambda movie: {name for _, name in swf.symbol_classes(swf.split(movie)[1])}
    if tree:
        # the tree's bitmaps in place of the setup's, and the setup's that a tree built before added but
        # this one no longer binds (bound there, not in this build) gone
        gone = bound(current) - bound(built)
        current = without_bitmaps(current, {b.name for b in flash_tree.bitmaps(tree)} | gone)
    new = bound(built) - bound(current)
    added = [b for b in flash_tree.bitmaps(tree) if b.name in new] if tree else []
    if new - {b.name for b in added}:
        raise Refused(f"the movie binds bitmaps the setup's lacks ({', '.join(sorted(new)[:5])}): "
                      'give its tree (srp-026g/flash/<movie>) instead')
    movie = flash_tree.assemble(current, code_tags, added)
    flash_tree.check(movie)
    return movie


class Setup:
    def __init__(self, path):
        self.config_path = path / 'config.json' if path.is_dir() else path
        self.folder = self.config_path.parent
        self.config = json.loads(self.config_path.read_text())
        self.game = Path(self.config['game_dir'])
        self.env = (self.config.get('server') or {}).get('env', {})
        self.layers, self.numbers = layers(self.game)

    @property
    def min_revision(self):
        raw = self.env.get('SRP_MIN_PATCH_REVISION')
        return int(raw) if raw else None


def run(command, **kw):
    done = subprocess.run([str(c) for c in command], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, **kw)
    if done.returncode:
        raise Refused(f'{Path(str(command[0])).name} failed:\n{done.stdout.strip()}')
    return done.stdout


def game_data_bin():
    if os.environ.get('SRP_GAME_DATA_BIN'):
        return Path(os.environ['SRP_GAME_DATA_BIN'])
    run(['cargo', 'build', '--quiet', '--release', '-p', 'resources-db', '--bin', 'srp-game-data'], cwd=ROOT / 'srp-026g')
    return ROOT / 'srp-026g' / 'target' / 'release' / 'srp-game-data'


def setup_file(bin, setup, path, out):
    """The file the setup's game mounts at `path`, or None."""
    try:
        run([bin, 'get', *setup.layers, '--out', out, path])
    except Refused as error:
        if 'no archive has this file' in str(error):
            return None
        raise
    return (out / path).read_bytes()


def gather(inputs, setup, bin, work):
    """{path: bytes or a file} of the inputs, and {path: whether the pack changed it} of the packs'
    files (against its earlier version beside it)."""
    import flash_tree
    files, from_packs = {}, {}
    for i, given in enumerate(inputs):
        if '=' in given:
            source, path = given.split('=', 1)
            data = Path(source).read_bytes()
            if path.startswith('flash_movies/') and path.endswith('.swf'):
                current = setup_file(bin, setup, path, work / f'current-{i}')
                data = code_into(current, data) if current else data
            files[path] = data
        elif (Path(given) / 'pack.json').is_file():
            why = other_ops_changed(Path(given))
            if why:
                raise Refused(why)
            spec, own, _ = pack_files(Path(given))
            earlier = earlier_version(Path(given), spec)
            before = pack_files(earlier)[1] if earlier else {}
            for path, file in own.items():
                files[path] = file
                from_packs[path] = path not in before or not filecmp.cmp(file, before[path], shallow=False)
        elif (Path(given) / 'movie.json').is_file():
            tree = Path(given).resolve()
            spec = flash_tree.load(tree)
            archive = [] if spec['archive'] == 'resources.db' else ['--patch', setup.game / spec['archive']]
            run([bin, 'get', '--db', setup.game / 'resources.db', *archive, '--out', work / f'original-{i}', spec['movie']])
            tools = Path(os.environ.get('SKHID_FLASH_TOOLS') or main_checkout() / 'private' / 'actionscript' / 'tools.json')
            built = flash_tree.build(tree, work / f'original-{i}' / spec['movie'], tools)
            current = setup_file(bin, setup, spec['movie'], work / f'current-{i}')
            files[spec['movie']] = code_into(current, built, tree) if current else built
        else:
            raise Refused(f'{given}: neither a pack folder (pack.json), a movie tree (movie.json) nor <file>=<path>')
    return files, from_packs


def main_checkout():
    common = run(['git', '-C', ROOT, 'rev-parse', '--path-format=absolute', '--git-common-dir']).strip()
    return Path(common).parent


def make_patch(setup, inputs, name, bin):
    n = max(setup.numbers, default=0) + 1
    if n > 255:
        raise Refused(f'{setup.game} has resources.db.patch_255: no number is free')
    out = setup.folder / f'content-{name}-{n}'
    link = setup.game / f'resources.db.patch_{n}'
    if out.exists() or os.path.lexists(link):
        raise Refused(f'{out} or {link} is already there; never overwritten')
    with tempfile.TemporaryDirectory(dir=setup.folder, prefix='.test-patch-') as temp:
        work = Path(temp)
        files, from_packs = gather(inputs, setup, bin, work)
        staged = work / 'staged'
        for path, data in files.items():
            (staged / path).parent.mkdir(parents=True, exist_ok=True)
            if isinstance(data, Path):
                shutil.copyfile(data, staged / path)
            else:
                (staged / path).write_bytes(data)
        lines = run([bin, 'compare', *setup.layers, '--dir', staged]).splitlines()
        packed, left = chosen(lines, edited(ROOT, from_packs))
        for path, lists in left:
            if from_packs[path]:
                print(f'left out {path}: the pack changed it, but {", ".join(lists)} lay on it in the setup: '
                      'this patch does not show that change (release/build-content --slow does)')
        if any(not from_packs[path] for path, _ in left):
            print(f'left out {sum(not from_packs[p] for p, _ in left)} files the pack did not change, which edit '
                  'lists lay on (the setup has them with the lists)')
        if not packed:
            print(f'nothing differs from what {setup.game} mounts: no patch made')
            return
        for path, _ in packed:
            target = work / 'patch' / path
            target.parent.mkdir(parents=True, exist_ok=True)
            os.replace(staged / path, target)
        out.mkdir()
        try:
            said = run([bin, 'pack', '--dir', work / 'patch', '--out', out / link.name])
        except Refused:
            shutil.rmtree(out)
            raise
    (out / 'files.tsv').write_text(''.join(f'{p}\t{"server" if s else "-"}\n' for p, s in packed))
    os.symlink(os.path.relpath(out / link.name, setup.game), link)
    print(f'{len(packed)} files differ from the setup\'s ({out / "files.tsv"}): {said.strip().splitlines()[-1]}')
    print(f'linked {link}')
    server = [p for p, s in packed if s]
    if not server:
        print(f"the server: no file it reads changed, its game data stays ({setup.env.get('SRP_GAME_DATA', 'unset')})")
        return
    data = out / 'server-game-data.db'
    web = setup.config['ports'][3]
    run([bin, 'build', '--dir', setup.game, '--patch-url', f'http://127.0.0.1:{web}/patches', '--out', data])
    print(f'the server reads {len(server)} of them ({server[0]}{", ..." if len(server) > 1 else ""}): '
          f'new game data {data}; in {setup.config_path} set server.env SRP_GAME_DATA to it and '
          f'SRP_MIN_PATCH_REVISION to {n}, the accounts on exes of revision {n} (--exe <name> --revision {n})')


def running(exe):
    """Whether a process runs `exe`: run.py starts it by its name from its folder."""
    for proc in Path('/proc').glob('[0-9]*'):
        try:
            if exe.name.encode() in (proc / 'cmdline').read_bytes() and \
                    Path(os.readlink(proc / 'cwd')).resolve() == exe.parent.resolve():
                return True
        except OSError:
            continue
    return False


def make_exe(setup, name, revision):
    patches = ROOT / 'srp-026g' / 'client-patches'
    known = int(re.search(r'^REVISION = (\d+)', (patches / 'patch_026g.py').read_text(), re.M)[1])
    revision = revision or max(known, setup.min_revision or 0)
    target = setup.game / 'binaries' / 'x86' / f'survarium_skhid.{name}.exe'
    if running(target):
        raise Refused(f'{target} is running: give another --exe name, or stop the setup first')
    original = Path(os.environ.get('SKHID_ORIGINAL_EXE') or main_checkout() / 'private/function-map/inputs/026g/survarium.exe')
    new = target.with_name(target.name + '.new')
    run([sys.executable, 'patch_026g.py', original, new, f'--revision={revision}'], cwd=patches)
    os.replace(new, target)
    using = [a['name'] for a in setup.config['accounts'] if a.get('executable') == target.name]
    print(f'exe: {target} (revision {revision})'
          + ('' if using else f'; set the accounts\' "executable" to {target.name} in {setup.config_path}'))


def main():
    parser = argparse.ArgumentParser(usage=__doc__.splitlines()[0], description=__doc__.split('\n', 2)[2],
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('setup', type=Path, nargs='?')
    parser.add_argument('inputs', nargs='*')
    parser.add_argument('--name')
    parser.add_argument('--exe')
    parser.add_argument('--revision', type=int)
    args = parser.parse_args()
    if not args.setup or not (args.inputs or args.exe):
        sys.exit(__doc__)
    try:
        setup = Setup(args.setup.resolve())
        if args.inputs:
            name = args.name or Path(args.inputs[0].split('=')[0]).stem.replace('_', '-')
            make_patch(setup, args.inputs, name, game_data_bin())
        if args.exe:
            make_exe(setup, args.exe, args.revision)
    except (Refused, OSError, ValueError, KeyError) as error:
        sys.exit(f'test-patch: {error}')
    print(f'start it: nix shell nixpkgs#python3 -c python3 {RUN_PY} --config {setup.config_path} restart --count 1')


if __name__ == '__main__':
    main()

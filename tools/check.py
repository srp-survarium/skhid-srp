"""tools/check: the checks a change affects, each skipped when it already passed on the same inputs.

    tools/check                 the checks whose inputs differ from the merge-base with origin/main
                                (committed, staged, unstaged and untracked changes)
    tools/check <name|glob>...  those checks, affected or not ('cargo:*' every crate)
    tools/check --list          the manifest, and what the current change selects
    --no-cache                  run even when a pass on the same inputs is recorded
    --base <rev>                compare with <rev> instead of the merge-base with origin/main
    -v                          stream each check's output (default: a log, its tail on failure)

The checks are tools/checks.tsv (its header says the format). A check's key is a sha256 of its row
(name, inputs, command) and of the content of every file its inputs match in the working tree, so a
pass is valid wherever the same inputs are: the cache (~/.cache/skhid-checks, $SKHID_CHECKS_CACHE)
is shared by every worktree, and its entries are safe to delete. Failures are never recorded.
Exit status: 0 every check passed, 1 a check failed, 2 a usage or manifest error.
"""
import argparse
import fnmatch
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib

TAIL_LINES = 40
LINE_WIDTH = 240  # a failure's tail: its last lines, each cut to this


def git(root, *args):
    return subprocess.run(['git', '-C', str(root), *args], check=True, capture_output=True).stdout


def glob_regex(pattern):
    """A repo path glob: `**` any path (and `dir/**` everything under dir), `*` and `?` within a name."""
    out, i = [], 0
    while i < len(pattern):
        if pattern.startswith('**/', i):
            out.append('(?:.*/)?')
            i += 3
        elif pattern.startswith('**', i):
            out.append('.*')
            i += 2
        elif pattern[i] == '*':
            out.append('[^/]*')
            i += 1
        elif pattern[i] == '?':
            out.append('[^/]')
            i += 1
        else:
            out.append(re.escape(pattern[i]))
            i += 1
    return re.compile(''.join(out) + r'\Z')


class Inputs:
    """A check's inputs: repo globs (`!glob` excludes) and files outside the tree (`{main}/...`, `~/...`)."""

    def __init__(self, words, main):
        self.words = words
        self.include, self.exclude, self.outside = [], [], []
        for word in words:
            if word.startswith('{main}/') or word.startswith('~/') or word.startswith('/'):
                self.outside.append(Path(os.path.expanduser(word.replace('{main}', str(main), 1))))
            elif word.startswith('!'):
                self.exclude.append(glob_regex(word[1:]))
            else:
                self.include.append(glob_regex(word))

    def matches(self, path):
        return any(r.match(path) for r in self.include) and not any(r.match(path) for r in self.exclude)


class Check:
    def __init__(self, name, inputs, command, notes):
        self.name, self.inputs, self.command, self.notes = name, inputs, command, notes


class Manifest:
    def __init__(self, checks, ignore):
        self.checks, self.ignore = checks, ignore


def workspace_crates(root):
    """The srp-026g workspace's crates: name -> (repo-relative dir, its inputs: its own dir, its path
    dependencies' dirs, transitively, and the files outside them its sources include)."""
    workspace = root / 'srp-026g'
    if not (workspace / 'Cargo.toml').is_file():
        return {}
    top = tomllib.loads((workspace / 'Cargo.toml').read_text())
    shared = {name: spec['path'] for name, spec in top.get('workspace', {}).get('dependencies', {}).items()
              if isinstance(spec, dict) and 'path' in spec}
    crates = {}
    for member in top['workspace']['members']:
        directory = (workspace / member).resolve()
        manifest = tomllib.loads((directory / 'Cargo.toml').read_text())
        tables = [manifest.get(kind, {}) for kind in ('dependencies', 'dev-dependencies', 'build-dependencies')]
        for target in manifest.get('target', {}).values():
            tables += [target.get(kind, {}) for kind in ('dependencies', 'dev-dependencies', 'build-dependencies')]
        deps = set()
        for table in tables:
            for name, spec in table.items():
                if isinstance(spec, dict) and 'path' in spec:
                    deps.add((directory / spec['path']).resolve())
                elif isinstance(spec, dict) and spec.get('workspace') and name in shared:
                    deps.add((workspace / shared[name]).resolve())
        crates[directory] = (manifest['package']['name'], deps)
    result = {}
    for directory, (name, _) in crates.items():
        closure, todo = set(), [directory]
        while todo:
            current = todo.pop()
            if current in closure:
                continue
            closure.add(current)
            todo += crates.get(current, ('', set()))[1]
        words = set()
        for crate_dir in closure:
            words.add(crate_dir.relative_to(root).as_posix() + '/**')
            words |= included_files(root, crate_dir)
        result[name] = (directory.relative_to(root).as_posix(), sorted(words))
    return result


INCLUDE = re.compile(r'include_(?:str|bytes)!\(\s*"([^"]+)"')
INCLUDE_CONCAT = re.compile(r'include_(?:str|bytes)!\(\s*concat!\(\s*"([^"]+)"')
MANIFEST_DIR = re.compile(r'env!\("CARGO_MANIFEST_DIR"\)\s*(?:,\s*|\)\s*\.join\(\s*)"([^"]+)"')


def included_files(root, crate_dir):
    """The files outside a crate its sources name: include_str!/include_bytes! of a literal (or a
    concat! starting with one: its folder), and literal paths joined to CARGO_MANIFEST_DIR."""
    words = set()
    for source in crate_dir.rglob('*.rs'):
        if 'target' in source.relative_to(crate_dir).parts:
            continue
        text = source.read_text(errors='replace')
        found = [(source.parent / m, False) for m in INCLUDE.findall(text)]
        found += [(source.parent / m, True) for m in INCLUDE_CONCAT.findall(text)]
        found += [(crate_dir / m.lstrip('/'), False) for m in MANIFEST_DIR.findall(text)]
        for path, prefix in found:
            path = Path(os.path.normpath(path))
            if prefix and not str(path).endswith('/'):
                path = path.parent
            if path.is_relative_to(crate_dir) or not path.is_relative_to(root) or path == root:
                continue
            relative = path.relative_to(root).as_posix()
            words.add(relative + '/**' if prefix or path.is_dir() else relative)
    return words


def read_manifest(root, main):
    path = root / 'tools' / 'checks.tsv'
    rows = []
    for number, line in enumerate(path.read_text().splitlines(), 1):
        if not line.strip() or line.startswith('#'):
            continue
        fields = line.split('\t')
        if fields[0] == 'name' and not rows:
            continue  # the header row
        if len(fields) < 3 or len(fields) > 4:
            raise ValueError(f'{path}:{number}: a row is name, inputs, command[, notes] (tab-separated)')
        rows.append((number, *[f.strip() for f in fields], *([''] * (4 - len(fields)))))
    crates = None
    checks, extras, ignore = {}, {}, []
    for number, name, inputs, command, notes in rows:
        words = inputs.split()
        if name == 'ignore':
            ignore += [glob_regex(w) for w in words]
            continue
        if command == '-':
            extras.setdefault(name, []).append((number, words))
            continue
        expansions = [(name, words, command)]
        if '{crate}' in name:
            if crates is None:
                crates = workspace_crates(root)
            expansions = [(name.replace('{crate}', crate),
                           [x for w in words for x in (crate_words if w == '{crate_inputs}' else [w])],
                           command.replace('{crate}', crate).replace('{crate_dir}', crate_dir))
                          for crate, (crate_dir, crate_words) in sorted(crates.items())]
        for check_name, check_words, check_command in expansions:
            if check_name in checks:
                raise ValueError(f'{path}:{number}: {check_name}: a second row of that name')
            checks[check_name] = [check_words, check_command, notes]
    for name, rows_of in extras.items():
        if name not in checks:
            raise ValueError(f'{path}:{rows_of[0][0]}: {name}: extra inputs for no check')
        for _, words in rows_of:
            checks[name][0] = checks[name][0] + words
    return Manifest([Check(name, Inputs(words, main), command, notes)
                     for name, (words, command, notes) in checks.items()], ignore)


def blob_id(path):
    """git's object id of a working-tree file (a symlink: its target), so a file put back as it was
    has the id it had."""
    data = os.readlink(path).encode() if path.is_symlink() else path.read_bytes()
    return hashlib.sha1(b'blob %d\0' % len(data) + data).hexdigest()


class Tree:
    """The working tree's files (tracked and untracked, not ignored) and their content ids."""

    def __init__(self, root):
        self.root = root
        self.ids = {}
        for entry in git(root, 'ls-files', '-s', '-z').split(b'\0'):
            if entry:
                meta, path = entry.split(b'\t', 1)
                self.ids[path.decode()] = meta.split()[1].decode()
        dirty = git(root, 'diff', '--name-only', '-z').split(b'\0')
        untracked = git(root, 'ls-files', '-o', '--exclude-standard', '-z').split(b'\0')
        for raw in dirty + untracked:
            if not raw:
                continue
            path = raw.decode()
            full = root / path
            if full.is_symlink() or full.is_file():
                self.ids[path] = blob_id(full)
            else:
                self.ids.pop(path, None)

    def changed(self, base):
        """The paths that differ from `base` (deleted ones too) and the untracked ones."""
        paths = git(self.root, 'diff', '--name-only', '--no-renames', '-z', base).split(b'\0')
        paths += git(self.root, 'ls-files', '-o', '--exclude-standard', '-z').split(b'\0')
        return sorted({p.decode() for p in paths if p})


def check_key(check, tree):
    digest = hashlib.sha256()
    digest.update(json.dumps(['skhid-check 1', check.name, check.inputs.words, check.command]).encode())
    for path in sorted(p for p in tree.ids if check.inputs.matches(p)):
        digest.update(f'\0{path}\0{tree.ids[path]}'.encode())
    for path in check.inputs.outside:
        digest.update(f'\0{path}\0{blob_id(path) if path.is_file() else "missing"}'.encode())
    return digest.hexdigest()


def cache_dir():
    if os.environ.get('SKHID_CHECKS_CACHE'):
        return Path(os.environ['SKHID_CHECKS_CACHE'])
    return Path(os.environ.get('XDG_CACHE_HOME') or Path.home() / '.cache') / 'skhid-checks'


def run_check(check, key, root, verbose):
    """Runs the command from the repo root; $CHECK_WORK is a new folder, removed after a pass."""
    logs = cache_dir() / 'logs'
    logs.mkdir(parents=True, exist_ok=True)
    slug = re.sub(r'[^A-Za-z0-9_.-]', '_', check.name)
    log = logs / f'{slug}-{key[:12]}.log'
    work = Path(tempfile.mkdtemp(prefix=f'check-{slug}-'))
    env = dict(os.environ, CHECK_WORK=str(work), PYTHON=sys.executable,
               PATH=os.path.dirname(sys.executable) + os.pathsep + os.environ.get('PATH', ''))
    start = time.monotonic()
    with open(log, 'wb') as out:
        if verbose:
            process = subprocess.Popen(['bash', '-c', check.command], cwd=root, env=env,
                                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            for line in process.stdout:
                out.write(line)
                sys.stdout.buffer.write(line)
                sys.stdout.flush()
            status = process.wait()
        else:
            status = subprocess.run(['bash', '-c', check.command], cwd=root, env=env,
                                    stdout=out, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL).returncode
    if status == 0:
        shutil.rmtree(work, ignore_errors=True)
    return status, time.monotonic() - start, log, work


def main(argv=None):
    parser = argparse.ArgumentParser(prog='tools/check', description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('names', nargs='*', help='checks to run (globs), affected or not')
    parser.add_argument('--list', action='store_true')
    parser.add_argument('--no-cache', action='store_true')
    parser.add_argument('--base')
    parser.add_argument('-v', '--verbose', action='store_true')
    args = parser.parse_args(argv)

    root = Path(git(Path.cwd(), 'rev-parse', '--show-toplevel').decode().strip())
    main_tree = Path(git(root, 'worktree', 'list', '--porcelain').decode().splitlines()[0].split(' ', 1)[1])
    try:
        manifest = read_manifest(root, main_tree)
    except (OSError, ValueError, KeyError, tomllib.TOMLDecodeError) as error:
        print(f'tools/check: {error}', file=sys.stderr)
        return 2
    tree = Tree(root)

    base = args.base
    if base is None:
        try:
            base = git(root, 'merge-base', 'HEAD', 'origin/main').decode().strip()
        except subprocess.CalledProcessError:
            print('tools/check: no merge-base with origin/main (git fetch origin main); comparing with HEAD',
                  file=sys.stderr)
            base = 'HEAD'
    changed = tree.changed(base)
    affected = {c.name for c in manifest.checks if any(c.inputs.matches(p) for p in changed)}
    uncovered = [p for p in changed if not any(c.inputs.matches(p) for c in manifest.checks)
                 and not any(r.match(p) for r in manifest.ignore)]

    if args.list:
        width = max(len(c.name) for c in manifest.checks)
        for c in manifest.checks:
            mark = 'affected' if c.name in affected else '-'
            print(f'{c.name:<{width}}  {mark:<8}  {c.notes}'.rstrip())
        print(f'{len(changed)} changed file(s) against {base[:12]}; {len(affected)} check(s) affected')
        for path in uncovered:
            print(f'not covered: {path}')
        return 0

    if args.names:
        unknown = [n for n in args.names if not any(fnmatch.fnmatchcase(c.name, n) for c in manifest.checks)]
        if unknown:
            print(f'tools/check: no check {", ".join(unknown)} (tools/check --list)', file=sys.stderr)
            return 2
        selected = [c for c in manifest.checks if any(fnmatch.fnmatchcase(c.name, n) for n in args.names)]
    else:
        selected = [c for c in manifest.checks if c.name in affected]
        for path in uncovered:
            print(f'not covered: {path}')

    passes = cache_dir() / 'pass'
    failed = False
    for check in manifest.checks:
        if check not in selected:
            if not args.names:
                print(f'not affected {check.name}')
            continue
        key = check_key(check, tree)
        record = passes / key
        if record.exists() and not args.no_cache:
            print(f'cached pass {check.name} ({key[:12]})')
            continue
        print(f'running {check.name} ...', flush=True)
        status, seconds, log, work = run_check(check, key, root, args.verbose)
        if status == 0 and check_key(check, Tree(root)) != key:
            print(f'ran pass {check.name} ({seconds:.1f} s; its inputs changed while it ran: not recorded)')
        elif status == 0:
            passes.mkdir(parents=True, exist_ok=True)
            record.write_text(json.dumps({'check': check.name, 'command': check.command, 'seconds': round(seconds, 1),
                                          'at': time.strftime('%Y-%m-%dT%H:%M:%S%z'), 'tree': str(root)}) + '\n')
            print(f'ran pass {check.name} ({seconds:.1f} s, {key[:12]})')
        else:
            failed = True
            if not args.verbose:
                lines = log.read_text(errors='replace').splitlines()
                for line in lines[-TAIL_LINES:]:
                    print(f'  | {line[:LINE_WIDTH]}' + (' ...' if len(line) > LINE_WIDTH else ''))
            print(f'ran FAIL {check.name} (exit {status}, {seconds:.1f} s; log {log}, work {work})')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())

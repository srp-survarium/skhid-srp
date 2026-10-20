#!/usr/bin/env python3
"""Compare two movies' ActionScript, method by method, as FFDec decompiles them.

    compare_decompiled.py --tools private/actionscript/tools.json A.swf B.swf OUT_DIR

Both movies are exported with the same FFDec (the bridge's `export`), then
every script is compared: identical text, or per method (a method's text, then
the same with parameter and local names numbered in order and whitespace
collapsed, which hides the debug names a compiler kept or dropped), plus the
script's other lines (declarations). OUT_DIR/report.txt lists every
difference with its diff. This is a diagnostic of behaviour, not a proof:
equal text after normalisation can still hide encoding differences, which the
diffs of DIFFERENT methods and declarations show for review.
"""
import argparse
import difflib
from pathlib import Path
import re
import sys

import movie_source as movie

HEAD = re.compile(r'^\s*((?:override|public|private|protected|internal|static|final|native|[\w$]+::)\s+)*'
                  r'function\s+(?:(get|set)\s+)?([\w$]+)\s*\(')


def members(text):
    """A script's methods by key ("[get|set] name [static]", numbered if repeated) and its other lines."""
    lines = text.replace('\r\n', '\n').split('\n')
    methods, other, seen, i = {}, [], {}, 0
    while i < len(lines):
        match = HEAD.match(lines[i])
        if not match or lines[i].rstrip().endswith(';'):
            if lines[i].strip():
                other.append(lines[i].strip())
            i += 1
            continue
        key = f"{match.group(2) or ''} {match.group(3)}{' static' if ' static ' in f' {lines[i]} ' else ''}"
        seen[key] = seen.get(key, 0) + 1
        if seen[key] > 1:
            key += f' #{seen[key]}'
        indent = len(lines[i]) - len(lines[i].lstrip())
        body, opened = [lines[i]], False
        i += 1
        while i < len(lines):
            body.append(lines[i])
            depth_line = len(lines[i]) - len(lines[i].lstrip()) == indent
            opened |= depth_line and lines[i].strip() == '{'
            i += 1
            if opened and depth_line and lines[i - 1].strip() == '}':
                break
        methods[key] = '\n'.join(body)
    return methods, other


def normalise(method):
    """Number parameters (P0..) and locals (L0..) in order of appearance; collapse whitespace."""
    head = method.split('\n', 1)[0]
    params = re.findall(r'([\w$]+)\s*:', head[head.index('(') + 1:head.rindex(')')]) if '(' in head else []
    names = {name: f'P{n}' for n, name in enumerate(params)}
    for n, local in enumerate(dict.fromkeys(re.findall(r'\bvar\s+([\w$]+)', method))):
        names.setdefault(local, f'L{n}')
    parts = re.split(r'("(?:[^"\\]|\\.)*")', method)
    text = ''.join(part if part.startswith('"') else
                   re.sub(r'(?<![\w$.])[\w$]+', lambda m: names.get(m.group(0), m.group(0)), part)
                   for part in parts)
    return re.sub(r'\s+', ' ', text).strip()


def compare(a, b):
    """Rows of (status, script, key, diff) for two export directories, and the identical script count."""
    rows, identical = [], 0
    for relative in sorted({p.relative_to(a) for p in a.rglob('*.as')} | {p.relative_to(b) for p in b.rglob('*.as')}):
        if not (a / relative).exists() or not (b / relative).exists():
            rows.append(('SCRIPT-ONLY-IN-' + ('B' if not (a / relative).exists() else 'A'), str(relative), '', ''))
            continue
        ta, tb = (a / relative).read_text(), (b / relative).read_text()
        if ta == tb:
            identical += 1
            continue
        ma, oa = members(ta)
        mb, ob = members(tb)
        if oa != ob:
            status = 'DECLARATIONS-ORDER' if sorted(oa) == sorted(ob) else 'DECLARATIONS'
            rows.append((status, str(relative), '', '\n'.join(difflib.unified_diff(oa, ob, 'A', 'B', lineterm='', n=0))))
        for key in sorted(ma.keys() | mb.keys()):
            if key not in ma or key not in mb:
                rows.append(('METHOD-ONLY-IN-' + ('B' if key not in ma else 'A'), str(relative), key, ''))
            elif ma[key] != mb[key]:
                same = normalise(ma[key]) == normalise(mb[key])
                diff = '' if same else '\n'.join(difflib.unified_diff(
                    ma[key].split('\n'), mb[key].split('\n'), 'A', 'B', lineterm='', n=1))
                rows.append(('NORMALISED-EQUAL' if same else 'DIFFERENT', str(relative), key, diff))
    return rows, identical


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--tools', required=True, type=Path)
    parser.add_argument('a', type=Path)
    parser.add_argument('b', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    tools = movie.Toolchain(args.tools, args.output / 'bridge')
    exports = []
    for name, path in (('a', args.a), ('b', args.b)):
        code = args.output / f'{name}-code.swf'
        movie.code_movie(path, code)
        tools.run('export', code, args.output / f'export-{name}', log=args.output / f'export-{name}.log').check_returncode()
        exports.append(args.output / f'export-{name}')
    rows, identical = compare(*exports)
    total = len({p.relative_to(exports[0]) for p in exports[0].rglob('*.as')} |
                {p.relative_to(exports[1]) for p in exports[1].rglob('*.as')})
    summary = f'{identical} of {total} scripts identical; A {movie.digest(args.a)}, B {movie.digest(args.b)}'
    with (args.output / 'report.txt').open('w') as stream:
        stream.write(summary + '\n')
        for status, script, key, diff in rows:
            stream.write(f'\n{status} {script} {key}\n' + (diff + '\n' if diff else ''))
    print(summary)
    for status, script, key, _ in rows:
        print(status, script, key)
    return 0


if __name__ == '__main__':
    sys.exit(main())

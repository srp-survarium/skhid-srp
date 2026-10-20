#!/usr/bin/env python3
"""Build changed ActionScript over a release movie, proving baseline equality and untouched-code preservation.

The input is a movie's source tree (srp-026g/flash/<movie>: every file that is
not the baseline's is compiled; changes may add methods, traits, nested
functions, constants and whole classes, and change declarations), or one
pinned feature manifest (schema 1: edits of existing method bodies). Every
class that did not change, and every record of a changed class its change
does not touch, must stay byte for byte as the release has it.
"""
import argparse
import difflib
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import zlib

import compare_decompiled as compare
import movie_source as movie
import source_patches as patches

CLASS_NAME = re.compile(r'[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)*')


def full_movie(original, code, output):
    movie.transplant(original, code, output)
    raw = output.read_bytes()
    if original.read_bytes()[:3] == b'CWS':
        output.write_bytes(b'CWS' + raw[3:8] + zlib.compress(raw[8:], 9))


def verify_container(original, built):
    old_header, old_tags = movie.split_movie(original)
    new_header, new_tags = movie.split_movie(built)
    if old_header[:4] + old_header[8:] != new_header[:4] + new_header[8:] or len(old_tags) != len(new_tags):
        raise ValueError('Feature changed movie headers or tag count')
    for (old_kind, old), (new_kind, new) in zip(old_tags, new_tags):
        if old_kind != new_kind:
            raise ValueError('Feature changed tag ordering or kinds')
        if old_kind not in (72, 82):
            if old != new:
                raise ValueError('Feature changed non-code movie data')
        elif old_kind == 82:
            def attributes(raw):
                word, = struct.unpack_from('<H', raw)
                payload = raw[6 if word & 63 == 63 else 2:]
                return payload[:payload.index(b'\0', 4) + 1]
            if attributes(old) != attributes(new):
                raise ValueError('Feature changed DoABC flags or names')


def contained(base, relative, what):
    """A relative path under `base`, refusing absolute paths and escapes."""
    path = Path(relative)
    if path.is_absolute() or '..' in path.parts or path.as_posix() != relative:
        raise ValueError(f'Invalid {what} path')
    resolved = (base / path).resolve()
    if not resolved.is_relative_to(base.resolve()):
        raise ValueError(f'{what} escapes its directory')
    return resolved


def class_plans(reconstruction_evidence):
    compilers = {entry['class']: entry.get('compiler', 'flex-strict') for entry in reconstruction_evidence['classes']}
    compositions = {entry['class']: patches.composition_plan(entry) for entry in reconstruction_evidence['classes']}
    reservations = {entry['class']: patches.reservation_plan(entry) for entry in reconstruction_evidence['classes']}
    return compilers, compositions, reservations


def feature_inputs(path, pinned, reconstructed, reconstruction_evidence, project):
    """Schema 1: one feature editing existing method bodies, every hash pinned."""
    raw = path.read_bytes()
    spec = json.loads(raw)
    if spec.get('schema') != 1 or spec.get('release') != pinned['release'] or spec.get('movie_sha256') != pinned['movie_sha256']:
        raise ValueError('Feature targets a different release')
    if spec.get('reconstruction_sha256') != reconstruction_evidence['manifest_sha256']:
        raise ValueError('Feature targets a different reconstruction manifest')
    seen, inputs = set(), []
    compilers, compositions, reservations = class_plans(reconstruction_evidence)
    for entry in spec['classes']:
        cls = entry['class']
        relative = cls.replace('.', '/') + '.as'
        if cls in seen or relative not in pinned['sources']:
            raise ValueError('Unknown or duplicate feature class')
        seen.add(cls)
        baseline = reconstructed.get(relative, (project / 'pristine' / relative).read_bytes())
        if patches.sha(baseline) != entry['baseline_sha256']:
            raise ValueError('Feature baseline source hash differs')
        diff = contained(path.parent, entry['patch'], 'Feature patch').read_bytes()
        if patches.sha(diff) != entry['patch_sha256']:
            raise ValueError('Feature patch hash differs')
        changed = patches.apply_patch(baseline, diff, relative)
        if patches.sha(changed) != entry['result_sha256'] or changed == baseline:
            raise ValueError('Feature result hash differs or source is unchanged')
        permissions, methods = [], set()
        for method in entry['methods']:
            owner = method['owner']
            if owner in methods or any(c in owner for c in '\t\r\n'):
                raise ValueError('Duplicate or invalid method permission')
            methods.add(owner)
            calls = method.get('calls_casts', {})
            before, after = calls.get('before', '-'), calls.get('after', '-')
            if any(c in before + after for c in '\t\r\n'):
                raise ValueError('Invalid call/cast fingerprints')
            permissions.append('\t'.join((owner, before, after)))
        if not permissions:
            raise ValueError('Feature must name its edited methods')
        inputs.append({'entry': entry, 'class': cls, 'kind': 'game', 'features': [path.parent.name],
                       'baseline': baseline, 'changed': changed, 'patch': diff,
                       'permissions': ('\n'.join(permissions) + '\n').encode(),
                       'compiler': compilers.get(cls, 'flex-strict'),
                       'composition': compositions.get(cls), 'reservations': reservations.get(cls)})
    if not inputs:
        raise ValueError('Feature changes no classes')
    return raw, inputs


def duplicate_functions(source):
    """FFDec's compiler keeps the last of two methods of one name: refuse that."""
    names = {}
    for name in re.findall(r'\bfunction\s+(?!get\b|set\b)(\w+)\s*\(', source):
        names[name] = names.get(name, 0) + 1
    return sorted(name for name, count in names.items() if count > 1)


def method_name(key):
    """The member name of a compare_decompiled.members key ("get x", " x static", " x #2")."""
    words = [w for w in key.split('#')[0].split() if w not in ('get', 'set', 'static')]
    return words[0]


def source_permissions(cls, baseline, changed):
    """A game class's permissions, from its baseline and changed source.

    Every method whose text changed may change (its call/cast sequence is
    reported for review, not refused); a method, var or const that is new or
    whose declaration changed is a member the class may add or redeclare.
    A removed method is refused; everything else must stay byte for byte."""
    before, before_lines = compare.members(baseline.decode('utf-8'))
    after, after_lines = compare.members(changed.decode('utf-8'))
    gone = sorted(before.keys() - after.keys())
    if gone:
        raise ValueError(f'{cls}: methods removed ({", ".join(gone)}); a feature may not remove one')
    edits = sorted({method_name(k) for k in before if before[k] != after[k]})
    members = {method_name(k) for k in after.keys() - before.keys()}
    for line in set(before_lines) ^ set(after_lines):
        found = re.search(r'\b(?:var|const)\s+([\w$]+)', line)
        if found:
            members.add(found.group(1))
    edits = [name for name in edits if name not in members]
    lines = ['\t'.join(('edit', cls, name, '?', '?')) for name in edits]
    lines += [f'member\t{cls}\t{name}' for name in sorted(members)]
    if not lines:
        raise ValueError(f'{cls}: its source changed, but no method or declaration did')
    return lines, edits, sorted(members)


def tree_inputs(movie_dir, pinned, reconstructed, reconstruction_evidence, project):
    """A movie's source tree (srp-026g/flash/<movie>) against its baseline.

    Every file whose hash is not the baseline's is compiled: a new file is a
    new class (srp.json names the class whose ABC takes it), a changed file of
    srp's own classes (srp.json) is owned whole, and a changed game class may
    change the methods and declarations its source diff touches. Unchanged
    files are not compiled: the release's bytes are what they compile to
    (flash_tree.py full)."""
    spec = json.loads((movie_dir / 'movie.json').read_bytes())
    if spec.get('release') != pinned['release'] or spec.get('movie_sha256') != pinned['movie_sha256']:
        raise ValueError('The tree is of another release movie')
    if spec.get('reconstruction_sha256') != reconstruction_evidence['manifest_sha256']:
        raise ValueError('The tree was exported with another reconstruction manifest')
    srp_path = movie_dir / 'srp.json'
    srp = json.loads(srp_path.read_bytes()) if srp_path.exists() else {'classes': []}
    own = {}
    for entry in srp['classes']:
        if not CLASS_NAME.fullmatch(entry.get('class', '')) or entry['class'] in own or set(entry) - {'class', 'abc_with'}:
            raise ValueError(f'srp.json: invalid or repeated entry {entry}')
        own[entry['class']] = entry.get('abc_with')
    files = {}
    root = movie_dir / 'src'
    for path in sorted(root.rglob('*')):
        if path.is_file():
            files[path.relative_to(root).as_posix()] = path.read_bytes()
    baseline = spec['sources']
    if baseline.keys() != pinned['sources'].keys():
        raise ValueError('The tree baseline and the pinned export name different scripts')
    removed = sorted(set(baseline) - set(files))
    if removed:
        raise ValueError('Scripts removed from the tree: ' + ', '.join(removed))
    compilers, compositions, reservations = class_plans(reconstruction_evidence)
    inputs = []
    for cls, anchor in own.items():
        relative = cls.replace('.', '/') + '.as'
        if relative in baseline:
            if anchor is not None:
                raise ValueError(f'srp.json: {cls} is a class of the release; it takes no abc_with')
            continue
        if anchor is None or anchor.replace('.', '/') + '.as' not in baseline:
            raise ValueError(f'srp.json: the new class {cls} needs abc_with, a class of the release')
        if relative not in files:
            raise ValueError(f'srp.json names {cls}, which the tree does not have')
        inputs.append({'class': cls, 'kind': 'new', 'baseline': None, 'changed': files[relative],
                       'source_name': Path(relative).name, 'anchor': anchor, 'features': ['tree'],
                       'patch': b''})
    for relative in sorted(set(files) - set(baseline)):
        cls = relative[:-3].replace('/', '.')
        if cls not in own:
            raise ValueError(f'{relative} is new: name it, and the class whose ABC takes it, in srp.json')
    owned, game = [], []
    for relative, data in files.items():
        if relative not in baseline or patches.sha(data) == baseline[relative]:
            continue
        cls = relative[:-3].replace('/', '.')
        # The tree keeps LF line ends; FFDec exports CRLF (flash_tree.py).
        base = reconstructed.get(relative, (project / 'pristine' / relative).read_bytes()).replace(b'\r\n', b'\n')
        if patches.sha(base) != baseline[relative]:
            raise ValueError(f'{relative}: the exported baseline is not the tree baseline')
        item = {'class': cls, 'baseline': base, 'changed': data, 'features': ['tree'],
                'patch': ''.join(difflib.unified_diff(base.decode('utf-8').splitlines(True), data.decode('utf-8').splitlines(True),
                                                      f'a/{relative}', f'b/{relative}')).encode('utf-8'),
                'compiler': compilers.get(cls, 'flex-strict'), 'composition': compositions.get(cls),
                'reservations': reservations.get(cls)}
        if cls in own:
            item.update(kind='owned', permissions=f'class\t{cls}\n'.encode())
            owned.append(item)
        else:
            lines, item['methods'], item['members'] = source_permissions(cls, base, data)
            item.update(kind='game', permissions=('\n'.join(lines) + '\n').encode())
            game.append(item)
    for item in inputs:
        item.update(compiler='ffdec', composition=None, reservations=None, permissions=f'class\t{item["class"]}\n'.encode())
    for item in inputs + owned:
        twice = duplicate_functions(item['changed'].decode('utf-8'))
        if twice:
            raise ValueError(f'{item["class"]} defines these more than once (FFDec keeps the last): ' + ', '.join(twice))
    # New classes first (game classes compiled by Flex need them), then srp's
    # own classes, then the game classes.
    inputs += owned + game
    if not inputs:
        raise ValueError('The tree is the baseline: nothing to build')
    raw = json.dumps({'movie': spec['movie'], 'classes': [
        {k: item[k] for k in ('class', 'kind', 'compiler', 'methods', 'members', 'anchor') if k in item}
        for item in inputs]}, indent=2).encode()
    return raw, inputs


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--tools', required=True, type=Path)
    parser.add_argument('--reconstruction', required=True, type=Path)
    parser.add_argument('--jobs', type=int, default=1, help='Concurrent baseline compilations (1-8)')
    parser.add_argument('project', type=Path)
    parser.add_argument('feature', type=Path, help='A source tree (srp-026g/flash/<movie>) or a schema 1 feature manifest')
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    if not 1 <= args.jobs <= 8:
        raise ValueError('--jobs must be between 1 and 8')
    build(args.tools, args.reconstruction, args.project, args.feature, args.output, args.jobs)
    return 0


def review_rows(folder, report_path):
    """Unreviewed call/cast changes of a class link, with their before/after assembly."""
    def plain(owner):
        # Compiler output (before linking) can number a private namespace.
        return re.sub(r'(PrivateNamespace\("[^"]*"),"\d+"\)', r'\1)', owner)

    def assembly(snapshot):
        rows = {}
        table = folder / snapshot / 'methods.tsv'
        if table.exists():
            for line in table.read_text().splitlines()[1:]:
                abc, method, owner = line.split('\t')[:3]
                rows[plain(owner)] = folder / snapshot / f'abc-{int(abc):04d}' / f'method-{int(method):05d}.asm'
        return rows
    before, after = assembly('baseline-snapshot'), assembly('feature-snapshot')
    for line in report_path.read_text().splitlines()[1:]:
        fields = line.split('\t')
        if fields[1] == 'unreviewed':
            yield fields[0], fields[4], fields[5], before.get(plain(fields[0]), '-'), after.get(plain(fields[0]), '-')


def build(tools_path, reconstruction, project, feature, output, jobs=1, log=print):
    """Build `feature` (a source tree or a manifest); returns the report written to `output`."""
    project, output = project.resolve(), output.resolve()
    pinned = movie.verify_project(project)
    reconstructed, evidence = patches.load_sources(reconstruction, project, pinned)
    schema = 1
    if feature.is_dir():
        schema = 'tree'
        feature_raw, inputs = tree_inputs(feature, pinned, reconstructed, evidence, project)
    else:
        feature_raw, inputs = feature_inputs(feature, pinned, reconstructed, evidence, project)
    output.mkdir(parents=True, exist_ok=False)
    (output / 'feature.json' if schema == 1 else output / 'tree-changes.json').write_bytes(feature_raw)
    shutil.copyfile(__file__, output / 'build_feature.py')
    correction_dir = output / 'reconstruction'
    correction_dir.mkdir()
    shutil.copyfile(reconstruction, correction_dir / 'manifest.json')
    for entry in evidence['classes']:
        if 'patch' not in entry:
            continue
        dest = correction_dir / entry['patch']
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(reconstruction.parent / entry['patch'], dest)
    if patches.load_sources(correction_dir / 'manifest.json', project, pinned) != (reconstructed, evidence):
        raise ValueError('Reconstruction changed while recording inputs')
    tools = movie.Toolchain(tools_path, output / 'bridge')
    original = project / 'original.swf'
    reference = output / 'original-code.swf'
    movie.code_movie(original, reference)
    report = {'schema': schema, 'release': pinned['release'], 'movie_sha256': pinned['movie_sha256'],
              'feature_manifest_sha256': patches.sha(feature_raw), 'reconstruction': evidence,
              'driver_sha256': movie.digest(output / 'build_feature.py'), 'toolchain': tools.identity(),
              'classes': [], 'complete': False, 'runtime_verified': False}
    movie.write_json(output / 'report.json', report)
    folders = {}
    for number, item in enumerate(inputs):
        cls = item['class']
        folder = output / f'class-{number:04d}'
        folder.mkdir()
        folders[cls] = folder
        files = [('feature.as', item['changed']), ('permissions.tsv', item['permissions'])]
        if item['baseline'] is not None:
            files += [('baseline.as', item['baseline'])]
        if item['patch']:
            files += [('source.patch', item['patch'])]
        if item['kind'] == 'new':
            files += [(item['source_name'], item['changed'])]
        for name, value in files:
            (folder / name).write_bytes(value)
        if item['composition'] is not None:
            (folder / 'compiler-composition.tsv').write_bytes(item['composition'])
        if item['reservations'] is not None:
            (folder / 'method-reservations.tsv').write_bytes(item['reservations'])
        row = {'class': cls, 'kind': item['kind'], 'features': item['features'], 'directory': folder.name,
               'compiler': item['compiler'], 'status': 'pending',
               'feature_source_sha256': patches.sha(item['changed'])}
        if item['baseline'] is not None:
            row['baseline_source_sha256'] = patches.sha(item['baseline'])
        if item['kind'] == 'new':
            row['abc_with'] = item['anchor']
        if item['composition'] is not None:
            row['composition_sha256'] = patches.sha(item['composition'])
        if item['reservations'] is not None:
            row['reservations_sha256'] = patches.sha(item['reservations'])
        report['classes'].append(row)
    movie.write_json(output / 'report.json', report)
    rows = {row['class']: row for row in report['classes']}

    def run(folder, cls, *arguments, log_name):
        result = tools.run(*arguments, log=folder / log_name)
        if result.returncode:
            raise ValueError(f'{cls}: {arguments[0]} failed; see {folder / log_name}')

    def compile_source(item, input_movie, name):
        cls, folder, row = item['class'], folders[item['class']], rows[item['class']]
        composition_args = [folder / 'compiler-composition.tsv'] if item['composition'] is not None else []
        reserved = item['reservations'] is not None
        compiled = folder / f'{name}-{"unreserved" if reserved else "compiled"}.swf'
        snapshot = folder / f'{name}-{"unreserved-snapshot" if reserved else "snapshot"}'
        run(folder, cls, 'compile', input_movie, compiled, cls, folder / f'{name}.as', snapshot,
            item['compiler'], tools.config.get('flex', ''), *composition_args, log_name=f'{name}-compile.log')
        if reserved:
            row[f'{name}_unreserved_code_sha256'] = movie.digest(compiled)
            run(folder, cls, 'reserve', compiled, folder / f'{name}-compiled.swf', cls,
                folder / 'method-reservations.tsv', folder / f'{name}-snapshot', log_name=f'{name}-reservation.log')

    def check_baseline(item):
        # Compile the baseline afresh: saved measurements cannot authorize a
        # feature built with a changed compiler or reconstruction source.
        cls, folder = item['class'], folders[item['class']]
        compile_source(item, reference, 'baseline')
        run(folder, cls, 'link-layout', reference, folder / 'baseline-compiled.swf', folder / 'baseline-linked.swf',
            cls, log_name='baseline-link.log')
        full_movie(original, folder / 'baseline-linked.swf', folder / 'baseline-full.swf')
        exact = (folder / 'baseline-full.swf').read_bytes() == original.read_bytes()
        (folder / 'baseline-full.swf').unlink()
        if not exact:
            raise ValueError(f'{cls}: baseline does not reproduce the complete compressed release movie')

    def refuse(cls, error):
        rows[cls].update(status='refused', error=str(error))
        movie.write_json(output / 'report.json', report)

    existing = [item for item in inputs if item['kind'] != 'new']
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        futures = {item['class']: pool.submit(check_baseline, item) for item in existing}
        failures = []
        for cls, future in futures.items():
            try:
                future.result()
                rows[cls].update(status='baseline_exact', baseline_exact=True)
            except (ValueError, OSError, subprocess.TimeoutExpired) as error:
                refuse(cls, error)
                failures.append(error)
        movie.write_json(output / 'report.json', report)
        if failures:
            raise failures[0]
    if existing:
        log(f'Baselines exact: {len(existing)} classes reproduce the complete compressed release movie')

    current = reference
    linked_reports = []
    for item in inputs:
        cls, folder, row = item['class'], folders[item['class']], rows[item['class']]
        try:
            if item['kind'] == 'new':
                run(folder, cls, 'add-script', current, folder / 'feature-compiled.swf', item['anchor'],
                    folder / item['source_name'], folder / 'feature-snapshot', log_name='feature-compile.log')
                link_class = item['anchor']
            else:
                compile_source(item, current, 'feature')
                link_class = cls
            run(folder, cls, 'link-feature', current, folder / 'feature-compiled.swf', folder / 'feature-linked.swf',
                link_class, folder / 'permissions.tsv', log_name='feature-link.log')
            if (folder / 'feature.as').read_bytes() != item['changed'] or (
                    item['baseline'] is not None and (folder / 'baseline.as').read_bytes() != item['baseline']):
                raise ValueError('Source changed during compilation')
            current = folder / 'feature-linked.swf'
            linked_reports.append(folder / 'feature-linked.swf.methods.tsv')
            row.update(status='verified', compiled_code_sha256=movie.digest(folder / 'feature-compiled.swf'),
                       linked_code_sha256=movie.digest(current))
            log(f'{cls} ({item["kind"]}): linked; every record the features may not change is the release\'s')
        except (ValueError, OSError, subprocess.TimeoutExpired) as error:
            refuse(cls, error)
            raise
        movie.write_json(output / 'report.json', report)
    unreviewed = [(item['class'], *row) for item in inputs
                  for row in review_rows(folders[item['class']], folders[item['class']] / 'feature-linked.swf.methods.tsv')]
    # Changed call, cast and construction sequences of edited game methods,
    # with their before/after assembly, for the review of the source diff.
    with (output / 'review-calls.tsv').open('w') as stream:
        stream.write('class\tmethod\tbefore_calls_casts\tafter_calls_casts\tbefore_assembly\tafter_assembly\n')
        for row in unreviewed:
            stream.write('\t'.join(map(str, row)) + '\n')
    # Publish a local build artifact only after every class passes all gates.
    pending = output / 'verified-movie.tmp'
    full_movie(original, current, pending)
    if pending.read_bytes() == original.read_bytes():
        raise ValueError('Feature produces no movie change')
    verify_container(original, pending)
    final_code = output / 'verified-code.swf'
    movie.code_movie(pending, final_code)
    for code, snapshot in ((reference, output / 'original-snapshot'), (final_code, output / 'verified-snapshot')):
        result = tools.run('snapshot', code, snapshot, log=snapshot.with_suffix('.log'))
        if result.returncode:
            raise ValueError('Cannot independently verify the final movie')
    comparison = movie.compare(original, pending, output / 'original-snapshot', output / 'verified-snapshot')
    # The final movie, independently reparsed: every change must be one a
    # class link reported (and so permitted); nothing may go missing.
    permitted = set()
    for path in linked_reports:
        for line in path.read_text().splitlines()[1:]:
            fields = line.split('\t')
            permitted.add((fields[0], 'added' if fields[1] == 'added' else 'changed'))
    if schema == 1 and comparison['reference_methods'] != comparison['candidate_methods']:
        raise ValueError('Final movie changed method ownership')
    for change in comparison['method_changes']:
        if (change['owner'], change['status']) not in permitted:
            raise ValueError(f"Final movie has a method {change['status']} that no feature link permitted: {change['owner']}")
    movie.write_json(output / 'comparison.json', comparison)
    with (output / 'changes.tsv').open('w') as stream:
        stream.write('abc\tmethod\tstatus\n')
        for change in comparison['method_changes']:
            stream.write(f"{change['abc']}\t{change['owner']}\t{change['status']}\n")
    pending.rename(output / 'feature.swf')
    statuses = [change['status'] for change in comparison['method_changes']]
    report.update(complete=True, feature_movie_sha256=movie.digest(output / 'feature.swf'),
                  changed_methods=statuses.count('changed'), added_methods=statuses.count('added'),
                  changed_abc_blocks=comparison['changed_abc_blocks'], noncode_identical=True)
    movie.write_json(output / 'report.json', report)
    return report


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        sys.exit(str(error))

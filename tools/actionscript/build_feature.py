#!/usr/bin/env python3
"""Build source features over the reconstructed tree, proving baseline equality and untouched-code preservation.

The input is either one pinned feature manifest (schema 1: edits of existing
method bodies) or an ordered build list of features (schema 2), which may also
add methods, traits, nested functions, constants and whole classes, and change
declarations. Every class a feature does not name, and every record of a named
class the feature does not permit, must stay byte for byte as the release has it.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import zlib

import movie_source as movie
import source_patches as patches

CLASS_NAME = re.compile(r'[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)*')
MEMBER_NAME = re.compile(r'[A-Za-z_$][\w$]*')
FEATURE_NAME = re.compile(r'[a-z0-9][a-z0-9-]*')


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


def build_list_inputs(path, pinned, reconstructed, reconstruction_evidence, project, review=False):
    """Schema 2: features in order, each over the source the ones before it left.

    A feature's class entry is one of:
      {"class": C, "patch": P, "methods": [...], "members": [...]}
          a game class: a diff of its current source; `methods` names the
          existing methods whose bodies change, `members` the members it adds
          or redeclares (their traits, signatures and bodies, nested included);
      {"class": C, "source": S}
          a repository-authored class of the release (SrpRooms...): its whole
          source is the repository's file, and the feature owns the class;
      {"class": C, "new": true, "source": S, "abc_with": A}
          a new repository-authored class, compiled by FFDec into the ABC of
          class A, after all of that ABC's records.
    `reviewed_calls` of the build list records, per "Class.method", the
    reviewed call/cast fingerprints of an edited method whose call, cast or
    construction sequence changes (the final, combined source's). With
    `review`, an edited method without them is reported, not refused.
    """
    raw = path.read_bytes()
    spec = json.loads(raw)
    if spec.get('schema') != 2 or spec.get('release') != pinned['release'] or spec.get('movie_sha256') != pinned['movie_sha256']:
        raise ValueError('Build list targets a different release')
    if spec.get('reconstruction_sha256') != reconstruction_evidence['manifest_sha256']:
        raise ValueError('Build list targets a different reconstruction manifest')
    root = path.parent
    features = spec.get('features')
    if not isinstance(features, list) or not features or len(set(features)) != len(features):
        raise ValueError('Build list names no features, or one twice')
    compilers, compositions, reservations = class_plans(reconstruction_evidence)
    state, order, manifests = {}, [], {}
    for name in features:
        if not isinstance(name, str) or not FEATURE_NAME.fullmatch(name):
            raise ValueError(f'Invalid feature name {name!r}')
        folder = contained(root, f'features/{name}', 'Feature')
        manifest = (folder / 'feature.json').read_bytes()
        manifests[name] = manifest
        feature = json.loads(manifest)
        if feature.get('schema') != 2 or not feature.get('classes'):
            raise ValueError(f'Feature {name} is not a schema 2 feature with classes')
        seen = set()
        for entry in feature['classes']:
            cls = entry.get('class', '')
            if not CLASS_NAME.fullmatch(cls) or cls in seen:
                raise ValueError(f'Feature {name}: invalid or repeated class {cls!r}')
            seen.add(cls)
            relative = cls.replace('.', '/') + '.as'
            keys = set(entry) - {'class', 'description'}
            if entry.get('new'):
                if keys != {'new', 'source', 'abc_with'}:
                    raise ValueError(f'Feature {name}: a new class takes source and abc_with')
                if relative in pinned['sources'] or cls in state:
                    raise ValueError(f'Feature {name}: {cls} is not a new class')
                if entry['abc_with'].replace('.', '/') + '.as' not in pinned['sources']:
                    raise ValueError(f'Feature {name}: unknown abc_with class for {cls}')
                source_path = contained(root, entry['source'], 'Class source')
                if source_path.name != cls.rsplit('.', 1)[-1] + '.as':
                    raise ValueError(f'Feature {name}: {cls} must be in a file of its name')
                state[cls] = {'class': cls, 'kind': 'new', 'baseline': None, 'changed': source_path.read_bytes(),
                              'source_name': source_path.name, 'anchor': entry['abc_with'], 'features': [name],
                              'methods': [], 'members': [], 'patches': []}
                order.append(cls)
                continue
            if relative not in pinned['sources'] and cls not in state:
                raise ValueError(f'Feature {name}: unknown class {cls}')
            if cls not in state:
                baseline = reconstructed.get(relative, (project / 'pristine' / relative).read_bytes())
                state[cls] = {'class': cls, 'kind': None, 'baseline': baseline, 'changed': baseline,
                              'features': [], 'methods': [], 'members': [], 'patches': []}
                order.append(cls)
            item = state[cls]
            item['features'].append(name)
            if 'source' in entry:
                if keys != {'source'} or item['kind'] == 'game':
                    raise ValueError(f'Feature {name}: {cls} takes its whole source, and nothing else')
                item['changed'] = contained(root, entry['source'], 'Class source').read_bytes()
                if item['kind'] != 'new':
                    item['kind'] = 'owned'
                continue
            if keys - {'patch', 'methods', 'members'} or 'patch' not in entry or item['kind'] in ('owned', 'new'):
                raise ValueError(f'Feature {name}: {cls} needs a patch (and methods/members) for a game class')
            item['kind'] = 'game'
            diff = contained(folder, entry['patch'], 'Feature patch').read_bytes()
            changed = patches.apply_patch(item['changed'], diff, relative)
            if changed == item['changed']:
                raise ValueError(f'Feature {name}: the patch of {cls} changes nothing')
            item['changed'] = changed
            item['patches'].append((name, diff))
            for key in ('methods', 'members'):
                values = entry.get(key, [])
                if not isinstance(values, list) or any(not isinstance(v, str) or not MEMBER_NAME.fullmatch(v) for v in values):
                    raise ValueError(f'Feature {name}: invalid {key} of {cls}')
                item[key] += [v for v in values if v not in item[key]]
            if not item['methods'] and not item['members']:
                raise ValueError(f'Feature {name}: {cls} names no edited method or member')
    reviewed = spec.get('reviewed_calls', {})
    used = set()
    inputs = []
    for cls in order:
        item = state[cls]
        if item['kind'] in ('owned', 'new'):
            twice = duplicate_functions(item['changed'].decode('utf-8'))
            if twice:
                raise ValueError(f'{cls} defines these more than once (FFDec keeps the last): ' + ', '.join(twice))
            lines = [f'class\t{cls}']
        else:
            if item['changed'] == item['baseline']:
                raise ValueError(f'The features leave {cls} unchanged')
            lines = []
            for method in item['methods']:
                calls = reviewed.get(f'{cls}.{method}', {})
                if calls:
                    used.add(f'{cls}.{method}')
                unset = '?' if review else '-'
                before, after = calls.get('before', unset), calls.get('after', unset)
                if not all(re.fullmatch(r'-|\?|[0-9a-f]{64}', v) for v in (before, after)) or calls and not all(
                        re.fullmatch(r'[0-9a-f]{64}', v) for v in (before, after)):
                    raise ValueError(f'Invalid reviewed calls of {cls}.{method}')
                lines.append('\t'.join(('edit', cls, method, before, after)))
            lines += [f'member\t{cls}\t{member}' for member in item['members']]
        item['permissions'] = ('\n'.join(lines) + '\n').encode()
        item['compiler'] = 'ffdec' if item['kind'] == 'new' else compilers.get(cls, 'flex-strict')
        item['composition'] = None if item['kind'] == 'new' else compositions.get(cls)
        item['reservations'] = None if item['kind'] == 'new' else reservations.get(cls)
        item['patch'] = b''.join(diff for _, diff in item['patches'])
        inputs.append(item)
    if set(reviewed) - used:
        raise ValueError('reviewed_calls names methods no feature edits: ' + ', '.join(sorted(set(reviewed) - used)))
    # New classes first (game classes compiled by Flex need them), then the
    # repository-authored ones, then the game classes; each kind in list order.
    rank = {'new': 0, 'owned': 1, 'game': 2}
    inputs.sort(key=lambda item: rank[item['kind']])
    return raw, inputs, manifests


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--tools', required=True, type=Path)
    parser.add_argument('--reconstruction', required=True, type=Path)
    parser.add_argument('--jobs', type=int, default=1, help='Concurrent baseline compilations (1-8)')
    parser.add_argument('--review-calls', action='store_true',
                        help='Report every unreviewed call/cast change of a build list (writes review-calls.tsv; no movie)')
    parser.add_argument('project', type=Path)
    parser.add_argument('feature', type=Path, help='A schema 1 feature manifest or a schema 2 build list')
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    if not 1 <= args.jobs <= 8:
        raise ValueError('--jobs must be between 1 and 8')
    build(args.tools, args.reconstruction, args.project, args.feature, args.output, args.jobs, review=args.review_calls)
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


def build(tools_path, reconstruction, project, feature, output, jobs=1, log=print, review=False):
    """Build `feature` (a manifest or build list); returns the report written to `output`."""
    project, output = project.resolve(), output.resolve()
    pinned = movie.verify_project(project)
    reconstructed, evidence = patches.load_sources(reconstruction, project, pinned)
    schema = json.loads(feature.read_bytes()).get('schema')
    manifests = {}
    if schema == 2:
        feature_raw, inputs, manifests = build_list_inputs(feature, pinned, reconstructed, evidence, project, review)
    else:
        feature_raw, inputs = feature_inputs(feature, pinned, reconstructed, evidence, project)
    output.mkdir(parents=True, exist_ok=False)
    (output / 'feature.json').write_bytes(feature_raw)
    for name, manifest in manifests.items():
        (output / 'features' / name).mkdir(parents=True)
        (output / 'features' / name / 'feature.json').write_bytes(manifest)
    shutil.copyfile(__file__, output / 'build_feature.py')
    correction_dir = output / 'reconstruction'
    correction_dir.mkdir()
    shutil.copyfile(reconstruction, correction_dir / 'manifest.json')
    for entry in evidence['classes']:
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
              'feature_manifests': {name: patches.sha(raw) for name, raw in manifests.items()},
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
    if review:
        with (output / 'review-calls.tsv').open('w') as stream:
            stream.write('class\tmethod\tbefore_calls_casts\tafter_calls_casts\tbefore_assembly\tafter_assembly\n')
            for row in unreviewed:
                stream.write('\t'.join(map(str, row)) + '\n')
        raise ValueError(f'Review mode: {len(unreviewed)} edited methods change their calls/casts; see '
                         f'{output / "review-calls.tsv"} (no movie is written in review mode)')
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
    if schema != 2 and comparison['reference_methods'] != comparison['candidate_methods']:
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

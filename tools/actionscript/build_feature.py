#!/usr/bin/env python3
"""Build pinned source diffs, proving baseline equality and untouched-code preservation."""
import argparse
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import zlib

import movie_source as movie
import source_patches as patches


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


def feature_inputs(path, pinned, reconstructed, reconstruction_evidence, project):
    raw = path.read_bytes()
    spec = json.loads(raw)
    if spec.get('schema') != 1 or spec.get('release') != pinned['release'] or spec.get('movie_sha256') != pinned['movie_sha256']:
        raise ValueError('Feature targets a different release')
    if spec.get('reconstruction_sha256') != reconstruction_evidence['manifest_sha256']:
        raise ValueError('Feature targets a different reconstruction manifest')
    seen, inputs = set(), []
    compilers = {entry['class']: entry.get('compiler', 'flex-strict') for entry in reconstruction_evidence['classes']}
    compositions = {entry['class']: patches.composition_plan(entry) for entry in reconstruction_evidence['classes']}
    for entry in spec['classes']:
        cls = entry['class']
        relative = cls.replace('.', '/') + '.as'
        if cls in seen or relative not in pinned['sources']:
            raise ValueError('Unknown or duplicate feature class')
        seen.add(cls)
        baseline = reconstructed.get(relative, (project / 'pristine' / relative).read_bytes())
        if patches.sha(baseline) != entry['baseline_sha256']:
            raise ValueError('Feature baseline source hash differs')
        patch_path = Path(entry['patch'])
        if patch_path.is_absolute() or '..' in patch_path.parts or patch_path.as_posix() != entry['patch']:
            raise ValueError('Invalid feature patch path')
        source_path = path.parent / patch_path
        if not source_path.resolve().is_relative_to(path.parent.resolve()):
            raise ValueError('Feature patch escapes its directory')
        diff = source_path.read_bytes()
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
        inputs.append({'entry': entry, 'baseline': baseline, 'changed': changed, 'patch': diff,
                       'permissions': ('\n'.join(permissions) + '\n').encode(),
                       'compiler': compilers.get(cls, 'flex-strict'),
                       'composition': compositions.get(cls)})
    if not inputs:
        raise ValueError('Feature changes no classes')
    return raw, inputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tools', required=True, type=Path)
    parser.add_argument('--reconstruction', required=True, type=Path)
    parser.add_argument('project', type=Path)
    parser.add_argument('feature', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    project, output = args.project.resolve(), args.output.resolve()
    pinned = movie.verify_project(project)
    reconstructed, evidence = patches.load_sources(args.reconstruction, project, pinned)
    feature_raw, inputs = feature_inputs(args.feature, pinned, reconstructed, evidence, project)
    output.mkdir(parents=True, exist_ok=False)
    (output / 'feature.json').write_bytes(feature_raw)
    shutil.copyfile(__file__, output / 'build_feature.py')
    correction_dir = output / 'reconstruction'
    correction_dir.mkdir()
    shutil.copyfile(args.reconstruction, correction_dir / 'manifest.json')
    for entry in evidence['classes']:
        dest = correction_dir / entry['patch']
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(args.reconstruction.parent / entry['patch'], dest)
    if patches.load_sources(correction_dir / 'manifest.json', project, pinned) != (reconstructed, evidence):
        raise ValueError('Reconstruction changed while recording inputs')
    tools = movie.Toolchain(args.tools, output / 'bridge')
    original = project / 'original.swf'
    reference = output / 'original-code.swf'
    movie.code_movie(original, reference)
    report = {'schema': 1, 'release': pinned['release'], 'movie_sha256': pinned['movie_sha256'],
              'feature_manifest_sha256': patches.sha(feature_raw), 'reconstruction': evidence,
              'driver_sha256': movie.digest(output / 'build_feature.py'), 'toolchain': tools.identity(),
              'classes': [], 'complete': False, 'runtime_verified': False}
    movie.write_json(output / 'report.json', report)
    current = reference
    for number, item in enumerate(inputs):
        entry, compiler = item['entry'], item['compiler']
        cls = entry['class']
        folder = output / f'class-{number:04d}'
        folder.mkdir()
        for name, value in [('baseline.as', item['baseline']), ('feature.as', item['changed']),
                            ('source.patch', item['patch']), ('permissions.tsv', item['permissions'])]:
            (folder / name).write_bytes(value)
        composition_args = []
        if item['composition'] is not None:
            plan_path = folder / 'compiler-composition.tsv'
            plan_path.write_bytes(item['composition'])
            composition_args = [plan_path]
        row = {'class': cls, 'directory': folder.name, 'compiler': compiler, 'status': 'checking_baseline',
               'baseline_source_sha256': patches.sha(item['baseline']), 'feature_source_sha256': patches.sha(item['changed'])}
        if composition_args:
            row['composition_sha256'] = movie.digest(composition_args[0])
        report['classes'].append(row)
        movie.write_json(output / 'report.json', report)

        def run(*arguments, log):
            result = tools.run(*arguments, log=folder / log)
            if result.returncode:
                raise ValueError(f'{cls}: {arguments[0]} failed; see {folder / log}')

        try:
            # Compile the baseline afresh: saved measurements cannot authorize a
            # feature built with a changed compiler or reconstruction source.
            run('compile', reference, folder / 'baseline-compiled.swf', cls, folder / 'baseline.as',
                folder / 'baseline-snapshot', compiler, tools.config.get('flex', ''), *composition_args,
                log='baseline-compile.log')
            run('link-layout', reference, folder / 'baseline-compiled.swf', folder / 'baseline-linked.swf', cls,
                log='baseline-link.log')
            full_movie(original, folder / 'baseline-linked.swf', folder / 'baseline-full.swf')
            if (folder / 'baseline-full.swf').read_bytes() != original.read_bytes():
                raise ValueError('Baseline does not reproduce the complete compressed release movie')
            row['baseline_exact'] = True
            (folder / 'baseline-full.swf').unlink()
            run('compile', current, folder / 'feature-compiled.swf', cls, folder / 'feature.as',
                folder / 'feature-snapshot', compiler, tools.config.get('flex', ''), *composition_args,
                log='feature-compile.log')
            run('link-feature', current, folder / 'feature-compiled.swf', folder / 'feature-linked.swf', cls,
                folder / 'permissions.tsv', log='feature-link.log')
            if (folder / 'baseline.as').read_bytes() != item['baseline'] or (folder / 'feature.as').read_bytes() != item['changed']:
                raise ValueError('Source changed during compilation')
            current = folder / 'feature-linked.swf'
            row.update(status='verified', compiled_code_sha256=movie.digest(folder / 'feature-compiled.swf'),
                       linked_code_sha256=movie.digest(current))
            print(f'{cls}: baseline exact; feature and untouched methods verified', flush=True)
        except (ValueError, OSError, subprocess.TimeoutExpired) as error:
            row.update(status='refused', error=str(error))
            movie.write_json(output / 'report.json', report)
            raise
        movie.write_json(output / 'report.json', report)
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
    allowed = {method['owner'] for item in inputs for method in item['entry']['methods']}
    if comparison['reference_methods'] != comparison['candidate_methods'] or any(
            change['status'] != 'changed' or change['owner'] not in allowed for change in comparison['method_changes']):
        raise ValueError('Final movie changed an untouched method or method ownership')
    movie.write_json(output / 'comparison.json', comparison)
    pending.rename(output / 'feature.swf')
    report.update(complete=True, feature_movie_sha256=movie.digest(output / 'feature.swf'),
                  changed_methods=len(comparison['method_changes']), noncode_identical=True)
    movie.write_json(output / 'report.json', report)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        sys.exit(str(error))

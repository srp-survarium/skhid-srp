"""Apply pinned, exact per-class source diffs without modifying pristine exports."""
import hashlib
import json
from pathlib import Path
import re


def sha(data):
    return hashlib.sha256(data).hexdigest()


def reservation_plan(entry):
    """Serialize explicit minimum capacities; these never supply executable code."""
    if 'method_reservations' not in entry:
        return None
    spec = entry['method_reservations']
    if not isinstance(spec, dict) or not spec:
        raise ValueError('Invalid method reservations')
    lines = []
    for owner, values in spec.items():
        if (not isinstance(owner, str) or not owner or owner.strip() != owner
                or any(c in owner for c in '\t\r\n|')
                or not isinstance(values, dict) or not values
                or set(values) - {'stack', 'locals'}
                or any(type(value) is not int or not 1 <= value <= 65535 for value in values.values())):
            raise ValueError('Invalid method reservation')
        lines.append(f"{owner}\t{values.get('stack', 0)}\t{values.get('locals', 0)}")
    return ('\n'.join(sorted(lines)) + '\n').encode()


def composition_plan(entry):
    """Validate an explicit compiler selection and serialize the bridge input."""
    if entry.get('compiler') != 'flex-mixed':
        if 'composition' in entry:
            raise ValueError('Compiler composition requires flex-mixed')
        return None
    spec = entry.get('composition')
    if not isinstance(spec, dict) or set(spec) - {'ffdec_methods', 'script_traits', 'ffdec_compiler', 'ffdec_hidden_static'}:
        raise ValueError('Invalid compiler composition')
    methods = spec.get('ffdec_methods')
    if (not isinstance(methods, list) or not methods
            or any(not isinstance(owner, str) or not owner or owner.strip() != owner
                   or any(c in owner for c in '\t\r\n|')
                   or owner.startswith(('script_traits=', 'ffdec_compiler=', 'ffdec_hide_static=')) for owner in methods)
            or len(set(methods)) != len(methods)):
        raise ValueError('Invalid or duplicate compiler method selection')
    declarations = spec.get('script_traits', 'native')
    if declarations not in ('native', 'ffdec'):
        raise ValueError('Invalid script-trait compiler selection')
    compiler = spec.get('ffdec_compiler', 'ffdec')
    if compiler not in ('ffdec', 'ffdec-source'):
        raise ValueError('Invalid secondary source compiler')
    hidden = spec.get('ffdec_hidden_static', [])
    identifier = r'[A-Za-z_$][A-Za-z0-9_$]*'
    if (not isinstance(hidden, list)
            or any(not isinstance(member, str)
                   or not re.fullmatch(identifier + r'(?:\.' + identifier + r')*/' + identifier, member)
                   for member in hidden)
            or len(set(hidden)) != len(hidden)
            or (hidden and compiler != 'ffdec-source')):
        raise ValueError('Invalid hidden static dependency declaration')
    lines = (['ffdec_compiler=ffdec-source'] if compiler == 'ffdec-source' else [])
    lines += ['ffdec_hide_static=' + member for member in hidden]
    lines += (['script_traits=ffdec'] if declarations == 'ffdec' else []) + methods
    return ('\n'.join(lines) + '\n').encode()


def apply_patch(source, patch, relative):
    text = source.decode('utf-8')
    newline = '\r\n' if '\r\n' in text else '\n'
    normalized = text.replace('\r\n', '\n')
    if '\r' in normalized or (newline == '\r\n' and '\n' in text.replace('\r\n', '')):
        raise ValueError('Mixed source line endings')
    if not normalized.endswith('\n'):
        raise ValueError('Source must end with a newline')
    lines = normalized.splitlines(keepends=True)
    edits = patch.decode('utf-8').splitlines(keepends=True)
    if edits[:2] != [f'--- a/{relative}\n', f'+++ b/{relative}\n']:
        raise ValueError('Patch must name exactly the selected class')
    result, cursor, index, hunks = [], 0, 2, 0
    while index < len(edits):
        match = re.fullmatch(r'@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@[^\n]*\n', edits[index])
        if not match:
            raise ValueError('Invalid hunk header')
        old, old_count, new, new_count = (int(v) if v is not None else 1 for v in match.groups())
        old_index = old - 1 if old_count else old
        new_index = new - 1 if new_count else new
        if old_index < cursor or old_index > len(lines):
            raise ValueError('Overlapping or out-of-range hunk')
        result.extend(lines[cursor:old_index])
        cursor = old_index
        if len(result) != new_index:
            raise ValueError('Incorrect new hunk position')
        index += 1
        removed = added = 0
        while index < len(edits) and not edits[index].startswith('@@ '):
            line = edits[index]
            if not line.endswith('\n') or line[:1] not in (' ', '-', '+'):
                raise ValueError('Unsupported patch line')
            if line[0] in ' -':
                if cursor >= len(lines) or lines[cursor] != line[1:]:
                    raise ValueError('Patch context differs from pristine source')
                cursor += 1
                removed += 1
            if line[0] in ' +':
                result.append(line[1:])
                added += 1
            index += 1
        if (removed, added) != (old_count, new_count):
            raise ValueError('Hunk lengths do not match header')
        hunks += 1
    if not hunks:
        raise ValueError('Empty patch')
    result.extend(lines[cursor:])
    return ''.join(result).replace('\n', newline).encode('utf-8')


def load_sources(manifest_path, project, pinned):
    """Authenticate all patches and return corrected bytes plus reproducible evidence."""
    manifest_path = Path(manifest_path)
    raw = manifest_path.read_bytes()
    spec = json.loads(raw)
    if spec.get('schema') != 1 or spec.get('release') != pinned['release'] or spec.get('movie_sha256') != pinned['movie_sha256']:
        raise ValueError('Source patches target a different release')
    sources, evidence = {}, {'manifest_sha256': sha(raw), 'classes': []}
    if not isinstance(spec.get('classes'), list):
        raise ValueError('Source patch classes must be a list')
    for entry in spec['classes']:
        if entry.get('compiler', 'flex-strict') not in ('ffdec', 'ffdec-source', 'flex', 'flex-strict', 'flex-library', 'flex-mixed'):
            raise ValueError('Unknown reconstruction compiler')
        composition_plan(entry)
        reservation_plan(entry)
        cls = entry['class']
        relative = cls.replace('.', '/') + '.as'
        if relative not in pinned['sources'] or relative in sources:
            raise ValueError('Unknown or duplicate patched class')
        patch_relative = Path(entry['patch'])
        if patch_relative.is_absolute() or '..' in patch_relative.parts or patch_relative.as_posix() != entry['patch']:
            raise ValueError('Invalid patch path')
        patch_path = manifest_path.parent / patch_relative
        if not patch_path.resolve().is_relative_to(manifest_path.parent.resolve()):
            raise ValueError('Patch escapes manifest directory')
        source = (project / 'pristine' / relative).read_bytes()
        patch = patch_path.read_bytes()
        if sha(source) != entry['pristine_sha256'] or sha(source) != pinned['sources'][relative]:
            raise ValueError('Patch source hash differs from pristine export')
        if sha(patch) != entry['patch_sha256']:
            raise ValueError('Patch hash mismatch')
        corrected = apply_patch(source, patch, relative)
        if sha(corrected) != entry['result_sha256']:
            raise ValueError('Corrected source hash mismatch')
        sources[relative] = corrected
        evidence['classes'].append(dict(entry))
    return sources, evidence

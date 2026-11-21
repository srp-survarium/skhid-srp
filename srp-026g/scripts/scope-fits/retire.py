#!/usr/bin/env python3
"""The last versions of the three scope-variant packs: rifle-scopes, scope-fits, scope-coverage.

usage: python3 -I retire.py <players' files> <packs dir> [--check]

<players' files>  release 68's gameplay/items/scopes/* at their game paths (what the players' game mounts)
<packs dir>       the packs' folders (<id>-<version>): the versions release 68 has are read, the new ones
                  written beside them (a folder that is there is compared, never written over)

A scope is its shop item on the modules table now (docs/notes/attachments-scopes.md); the variants
(a dictionary entry, a config and a pair of models for every weapon and scope) are used by nothing.
A patch only adds files, so a pack is retired by a version that makes less (release/content.tsv's
header): what the earlier version added and this one does not stays in the players' archives as it
is, named by nothing. Each new version is its earlier one less, by rule:

- the variants' dictionary entries (every entry that is not a shop scope item), and the server's
  rows for them;
- every model (models/**): the variants' assemblies, the gun bodies with a moved eye, and the shop
  items' own first models, which scope-modules-1.json replaces with the modules pack's;
- a variant's config, unless an edit list changed it after the pack (scope-centering-1.json, 304 of
  them): release/build-content takes a file as left by an earlier version only when the players have
  that version's bytes, so such a config stays in the pack, with the players' bytes, and its edit
  list goes. It is in no dictionary.

What stays is the shop scope items (their configs, dictionary entries, texts and icons) and the
textures and materials. ./retired-variants.tsv lists every retired entry: its dict id is never used
again, and ../content-edit/items.tsv loses its row.

--check: the folders in <packs dir> and the two tables are what this makes (nothing written).
"""
import hashlib
import json
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
# (id, the version release 68 has, the retiring version)
PACKS = (('rifle-scopes', 18, 19), ('scope-fits', 13, 14), ('scope-coverage', 2, 3))
SHOP = 'shop_scope_'
VARIANTS = HERE / 'retired-variants.tsv'
ITEMS = HERE.parent / 'content-edit' / 'items.tsv'
HEAD = ('# The scope variants retired with the modules table (retire.py; docs/notes/attachments-scopes.md): entries of the\n'
        '# items dictionary no release has any more. A dict id here is never given to another item: accounts saved up to\n'
        '# release 68 hold them in their scopes\' records until their first load. left: the config stays in the players\'\n'
        '# archives, made by nothing; kept: its pack still makes it, as an edit list left it.\n'
        '# dict id\tname\tits pack\tits config\n')


def retire(ident, old, new, players, packs):
    """(the new pack.json's text, {path: bytes} of its files, its icons folder, its retired rows)."""
    source = packs / f'{ident}-{old}'
    spec = json.loads((source / 'pack.json').read_text(encoding='utf-8'))
    assert (spec['id'], str(spec['version'])) == (ident, str(old)), source
    operations, files, rows, configs = [], {}, [], {}
    for op in spec['operations']:
        if op['op'] == 'dictionary_add':
            assert op['table'] == 'items_dict', op['table']
            kept = [e for e in op['entries'] if e['name'].startswith(SHOP)]
            for entry in op['entries']:
                if entry not in kept:
                    assert entry['item_category'] == 22 and entry['base_cost'] == 0, entry
                    assert entry['cfg_name'].startswith('gameplay/items/scopes/fit'), entry
                    configs[entry['cfg_name']] = entry
            if kept:
                operations.append(dict(op, entries=kept))
    shop = {e['dict_id'] for op in operations if op['op'] == 'dictionary_add' for e in op['entries']}
    added = []
    for op in spec['operations']:
        if op['op'] == 'add_file':
            path = op['path']
            was = (source / 'files' / path).read_bytes()
            if path in configs:
                now = (players / path).read_bytes()
                state = 'left' if now == was else 'kept'
                entry = configs[path]
                rows.append((entry['dict_id'], entry['name'], f'{ident}@{old}', state))
                if state == 'kept':
                    files[path] = now
                    added.append(op)
            elif not path.startswith('models/'):
                files[path] = was
                added.append(op)
        elif op['op'] != 'dictionary_add':
            operations.append(op)
    assert len(rows) == len(configs), (ident, 'a variant without its config')
    kept = sum(1 for row in rows if row[3] == 'kept')
    spec['version'] = str(new)
    spec['description'] = (
        f'{ident}, retired (srp-026g/scripts/scope-fits/retire.py, docs/notes/attachments-scopes.md): version {old} '
        f'less its {len(rows)} scope variants (their dictionary entries, their models) and every model; '
        + (f'the shop scope items {", ".join(str(i) for i in sorted(shop))} stay, with their texts and icons' if shop
           else 'no item')
        + ', and the textures and materials'
        + (f'; {kept} variant configs an edit list changed stay as the players have them, in no dictionary' if kept else ''))
    spec['operations'] = added + operations
    spec['server'] = {'items': [item for item in spec['server']['items'] if item['dict_id'] in shop]}
    spec['notes'] = [f'made by srp-026g/scripts/scope-fits/retire.py from {ident}-{old} and release 68\'s scope configs']
    icons = source / 'icons' if any(op['op'] == 'atlas_icons_add' for op in operations) else None
    return json.dumps(spec, indent=2, ensure_ascii=False) + '\n', files, icons, rows


def same_folder(folder, text, files, icons):
    if (folder / 'pack.json').read_text(encoding='utf-8') != text:
        return False
    have = {str(p.relative_to(folder / 'files')): p for p in (folder / 'files').rglob('*') if p.is_file()}
    if set(have) != set(files) or any(have[p].read_bytes() != files[p] for p in files):
        return False
    old = sorted(p.relative_to(icons) for p in icons.rglob('*') if p.is_file()) if icons else []
    new = sorted(p.relative_to(folder / 'icons') for p in (folder / 'icons').rglob('*') if p.is_file())
    return old == new and all((icons / p).read_bytes() == (folder / 'icons' / p).read_bytes() for p in old)


def main():
    args = [a for a in sys.argv[1:] if a != '--check']
    check = '--check' in sys.argv
    players, packs = Path(args[0]), Path(args[1])
    retired, problems = [], []
    for ident, old, new in PACKS:
        text, files, icons, rows = retire(ident, old, new, players, packs)
        retired += rows
        folder = packs / f'{ident}-{new}'
        if folder.exists():
            if not same_folder(folder, text, files, icons):
                problems.append(f'{folder} is not what {ident}-{old} gives')
        elif check:
            problems.append(f'{folder} is not there')
        else:
            for path, data in files.items():
                (folder / 'files' / path).parent.mkdir(parents=True, exist_ok=True)
                (folder / 'files' / path).write_bytes(data)
            if icons:
                shutil.copytree(icons, folder / 'icons')
            (folder / 'pack.json').write_text(text, encoding='utf-8')
        states = {state: sum(1 for row in rows if row[3] == state) for state in ('left', 'kept')}
        print(f'pack\t{ident}@{new}\t{hashlib.sha256(text.encode()).hexdigest()}\t{len(files)} files; '
              f'{len(rows)} variants retired ({states["left"]} configs left, {states["kept"]} kept)')
    table = HEAD + ''.join(f'{dict_id}\t{name}\t{pack}\t{state}\n' for dict_id, name, pack, state in sorted(retired))
    gone = {str(row[0]) for row in retired}
    lines = ITEMS.read_text(encoding='utf-8').splitlines(keepends=True)
    items = ''.join(line for line in lines if line.split('\t')[0] not in gone)
    if check:
        if not VARIANTS.is_file() or VARIANTS.read_text() != table:
            problems.append(f'{VARIANTS.name} is not what the packs give')
        if items != ''.join(lines):
            problems.append(f'{ITEMS.name} has a row of a retired variant')
    else:
        VARIANTS.write_text(table)
        ITEMS.write_text(items, encoding='utf-8')
    for problem in problems:
        print(f'retire: {problem}')
    if problems:
        raise SystemExit(1)
    print(f'{len(retired)} variants retired; {len(lines) - len(items.splitlines())} rows out of {ITEMS.name}')


if __name__ == '__main__':
    main()

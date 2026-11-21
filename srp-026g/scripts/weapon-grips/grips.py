"""The grips: 0.69d0's foregrips as its data has them, and the stats they get here.

Read from donor.sh's folder and ../weapon-modules/ids.tsv; nothing is typed per grip. make_pack.py
makes the items of it, texts.py their texts (docs/notes/attachments-grips.md).

**The stats' rule** (docs/notes/attachments.md, "Stats for now"; the owner, 2026-11-20: what needs
an exe reader is simplified). 0.69d0 rolls two modifier groups on a module, an effect and a
penalty, and a bought module has both at grade 7. A 0.26g weapon reads neither aimed, hip nor
first-shots recoil, nor an aimed movement speed, so:
- the effect, a conditional recoil, is plain recoil at a share (SHARE);
- a penalty on the aim time is kept as it is;
- a penalty on a key the weapon does not read is dropped.
A modifier the rule does not know stops the build.
"""
from dataclasses import dataclass
import hashlib
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
IDS = HERE.parent / 'weapon-modules' / 'ids.tsv'

# 0.69d0's bought grade (every module's premium_modifiers are [group, 7]).
GRADE = 7
# A conditional recoil's share of plain recoil: aimed fire is most of a fight's shots.
SHARE = {'recoil_aim_modifier': 0.75, 'recoil_hip_modifier': 0.25, 'recoil_shot_first_three_modifier': 0.25}
AIM_TIME = 'aim_time_modifier'
# Penalties on keys a 0.26g weapon does not read.
DROPPED = {'recoil_aim_modifier', 'recoil_hip_modifier', 'aimed_movement_speed_factor'}
# Silver: one price a family (attachments.md; 0.69d0 prices every module the same, 200).
PRICE = 6000
# The lobby's consumables atlas: the grips' column, a row a grip from the first, by dict id.
ICON_ATLAS, ICON_COLUMN, ICON_FIRST_ROW = 'LobbyMenu_AmmoIcons', 8, 1
# A fitted item's modification id: clear of the scopes' (991000.., 992000.., 993000..).
MODIFICATION_BASE = 994000
# The pack weapon-modules keeps 0.69d0's module models under this name.
MODELS = ('weapons/modules/', 'weapons/modules_069/')


@dataclass(frozen=True)
class Modifier:
    """One modifier of 0.69d0 at the bought grade: a key of the weapon's parameters and its sum."""
    key: str
    value: float


@dataclass(frozen=True)
class Grip:
    module: str          # 0.69d0's module
    dict_id: int         # our item's
    item: str            # our item's name
    donor_dict_id: int
    model: str           # the grip's model in the pack weapon-modules
    name_key: str        # 0.69d0's own text keys
    description_key: str
    effect: Modifier     # 0.69d0's first group at the bought grade
    penalty: Modifier    # its second
    recoil: float        # ours: a factor's sum on the six kick bounds (type 2)
    aim_time: float      # and on parameters/aim_time; 0.0 none
    icon: tuple          # (row, column) of ICON_ATLAS

    @property
    def modification(self):
        return MODIFICATION_BASE + self.dict_id

    @property
    def config(self):
        return f'gameplay/items/attachments/grip/{self.item}.options'


def leaves(path):
    """{key: value} of a `dump-db-config --leaves` file: ints, floats, bools and texts."""
    out = {}
    for line in Path(path).read_text(encoding='utf-8').splitlines():
        key, _, raw = line.partition('\t')
        kind, _, value = raw.partition(':')
        if kind == 'string':
            out[key] = value[1:-1].replace('\\"', '"')
        elif kind == 'bool':
            out[key] = value == 'true'
        elif kind == 'i32':
            out[key] = int(value)
        elif kind == 'f32':
            out[key] = float(value)
        else:
            raise ValueError(f'{path}: {line!r}')
    return out


def check_donor(donor):
    """Every file of the donor folder is donor.sha256's, and nothing else is there."""
    want = dict(reversed(line.split('  ', 1)) for line in (HERE / 'donor.sha256').read_text().splitlines())
    have = {'./' + str(p.relative_to(donor)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(Path(donor).rglob('*')) if p.is_file()}
    if have != want:
        odd = sorted(k for k in set(have) | set(want) if have.get(k) != want.get(k))
        raise SystemExit(f'{donor}: not donor.sha256\'s files: {", ".join(odd)}')


def bought(game):
    """{0.69d0 dict id: [Modifier, Modifier]}: a module's two groups at the bought grade."""
    modifications = {}
    for key, value in game.items():
        m = re.fullmatch(r'modifications\[(\d+)\]\.(group|grade|modifiers\[(\d+)\]\.(value|type|path\[(\d+)\]))', key)
        if m:
            modifications.setdefault(int(m.group(1)), {})[m.group(2)] = value
    by_group = {}
    for row in modifications.values():
        if row.get('grade') != GRADE or 'group' not in row:
            continue
        if any(k.startswith('modifiers[1]') for k in row) or row.get('modifiers[0].type') != 1 \
                or row.get('modifiers[0].path[0]') != 'parameters' or 'modifiers[0].path[2]' in row:
            continue  # not one added parameter: no grip rolls such a group (checked where it is used)
        by_group[row['group']] = Modifier(row['modifiers[0].path[1]'], float(row['modifiers[0].value']))
    out = {}
    infos = {}
    for key, value in game.items():
        m = re.fullmatch(r'modules_drop_options\.modules_additional_infos\[(\d+)\]\.(.+)', key)
        if m:
            infos.setdefault(int(m.group(1)), {})[m.group(2)] = value
    for info in infos.values():
        groups = [info.get(f'premium_modifiers[{i}][0]') for i in (0, 1)]
        grades = [info.get(f'premium_modifiers[{i}][1]') for i in (0, 1)]
        if None in groups or 'premium_modifiers[2][0]' in info:
            continue
        if grades != [GRADE, GRADE] or groups != [info.get('first_drop_modifier'), info.get('second_drop_modifier')]:
            raise SystemExit(f'0.69d0 module {info.get("dict_id")}: not two groups at grade {GRADE}: {groups} {grades}')
        out[info['dict_id']] = [by_group.get(g) for g in groups]
    return out


def simple(module, effect, penalty):
    """(recoil, aim time) of the rule above."""
    if effect.key not in SHARE or effect.value >= 0:
        raise SystemExit(f'{module}: its effect is {effect}, which the rule does not know')
    recoil = round(effect.value * SHARE[effect.key], 4)
    if penalty.key == AIM_TIME:
        if penalty.value <= 0:
            raise SystemExit(f'{module}: its aim time {penalty} is no penalty')
        return recoil, round(penalty.value, 4)
    if penalty.key not in DROPPED:
        raise SystemExit(f'{module}: its penalty is {penalty}, which the rule does not know')
    return recoil, 0.0


def grips(donor):
    """The grips of ids.tsv, by dict id, from the donor folder."""
    check_donor(donor)
    game = leaves(Path(donor) / 'game.leaves')
    rolls = bought(game)
    rows = [line.split('\t') for line in IDS.read_text().splitlines() if line and not line.startswith('#')]
    out = []
    for nth, (module, dict_id, _, item) in enumerate(sorted((r for r in rows if r[2] == 'grip'), key=lambda r: int(r[1]))):
        at = f'weapon_modules.{module}.'
        if at + 'model' not in game:
            raise SystemExit(f'{module}: no such module in 0.69d0')
        if game.get(at + 'finger_locators[15]') != 'LeftHand_idle':
            raise SystemExit(f'{module}: not a grip (no left hand)')
        model = game[at + 'model']
        if not model.startswith(MODELS[0]):
            raise SystemExit(f'{module}: its model is {model}')
        donor_id = game[at + 'parameters.dict_id']
        pair = rolls.get(donor_id)
        if not pair or None in pair:
            raise SystemExit(f'{module} ({donor_id}): 0.69d0 rolls no two single-parameter groups on it')
        recoil, aim_time = simple(module, *pair)
        out.append(Grip(module=module, dict_id=int(dict_id), item=item, donor_dict_id=donor_id,
                        model=MODELS[1] + model[len(MODELS[0]):],
                        name_key=game[at + 'ui_desc.name'], description_key=game[at + 'ui_desc.description'],
                        effect=pair[0], penalty=pair[1], recoil=recoil, aim_time=aim_time,
                        icon=(ICON_FIRST_ROW + nth, ICON_COLUMN)))
    if not out:
        raise SystemExit('ids.tsv names no grip')
    return out


def percent(value):
    """A factor's sum as the card writes it: -6, +1.5."""
    return f'{value * 100:+.2f}'.rstrip('0').rstrip('.')

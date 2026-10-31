"""Check the Flash menus' own texts against the dictionaries.

The lobby, login and hud movies hold no language text: they show srp's texts by their dictionary keys,
LocalizationData.getName("st_srp_...") or LocalizationData.format("st_srp_...", values), and the
content compile maps every st_srp_ key of the movie's prefix to itself for it
(localization_026g.FLASH_MOVIES: the hud's st_srp_hud_ only). Every key a movie names must be in the
dictionaries and of its movie's prefix; no other movie may name one (it is not given srp's texts).
"""
from pathlib import Path
import re

from localization_026g import FLASH_MOVIES, FLASH_PREFIX

KEY = re.compile(r'"(' + re.escape(FLASH_PREFIX) + r'[A-Za-z0-9_]*)"')


def audit_flash(root, keys):
    root = Path(root)
    if not all((root / movie / 'src').is_dir() for movie in FLASH_MOVIES):
        raise ValueError(f'{root}: missing Flash sources')
    issues, used = [], set()
    for path in sorted(root.rglob('*.as')):
        source = path.read_text(encoding='utf-8')
        movie = path.relative_to(root).parts[0]
        for match in KEY.finditer(source):
            where = f'{path.relative_to(root)}:{source.count(chr(10), 0, match.start()) + 1}'
            if movie not in FLASH_MOVIES:
                issues.append(f'{where}: {match[1]}: the {movie} movie is not given srp\'s texts')
            elif not match[1].startswith(FLASH_MOVIES[movie]):
                issues.append(f'{where}: {match[1]}: the {movie} movie is given {FLASH_MOVIES[movie]}* texts only')
            elif match[1] not in keys:
                issues.append(f'{where}: {match[1]} is not in the dictionaries')
            used.add(match[1])
    return {'checked': len(used), 'issues': issues}

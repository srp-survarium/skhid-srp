#!/usr/bin/env python3
"""texts.py <bundle's port-manifest.json> <client-patches/localization folder>

The muzzle devices' names and descriptions in the ten dictionaries (docs/notes/muzzle-devices.md),
written into every <language>.tsv in key order (a key already there is replaced), and the weapon
window's two muzzle-slot labels. A name is the maker's and model's (Latin in every language, as the
devices are sold), the kind and the muzzle thread; a description says what the device changes, in
the stock dictionaries' own words for recoil and aiming. 0.69d0's own strings are not taken: they
mislabel kinds and threads.
"""
import json
import sys
from pathlib import Path

LANGS = ('english', 'german', 'polish', 'russian', 'ukrainian', 'belarusian', 'czech', 'finnish',
         'hungarian', 'serbian')

BRANDS = [  # module id prefix -> the device's name
    ('aek919k', 'AEK-919K'), ('aws_rapira', 'AWS "Rapira"'), ('cncwarrior_rofh', 'CNC Warrior ROFH'),
    ('custom_01', 'Custom 01'), ('custom_02', 'Custom 02'), ('custom_03', 'Custom 03'),
    ('custom_04', 'Custom 04'), ('diamondhead_tbrake', 'Diamondhead T-Brake'),
    ('grizzlygunworks_defcon1', 'Grizzly Defcon 1'), ('gunsru_custom', 'Guns.ru Custom'),
    ('hartman_kochevnik', 'Hartman "Kochevnik"'), ('lantac_dragon', 'Lantac Dragon'),
    ('lantac_drakon', 'Lantac Dragon'), ('manticorearms_nightbrake', 'Manticore NightBrake'),
    ('redheat_ilgynagk01', 'Red Heat Ilgynag K01'), ('redheat_krymske2', 'Red Heat Krymsk-2'),
    ('redheat_shturm', 'Red Heat Shturm'), ('spikestactical_dynacomp', 'Spikes Dynacomp'),
    ('taktikatula_saiga419', 'Taktika Tula 419'), ('venometactical_antidote', 'Venom Antidote'),
    ('zenit_dtk2', 'Zenit DTK-2'), ('zenit_dtk3', 'Zenit DTK-3'),
]
THREADS = {'m12x1l': 'M12x1L', 'm14x1l': 'M14x1L', 'm141l': 'M14x1L', 'm16x1l': 'M16x1L',
           'm24x15': 'M24x1.5', 'm22x075': 'M22x0.75', 'm30x1': 'M30x1', 'i05x28': '1/2"x28',
           'i06x24': '5/8"x24'}
# (the kind's word, whether it goes before the brand), per language
KIND = {
    'muzzle_brake': {
        'english': ('muzzle brake', False), 'german': ('Mündungsbremse', False),
        'polish': ('Hamulec wylotowy', True), 'russian': ('Дульный тормоз', True),
        'ukrainian': ('Дульне гальмо', True), 'belarusian': ('Дульны тормаз', True),
        'czech': ('Úsťová brzda', True), 'finnish': ('suujarru', False),
        'hungarian': ('csőszájfék', False), 'serbian': ('Usna kočnica', True)},
    'flash_hider': {
        'english': ('flash hider', False), 'german': ('Mündungsfeuerdämpfer', False),
        'polish': ('Tłumik płomieni', True), 'russian': ('Пламегаситель', True),
        'ukrainian': ("Полум'ягасник", True), 'belarusian': ('Полымягасільнік', True),
        'czech': ('Tlumič plamene', True), 'finnish': ('liekinvaimennin', False),
        'hungarian': ('lángrejtő', False), 'serbian': ('Skrivač plamena', True)},
}
CLAMP = {'english': 'clamp-on', 'german': 'Klemmmontage', 'polish': 'zaciskowy', 'russian': 'на зажиме',
         'ukrainian': 'на затискачі', 'belarusian': 'на заціску', 'czech': 'upínací',
         'finnish': 'puristinkiinnitys', 'hungarian': 'bilincses', 'serbian': 'sa obujmicom'}
RECOIL = {'english': 'Recoil', 'german': 'Rückstoß', 'polish': 'Odrzut', 'russian': 'Отдача',
          'ukrainian': 'Віддача', 'belarusian': 'Аддача', 'czech': 'Zpětný ráz', 'finnish': 'Rekyyli',
          'hungarian': 'Visszarúgás', 'serbian': 'Trzaj'}
AIM = {'english': 'aim time', 'german': 'Zielzeit', 'polish': 'czas celowania',
       'russian': 'время прицеливания', 'ukrainian': 'час прицілювання', 'belarusian': 'час прыцэльвання',
       'czech': 'doba míření', 'finnish': 'tähtäysaika', 'hungarian': 'célzási idő',
       'serbian': 'vreme nišanjenja'}
HIDES = {'english': 'Hides the shot\'s flash.', 'german': 'Verbirgt das Mündungsfeuer.',
         'polish': 'Ukrywa błysk wystrzału.', 'russian': 'Скрывает вспышку выстрела.',
         'ukrainian': 'Приховує спалах пострілу.', 'belarusian': 'Хавае ўспышку стрэлу.',
         'czech': 'Skrývá záblesk výstřelu.', 'finnish': 'Piilottaa suuliekin.',
         'hungarian': 'Elrejti a torkolattüzet.', 'serbian': 'Skriva bljesak pucnja.'}
LOOKS = {'english': 'Looks only.', 'german': 'Nur Optik.', 'polish': 'Tylko wygląd.',
         'russian': 'Только внешний вид.', 'ukrainian': 'Лише зовнішній вигляд.',
         'belarusian': 'Толькі знешні выгляд.', 'czech': 'Jen vzhled.', 'finnish': 'Vain ulkonäkö.',
         'hungarian': 'Csak megjelenés.', 'serbian': 'Samo izgled.'}
UI = {
    'st_srp_muzzle_slot': {'english': 'Muzzle device', 'german': 'Mündungsaufsatz', 'polish': 'Nasadka wylotowa',
                           'russian': 'Дульное устройство', 'ukrainian': 'Дульний пристрій',
                           'belarusian': 'Дульная прылада', 'czech': 'Úsťové zařízení',
                           'finnish': 'Suulaite', 'hungarian': 'Csőszájeszköz', 'serbian': 'Uređaj na ustima cevi'},
}


def pct(v):
    return f'{"+" if v > 0 else "-"}{round(abs(v) * 100)} %'


def texts(device):
    mid = device['module']
    brand = next(b for p, b in BRANDS if mid.startswith(p))
    thread = next(THREADS[t] for t in THREADS if f'_{t}_' in f'_{mid}_')
    clamp = '_clamp_' in mid
    stats = {s['path']: s['value'] for s in device['stats']}
    out = {}
    for lang in LANGS:
        word, first = KIND[device['kind']][lang]
        name = f'{word} {brand}' if first else f'{brand} {word}'
        name += f', {thread}' + (f', {CLAMP[lang]}' if clamp else '')
        parts = []
        if device['kind'] == 'flash_hider':
            parts.append(HIDES[lang])
        lines = []
        if 'parameters/recoil_modifier' in stats:
            lines.append(f'{RECOIL[lang]} {pct(stats["parameters/recoil_modifier"])}')
        if 'parameters/aim_time_modifier' in stats:
            aim = AIM[lang] if lines else AIM[lang][:1].upper() + AIM[lang][1:]
            lines.append(f'{aim} {pct(stats["parameters/aim_time_modifier"])}')
        if lines:
            parts.append(', '.join(lines) + '.')
        if not parts:
            parts.append(LOOKS[lang])
        out[lang] = (name, ' '.join(parts))
    return out


def main():
    manifest, folder = Path(sys.argv[1]), Path(sys.argv[2])
    devices = json.loads(manifest.read_text())['devices']
    rows = {lang: {} for lang in LANGS}
    for d in devices:
        for lang, (name, desc) in texts(d).items():
            rows[lang][f"st_{d['item']}_name"] = name
            rows[lang][f"st_{d['item']}_description"] = desc
    for key, by_lang in UI.items():
        for lang in LANGS:
            rows[lang][key] = by_lang[lang]
    for lang in LANGS:
        path = folder / f'{lang}.tsv'
        lines = path.read_text(encoding='utf-8').split('\n')
        assert lines[0] == 'key\ttext' and lines[-1] == '', path
        table = dict(line.split('\t', 1) for line in lines[1:-1])
        table.update(rows[lang])
        body = '\n'.join(f'{k}\t{v}' for k, v in sorted(table.items()))
        path.write_text(f'key\ttext\n{body}\n', encoding='utf-8')
        print(f'{lang}: {len(rows[lang])} keys')


if __name__ == '__main__':
    main()

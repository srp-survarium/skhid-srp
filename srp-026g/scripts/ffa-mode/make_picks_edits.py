#!/usr/bin/env python3
"""A map's Free for All spawns at the owner's picks (2026-11-14, Rudnya first: "there should be a rule
that spawns are inside buildings or close to two walls or some cover", "I can give you many examples
with ~where"; his `where` places, private/spawn-examples/<map>-2026-11-14.tsv). MAPS names each map:
its picks, its edit list, its new points' names and the list's about text; a map more is a row there.

<map>-picks.tsv is the reviewed output of check-points' picks binary (its doc says how a pick is put
on its ground and checked), run on the map's projects as every row of release/content.tsv above the
map's list leaves them (the stock db, every patch of release/patches.tsv and, for rows not yet
released, a build-content patch):

  cd check-points && cargo run --release --bin picks -- --db <resources.db> \\
      --patch <resources.db.patch_12> ... --patch <resources.db.patch_59> \\
      --points <rudnya-2026-11-14.tsv> --roofs 49,8,10,42 --harmful-ok 45 \\
      --project level_01/day_8 --project level_01/evening_8 --project level_01/rain_8 > ../rudnya-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <pool-2026-11-14.tsv> --as-stood 12 --project v010b_cs_mp_bath/day_8 > ../pool-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <school-2026-11-14.tsv> --harmful-ok 7,8,15,45,46,48,49 --as-stood 17 \\
      --project level_02/day_8 --project level_02/evening_8 --project level_02/rain_8 > ../school-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <vostok-2026-11-14.tsv> --roofs 13 --stranded-ok 13 --roof-at 24.563,0.912,30.789 \\
      --project level_03/day_8 --project level_03/evening_8 --project level_03/rain_8 > ../vostok-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <vostok-2026-11-14.tsv> --shift 1.5 --roofs 13 --roof-at 24.563,0.912,30.789 \\
      --project v021d_level_03/day_8 > ../vostok-v021-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <vostok-2026-11-14.tsv> --shift 1.5 --roofs 13 --stranded-ok 13 --roof-at 24.563,0.912,30.789 \\
      --project v011e_level_03/day_8 > ../vostok-v011-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <rudnya-2026-11-14.tsv> --shift 1.5 --roofs 49,8,10,42 --harmful-ok 45 \\
      --roof-place 49=-23.695,29.232374,-118.537 --roof-place 8=35.072,24.732464,-10.762001 \\
      --roof-place 10=26.8,25.926086,2.816 --roof-place 42=51.335,23.952936,-52.563995 \\
      --project v069d_level_01/day_8 > ../rudnya-v069-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <mamayev021-2026-11-14.tsv> --harmful-ok 20,21,57 --project v021d_level_06/day_8 \\
      > ../mamayev-v021-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <mamayev026-2026-11-15.tsv> --harmful-ok 17,30,31,55,57 \\
      --project level_06/day_8 --project level_06/evening_8 --project level_06/rain_8 > ../mamayev-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <training-camp-2026-11-15.tsv> --as-stood 13,25,26,27,28,44,45,46,47 \\
      --project v010b_cs_training_camp/day_8 > ../training-camp-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <london-2026-11-16.tsv> --project level_09/day_8 > ../london-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <fire-station-2026-11-27.tsv> --project v010b_cs_mp_firestation/day_8 > ../fire-station-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <chemical-plant-2026-11-16.tsv> \\
      --project level_04/day_8 --project level_04/evening_8 --project level_04/rain_8 > ../chemical-plant-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <london-v032-2026-11-27.tsv> --add --shift 1.5 --project v032a_level_09/day_8 \\
      > ../london-v032-picks.tsv
  cd check-points && cargo run --release --bin picks -- --db <resources.db> --patch ... \\
      --points <station-2026-11-27.tsv> --add --cap 128 --shift 1 --project v010b_soc_ug_station/day_8 \\
      > ../station-picks.tsv

(fire-station-2026-11-27.tsv is the 42 picks of 2026-11-16 and his 9 more of 2026-11-27, 43-51, on the
projects as the rows above ffa-fire-station-picks-1 leave them. London (v0.32)'s are picks more on a
map whose spawns are already his: `--add` makes each a new point and leaves the ones there as they
are, its `Map` says `adds`; `--shift` puts a pick the walk grid does not reach on the nearest floor it
does. Station's too (2026-11-27, "use them too": his 60 over the spawn rule's 96), past the 128 the
respawn chooses among: `--cap 128` gives the ids of the 28 of the 96 nearest a pick to picks, `moved`.)

(the owner, 2026-11-14, of the picks the checks dropped: "keep all my positions"; 2026-11-15, in
Vostok's checkpoint at `--roof-at`'s place: "we want the spawn on top of the building those coords
have", "to all vostok maps": the three Vostok tables are of the projects with ffa-vostok-roofs-1's
roof collision, the row before their lists).

Then

  python3 make_picks_edits.py <map> --live <folder>

with the projects at their game paths as those rows leave them, writes the map's list in
../content-edit/: each `moved` row's solo spawn, matched by its point id, name and team 2, gets the
pick's position and rotation [0, yaw, 0] (its id, name, radii and protection stay); each `added` and
`roof` row is a new solo spawn after the project's last point, a copy of its first solo spawn (20 m
exclusion, 500 ms protection, no team zones) at the row's place, named <prefix><point id>; each
`removed` row's spawn, the project's last entries, goes. Points stand as near one another as they are
(the owner, 2026-11-14, of the 12 m spacing: "we don't need this rules at all"). Only the free for
all's own projects (`<weather>_8`); Team Deathmatch's are never named. A map's list comes after every
list that touches its Free for All spawns: it adds and removes points, which the lists before it
address by index, and its new points copy the 500 ms that ffa-protection-ms-1 gave the others.
"""
import argparse
import csv
import math
from pathlib import Path
from typing import NamedTuple

import make_edits as base
from make_center_edits import path_of


class Map(NamedTuple):
    picks: Path
    output: Path
    prefix: str
    about: str
    # Picks more (picks --add): the solo spawns they do not name stay as they are, those they name
    # (`moved`, --cap) take a pick's place; none is removed.
    adds: bool = False


MAPS = {
    'rudnya': Map(
        base.HERE / 'rudnya-picks.tsv', base.HERE.parent / 'content-edit/ffa-rudnya-picks-1.json', 'ffa_rudnya_',
        'Rudnya\'s Free for All spawns at the owner\'s picks (2026-11-14: "there should be a rule that '
        'spawns are inside buildings or close to two walls or some cover"; his `where` places; "for 49, 8, 10 and '
        '42 we need spawn points on top of those builings (roofs)"). Generated by scripts/ffa-mode/'
        'make_picks_edits.py from rudnya-picks.tsv (check-points picks): each kept pick on its ground, body clear, '
        '2 m from harmful zones (but 45, the owner\'s), facing as he faced, in a solo spawn\'s place (its id, name, '
        'radii and protection kept) or a new one; a roof spawn on each building asked for, with a way down; the '
        'solo spawns left over (the last entries) go. The free for all\'s own projects only; the last Free for '
        'All list (its new points copy the 500 ms ffa-protection-ms-1 gave the others).'),
    'pool': Map(
        base.HERE / 'pool-picks.tsv', base.HERE.parent / 'content-edit/ffa-pool-picks-1.json', 'ffa_pool_',
        'Experimental Pool\'s Free for All spawns at the owner\'s picks (2026-11-14: his `where` places, as '
        'Rudnya\'s). Generated by scripts/ffa-mode/make_picks_edits.py from pool-picks.tsv (check-points picks): '
        'all 28 ("keep all my positions"), each on its ground, body clear (but 12, where he stood: the check has no '
        'floor under its centre), 2 m from harmful zones, facing as he faced, in a solo spawn\'s place (its id, '
        'name, radii and protection kept), however near another; the solo spawns left over (the last entries) go. '
        'The free for all\'s own project v010b_cs_mp_bath/day_8 only; after ffa-cover-maps-1, whose points it '
        'replaces.'),
    'school': Map(
        base.HERE / 'school-picks.tsv', base.HERE.parent / 'content-edit/ffa-school-picks-1.json', 'ffa_school_',
        'School\'s Free for All spawns at the owner\'s picks (2026-11-14: his `where` places, as Rudnya\'s). '
        'Generated by scripts/ffa-mode/make_picks_edits.py from school-picks.tsv (check-points picks): all 49 '
        '("keep all my positions"), each on its ground, body clear (but 17, where he stood: the check has no floor '
        'under its centre), 2 m from harmful zones (but 7, 8, 15, 45, 46, 48 and 49, the owner\'s), facing as he '
        'faced, in a solo spawn\'s place (its id, name, radii and protection kept), however near another; the solo '
        'spawns left over (the last entries) go. The free for all\'s own projects level_02/<weather>_8 only; after '
        'ffa-cover-maps-1, whose points it replaces.'),
    'vostok': Map(
        base.HERE / 'vostok-picks.tsv', base.HERE.parent / 'content-edit/ffa-vostok-picks-1.json', 'ffa_vostok_',
        'Vostok\'s Free for All spawns at the owner\'s picks (2026-11-14: his `where` places, as Rudnya\'s; "also for '
        'vostok also all"). Generated by scripts/ffa-mode/make_picks_edits.py from vostok-picks.tsv (check-points '
        'picks): all 73, each on its ground, body clear, 2 m from harmful zones, facing as he faced, however near '
        'another, in a solo spawn\'s place (its id, name, radii and protection kept) or a new one after the last '
        '(a copy of the first solo spawn: 20 m exclusion, 500 ms protection); and a roof spawn on the building at 13 '
        '("set one spawn on top of the building where 13 is"), without a way down within 5 m drops, as he asked; '
        'and one on the checkpoint\'s roof (2026-11-15, standing in it: "we want the spawn on top of the building '
        'those coords have"), on the collision ffa-vostok-roofs-1 gives it. The free for all\'s own projects '
        'level_03/<weather>_8 only; after ffa-cover-maps-1, whose points it replaces, and ffa-vostok-roofs-1.'),
    # Vostok's picks on its ports, which share its world coordinates (the owner, 2026-11-14: "all of my
    # spawns most likely can be reused on 0.11 and 0.21 maps"): a pick the checks drop there moves up to
    # 1.5 m (picks --shift), else stays out.
    'vostok-v021': Map(
        base.HERE / 'vostok-v021-picks.tsv', base.HERE.parent / 'content-edit/ffa-vostok-v021-picks-1.json',
        'ffa_vostok_v021_',
        'Vostok v0.21\'s Free for All spawns at the owner\'s Vostok picks (2026-11-14: "all of my spawns most likely '
        'can be reused on 0.11 and 0.21 maps"; the port shares Vostok\'s world coordinates). Generated by '
        'scripts/ffa-mode/make_picks_edits.py from vostok-v021-picks.tsv (check-points picks): 71 of the 73, each on '
        'its ground, body clear, 2 m from harmful zones, facing as he faced, however near another, in a solo spawn\'s '
        'place (its id, name, radii and protection kept) or a new one after the last (a copy of the first solo '
        'spawn: 20 m exclusion, 500 ms protection); 67 and 68 stay out (inside the rust anomaly\'s gas, none 2 m '
        'clear within 1.5 m); the roof spawn on the building at 13, which has a way down on foot here, and one on '
        'the checkpoint\'s roof (2026-11-15: "we want the spawn on top of the building those coords have", "to all '
        'vostok maps"), both on the roofs as ffa-vostok-roofs-1 makes them walked. The free for all\'s own project '
        'v021d_level_03/day_8 only; after ffa-cover-maps-1, whose points it replaces, and ffa-vostok-roofs-1.'),
    'vostok-v011': Map(
        base.HERE / 'vostok-v011-picks.tsv', base.HERE.parent / 'content-edit/ffa-vostok-v011-picks-1.json',
        'ffa_vostok_v011_',
        'Vostok v0.11\'s Free for All spawns at the owner\'s Vostok picks (2026-11-14: "all of my spawns most likely '
        'can be reused on 0.11 and 0.21 maps"; the port shares Vostok\'s world coordinates). Generated by '
        'scripts/ffa-mode/make_picks_edits.py from vostok-v011-picks.tsv (check-points picks): 71 of the 73, each on '
        'its ground, body clear, 2 m from harmful zones, facing as he faced, however near another, in a solo spawn\'s '
        'place (its id, name, radii and protection kept) or a new one after the last (a copy of the first solo '
        'spawn: 20 m exclusion, 500 ms protection); 71 and 72 stay out (no floor players reach within 1.5 m); the '
        'roof spawn on the building at 13, without a way down within 5 m drops as native Vostok\'s, and one on the '
        'checkpoint\'s roof (2026-11-15: "we want the spawn on top of the building those coords have", "to all '
        'vostok maps"), both on the roofs as ffa-vostok-roofs-1 makes them walked. The free for all\'s own project '
        'v011e_level_03/day_8 only; after ffa-cover-maps-1, whose points it replaces, and ffa-vostok-roofs-1.'),
    # Rudnya's picks on Rudnya (v0.69), which shares native Rudnya's world coordinates (the owner, 2026-11-27:
    # "can rudnya 69 use the same ffa spots as 26?"): its four roof spawns kept where native Rudnya has them
    # (picks --roof-place), each on its roof there.
    'rudnya-v069': Map(
        base.HERE / 'rudnya-v069-picks.tsv', base.HERE.parent / 'content-edit/ffa-rudnya-v069-picks-1.json',
        'ffa_rudnya_v069_',
        'Rudnya (v0.69)\'s Free for All spawns at native Rudnya\'s (the owner, 2026-11-27: "can rudnya 69 use the same '
        'ffa spots as 26?"; the port shares native Rudnya\'s world coordinates). Generated by scripts/ffa-mode/'
        'make_picks_edits.py from rudnya-v069-picks.tsv (check-points picks): all 56 of the owner\'s Rudnya picks, '
        'each on its ground here, body clear, 2 m from harmful zones (but 45, the owner\'s), facing as he faced, in a '
        'solo spawn\'s place (its id, name, radii and protection kept) or a new one after the last (a copy of the '
        'first solo spawn: 20 m exclusion, 500 ms protection); and the four roof spawns at native Rudnya\'s places '
        '(picks --roof-place), on their roofs here, each with a way down. The free for all\'s own project '
        'v069d_level_01/day_8 only; after ffa-cover-maps-1, whose points it replaces.'),
    'mamayev-v021': Map(
        base.HERE / 'mamayev-v021-picks.tsv', base.HERE.parent / 'content-edit/ffa-mamayev-v021-picks-1.json',
        'ffa_mamayev_v021_',
        'Mamayev Kurgan v0.21\'s Free for All spawns at the owner\'s picks (2026-11-15: his `where` places, as '
        'Vostok\'s; "just mine for all maps"). Generated by scripts/ffa-mode/make_picks_edits.py from '
        'mamayev-v021-picks.tsv (check-points picks): all 66, each on its ground, body clear, 2 m from harmful zones '
        '(but 20, 21 and 57, the owner\'s), facing as he faced, however near another, in a solo spawn\'s place (its '
        'id, name, radii and protection kept) or a new one after the last (a copy of the first solo spawn: 20 m '
        'exclusion, 500 ms protection). The free for all\'s own project v021d_level_06/day_8 only; after '
        'ffa-cover-maps-1, whose points it replaces.'),
    'mamayev': Map(
        base.HERE / 'mamayev-picks.tsv', base.HERE.parent / 'content-edit/ffa-mamayev-picks-1.json', 'ffa_mamayev_',
        'Mamayev Kurgan\'s Free for All spawns at the owner\'s picks (2026-11-15: his `where` places on day_8, as '
        'School\'s; "just mine for all maps"). Generated by scripts/ffa-mode/make_picks_edits.py from '
        'mamayev-picks.tsv (check-points picks): all 68, each on its ground, body clear, 2 m from harmful zones (but '
        '17, 30, 31, 55 and 57, the owner\'s), facing as he faced, however near another, in a solo spawn\'s place '
        '(its id, name, radii and protection kept) or a new one after the last (a copy of the first solo spawn: '
        '20 m exclusion, 500 ms protection). The free for all\'s own projects level_06/<weather>_8 only; after '
        'ffa-cover-maps-1, whose points it replaces.'),
    'training-camp': Map(
        base.HERE / 'training-camp-picks.tsv', base.HERE.parent / 'content-edit/ffa-training-camp-picks-1.json',
        'ffa_training_camp_',
        'Training Camp\'s Free for All spawns at the owner\'s picks (2026-11-15: his `where` places on day_8, as '
        'Mamayev Kurgan\'s; "i also set spawns"). Generated by scripts/ffa-mode/make_picks_edits.py from '
        'training-camp-picks.tsv (check-points picks): all 47, each on its ground (but 13, 25-28 and 44-47, where he '
        'stood: the walk model reaches no floor there from the other spawns), body clear, 2 m from harmful zones (the '
        'bonfires\' fire zones among them), facing as he faced, however near another, in a solo spawn\'s place (its '
        'id, name, radii and protection kept); the solo spawns left over (the last entries) go. The free for all\'s '
        'own project v010b_cs_training_camp/day_8 only; after training-camp-arena-1, ffa-cover-maps-1 and '
        'training-camp-fires-1, whose points it replaces.'),
    'london': Map(
        base.HERE / 'london-picks.tsv', base.HERE.parent / 'content-edit/ffa-london-picks-1.json', 'ffa_london_',
        'London\'s Free for All spawns at the owner\'s picks (2026-11-16: his `where` places on day_8, as '
        'Training Camp\'s). Generated by scripts/ffa-mode/make_picks_edits.py from london-picks.tsv (check-points '
        'picks): all 50, each on its ground, body clear, 2 m from harmful zones, facing as he faced, however near '
        'another, in a solo spawn\'s place (its id, name, radii and protection kept); the solo spawns left over (the '
        'last entries) go. The free for all\'s own project level_09/day_8 only; after ffa-center-1, '
        'ffa-london-underpass-1 and ffa-cover-maps-1, whose points it replaces.'),
    'fire-station': Map(
        base.HERE / 'fire-station-picks.tsv', base.HERE.parent / 'content-edit/ffa-fire-station-picks-1.json',
        'ffa_fire_station_',
        'Fire Station\'s Free for All spawns at the owner\'s picks (2026-11-16: his `where` places on day_8, as '
        'London\'s; 2026-11-27: nine more). Generated by scripts/ffa-mode/make_picks_edits.py from '
        'fire-station-picks.tsv (check-points picks): all 51, each on its ground, body clear, 2 m from harmful zones, '
        'facing as he faced, however near another, in a solo spawn\'s place (its id, name, radii and protection kept) '
        'or a new one after the last (a copy of the first solo spawn: 20 m exclusion, 500 ms protection). The free '
        'for all\'s own project v010b_cs_mp_firestation/day_8 only; after ffa-stalker-2, whose points it replaces.'),
    'london-v032': Map(
        base.HERE / 'london-v032-picks.tsv', base.HERE.parent / 'content-edit/ffa-london-v032-picks-1.json',
        'ffa_london_v032_',
        'London (v0.32)\'s Free for All spawns at the owner\'s picks more (2026-11-27: his `where` places on '
        'v032a_level_09/day_8). Generated by scripts/ffa-mode/make_picks_edits.py from london-v032-picks.tsv '
        '(check-points picks --add --shift 1.5): all 26, each on its ground (pick 24 0.75 m off his place, the '
        'nearest floor the walk grid reaches), body clear, 2 m from harmful zones, facing as he faced, as new points '
        'after the last (copies of the first solo spawn: 20 m exclusion, 500 ms protection); the 50 there stay as '
        'they are. The free for all\'s own project v032a_level_09/day_8 only; after ffa-london-v032-1, which makes '
        'it.', adds=True),
    'chemical-plant': Map(
        base.HERE / 'chemical-plant-picks.tsv', base.HERE.parent / 'content-edit/ffa-chemical-plant-picks-1.json',
        'ffa_chemical_plant_',
        'Chemical Plant\'s Free for All spawns at the owner\'s picks (2026-11-16: his `where` places on day_8, as '
        'Fire Station\'s). Generated by scripts/ffa-mode/make_picks_edits.py from chemical-plant-picks.tsv '
        '(check-points picks): all 97, each on its ground, body clear, 2 m from harmful zones, facing as he faced, '
        'however near another, in a solo spawn\'s place (its id, name, radii and protection kept) or a new one after '
        'the last (a copy of the first solo spawn: 20 m exclusion, 500 ms protection). The free for all\'s own '
        'projects level_04/<weather>_8 only; after ffa-cover-maps-1, whose points it replaces.'),
    'station': Map(
        base.HERE / 'station-picks.tsv', base.HERE.parent / 'content-edit/ffa-station-picks-1.json',
        'ffa_station_',
        'Station\'s Free for All spawns at the owner\'s picks too (2026-11-27: his `where` places on '
        'v010b_soc_ug_station/day_8, "use them too"), with the spawn rule\'s (ffa-stalker-3). Generated by '
        'scripts/ffa-mode/make_picks_edits.py from station-picks.tsv (check-points picks --add --cap 128 --shift 1): '
        'all 60, each on its ground, body clear, 2 m from harmful zones, facing as he faced, however near another; '
        '28 in the places of the 28 spawn rule\'s points nearest a pick (their ids, names, radii and protection '
        'kept: past 128 points the respawn chooses among none, exe patch 142), 32 new points after the last (copies '
        'of the first solo spawn: 20 m exclusion, 500 ms protection); the other 68 stay as they are. The free for '
        'all\'s own project v010b_soc_ug_station/day_8 only; after ffa-stalker-3, which made its points.', adds=True),
}


VERDICTS = ('moved', 'added', 'roof', 'removed')
# A new point's radii: 20 m of enemy exclusion, as every solo spawn.
NEW_RADII = {'enemy_check_radius': 20., 'ally_check_radius': 1.}


def picks(map):
    """The map's picks rows by project and verdict: {verdict: {point id: (position, yaw)}}."""
    with map.picks.open(newline='') as stream:
        rows = list(csv.DictReader(stream, delimiter='\t'))
    out = {}
    for row in rows:
        by = out.setdefault(path_of(row['project']), {v: {} for v in VERDICTS})
        point = int(row['point'])
        if any(point in points for points in by.values()):
            raise ValueError(f'{row["project"]}: point {point} twice')
        if row['verdict'] not in VERDICTS:
            raise ValueError(f'{row["project"]} {point}: not a checked pick')
        values = [float(row[k]) for k in ('x', 'y', 'z', 'ry')]
        if not all(math.isfinite(v) for v in values):
            raise ValueError(f'{row["project"]} {point}: not finite')
        by[row['verdict']][point] = (values[:3], values[3])
    return out


def edits(map, live):
    out = {'_about': map.about, 'set_entry': [], 'add_entry': [], 'remove_entry': []}
    for path, by in picks(map).items():
        buf = (live / path).read_bytes()
        entries = base.tree(buf)['server_objects']['respawn_points']
        ids = [e['point_id'] for e in entries if e['team'] == 2]
        solo = {e['point_id']: e for e in entries if e['team'] == 2}
        moved, removed = by['moved'], by['removed']
        new = {**by['added'], **by['roof']}
        if map.adds and (removed or not set(moved) <= set(solo)):
            raise ValueError(f'{path}: a list of picks more removes {sorted(removed)} or moves '
                             f'{sorted(set(moved) - set(solo))}, no solo spawn')
        if not map.adds and set(moved) | set(removed) != set(solo):
            raise ValueError(f'{path}: the picks name {sorted(set(moved) | set(removed))}, '
                             f'the solo spawns are {sorted(solo)}')
        # Point ids count on over the teams (Rudnya, School) or anew in each (the STALKER maps): the
        # solo spawns are the last entries, numbered in a run, and new points follow the highest id.
        if (not ids or ids != list(range(ids[0], ids[0] + len(ids)))
                or any(e['team'] != 2 for e in entries[len(entries) - len(ids):])):
            raise ValueError(f'{path}: the solo spawns are not the last entries, numbered in a run')
        if sorted(removed) != ids[len(ids) - len(removed):]:
            raise ValueError(f'{path}: a removed spawn is not among the last entries')
        if new and removed:
            raise ValueError(f'{path}: points both added and removed')
        last = max(e['point_id'] for e in entries)
        if sorted(new) != list(range(last + 1, last + 1 + len(new))):
            raise ValueError(f'{path}: new points are not numbered after the last')
        for i in sorted(moved):
            entry = solo[i]
            position, yaw = moved[i]
            out['set_entry'].append({'file': path, 'list': base.POINTS,
                'where': {k: entry[k] for k in ('point_id', 'name', 'team')},
                'from': {'position': [round(v, 6) for v in entry['position']],
                         'rotation': [round(v, 6) for v in entry['rotation']]},
                'to': {'position': position, 'rotation': [0., yaw, 0.]}})
        model = solo[min(solo)]
        types = base.walked_types(buf, f"{base.POINTS}[{entries.index(model)}]")
        # The model of the new points, when there are any (School's radii are ints: it needs none). A
        # copy keeps the model's types: an int radius is one already of the value set (London (v0.32)'s
        # ally_check_radius 1, as its 50 other points).
        if new and (any(model[z]['collision_geometries'] for z in ('ally_zones', 'enemy_zones'))
                or any(types[k] != base.c.T_FLOAT and not (types[k] == base.c.T_INT and model[k] == NEW_RADII[k])
                       for k in base.RADII)
                or model.get('invulnerability_time_ms') != 500 or model['invulnerability_time'] != 0):
            raise ValueError(f'{path}: solo point {model["point_id"]} is no template (zones, float radii, 500 ms)')
        # Where point ids count anew in each team (the STALKER maps), the team names the entry too.
        unique = len({e['point_id'] for e in entries}) == len(entries)
        named = (lambda i: {'point_id': i}) if unique else (lambda i: {'point_id': i, 'team': 2})
        for i in sorted(new):
            position, yaw = new[i]
            name = f'{map.prefix}{i}'
            out['add_entry'].append({'files': path, 'list': base.POINTS, 'after': named(i - 1),
                'like': named(model['point_id']), 'expected_files': 1, 'values': {
                    'name': name, 'full_name': 'logic/modes/ffa/respawn/' + name, 'point_id': i, 'team': 2,
                    'position': position, 'rotation': [0., yaw, 0.], **NEW_RADII,
                    'invulnerability_time': 0, 'invulnerability_time_ms': 500}})
        for i in sorted(removed, reverse=True):
            out['remove_entry'].append({'file': path, 'list': base.POINTS,
                'where': {k: solo[i][k] for k in ('point_id', 'name', 'team')}})
    return {k: v for k, v in out.items() if v}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('map', choices=sorted(MAPS))
    parser.add_argument('--live', type=Path, required=True)
    args = parser.parse_args()
    map = MAPS[args.map]
    base.write(edits(map, args.live), map.output)
    print(map.output)


if __name__ == '__main__':
    main()

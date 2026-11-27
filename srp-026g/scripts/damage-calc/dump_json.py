import sys, os, json, re
sys.path.insert(0, sys.argv[3])
import bincfg
src, out = sys.argv[1], sys.argv[2]
ok=bad=0
for f in sorted(os.listdir(src)):
    # The configs, every language's dictionary, the movies' texts and the materials.
    if not (f.startswith('gameplay__') or re.fullmatch(r'localization__\w+__localization', f) or f in ('localization__flash_movies_localizations.options', 'game_materials__game.materials')): continue
    try:
        d = bincfg.load(os.path.join(src,f))
        json.dump(d, open(os.path.join(out,f+'.json'),'w'), indent=1, ensure_ascii=False, default=str); ok+=1
    except Exception as e:
        bad+=1; print('FAIL', f, e)
print(ok, bad)

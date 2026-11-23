#!/usr/bin/env bash
# usage: donor.sh <0.69d0 split list> <vostok-port that reads it> <out folder>
#
# What the grips pack (make_pack.py) takes of Survarium 0.69d0, read from its volumes:
#   game.leaves             gameplay/game.options as leaves, the lines the grips' rows need: the
#                           foregrip modules, the modifications (their groups and grades) and the
#                           modules' drop options (which groups a module rolls)
#   names/<language>.leaves 0.69d0's own names of the foregrips, in the languages it ships that we
#                           have (english, russian, ukrainian, german, polish)
#   icons/<module>.png      0.69d0's own picture of each foregrip (its module's
#                           ui_desc.icon_renderer <n>: flash_movies/textures/ui_icons/modules/module_renderer_<n>.png)
# <0.69d0 split list>: the volumes of the 0.69d0 archive, one a line (the weapon survey's
# work/full069.split). make_pack.py checks every file against donor.sha256.
set -euo pipefail
(($# == 3)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
split=$1 port=$2 out=$3
[[ ! -e $out ]] || { echo "$out: already there" >&2; exit 1; }
mkdir -p "$out/names" "$out/icons"
"$port" dump-db-config --leaves --db "$split" --path gameplay/game.options \
    | grep -E '^(weapon_modules\.[a-z0-9_]*foregrip\.|modifications\[|modules_drop_options\.)' > "$out/game.leaves"
for language in english russian ukrainian german polish; do
    "$port" dump-db-config --leaves --db "$split" --path "localization/$language/localization" \
        | grep -E '^strings\.st_[a-z0-9_]*foregrip_name	' > "$out/names/$language.leaves"
done
grep -E '^weapon_modules\.[a-z0-9_]*\.ui_desc\.icon_renderer	' "$out/game.leaves" | while IFS=$'\t' read -r key value; do
    module=${key#weapon_modules.} module=${module%%.*}
    "$port" extract --db "$split" --path "flash_movies/textures/ui_icons/modules/module_renderer_${value#i32:}.png" \
        --output "$out/icons/$module.png" >/dev/null
done
(cd "$out" && find . -type f | sort | xargs sha256sum)

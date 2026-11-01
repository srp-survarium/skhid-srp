#!/usr/bin/env bash
# The donor files of the rifle scopes pack (make_pack.py), from Survarium 0.69d0:
#   prepare_donor.sh <0.69d0 split list> <0.26g patch 19> <resource-porter checkout> <players' paths> <out folder>
# <0.69d0 split list>: the volumes of the 0.69d0 archive, one a line (the weapon survey's
# work/full069.split); <0.26g patch 19>: resources.db.patch_19, whose weapons/loc2 materials are the
# templates (materials.tsv); <resource-porter checkout>: one whose vostok-port reads split 16-byte
# archives (its BinaryConfig reads 0.69d0's configs and writes 0.26g's); <players' paths>: every
# path the players' game has, one a line (srp-game-data files of its archives, the first column).
# For each optic of
# scopes.tsv: its model and aimed model (models/weapons/modules/<module>{,_aim}.model: the meshes as
# they are, the configs written again in 0.26g's layout, widen_config.rs); the model of each
# module of references.tsv (the optics of ours whose placement make_pack.py measures); each material of
# materials.tsv that is made again: its template with the donor's textures slot for slot
# (retexture.py), with the textures it names that the 0.26g archives lack (the .dds as they are, their .options widened). make_pack.py checks every file
# against donor.sha256. And each optic's 0.69d0 shop icon, icons/<module>.png (its module's
# ui_desc.icon_renderer <n>: flash_movies/textures/ui_icons/modules/module_renderer_<n>.png).
set -euo pipefail
split=$1 target=$2 porter=$3 have=$4 out=$5
here=$(cd "$(dirname "$0")" && pwd)
[[ ! -e $out ]] || { echo "$out: already there" >&2; exit 1; }
port=${VOSTOK_PORT:-$porter/target/release/vostok-port}
[[ -x $port ]] || { echo "$port: no vostok-port built there (cargo build --release)" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/widen/src"
cp "$here/widen_config.rs" "$work/widen/src/main.rs"
cat > "$work/widen/Cargo.toml" <<TOML
[package]
name = "widen-config"
version = "0.1.0"
edition = "2024"
[dependencies]
vostok-resource-porter = { path = "$porter" }
anyhow = "1"
TOML
(cd "$work/widen" && CARGO_TARGET_DIR=$work/widen/target nix-shell -p cargo rustc gcc zlib --run 'cargo build --release --quiet')
widen=$work/widen/target/release/widen-config
mkdir -p "$out"
grep -v '^#' "$here/scopes.tsv" | cut -f3 | while read -r module; do
    name=$(basename "$module")
    for model in "models/weapons/modules/$module/$name.model" "models/weapons/modules/$module/${name}_aim.model"; do
        "$port" list --db "$split" --prefix "$model/" | while read -r path; do
            mkdir -p "$out/$(dirname "$path")"
            "$port" extract --db "$split" --path "$path" --output "$work/raw" >/dev/null
            case $path in
                */converted_model) cp "$work/raw" "$out/$path" ;;
                *) "$widen" "$work/raw" "$out/$path" ;;
            esac
        done
    done
done
# the reference modules (references.tsv): their model only
grep -v '^#' "$here/references.tsv" | while read -r module; do
    model="models/weapons/modules/$module/$(basename "$module").model"
    "$port" list --db "$split" --prefix "$model/" | while read -r path; do
        mkdir -p "$out/$(dirname "$path")"
        "$port" extract --db "$split" --path "$path" --output "$work/raw" >/dev/null
        case $path in
            */converted_model) cp "$work/raw" "$out/$path" ;;
            *) "$widen" "$work/raw" "$out/$path" ;;
        esac
    done
done
grep -v '^#' "$here/materials.tsv" | while IFS=$'\t' read -r name how; do
    [[ $how == loc2:* ]] && continue
    material=material_instances/weapons/modules/$name.material
    mkdir -p "$out/$(dirname "$material")"
    "$port" extract --db "$target" --path "$how" --output "$work/template" >/dev/null
    "$port" dump-db-config --db "$split" --path "$material" --leaves > "$work/leaves"
    nix shell nixpkgs#python3 -c python3 -I "$here/retexture.py" "$work/template" "$work/leaves" "$out/$material"
    "$port" dump-config --file "$out/$material" --leaves | grep 'texture[a-z_]*\.value	string:"[^"]' | cut -f2 \
        | sed 's/^string:"//; s/"$//' | sort -u | while read -r texture; do
        [[ -e $out/textures/$texture.dds ]] && continue
        grep -qxF "textures/$texture.dds" "$have" && continue
        "$port" list --db "$split" --prefix "textures/$texture.dds" | grep -q . || continue
        mkdir -p "$out/textures/$(dirname "$texture")"
        "$port" extract --db "$split" --path "textures/$texture.dds" --output "$out/textures/$texture.dds" >/dev/null
        "$port" extract --db "$split" --path "textures/$texture.options" --output "$work/raw" >/dev/null
        "$widen" "$work/raw" "$out/textures/$texture.options"
    done
done
# each optic's shop icon as 0.69d0 draws it (its module's ui_desc.icon_renderer in gameplay/game.options)
"$port" dump-db-config --db "$split" --path gameplay/game.options --leaves > "$work/game"
grep -v '^#' "$here/scopes.tsv" | cut -f3 | while read -r module; do
    n=$(grep -F "weapon_modules.$(basename "$module").ui_desc.icon_renderer	" "$work/game" | cut -f2 | sed 's/^i32://')
    [[ -n $n ]] || { echo "$module: no icon_renderer in game.options" >&2; exit 1; }
    mkdir -p "$out/icons"
    "$port" extract --db "$split" --path "flash_movies/textures/ui_icons/modules/module_renderer_$n.png" \
        --output "$out/icons/$(basename "$module").png" >/dev/null
done
(cd "$out" && find . -type f | sed 's|^\./||' | sort | xargs sha256sum)

#!/usr/bin/env bash
# The donor files of the weapon attachments pack (make_pack.py), from Survarium 0.46c2:
#   prepare_donor.sh <0.46c2 split list> <0.26g retail resources.db> <resource-porter checkout> <out folder>
# <0.46c2 split list>: the volumes of the 0.46c2 archive, one a line (the weapon survey's
# work/builds/v046c2/v046.split); <resource-porter checkout>: one whose vostok-port reads split
# 16-byte archives (82e2662 and later: its BinaryConfig reads 0.46c2's 16-byte configs and writes
# 0.26g's). For each device of devices.tsv: its model's four files, its material made again on a
# 0.26g template (vostok-port build-material-patch; pp9_klin's, the template rank-material-templates
# rates exact for every attach material), its textures (the .dds as they are); every config (the
# model's, the textures' .options) written again in 0.26g's 24-byte layout (widen_config.rs). Then
# each file of extras.tsv (the effects the devices name: particles, their materials, textures).
# make_pack.py checks every file against donor.sha256.
set -euo pipefail
split=$1 target=$2 porter=$3 out=$4
here=$(cd "$(dirname "$0")" && pwd)
[[ ! -e $out ]] || { echo "$out: already there" >&2; exit 1; }
# VOSTOK_PORT: that checkout's built vostok-port (default <checkout>/target/release/vostok-port)
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
grep -v '^#' "$here/devices.tsv" | while IFS=$'\t' read -r dict_id name kind model rest; do
    material=material_instances/${model#models/}
    material=${material%.model}.material
    texture=textures/${model#models/}
    texture=${texture%.model}
    for path in "$model/settings" "$model/render/export_properties" \
                "$model/render/$(basename "${model%.model}")_s/export_properties" \
                "$model/render/$(basename "${model%.model}")_s/converted_model" \
                "$texture/$(basename "$texture").dds" "$texture/$(basename "$texture")_nmap.dds" \
                "$texture/$(basename "$texture")_smap.dds" "$texture/$(basename "$texture")_fmap.dds" \
                "$texture/$(basename "$texture").options" "$texture/$(basename "$texture")_nmap.options" \
                "$texture/$(basename "$texture")_smap.options" "$texture/$(basename "$texture")_fmap.options"; do
        mkdir -p "$out/$(dirname "$path")"
        "$port" extract --db "$split" --path "$path" --output "$work/raw" >/dev/null
        case $path in
            *.dds | */converted_model) cp "$work/raw" "$out/$path" ;;
            *) "$widen" "$work/raw" "$out/$path" ;;
        esac
    done
    rm -rf "$work/material"
    "$port" build-material-patch --source-db "$split" --target-db "$target" \
        --material "$material=material_instances/weapons/pp9_klin/pp9_klin.material" --output "$work/material" >/dev/null
    mkdir -p "$out/$(dirname "$material")"
    cp "$work/material/overlay/$material" "$out/$material"
done
grep -v '^#' "$here/extras.tsv" | while IFS=$'\t' read -r how path template; do
    mkdir -p "$out/$(dirname "$path")"
    case $how in
        raw) "$port" extract --db "$split" --path "$path" --output "$out/$path" >/dev/null ;;
        config)
            "$port" extract --db "$split" --path "$path" --output "$work/raw" >/dev/null
            "$widen" "$work/raw" "$out/$path" ;;
        particle)
            "$port" extract --db "$split" --path "$path" --output "$work/raw" >/dev/null
            "$widen" --particle "$work/raw" "$out/$path" ;;
        material)
            rm -rf "$work/material"
            "$port" build-material-patch --source-db "$split" --target-db "$target" \
                --material "$path=$template" --output "$work/material" >/dev/null
            cp "$work/material/overlay/$path" "$out/$path" ;;
        *) echo "extras.tsv: $how: no such way" >&2; exit 1 ;;
    esac
done
(cd "$out" && find . -type f | sed 's|^\./||' | sort | xargs sha256sum)

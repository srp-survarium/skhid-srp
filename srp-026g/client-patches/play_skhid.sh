#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Put beside survarium_skhid.exe, then run: bash play_skhid.sh
set -euo pipefail

# Game language; server is empty when the executable has an embedded default.
localization=english
server=

die() { echo "Error: $*" >&2; exit 1; }

detect_resolution() {
    local resolution output status
    # On X11, prefer the primary monitor's preferred (usually native) mode.
    if [[ ${XDG_SESSION_TYPE-} != wayland ]] && command -v xrandr >/dev/null; then
        resolution=$(xrandr --query 2>/dev/null | awk '
            /^[^ ]/ { active = ($2 == "connected" && $0 ~ / [0-9]+x[0-9]+[+-]/); primary = ($3 == "primary") }
            active && $1 ~ /^[1-9][0-9]*x[1-9][0-9]*$/ && /\+/ {
                if (!first) first = $1
                if (primary && !preferred) preferred = $1
            }
            END { print preferred ? preferred : first }
        ') || resolution=
        if [[ $resolution =~ ^[1-9][0-9]*x[1-9][0-9]*$ ]]; then
            echo "$resolution"
            return
        fi
    fi
    # Wayland/no xrandr: the first enabled connector lists its preferred mode first.
    for output in /sys/class/drm/card*-*; do
        [[ -r "$output/status" && -r "$output/enabled" && -r "$output/modes" ]] || continue
        read -r status < "$output/status" || continue
        [[ $status == connected ]] || continue
        read -r status < "$output/enabled" || continue
        [[ $status == enabled ]] || continue
        read -r resolution < "$output/modes" || continue
        if [[ $resolution =~ ^[1-9][0-9]*x[1-9][0-9]*$ ]]; then
            echo "$resolution"
            return
        fi
    done
    echo 1920x1080
}

for command in wine winetricks wineserver; do
    command -v "$command" >/dev/null || die "Install $command first."
done
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
exe=survarium_skhid.exe
[[ -f "$exe" && ! -L "$exe" ]] || die "Put this script beside $exe."
[[ $# == 0 ]] || die 'No arguments needed.'
exec 9>.skhid-launcher.lock
flock -n 9 || die 'This launcher is already running.'

# Check the memory-test code, not the whole EXE: other Skhid code may be updated.
bytes() { dd if="$1" bs=1 skip="$2" count="$3" status=none | od -An -tx1 | tr -d ' \n'; }
offset=$((0xf5811))
original=85ff7719720881fb00000014730f6840010000
patched=85ff7719720881fb00000014eb0f6840010000
[[ "$(bytes "$exe" 0 2)" == 4d5a ]] || die 'Not a Windows executable.'
[[ "$(bytes "$exe" $((offset + 17)) 11)" == 8bd75f5e8bc35b83c408c3 ]] || die 'Memory-check code changed. EXE left unchanged.'
case "$(bytes "$exe" $((offset - 12)) 19)" in
    "$original")
        backup="$exe.before-memory-patch"
        if [[ -e "$backup" || -L "$backup" ]]; then
            [[ -f "$backup" && ! -L "$backup" ]] || die 'Backup path is not a regular file.'
        fi
        # Keep previous backups too when a newer Skhid EXE is installed.
        cp --backup=numbered -p -- "$exe" "$backup"
        temporary="$(mktemp .skhid-memory-XXXXXX)"
        trap 'rm -f -- "$temporary"' EXIT
        cp -p -- "$exe" "$temporary"
        printf '\xeb' | dd of="$temporary" bs=1 seek="$offset" conv=notrunc status=none
        [[ "$(bytes "$temporary" "$offset" 1)" == eb ]] || die 'Patch verification failed.'
        cmp -s -n "$offset" "$exe" "$temporary" && \
            cmp -s -i "$((offset + 1))" "$exe" "$temporary" || die 'Unexpected changes outside the memory patch.'
        mv -- "$temporary" "$exe"
        trap - EXIT
        echo 'Memory check patched; original EXE backed up.' ;;
    "$patched") echo 'Memory patch already applied.' ;;
    *) die 'Memory-check code changed. EXE left unchanged; launcher needs updating.' ;;
esac

export WINEPREFIX="$PWD/wineprefix" WINEDEBUG=-all
export WINE="$(command -v wine)" WINESERVER="$(command -v wineserver)"
# Disable Wine's broken optional QoS support and unwanted installer dialogs.
export WINEDLLOVERRIDES='mscoree,mshtml,qwave=;winemenubuilder.exe=d'
unset WINEARCH WINELOADER
log="$PWD/skhid-wine.log"

if [[ ! -f "$WINEPREFIX/.skhid-ready" || ! -f "$WINEPREFIX/system.reg" ]]; then
    rm -f -- "$WINEPREFIX/.skhid-dxvk"
    echo 'Initializing Wine prefix...'
    timeout 180 "$WINE" wineboot.exe --init >>"$log" 2>&1 || die "Wine setup failed. See $log"
    timeout 180 "$WINESERVER" -w >>"$log" 2>&1 || die "Wine setup timed out. See $log"
    [[ -f "$WINEPREFIX/system.reg" ]] || die "Wine registry missing. See $log"
    # Keep game settings inside this prefix instead of the user's real Documents.
    documents="$WINEPREFIX/drive_c/users/$(id -un)/Documents"
    [[ ! -L "$documents" ]] || rm -- "$documents"
    mkdir -p "$documents/Survarium"
    if [[ ! -f "$documents/Survarium/user.cfg" ]]; then
        resolution=$(detect_resolution)
        printf 'r_fullscreen on\r\nr_resolution %s\r\n' "$resolution" > "$documents/Survarium/user.cfg"
        echo "Fullscreen resolution: $resolution"
    fi
    touch "$WINEPREFIX/.skhid-ready"
fi
if [[ ! -f "$WINEPREFIX/.skhid-dxvk" ]]; then
    echo 'Installing DXVK...'
    winetricks -q dxvk >>"$log" 2>&1 || die "DXVK installation failed. See $log"
    touch "$WINEPREFIX/.skhid-dxvk"
fi

export WINEDLLOVERRIDES="$WINEDLLOVERRIDES;d3d11,dxgi=n,b"
echo "Starting Survarium. Log: $log"
args=("-localization=$localization" -mt_allocator=768 -minimum_resources_memory_size=768)
[[ -z "$server" ]] || args+=("-server=$server")
"$WINE" "$exe" "${args[@]}" >>"$log" 2>&1 \
    || die "Game exited with an error. See $log"

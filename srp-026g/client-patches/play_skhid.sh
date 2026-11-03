#!/usr/bin/env bash
# Put beside survarium_skhid.exe, then run: bash play_skhid.sh
# With the Skhid launcher beside it too: bash play_skhid.sh launcher
# (it looks for newer Skhid files, and its PLAY button starts the game).
set -euo pipefail

# The game's language is chosen in its Settings. Server: empty when the
# executable has an embedded default.
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
mode=${1-}
[[ $# -le 1 && ( -z $mode || $mode == launcher ) ]] || die 'Usage: bash play_skhid.sh [launcher]'
if [[ $mode == launcher ]]; then
    for file in launcher_skhid.exe updater_skhid.exe; do
        [[ -f "$file" && ! -L "$file" ]] || die "Put $file beside this script to use the launcher."
    done
fi
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
    "$original") memory_patch=needed ;;
    "$patched") memory_patch=present ;;
    *) die 'Memory-check code changed. EXE left unchanged; launcher needs updating.' ;;
esac

export WINEPREFIX="$PWD/wineprefix" WINEDEBUG=-all
WINE=$(command -v wine)
WINESERVER=$(command -v wineserver)
export WINE WINESERVER
# Disable Wine's broken optional QoS support and unwanted installer dialogs.
export WINEDLLOVERRIDES='mscoree,mshtml,qwave=;winemenubuilder.exe=d'
# Wine 11 supports forcing the new WoW64 loader even in a mixed installation.
# win64 alone selects a prefix architecture, not the loader used for 32-bit EXEs.
export WINEARCH=wow64
unset WINELOADER
log="$PWD/skhid-wine.log"

version=$(LC_ALL=C "$WINE" --version 2>&1) || die "Cannot query Wine: $version
Install Wine 11.0 or newer with modern WoW64 support (tested: 11.8 Staging WoW64).
See LINUX.txt."
[[ $version =~ ^wine-([0-9]+)\.([0-9]+)([^0-9].*)?$ ]] || die "Unrecognized Wine version: $version
Use Wine 11.0 or newer with modern WoW64 support; see LINUX.txt."
(( 10#${BASH_REMATCH[1]} >= 11 )) || die "Unsupported Wine: $version
Wine 11.0 or newer with modern WoW64 is required (tested: 11.8 Staging WoW64).
Select its wine and wineserver in PATH; see LINUX.txt."

verify_prefix_arch() {
    local prefix_arch
    prefix_arch=$(awk '/^#arch=/ { sub(/^#arch=/, ""); sub(/\r$/, ""); print; exit }' "$WINEPREFIX/system.reg")
    [[ $prefix_arch == win64 ]] || die "This Wine prefix is not a supported 64-bit prefix for modern WoW64.
Close the game and launcher, rename wineprefix to an unused backup name, and run again.
Your existing prefix has been kept. See LINUX.txt for preserving game settings."
}
if [[ -f "$WINEPREFIX/system.reg" ]]; then
    verify_prefix_arch
fi
printf 'Wine runtime: %s; required mode: WINEARCH=wow64\n' "$version" >>"$log"

if [[ ! -f "$WINEPREFIX/.skhid-ready" || ! -f "$WINEPREFIX/system.reg" ]]; then
    rm -f -- "$WINEPREFIX/.skhid-dxvk"
    echo 'Initializing Wine prefix...'
    timeout 180 "$WINE" wineboot.exe --init >>"$log" 2>&1 || die "Wine setup failed in modern WoW64 mode. See LINUX.txt and $log"
    timeout 180 "$WINESERVER" -w >>"$log" 2>&1 || die "Wine setup timed out. See $log"
    [[ -f "$WINEPREFIX/system.reg" ]] || die "Wine registry missing. See $log"
    verify_prefix_arch
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
# Exercise a 32-bit PE under the forced modern loader. A 64-bit cmd.exe would
# not prove the mode needed by the game. Refuse failures without an old-mode retry.
timeout 60 "$WINE" 'C:\windows\syswow64\cmd.exe' /d /c 'exit 0' >>"$log" 2>&1 || die "Modern WoW64 could not run a 32-bit Windows program.
Use Wine 11.0 or newer with modern WoW64 support and matching wineserver.
For an old prefix, keep it as a backup and create a fresh one; see LINUX.txt and $log."

case "$memory_patch" in
    needed)
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
        if ! cmp -s -n "$offset" "$exe" "$temporary" || \
                ! cmp -s -i "$((offset + 1))" "$exe" "$temporary"; then
            die 'Unexpected changes outside the memory patch.'
        fi
        mv -- "$temporary" "$exe"
        trap - EXIT
        echo 'Memory check patched; original EXE backed up.' ;;
    present) echo 'Memory patch already applied.' ;;
esac

if [[ ! -f "$WINEPREFIX/.skhid-dxvk" ]]; then
    echo 'Installing DXVK...'
    winetricks -q dxvk >>"$log" 2>&1 || die "DXVK installation failed. See $log"
    touch "$WINEPREFIX/.skhid-dxvk"
fi

export WINEDLLOVERRIDES="$WINEDLLOVERRIDES;d3d11,dxgi=n,b"
if [[ $mode == launcher ]]; then
    # The game it starts inherits this Wine environment.
    echo "Starting the Skhid launcher. Log: $log"
    "$WINE" launcher_skhid.exe >>"$log" 2>&1 \
        || die "Launcher exited with an error. See $log"
    exit 0
fi
# resources.db's patches, the ones the server lists, must be in place first:
# the exe of a release needs the content of its patches. The updater checks
# them (it reads only); no updater beside this script, as without the
# launcher: no check.
if [[ -f updater_skhid.exe && ! -L updater_skhid.exe ]] \
    && ! "$WINE" updater_skhid.exe ready quiet >>"$log" 2>&1; then
    die "Skhid has not finished updating the game's files (game/resources.db.patch_*), so the game is not started.
Start the launcher (bash Skhid.sh launcher, or bash play_skhid.sh launcher): it finishes the update.
Details: game/binaries/x86/updater_skhid.log"
fi
echo "Starting Survarium. Log: $log"
args=(-mt_allocator=768 -minimum_resources_memory_size=768)
[[ -z "$server" ]] || args+=("-server=$server")
"$WINE" "$exe" "${args[@]}" >>"$log" 2>&1 \
    || die "Game exited with an error. See $log"
